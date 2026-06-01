//! 3-band parametric EQ: low shelf, peak mid, high shelf.

use super::biquad::{Biquad, high_shelf_coeffs, low_shelf_coeffs, peak_eq_coeffs};

/// 3-band parametric EQ with a low shelf, mid peak, and high shelf.
///
/// Each band is a second-order biquad. Coefficients are computed once
/// when parameters change and applied sample-by-sample — never
/// recomputed at audio rate.
///
/// Both stereo channels share the same coefficients but maintain
/// independent state to preserve stereo image.
#[derive(Debug, Clone)]
pub struct Eq3Band {
    sample_rate_hz: f32,

    // Per-band state (L and R)
    low_l: Biquad,
    low_r: Biquad,
    mid_l: Biquad,
    mid_r: Biquad,
    high_l: Biquad,
    high_r: Biquad,

    // Stored params so we can detect changes
    low_freq_hz: f32,
    low_gain_db: f32,
    mid_freq_hz: f32,
    mid_gain_db: f32,
    mid_q: f32,
    high_freq_hz: f32,
    high_gain_db: f32,
}

impl Eq3Band {
    /// Construct with default settings (all bands at 0 dB gain).
    pub fn new(sample_rate_hz: f32) -> Self {
        let mut eq = Self {
            sample_rate_hz,
            low_l: Biquad::default(),
            low_r: Biquad::default(),
            mid_l: Biquad::default(),
            mid_r: Biquad::default(),
            high_l: Biquad::default(),
            high_r: Biquad::default(),
            low_freq_hz: 200.0,
            low_gain_db: 0.0,
            mid_freq_hz: 1_000.0,
            mid_gain_db: 0.0,
            mid_q: 0.7,
            high_freq_hz: 6_000.0,
            high_gain_db: 0.0,
        };
        eq.recompute_all();
        eq
    }

    /// Update low-shelf parameters and recompute coefficients.
    pub fn set_low(&mut self, freq_hz: f32, gain_db: f32) {
        self.low_freq_hz = freq_hz.clamp(20.0, 2_000.0);
        self.low_gain_db = gain_db.clamp(-15.0, 15.0);
        let (b0, b1, b2, a1, a2) = low_shelf_coeffs(self.low_freq_hz, self.low_gain_db, self.sample_rate_hz);
        self.low_l.set_coeffs(b0, b1, b2, a1, a2);
        self.low_r.set_coeffs(b0, b1, b2, a1, a2);
    }

    /// Update mid-peak parameters and recompute coefficients.
    pub fn set_mid(&mut self, freq_hz: f32, gain_db: f32, q: f32) {
        self.mid_freq_hz = freq_hz.clamp(200.0, 8_000.0);
        self.mid_gain_db = gain_db.clamp(-15.0, 15.0);
        self.mid_q = q.clamp(0.1, 10.0);
        let (b0, b1, b2, a1, a2) = peak_eq_coeffs(self.mid_freq_hz, self.mid_gain_db, self.mid_q, self.sample_rate_hz);
        self.mid_l.set_coeffs(b0, b1, b2, a1, a2);
        self.mid_r.set_coeffs(b0, b1, b2, a1, a2);
    }

    /// Update high-shelf parameters and recompute coefficients.
    pub fn set_high(&mut self, freq_hz: f32, gain_db: f32) {
        self.high_freq_hz = freq_hz.clamp(2_000.0, 20_000.0);
        self.high_gain_db = gain_db.clamp(-15.0, 15.0);
        let (b0, b1, b2, a1, a2) = high_shelf_coeffs(self.high_freq_hz, self.high_gain_db, self.sample_rate_hz);
        self.high_l.set_coeffs(b0, b1, b2, a1, a2);
        self.high_r.set_coeffs(b0, b1, b2, a1, a2);
    }

    /// Process one stereo sample through all three bands.
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let l = self.high_l.process(self.mid_l.process(self.low_l.process(left)));
        let r = self.high_r.process(self.mid_r.process(self.low_r.process(right)));
        (l, r)
    }

    fn recompute_all(&mut self) {
        let (b0, b1, b2, a1, a2) = low_shelf_coeffs(self.low_freq_hz, self.low_gain_db, self.sample_rate_hz);
        self.low_l.set_coeffs(b0, b1, b2, a1, a2);
        self.low_r.set_coeffs(b0, b1, b2, a1, a2);

        let (b0, b1, b2, a1, a2) = peak_eq_coeffs(self.mid_freq_hz, self.mid_gain_db, self.mid_q, self.sample_rate_hz);
        self.mid_l.set_coeffs(b0, b1, b2, a1, a2);
        self.mid_r.set_coeffs(b0, b1, b2, a1, a2);

        let (b0, b1, b2, a1, a2) = high_shelf_coeffs(self.high_freq_hz, self.high_gain_db, self.sample_rate_hz);
        self.high_l.set_coeffs(b0, b1, b2, a1, a2);
        self.high_r.set_coeffs(b0, b1, b2, a1, a2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_eq_is_transparent() {
        let mut eq = Eq3Band::new(48_000.0);
        // Warm up
        for _ in 0..1000 {
            eq.process(1.0, 1.0);
        }
        let (l, r) = eq.process(1.0, 1.0);
        assert!((l - 1.0).abs() < 1e-3, "L should be ~1.0, got {l}");
        assert!((r - 1.0).abs() < 1e-3, "R should be ~1.0, got {r}");
    }
}
