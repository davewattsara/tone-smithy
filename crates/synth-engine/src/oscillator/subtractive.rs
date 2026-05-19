//! Multi-shape phase-accumulating oscillator for the subtractive slot.
//!
//! One oscillator owns one phase accumulator and produces one of the
//! four subtractive shapes: sine, saw, square, triangle. Saw and
//! square are band-limited with [`crate::oscillator::polyblep`] so
//! they stay clean up into the highest playable octaves; sine and
//! triangle are generated naively (sine has no harmonics above the
//! fundamental, and the polynomial-triangle approximation is
//! perceptually clean enough that integrating polyBLEP into a square
//! is overkill for v1 — the trade-off in
//! `docs/planning/05-design/dsp-and-sound.md` allows either form).
//!
//! Phase is stored in normalised \[0, 1\) units rather than radians so
//! the polyBLEP math matches the published derivation 1:1. The
//! conversion to radians for the sine path is a single multiply.
//!
//! The voice owns three of these (plus a dedicated sub oscillator)
//! and sums them into the slot mixer.

use core::f32::consts::TAU;

use super::polyblep::poly_blep;

/// The shape an oscillator produces.
///
/// `Waveform` is a discrete parameter — switching mid-block would
/// cause an audible step. Adapters must change it via [`EngineEvent`]
/// so the change lands at a block boundary; the engine drains events
/// once per block before processing samples
/// (`docs/planning/03-architecture/design-patterns.md` §2.7).
///
/// [`EngineEvent`]: crate::EngineEvent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Waveform {
    /// Pure sine, fundamental only. No aliasing concerns.
    #[default]
    Sine,

    /// Band-limited (polyBLEP) ascending sawtooth.
    Saw,

    /// Band-limited (polyBLEP) 50%-duty square.
    Square,

    /// Naive (linear-segment) triangle. The fundamental dominates so
    /// audible aliasing only sets in at very high pitches; the
    /// polyBLAMP-integrated form is a v1.1 upgrade.
    Triangle,
}

/// A single-shape phase-accumulating oscillator.
///
/// Carries no state about pitch beyond `phase_increment`. The caller
/// (the voice) is responsible for converting MIDI note + tuning into
/// a frequency and calling [`Oscillator::set_frequency_hz`] when it
/// changes.
pub struct Oscillator {
    /// Sample rate in Hz, captured at construction.
    sample_rate_hz: f32,

    /// Current phase, normalised to \[0, 1\). Stored in this unit
    /// rather than radians so polyBLEP arithmetic matches the
    /// published form without extra conversions.
    phase: f32,

    /// Per-sample phase advance, also in \[0, 1\) (frequency divided
    /// by sample rate).
    phase_increment: f32,

    /// Current waveform. Changes take effect on the next sample; the
    /// engine guarantees changes land at a block boundary.
    waveform: Waveform,
}

impl Oscillator {
    /// Creates an oscillator producing the default waveform
    /// ([`Waveform::Sine`]) at 0 Hz. Call
    /// [`Oscillator::set_frequency_hz`] before processing.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            phase: 0.0,
            phase_increment: 0.0,
            waveform: Waveform::default(),
        }
    }

    /// Sets the oscillator frequency in Hz. Negative and zero values
    /// are accepted and produce no useful sound; the caller should
    /// clamp.
    pub fn set_frequency_hz(&mut self, frequency_hz: f32) {
        self.phase_increment = frequency_hz / self.sample_rate_hz;
    }

    /// Sets the active waveform. Takes effect on the next call to
    /// [`Oscillator::next_sample`].
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    /// Resets the phase to zero. Called on note-on so each note
    /// starts from a known phase; this avoids the random-DC artefacts
    /// that follow from leaving the phase wherever the last note
    /// ended.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Produces one sample and advances the internal phase.
    pub fn next_sample(&mut self) -> f32 {
        let t = self.phase;
        let dt = self.phase_increment;

        let sample = match self.waveform {
            Waveform::Sine => (t * TAU).sin(),
            Waveform::Saw => {
                // Naive saw rises from -1 to +1 across [0, 1) and
                // steps back to -1 at the wrap. polyBLEP smooths the
                // step.
                let naive = 2.0 * t - 1.0;
                naive - poly_blep(t, dt)
            }
            Waveform::Square => {
                // Naive square is +1 on the first half, -1 on the
                // second. Two discontinuities per cycle: an upward
                // step at t == 0 (corrected by + polyBLEP) and a
                // downward step at t == 0.5 (corrected by - polyBLEP
                // of the half-shifted phase).
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                naive + poly_blep(t, dt) - poly_blep(fract(t + 0.5), dt)
            }
            Waveform::Triangle => {
                // 4 * |t - 0.5| - 1 traces 1 → -1 → 1 across one cycle.
                4.0 * (t - 0.5).abs() - 1.0
            }
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        sample
    }
}

