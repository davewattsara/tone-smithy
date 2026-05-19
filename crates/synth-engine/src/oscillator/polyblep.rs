//! Band-limiting residual for trivial waveforms (Välimäki–Huovilainen
//! "polyBLEP").
//!
//! A naive sawtooth or square wave has instantaneous discontinuities
//! whose spectra extend infinitely. Once sampled, that infinite content
//! aliases back into the audible band — the higher the fundamental,
//! the more obvious it becomes. polyBLEP replaces each step with a
//! short, smooth polynomial that takes ~2 samples to traverse, killing
//! most of the aliasing for a few cycles of extra arithmetic per
//! sample.
//!
//! The function returns the residual that the *naive* generator's
//! value should be **subtracted** from (for a downward step) or
//! **added** to (for an upward step) to obtain the band-limited form.
//! Phase and increment are in normalised \[0, 1\) units, not radians,
//! to keep the math close to the published derivation.

/// Computes the polyBLEP residual at normalised phase `t` for a
/// step-discontinuity at `t == 0` (equivalently `t == 1`), with the
/// per-sample phase increment `dt`.
///
/// Returns 0 away from the step (i.e. for `dt <= t <= 1 - dt`) so the
/// callsite can unconditionally subtract/add without branching twice.
/// The returned magnitude is bounded by 1.0.
pub fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        // Near the leading edge of the discontinuity: t in [0, dt).
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        // Near the trailing edge: t in (1 - dt, 1).
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_zero_well_away_from_step() {
        let dt = 0.01;
        for &t in &[0.1, 0.25, 0.5, 0.75, 0.9] {
            assert_eq!(poly_blep(t, dt), 0.0, "t = {t}");
        }
    }

    #[test]
    fn matches_naive_step_at_exact_edges() {
        // At t == 0 the residual is -1.0; as t approaches 1 (i.e. just
        // before wrapping) the residual approaches +1.0. The two
        // halves are exact mirror images, so the corrected saw lands
        // smoothly on zero at the discontinuity.
        let dt = 0.01;
        assert!((poly_blep(0.0, dt) - -1.0).abs() < 1e-6);
        // Just inside the trailing window. t' = -1, residual = 0.
        assert!(poly_blep(1.0 - dt, dt).abs() < 1e-6);
        // Closer to t = 1.
        let near_end = 1.0 - dt / 4.0;
        let r = poly_blep(near_end, dt);
        assert!(r > 0.4 && r <= 1.0, "expected ~+0.5..1.0 near end, got {r}");
    }

    #[test]
    fn residual_is_bounded_by_one() {
        let dt = 0.05;
        // Walk the full unit interval at fine resolution and check
        // that the residual never escapes [-1, 1] — that bound is what
        // keeps the saw/square output close to ±1.
        let steps = 10_000;
        for i in 0..=steps {
            #[allow(clippy::cast_precision_loss)]
            let t = (i as f32) / (steps as f32);
            let r = poly_blep(t, dt);
            assert!(r.abs() <= 1.0 + 1e-6, "residual {r} out of bounds at t={t}");
        }
    }
}
