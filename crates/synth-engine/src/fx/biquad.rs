//! Second-order biquad filter — Direct Form II Transposed.
//!
//! Used internally by [`super::eq`] and [`super::delay`]. Not exposed on
//! the public engine surface; callers construct coefficients using the
//! helper functions below and pass them to [`Biquad::set_coeffs`].
//!
//! Coefficients follow the Audio EQ Cookbook convention:
//!   H(z) = (b0 + b1*z⁻¹ + b2*z⁻²) / (1 + a1*z⁻¹ + a2*z⁻²)
//! with the leading `a0` normalised out (all `a`s divided by `a0`).

use std::f32::consts::PI;

/// Second-order IIR filter, Direct Form II Transposed.
///
/// Maintains two samples of state (`w1`, `w2`). Coefficients can be
/// updated between samples with [`Biquad::set_coeffs`]; the transition
/// is not click-free but is inaudible for slow parameter changes.
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    w1: f32,
    w2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            w1: 0.0,
            w2: 0.0,
        }
    }
}

impl Biquad {
    /// Update filter coefficients without clearing state.
    pub fn set_coeffs(&mut self, b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) {
        self.b0 = b0;
        self.b1 = b1;
        self.b2 = b2;
        self.a1 = a1;
        self.a2 = a2;
    }

    /// Process one sample.
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.w1;
        self.w1 = self.b1 * x - self.a1 * y + self.w2;
        self.w2 = self.b2 * x - self.a2 * y;
        y
    }

    /// Reset filter memory to zero (used on preset change).
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.w1 = 0.0;
        self.w2 = 0.0;
    }
}

// ── Coefficient helpers ────────────────────────────────────────────────────

/// Coefficients for a low-shelf filter.
/// `freq_hz` is the shelf midpoint; `gain_db` is the shelf gain (positive = boost).
pub fn low_shelf_coeffs(freq_hz: f32, gain_db: f32, sample_rate_hz: f32) -> (f32, f32, f32, f32, f32) {
    let a = 10.0_f32.powf(gain_db / 40.0); // sqrt of linear gain
    let w0 = 2.0 * PI * freq_hz / sample_rate_hz;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let s = 1.0_f32; // shelf slope, 1 = maximally steep
    // alpha = sin(w0)/2 * sqrt( (A + 1/A)*(1/S - 1) + 2 )   (Audio EQ Cookbook)
    let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();

    let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
    let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
    let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
    let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
    let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
    let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;

    (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
}

/// Coefficients for a high-shelf filter.
pub fn high_shelf_coeffs(freq_hz: f32, gain_db: f32, sample_rate_hz: f32) -> (f32, f32, f32, f32, f32) {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq_hz / sample_rate_hz;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let s = 1.0_f32;
    let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();

    let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
    let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
    let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
    let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
    let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
    let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;

    (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
}

/// Coefficients for a peaking EQ band.
/// `q` controls bandwidth; `gain_db` is the peak/cut amount.
pub fn peak_eq_coeffs(freq_hz: f32, gain_db: f32, q: f32, sample_rate_hz: f32) -> (f32, f32, f32, f32, f32) {
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq_hz / sample_rate_hz;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha = sin_w0 / (2.0 * q);

    let b0 = 1.0 + alpha * a;
    let b1 = -2.0 * cos_w0;
    let b2 = 1.0 - alpha * a;
    let a0 = 1.0 + alpha / a;
    let a1 = -2.0 * cos_w0;
    let a2 = 1.0 - alpha / a;

    (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
}

/// Coefficients for a one-pole high-pass (low-cut) filter.
/// Uses a simple first-order approximation; stored in the biquad with
/// `b2 = a2 = 0`.
pub fn one_pole_highpass_coeffs(freq_hz: f32, sample_rate_hz: f32) -> (f32, f32, f32, f32, f32) {
    let x = (-2.0 * PI * freq_hz / sample_rate_hz).exp();
    let b0 = (1.0 + x) / 2.0;
    let b1 = -(1.0 + x) / 2.0;
    let a1 = -x;
    (b0, b1, 0.0, a1, 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_gain_passes_dc() {
        let mut b = Biquad::default();
        for _ in 0..100 {
            let y = b.process(1.0);
            assert!((y - 1.0).abs() < 1e-5, "expected 1.0, got {y}");
        }
    }

    #[test]
    fn low_shelf_boost_at_low_freq() {
        let sr = 48_000.0_f32;
        let (b0, b1, b2, a1, a2) = low_shelf_coeffs(200.0, 12.0, sr);
        let mut filt = Biquad::default();
        filt.set_coeffs(b0, b1, b2, a1, a2);
        // Drive with DC to measure low-frequency gain (should be ~4× = +12 dB)
        let mut out = 0.0_f32;
        for _ in 0..1000 {
            out = filt.process(1.0);
        }
        assert!(out > 2.5, "expected boost at DC, got {out}");
    }

    #[test]
    fn peak_eq_zero_gain_is_transparent() {
        let sr = 48_000.0_f32;
        let (b0, b1, b2, a1, a2) = peak_eq_coeffs(1000.0, 0.0, 0.7, sr);
        let mut filt = Biquad::default();
        filt.set_coeffs(b0, b1, b2, a1, a2);
        let mut out = 0.0_f32;
        for _ in 0..1000 {
            out = filt.process(1.0);
        }
        assert!((out - 1.0).abs() < 1e-4, "0 dB peak should be transparent, got {out}");
    }
}