/// Fractional part for non-negative inputs in \[0, 2). Equivalent to
/// `x - floor(x)`; used for the half-shifted polyBLEP phase in the
/// square wave. Avoids a libm call.
#[inline]
fn fract(x: f32) -> f32 {
    if x >= 1.0 { x - 1.0 } else { x }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(osc: &mut Oscillator, frames: usize) -> Vec<f32> {
        (0..frames).map(|_| osc.next_sample()).collect()
    }

    #[test]
    fn sine_oscillator_stays_in_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.0 + 1e-6);
        }
    }

    #[test]
    fn saw_oscillator_stays_close_to_unit_bounds() {
        // polyBLEP can overshoot ±1 by a small fraction of dt near the
        // discontinuity. At 440 Hz / 48 kHz that fraction is tiny;
        // allow a small headroom rather than asserting exact ±1.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.05, "saw out of bounds: {s}");
        }
    }

    #[test]
    fn square_oscillator_stays_close_to_unit_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Square);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.05, "square out of bounds: {s}");
        }
    }

    #[test]
    fn triangle_oscillator_stays_in_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Triangle);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.0 + 1e-6, "triangle out of bounds: {s}");
        }
    }

    #[test]
    fn saw_completes_full_swing_per_period() {
        // At 480 Hz on a 48 kHz sample rate, one period is exactly 100
        // samples. Even after polyBLEP smooths the discontinuity, the
        // ramp still sweeps near ±1 in the body of the cycle.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(480.0);
        let samples = run(&mut osc, 100);
        let min = samples.iter().copied().fold(f32::INFINITY, f32::min);
        let max = samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(min < -0.9, "expected near -1.0, got {min}");
        assert!(max > 0.9, "expected near +1.0, got {max}");
    }

    #[test]
    fn square_has_zero_dc_over_one_period() {
        // Symmetric 50% duty: positive half cancels negative half.
        // 100-sample period at 480 Hz / 48 kHz divides evenly.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Square);
        osc.set_frequency_hz(480.0);
        let samples = run(&mut osc, 100);
        #[allow(clippy::cast_precision_loss)]
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.05, "square DC = {mean}, expected near 0");
    }

    #[test]
    fn triangle_is_symmetric_over_one_period() {
        // 1 → -1 → 1 trace across [0, 1) is mirror-symmetric about
        // t = 0.5; sum over a full cycle is zero.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Triangle);
        osc.set_frequency_hz(480.0);
        let samples = run(&mut osc, 100);
        #[allow(clippy::cast_precision_loss)]
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.02, "triangle DC = {mean}, expected near 0");
    }

    #[test]
    fn polyblep_saw_matches_naive_away_from_discontinuity() {
        // In the bulk of a cycle (well away from the wrap) polyBLEP
        // returns zero, so the output equals the naive ramp 2t - 1.
        // At 48 Hz / 48 kHz, dt = 0.001; after 100 samples the phase
        // is 0.100, well outside the discontinuity window. Naive
        // saw at t = 0.100 is 2*0.100 - 1 = -0.800.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(48.0);
        let _warmup = run(&mut osc, 100);
        let s = osc.next_sample();
        assert!((s - (-0.798)).abs() < 0.01, "expected ~-0.798, got {s}");
    }

    #[test]
    fn reset_phase_returns_to_zero() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_frequency_hz(440.0);
        let _ = run(&mut osc, 250);
        osc.reset_phase();
        // First sample after reset is sin(0) = 0 for sine.
        assert_eq!(osc.next_sample(), 0.0);
    }
}
