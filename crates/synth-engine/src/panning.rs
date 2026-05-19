//! Pan-law helpers shared by the slot mixer and the unison
//! oscillator.
//!
//! Lives in its own module because both `voice` and
//! `oscillator::subtractive` need to apply equal-power pans (voice for
//! the per-oscillator center pan, the unison oscillator for the
//! per-unison-voice pan within an oscillator's stereo field) and
//! either of them importing from the other would create a cycle.

/// Equal-power pan. `pan` is in \[-1, 1\]: -1 is full left, +1 is full
/// right, 0 is centred (each channel at `1 / sqrt(2)`). Two sqrts per
/// call, no transcendentals. Out-of-range inputs are clamped.
#[inline]
pub(crate) fn equal_power_pan(pan: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    let l = ((1.0 - p) * 0.5).sqrt();
    let r = ((1.0 + p) * 0.5).sqrt();
    (l, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_power_across_the_full_range() {
        for i in -100..=100 {
            #[allow(clippy::cast_precision_loss)]
            let p = (i as f32) / 100.0;
            let (l, r) = equal_power_pan(p);
            let power = l * l + r * r;
            assert!((power - 1.0).abs() < 1e-6, "pan {p}: L²+R² = {power}");
        }
    }

    #[test]
    fn hard_left_silences_right() {
        let (l, r) = equal_power_pan(-1.0);
        assert!((l - 1.0).abs() < 1e-6, "L = {l}");
        assert!(r.abs() < 1e-6, "R = {r}");
    }

    #[test]
    fn hard_right_silences_left() {
        let (l, r) = equal_power_pan(1.0);
        assert!(l.abs() < 1e-6, "L = {l}");
        assert!((r - 1.0).abs() < 1e-6, "R = {r}");
    }

    #[test]
    fn out_of_range_inputs_are_clamped() {
        assert_eq!(equal_power_pan(-2.0), equal_power_pan(-1.0));
        assert_eq!(equal_power_pan(2.0), equal_power_pan(1.0));
    }
}
