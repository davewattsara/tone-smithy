//! Single-slot oscillator for M1.
//!
//! Produces either a pure sine or a naive saw at a configurable frequency.
//! The full multi-shape oscillator slot (saw / square / triangle / noise
//! with PolyBLEP anti-aliasing, sub oscillator, unison) lives at
//! `oscillator/` in later milestones — we stay in one file while there
//! are only two shapes and no anti-aliasing to host.
//!
//! Saw is the naive ramp `(phase / PI) - 1.0`. It aliases above ~2 kHz
//! at 48 kHz sample rate; PolyBLEP anti-aliasing is on the M2 list per
//! `docs/planning/06-implementation/project-structure.md` and
//! `docs/planning/06-implementation/milestones.md`.
//!
//! See `docs/planning/03-architecture/design-patterns.md` §2 for the
//! real-time safety rules this code obeys, and §2.7 for the rule that
//! discrete parameters like [`Waveform`] are latched at block boundary
//! (the engine drains events before each block, satisfying this).

use core::f32::consts::{PI, TAU};

/// The shape an oscillator produces.
///
/// `Waveform` is a discrete parameter — switching mid-block would cause
/// an audible step. Adapters must change it via [`EngineEvent`] so the
/// change lands at a block boundary; the engine drains events once per
/// block before processing samples (`docs/planning/03-architecture/
/// design-patterns.md` §2.7).
///
/// [`EngineEvent`]: crate::EngineEvent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Waveform {
    /// Pure sine, fundamental only, no aliasing.
    #[default]
    Sine,

    /// Naive sawtooth. Aliases at high frequencies; M2 replaces this
    /// with a PolyBLEP-corrected variant.
    Saw,
}

/// A phase-accumulating oscillator.
///
/// The oscillator carries no state about pitch beyond `phase_increment`.
/// The caller is responsible for converting MIDI note + tuning into a
/// frequency and calling [`Oscillator::set_frequency_hz`] when it changes.
pub struct Oscillator {
    /// Sample rate in Hz, captured at `prepare()` time.
    sample_rate_hz: f32,

    /// Current phase in radians, kept in 0..TAU.
    phase: f32,

    /// Per-sample phase advance, derived from frequency and sample rate.
    phase_increment: f32,

    /// Current waveform. Changes take effect on the next sample; the
    /// engine guarantees the change lands at a block boundary.
    waveform: Waveform,
}

impl Oscillator {
    /// Creates an oscillator producing the default waveform ([`Waveform::Sine`])
    /// at 0 Hz. Call [`Oscillator::set_frequency_hz`] before processing.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            phase: 0.0,
            phase_increment: 0.0,
            waveform: Waveform::default(),
        }
    }

    /// Sets the oscillator frequency in Hz. Negative and zero values are
    /// accepted and produce no useful sound; the caller should clamp.
    pub fn set_frequency_hz(&mut self, frequency_hz: f32) {
        self.phase_increment = TAU * frequency_hz / self.sample_rate_hz;
    }

    /// Sets the active waveform. Takes effect on the next call to
    /// [`Oscillator::next_sample`].
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    /// Resets the phase to zero. Called on note-on so each note starts
    /// from a known phase; this avoids the random-DC artefacts that
    /// follow from leaving the phase wherever the last note ended.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Produces one sample. Advances internal phase.
    pub fn next_sample(&mut self) -> f32 {
        let sample = match self.waveform {
            Waveform::Sine => self.phase.sin(),
            // Naive saw: map phase 0..TAU to -1..1 linearly.
            Waveform::Saw => (self.phase / PI) - 1.0,
        };
        self.phase += self.phase_increment;
        if self.phase >= TAU {
            self.phase -= TAU;
        }
        sample
    }
}

/// Converts a MIDI note number to a frequency in Hz using equal temperament
/// with A4 (note 69) at 440 Hz. Accepts the standard MIDI range and beyond;
/// the caller is responsible for clamping if needed.
#[must_use]
pub fn midi_note_to_hz(note_midi: u8) -> f32 {
    // f = 440 * 2^((n - 69) / 12)
    440.0 * 2.0_f32.powf((f32::from(note_midi) - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_note_69_is_440_hz() {
        let hz = midi_note_to_hz(69);
        assert!((hz - 440.0).abs() < 1e-3, "got {hz}");
    }

    #[test]
    fn midi_note_60_is_middle_c() {
        // Middle C in equal temperament with A4=440 is ~261.626 Hz.
        let hz = midi_note_to_hz(60);
        assert!((hz - 261.625_55).abs() < 1e-2, "got {hz}");
    }

    #[test]
    fn sine_oscillator_stays_in_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_frequency_hz(440.0);
        for _ in 0..10_000 {
            let s = osc.next_sample();
            assert!(s.abs() <= 1.0 + 1e-6);
        }
    }

    #[test]
    fn saw_oscillator_stays_in_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(440.0);
        for _ in 0..10_000 {
            let s = osc.next_sample();
            assert!(s.abs() <= 1.0 + 1e-6, "saw out of bounds: {s}");
        }
    }

    #[test]
    fn saw_completes_full_swing_per_period() {
        // At 480 Hz on a 48 kHz sample rate, one period is exactly 100
        // samples. Across one period the naive saw should sweep through
        // its full range; check it touches near both extremes.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(480.0);
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for _ in 0..100 {
            let s = osc.next_sample();
            min = min.min(s);
            max = max.max(s);
        }
        assert!(min < -0.95, "expected near -1.0, got {min}");
        assert!(max > 0.95, "expected near +1.0, got {max}");
    }
}
