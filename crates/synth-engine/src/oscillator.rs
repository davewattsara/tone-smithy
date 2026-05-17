//! Single-shape oscillator for M1.
//!
//! Produces a pure sine wave at a configurable frequency. The full
//! multi-shape oscillator slot (saw / square / triangle / noise with
//! PolyBLEP anti-aliasing) lives at `oscillator/` in later milestones —
//! we stay in one file while there is only one shape to host.
//!
//! See `docs/planning/06-implementation/project-structure.md` for the
//! ultimate layout, and `docs/planning/03-architecture/design-patterns.md`
//! §2 for the real-time safety rules this code obeys.

use core::f32::consts::TAU;

/// A phase-accumulating sine oscillator.
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
}

impl Oscillator {
    /// Creates an oscillator silent at 0 Hz. Call
    /// [`Oscillator::set_frequency_hz`] before processing.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            phase: 0.0,
            phase_increment: 0.0,
        }
    }

    /// Sets the oscillator frequency in Hz. Negative and zero values are
    /// accepted and produce no useful sound; the caller should clamp.
    pub fn set_frequency_hz(&mut self, frequency_hz: f32) {
        self.phase_increment = TAU * frequency_hz / self.sample_rate_hz;
    }

    /// Resets the phase to zero. Called on note-on so each note starts
    /// from a known phase; this avoids the random-DC artefacts that
    /// follow from leaving the phase wherever the last note ended.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Produces one sample. Advances internal phase.
    pub fn next_sample(&mut self) -> f32 {
        let sample = self.phase.sin();
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
    440.0 * libm_powf(2.0, (f32::from(note_midi) - 69.0) / 12.0)
}

/// Local `powf` shim. `f32::powf` is in `std`, which is fine here — kept as
/// a thin wrapper so future no_std experiments only change one line.
#[inline]
fn libm_powf(base: f32, exponent: f32) -> f32 {
    base.powf(exponent)
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
}
