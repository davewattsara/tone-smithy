//! The top-level DSP engine.
//!
//! Holds the (currently single) voice and the [`ParameterTree`] that
//! owns all sound-affecting state. [`Engine::prepare`] is the
//! once-per-stream setup point where all buffer sizing and pool
//! allocation must happen — see
//! `docs/planning/03-architecture/design-patterns.md` §2.5.
//! [`Engine::process_stereo`] is allowed zero heap allocations.
//!
//! [`ParameterTree`]: crate::params::ParameterTree

use crate::events::EngineEvent;
use crate::params::{ParamId, ParamSnapshot, ParameterTree};
use crate::voice::Voice;

/// Maximum block size the engine promises to handle, in frames.
///
/// `process_stereo` is given the actual block size each call; this
/// constant is a soft upper bound used by future internal scratch
/// buffers. M2.0 has none yet, so the value is informative until later
/// M2 work needs it.
pub const MAX_BLOCK_SIZE: usize = 4096;

/// The DSP engine. Owns the parameter tree and the voice.
///
/// Construct with [`Engine::new`], wire to the audio thread, and call
/// [`Engine::handle`] for each input event before each block.
pub struct Engine {
    /// Sample rate the audio device opened with, captured at `new()`.
    sample_rate_hz: f32,

    /// Single source of truth for sound-affecting parameters per
    /// design-patterns.md §1.3.
    params: ParameterTree,

    /// The single M2 voice. Replaced by a voice manager at M3.
    voice: Voice,
}

impl Engine {
    /// Creates an engine ready to process at the given sample rate.
    ///
    /// The sample rate is fixed for the engine's lifetime; if the audio
    /// device changes rate, build a new engine.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        let params = ParameterTree::new(sample_rate_hz);
        let mut voice = Voice::new(sample_rate_hz);
        // Seed the voice with the parameter defaults so its DSP
        // components see the same values the tree publishes in the
        // first snapshot.
        voice.set_release_secs(params.amp_release_secs());
        voice.set_main_waveform(params.waveform());
        Self {
            sample_rate_hz,
            params,
            voice,
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
                // Snap smoothed per-voice params so the first sample of
                // the new note plays exactly at the current target.
                self.params.snap_for_note_on();
                self.voice.note_on(note_midi);
            }
            EngineEvent::NoteOff { note_midi } => {
                self.voice.note_off(note_midi);
            }
            EngineEvent::SetOscillatorWaveform { waveform } => {
                self.params.set_waveform(waveform);
                self.voice.set_main_waveform(waveform);
            }
            EngineEvent::ParameterChange { id, value } => {
                self.params.set_continuous(id, value);
                // Stepped params are read by DSP only on edge
                // transitions, so push the new value to the consumer
                // immediately; smoothed params are sampled per frame
                // from the tree and need no fan-out here.
                if matches!(id, ParamId::AmpReleaseSecs) {
                    self.voice.set_release_secs(value);
                }
            }
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
            let smoothed = self.params.next_sample();
            let sample = self.voice.next_sample(smoothed.pitch_offset_semis);
            // Mono signal duplicated to both channels. Real stereo
            // (per-oscillator pan via the slot mixer) arrives in M2.3.
            output[frame_index * 2] = sample;
            output[frame_index * 2 + 1] = sample;
        }

        // Mirror the post-block voice state into the tree so the next
        // snapshot reflects what just played.
        self.params.set_voice_active(!self.voice.is_idle());
    }

    /// Returns the current parameter snapshot by value, without
    /// allocating. The caller is responsible for wrapping it in an
    /// `Arc` and publishing into the snapshot slot (see
    /// `docs/planning/03-architecture/design-patterns.md` §1.4).
    #[must_use]
    pub fn snapshot(&self) -> ParamSnapshot {
        self.params.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oscillator::Waveform;

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
        engine.handle(EngineEvent::NoteOn {
            note_midi: 69,
            velocity: 100,
        });
        let mut buffer = [0.0f32; 1024 * 2];
        engine.process_stereo(&mut buffer, 1024);
        let peak = buffer.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(peak > 0.1, "expected audible output after NoteOn, peak was {peak}");
    }

    #[test]
    fn parameter_change_updates_snapshot() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::PitchOffsetSemis,
            value: 7.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::AmpReleaseSecs,
            value: 1.5,
        });
        engine.handle(EngineEvent::SetOscillatorWaveform {
            waveform: Waveform::Saw,
        });

        let snap = engine.snapshot();
        // Smoothed param: snapshot reads `current`, which has not yet
        // advanced toward the new target (no block has run), so the
        // value is still the default.
        assert_eq!(snap.pitch_offset_semis, 0.0);
        // Stepped param latches immediately.
        assert_eq!(snap.amp_release_secs, 1.5);
        assert_eq!(snap.waveform, Waveform::Saw);
        assert!(!snap.voice_active);
    }

    #[test]
    fn voice_active_flag_tracks_note_state() {
        let mut engine = Engine::new(48_000.0);
        assert!(!engine.snapshot().voice_active);
        engine.handle(EngineEvent::NoteOn {
            note_midi: 60,
            velocity: 100,
        });
        // One sample is enough to leave idle.
        let mut buffer = [0.0f32; 2];
        engine.process_stereo(&mut buffer, 1);
        assert!(engine.snapshot().voice_active);
    }

    #[test]
    fn smoothed_param_snapshot_advances_with_processing() {
        // After enough samples the smoothed param reaches its target
        // and the snapshot reflects it.
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::PitchOffsetSemis,
            value: 5.0,
        });
        let mut buffer = [0.0f32; 4096 * 2];
        engine.process_stereo(&mut buffer, 4096);
        let snap = engine.snapshot();
        assert!(
            (snap.pitch_offset_semis - 5.0).abs() < 0.05,
            "expected ~5.0 after smoothing, got {}",
            snap.pitch_offset_semis
        );
    }
}
