//! Parameter smoothing on the audio thread.
//!
//! Continuous parameters must not jump instantaneously when the UI
//! changes them — sudden value changes cause audible clicks ("zipper
//! noise") on whatever the parameter modulates. The fix is the one-pole
//! filter from `docs/planning/03-architecture/design-patterns.md` §2.6:
//! the UI sets a target, the audio thread advances `current` toward
//! `target` each sample.

/// Default smoothing time, in milliseconds.
///
/// 10 ms is the middle of the §2.6 "typical 5–20 ms" range. Short
/// enough that the UI feels responsive, long enough to hide a hard
/// step in cutoff or pitch. Parameters with their own preferred time
/// constant (slow level fades, fast filter sweeps) pass their own
/// value to [`SmoothedParam::with_time_constant_ms`].
pub const DEFAULT_TIME_CONSTANT_MS: f32 = 10.0;

/// One-pole low-pass smoother for a continuous parameter.
///
/// `set_target` is called from the audio thread when an event arrives;
/// `next` runs per sample to advance `current` toward `target`. The
/// filter coefficient is computed at construction from sample rate and
/// time constant — no `f32::exp` or `f32::tan` on the audio path.
pub struct SmoothedParam {
    target: f32,
    current: f32,

    /// Per-sample interpolation coefficient in 0.0..=1.0. The chosen
    /// approximation (`dt / tau`) keeps construction simple and is
    /// accurate enough at typical sample rates (<1% error for
    /// `tau >> 1 / sample_rate`).
    coeff: f32,
}

impl SmoothedParam {
    /// Creates a smoother starting at `initial_value` with the default
    /// time constant ([`DEFAULT_TIME_CONSTANT_MS`]).
    #[must_use]
    pub fn new(initial_value: f32, sample_rate_hz: f32) -> Self {
        Self::with_time_constant_ms(initial_value, sample_rate_hz, DEFAULT_TIME_CONSTANT_MS)
    }

    /// Creates a smoother with a caller-chosen time constant.
    #[must_use]
    pub fn with_time_constant_ms(
        initial_value: f32,
        sample_rate_hz: f32,
        time_constant_ms: f32,
    ) -> Self {
        let tau_samples = (time_constant_ms / 1000.0) * sample_rate_hz;
        // Guard against zero / negative tau producing NaN or > 1 coeff.
        let coeff = if tau_samples > 1.0 { 1.0 / tau_samples } else { 1.0 };
        Self { target: initial_value, current: initial_value, coeff }
    }

    /// Sets the target value the smoother is heading toward. Called
    /// from the audio thread when an event arrives.
    pub fn set_target(&mut self, value: f32) {
        self.target = value;
    }

    /// Snaps `current` to `target` immediately. Used at note-on or
    /// initialisation when smoothing in from the previous value would
    /// be wrong.
    pub fn snap_to_target(&mut self) {
        self.current = self.target;
    }

    /// Advances one sample and returns the new value.
    pub fn next_sample(&mut self) -> f32 {
        self.current += self.coeff * (self.target - self.current);
        self.current
    }

    /// Returns the current smoothed value without advancing.
    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_to_target_short_circuits_smoothing() {
        let mut p = SmoothedParam::new(0.0, 48_000.0);
        p.set_target(5.0);
        p.snap_to_target();
        assert_eq!(p.current(), 5.0);
    }

    #[test]
    fn reaches_target_after_several_time_constants() {
        let sample_rate = 48_000.0;
        let time_constant_ms = 10.0;
        let mut p = SmoothedParam::with_time_constant_ms(0.0, sample_rate, time_constant_ms);
        p.set_target(1.0);

        // After 5 time constants the one-pole is within ~1% of target
        // (one-pole step response: 1 - e^-5 ~= 0.993).
        let frames = (5.0 * time_constant_ms / 1000.0 * sample_rate) as usize;
        for _ in 0..frames {
            p.next_sample();
        }
        assert!(
            (p.current() - 1.0).abs() < 0.05,
            "after 5 time constants, expected ~1.0, got {}",
            p.current(),
        );
    }

    #[test]
    fn no_jump_on_target_change() {
        let mut p = SmoothedParam::new(0.0, 48_000.0);
        p.set_target(1.0);
        let first = p.next_sample();
        // The step from 0 to 1 must not happen in a single sample; the
        // coefficient is 1/(0.010 * 48000) = ~0.00208, so the first
        // sample should be a tiny fraction of 1.
        assert!(first < 0.01, "expected gradual rise, got {first} on first sample");
    }
}
