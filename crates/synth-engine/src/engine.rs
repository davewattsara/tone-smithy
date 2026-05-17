//! The top-level DSP engine.
//!
//! Holds the (currently single) voice and applies events to it.
//! [`Engine::prepare`] is the once-per-stream setup point where all
//! buffer sizing and pool allocation must happen — see
//! `docs/planning/03-architecture/design-patterns.md` §2.5.
//! [`Engine::process`] is allowed zero heap allocations.

use crate::events::EngineEvent;
use crate::voice::Voice;

/// Maximum block size the engine promises to handle, in frames.
///
/// `process` is given the actual block size each call; this constant
/// is a soft upper bound used by future internal scratch buffers. M1
/// has none yet, so the value is informative until M2 needs it.
pub const MAX_BLOCK_SIZE: usize = 4096;

/// The DSP engine. Owns the voice and the parameter state.
///
/// Construct with [`Engine::new`], wire to the audio thread, and call
/// [`Engine::handle`] for each input event before each block.
pub struct Engine {
    /// Sample rate the audio device opened with, captured at `prepare()`.
    sample_rate_hz: f32,

    /// The single M1 voice. Replaced by a voice manager at M3.
    voice: Voice,
}

impl Engine {
    /// Creates an engine ready to process at the given sample rate.
    ///
    /// The sample rate is fixed for the engine's lifetime; if the audio
    /// device changes rate, build a new engine.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            voice: Voice::new(sample_rate_hz),
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
}
