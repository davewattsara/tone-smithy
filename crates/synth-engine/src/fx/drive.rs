//! Soft-clip drive with optional asymmetry.
//!
//! Signal path: pre-gain → bias (asymmetry) → tanh clip → remove bias →
//! output level compensation.

/// Soft-clip saturation stage.
///
/// `drive` (1.0–20.0) controls the pre-gain before the tanh clip.
/// `asymmetry` (-1.0–1.0) biases the signal before clipping and removes
/// the bias afterwards, creating even-harmonic content (asymmetric
/// waveshaping) when non-zero. Zero asymmetry is purely odd-harmonic.
///
/// Parameters are smoothed with a one-pole filter (~10 ms) to prevent
/// audible clicks when knobs are adjusted during playback.
#[derive(Debug, Clone, Copy)]
pub struct Drive {
    // Targets updated by set_params
    target_drive: f32,
    target_asymmetry: f32,
    target_output_gain: f32,
    // Smoothed values used in process()
    drive: f32,
    asymmetry: f32,
    output_gain: f32,
    /// One-pole smoothing coefficient.
    smooth: f32,
}

impl Drive {
    pub fn new(sample_rate_hz: f32) -> Self {
        // ~10 ms time constant
        let smooth = (-1.0 / (0.010 * sample_rate_hz)).exp();
        Self {
            target_drive: 1.0,
            target_asymmetry: 0.0,
            target_output_gain: 1.0,
            drive: 1.0,
            asymmetry: 0.0,
            output_gain: 1.0,
            smooth,
        }
    }

    /// Update drive and asymmetry. Output gain is recomputed automatically.
    pub fn set_params(&mut self, drive: f32, asymmetry: f32) {
        self.target_drive = drive.clamp(1.0, 20.0);
        self.target_asymmetry = asymmetry.clamp(-1.0, 1.0);
        // Normalise at a reference input amplitude of 0.5: for any drive setting,
        // a 0.5-peak signal produces a 0.5-peak output. Signals above 0.5 are
        // progressively clipped; signals below 0.5 pass with near-unity gain.
        let ref_out = (self.target_drive * 0.5).tanh();
        self.target_output_gain = if ref_out > 1e-6 { 0.5 / ref_out } else { 1.0 };
    }

    /// Process one stereo sample.
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Advance smoothed values one step toward targets
        self.drive = self.target_drive + (self.drive - self.target_drive) * self.smooth;
        self.asymmetry = self.target_asymmetry + (self.asymmetry - self.target_asymmetry) * self.smooth;
        self.output_gain = self.target_output_gain + (self.output_gain - self.target_output_gain) * self.smooth;
        (self.shape(left), self.shape(right))
    }

    #[inline]
    fn shape(&self, x: f32) -> f32 {
        // Bias is injected after the drive gain so it stays bounded to ±1
        // regardless of drive level. DC removal uses tanh(asymmetry), not the
        // raw asymmetry value, because the tanh clips the bias too.
        let clipped = (x * self.drive + self.asymmetry).tanh();
        let dc = self.asymmetry.tanh();
        (clipped - dc) * self.output_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_drive_is_near_transparent() {
        let mut d = Drive::new(48_000.0);
        // Warm up smoother so current values equal targets
        for _ in 0..4800 {
            d.process(0.5, -0.3);
        }
        let (l, r) = d.process(0.5, -0.3);
        // At drive=1 output_gain ≈ 1.08; a 0.5 input maps to ≈ 0.5 (level-neutral).
        assert!(l > 0.0 && l < 1.0, "expected 0..1, got {l}");
        assert!(r > -1.0 && r < 0.0, "expected -1..0, got {r}");
    }

    #[test]
    fn drive_clips_hard_signals() {
        let mut d = Drive::new(48_000.0);
        d.set_params(10.0, 0.0);
        // Warm up smoother
        for _ in 0..4800 {
            d.process(1.0, 0.0);
        }
        // Reference amplitude is 0.5, so a 1.0 input should clip near the
        // ceiling (~0.5) and a 0.7 input should produce nearly the same output.
        let (l1, _) = d.process(1.0, 0.0);
        let (l2, _) = d.process(0.7, 0.0);
        assert!(l1 > 0.4, "expected output near ceiling, got {l1}");
        assert!(
            (l1 - l2).abs() < 0.05,
            "both should be clipped to same ceiling: {l1} vs {l2}"
        );
    }

    #[test]
    fn output_stays_bounded() {
        let mut d = Drive::new(48_000.0);
        d.set_params(20.0, 0.5);
        for amp in [-2.0_f32, -1.0, 0.0, 1.0, 2.0] {
            let (l, r) = d.process(amp, amp);
            assert!(l.abs() <= 1.5, "output out of range: {l}");
            assert!(r.abs() <= 1.5, "output out of range: {r}");
        }
    }
}
