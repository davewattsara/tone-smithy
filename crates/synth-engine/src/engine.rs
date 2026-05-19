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
/// buffers. M2.2 has none yet, so the value is informative until later
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
        voice.set_filter_mode(params.filter_mode());
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
            // TODO: M3 — scale envelope peak by velocity (0..=127 → 0.0..=1.0).
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
            EngineEvent::SetFilterMode { mode } => {
                self.params.set_filter_mode(mode);
                self.voice.set_filter_mode(mode);
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
            let (left, right) = self.voice.next_sample(&smoothed);
            output[frame_index * 2] = left;
            output[frame_index * 2 + 1] = right;
        }

        // Mirror the post-block voice state into the tree so the next
        // snapshot reflects what just played. At M3 the voice manager
        // will supply the real count; for now it's 0 or 1.
        self.params.set_active_voice_count(u8::from(!self.voice.is_idle()));
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
    use crate::filter::FilterMode;
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
        engine.handle(EngineEvent::SetFilterMode {
            mode: FilterMode::BandPass,
        });

        let snap = engine.snapshot();
        // Smoothed param: snapshot reads `current`, which has not yet
        // advanced toward the new target (no block has run), so the
        // value is still the default.
        assert_eq!(snap.pitch_offset_semis, 0.0);
        // Stepped param latches immediately.
        assert_eq!(snap.amp_release_secs, 1.5);
        assert_eq!(snap.waveform, Waveform::Saw);
        assert_eq!(snap.filter_mode, FilterMode::BandPass);
        assert_eq!(snap.active_voice_count, 0);
    }

    #[test]
    fn active_voice_count_tracks_note_state() {
        let mut engine = Engine::new(48_000.0);
        assert_eq!(engine.snapshot().active_voice_count, 0);
        engine.handle(EngineEvent::NoteOn {
            note_midi: 60,
            velocity: 100,
        });
        // One sample is enough to leave idle.
        let mut buffer = [0.0f32; 2];
        engine.process_stereo(&mut buffer, 1);
        assert_eq!(engine.snapshot().active_voice_count, 1);
    }

    #[test]
    fn smoothed_param_snapshot_advances_with_processing() {
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

    #[test]
    fn hard_pan_routes_audio_to_one_stereo_channel() {
        // End-to-end: pan osc1 hard left, mute oscs 2/3/sub, render a
        // note. The right channel of the interleaved output should
        // contain only essentially silent samples.
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::Osc2Level,
            value: 0.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::Osc3Level,
            value: 0.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::SubLevel,
            value: 0.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::Osc1Pan,
            value: -1.0,
        });
        engine.handle(EngineEvent::NoteOn {
            note_midi: 69,
            velocity: 100,
        });

        let mut buffer = vec![0.0f32; 4096 * 2];
        // Render long enough for the pan/level smoothers to settle.
        for _ in 0..6 {
            engine.process_stereo(&mut buffer, 4096);
        }
        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        for frame in buffer.chunks_exact(2) {
            peak_l = peak_l.max(frame[0].abs());
            peak_r = peak_r.max(frame[1].abs());
        }
        assert!(peak_l > 0.1, "expected audible left, got {peak_l}");
        assert!(peak_r < 0.01, "expected silent right, got {peak_r}");
    }

    #[test]
    fn unison_voices_change_stereo_width_end_to_end() {
        // Solo osc1 wide, render a note with 1 unison voice then with
        // 5. The second case should produce a meaningfully wider
        // L vs R difference summed across the buffer.
        fn render_diff(voice_count: f32) -> f32 {
            let mut engine = Engine::new(48_000.0);
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc2Level,
                value: 0.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc3Level,
                value: 0.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::SubLevel,
                value: 0.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc1UnisonVoices,
                value: voice_count,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc1UnisonDetuneCents,
                value: 25.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc1UnisonSpread,
                value: 1.0,
            });
            engine.handle(EngineEvent::NoteOn {
                note_midi: 69,
                velocity: 100,
            });
            let mut buffer = vec![0.0f32; 4096 * 2];
            for _ in 0..6 {
                engine.process_stereo(&mut buffer, 4096);
            }
            buffer
                .chunks_exact(2)
                .map(|frame| (frame[0] - frame[1]).abs())
                .sum::<f32>()
        }
        let one = render_diff(1.0);
        let five = render_diff(5.0);
        assert!(
            five > one * 3.0,
            "expected unison to widen stereo: 1-voice diff {one}, 5-voice diff {five}"
        );
    }

    #[test]
    fn closing_filter_silences_a_held_saw() {
        // End-to-end: a held saw note through the engine, then close
        // the LP filter — the steady-state output should drop near
        // silence. Smoothing on cutoff means we have to render long
        // enough for the new target to take effect.
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::SetOscillatorWaveform {
            waveform: Waveform::Saw,
        });
        engine.handle(EngineEvent::NoteOn {
            note_midi: 69,
            velocity: 100,
        });
        let mut buffer = vec![0.0f32; 4096 * 2];
        // Let the envelope reach sustain with the filter wide open.
        engine.process_stereo(&mut buffer, 4096);
        let open_peak = buffer.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(
            open_peak > 0.1,
            "expected audible saw before closing filter, got {open_peak}"
        );

        // Now close the filter.
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::FilterCutoffHz,
            value: 30.0,
        });
        // Let smoothing reach the new target and the filter settle.
        for _ in 0..4 {
            engine.process_stereo(&mut buffer, 4096);
        }
        let closed_peak = buffer.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(
            closed_peak < open_peak * 0.1,
            "filter did not close the voice: open {open_peak}, closed {closed_peak}"
        );
    }
}
