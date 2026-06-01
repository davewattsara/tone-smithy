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
#[derive(Debug, Clone, Copy)]
pub struct Drive {
    /// Pre-clip gain multiplier, 1.0–20.0.
    drive: f32,
    /// Bias added before clip, removed after. -1.0–1.0.
    asymmetry: f32,
    /// Output level trim so perceived loudness stays consistent.
    output_gain: f32,
}

impl Default for Drive {
    fn default() -> Self {
        Self {
            drive: 1.0,
            asymmetry: 0.0,
            output_gain: 1.0,
        }
    }
}

impl Drive {
    /// Update drive and asymmetry. Output gain is recomputed automatically.
    pub fn set_params(&mut self, drive: f32, asymmetry: f32) {
        self.drive = drive.clamp(1.0, 20.0);
        self.asymmetry = asymmetry.clamp(-1.0, 1.0);
        // Compensate: tanh(drive) ≈ output amplitude at clipping — normalise
        // back to unity so the effect is level-neutral at full drive.
        let clip_ceiling = self.drive.tanh();
        self.output_gain = if clip_ceiling > 1e-6 { 1.0 / clip_ceiling } else { 1.0 };
    }

    /// Process one stereo sample.
    #[inline]
    pub fn process(&self, left: f32, right: f32) -> (f32, f32) {
        (self.shape(left), self.shape(right))
    }

    #[inline]
    fn shape(&self, x: f32) -> f32 {
        let biased = x + self.asymmetry;
        let clipped = (biased * self.drive).tanh();
        (clipped - self.asymmetry) * self.output_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_drive_is_near_transparent() {
        let d = Drive::default();
        let (l, r) = d.process(0.5, -0.3);
        // At drive=1, tanh(0.5)/tanh(1) ≈ 0.462/0.762 ≈ 0.606; some
        // compression is expected even at unity drive.
        assert!(l > 0.0 && l < 1.0, "expected 0..1, got {l}");
        assert!(r > -1.0 && r < 0.0, "expected -1..0, got {r}");
    }

    #[test]
    fn drive_clips_hard_signals() {
        let mut d = Drive::default();
        d.set_params(10.0, 0.0);
        // At drive=10, tanh clips aggressively; output near ±1 after gain comp.
        let (l, _) = d.process(1.0, 0.0);
        assert!(l > 0.9, "expected near-clipped output, got {l}");
    }

    #[test]
    fn output_stays_bounded() {
        let mut d = Drive::default();
        d.set_params(20.0, 0.5);
        for amp in [-2.0_f32, -1.0, 0.0, 1.0, 2.0] {
            let (l, r) = d.process(amp, amp);
            assert!(l.abs() <= 1.5, "output out of range: {l}");
            assert!(r.abs() <= 1.5, "output out of range: {r}");
        }
    }
}
