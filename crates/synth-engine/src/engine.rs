//! The top-level DSP engine.
//!
//! Holds the (currently single) voice and applies events to it.
//! [`Engine::prepare`] is the once-per-stream setup point where all
//! buffer sizing and pool allocation must happen — see
//! `docs/planning/03-architecture/design-patterns.md` §2.5.
//! [`Engine::process_stereo`] is allowed zero heap allocations.

use crate::events::EngineEvent;
use crate::oscillator::Waveform;
use crate::params::{ParamId, ParamSnapshot};
use crate::voice::Voice;

/// Maximum block size the engine promises to handle, in frames.
///
/// `process_stereo` is given the actual block size each call; this
/// constant is a soft upper bound used by future internal scratch
/// buffers. M1 has none yet, so the value is informative until M2
/// needs it.
pub const MAX_BLOCK_SIZE: usize = 4096;

/// The DSP engine. Owns the voice and the parameter state.
///
/// Construct with [`Engine::new`], wire to the audio thread, and call
/// [`Engine::handle`] for each input event before each block.
pub struct Engine {
    /// Sample rate the audio device opened with, captured at `new()`.
    sample_rate_hz: f32,

    /// The single M1 voice. Replaced by a voice manager at M3.
    voice: Voice,

    /// Current value mirror of the snapshot fields. Kept on the engine
    /// so [`Engine::snapshot`] can produce a `ParamSnapshot` without
    /// allocating. M2 replaces this with the typed parameter tree.
    pitch_offset_semis: f32,
    amp_release_secs: f32,
    waveform: Waveform,
}

impl Engine {
    /// Creates an engine ready to process at the given sample rate.
    ///
    /// The sample rate is fixed for the engine's lifetime; if the audio
    /// device changes rate, build a new engine.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        let defaults = ParamSnapshot::default();
        Self {
            sample_rate_hz,
            voice: Voice::new(sample_rate_hz),
            pitch_offset_semis: defaults.pitch_offset_semis,
            amp_release_secs: defaults.amp_release_secs,
            waveform: defaults.waveform,
        }
    }

    /// Returns the sample rate the engine was built with, in Hz.
    #[must_use]
    pub fn sample_rate_hz(&self) -> f32 {
        self.sample_rate_hz
    }

    /// Applies a single event. Called by the audio thread at the top
    /// of each block, draining whatever the adapters have queued.
    pub fn handle(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::NoteOn { note_midi, velocity: _ } => {
                self.voice.note_on(note_midi);
            }
            EngineEvent::NoteOff { note_midi } => {
                self.voice.note_off(note_midi);
            }
            EngineEvent::SetOscillatorWaveform { waveform } => {
                self.waveform = waveform;
                self.voice.set_waveform(waveform);
            }
            EngineEvent::ParameterChange { id, value } => match id {
                ParamId::PitchOffsetSemis => {
                    self.pitch_offset_semis = value;
                    self.voice.set_pitch_offset_semis(value);
                }
                ParamId::AmpReleaseSecs => {
                    self.amp_release_secs = value;
                    self.voice.set_release_secs(value);
                }
            },
        }
    }

    /// Fills an interleaved stereo output buffer with `frames` frames
    /// of audio. The buffer length must equal `frames * 2`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `output.len() != frames * 2`.
    pub fn process_stereo(&mut self, output: &mut [f32], frames: usize) {
        debug_assert_eq!(output.len(), frames * 2);

        for frame_index in 0..frames {
            let sample = self.voice.next_sample();
            // Mono signal duplicated to both channels. Stereo pan
            // arrives at M2 (per-oscillator pan) and via the master
            // panner later.
            output[frame_index * 2] = sample;
            output[frame_index * 2 + 1] = sample;
        }
    }

    /// Returns the current parameter snapshot by value, without
    /// allocating. The caller is responsible for wrapping it in an
    /// `Arc` and publishing into the snapshot slot (see
    /// `docs/planning/03-architecture/design-patterns.md` §1.4).
    #[must_use]
    pub fn snapshot(&self) -> ParamSnapshot {
        ParamSnapshot {
            pitch_offset_semis: self.pitch_offset_semis,
            amp_release_secs: self.amp_release_secs,
            waveform: self.waveform,
            voice_active: !self.voice.is_idle(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_starts_silent() {
        let mut engine = Engine::new(48_000.0);
        let mut buffer = [0.0f32; 256 * 2];
        engine.process_stereo(&mut buffer, 256);
        assert!(buffer.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn note_on_produces_non_zero_audio() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::NoteOn { note_midi: 69, velocity: 100 });
        let mut buffer = [0.0f32; 1024 * 2];
        engine.process_stereo(&mut buffer, 1024);
        let peak = buffer.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(peak > 0.1, "expected audible output after NoteOn, peak was {peak}");
    }

    #[test]
    fn parameter_change_updates_snapshot() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ParameterChange { id: ParamId::PitchOffsetSemis, value: 7.0 });
        engine.handle(EngineEvent::ParameterChange { id: ParamId::AmpReleaseSecs, value: 1.5 });
        engine.handle(EngineEvent::SetOscillatorWaveform { waveform: Waveform::Saw });

        let snap = engine.snapshot();
        assert_eq!(snap.pitch_offset_semis, 7.0);
        assert_eq!(snap.amp_release_secs, 1.5);
        assert_eq!(snap.waveform, Waveform::Saw);
        assert!(!snap.voice_active);
    }

    #[test]
    fn voice_active_flag_tracks_note_state() {
        let mut engine = Engine::new(48_000.0);
        assert!(!engine.snapshot().voice_active);
        engine.handle(EngineEvent::NoteOn { note_midi: 60, velocity: 100 });
        // One sample is enough to leave idle.
        let mut buffer = [0.0f32; 2];
        engine.process_stereo(&mut buffer, 1);
        assert!(engine.snapshot().voice_active);
    }
}
