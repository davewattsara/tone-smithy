//! Post-mix effects chain: EQ → Drive → Chorus → Delay → Reverb.
//!
//! [`FxChain`] is owned by the [`crate::engine::Engine`] and called once
//! per sample after the voice pool sums. It is entirely heap-allocated
//! at construction — zero allocations on the audio thread in steady state.

mod biquad;
pub mod chorus;
pub mod delay;
pub mod drive;
pub mod eq;
pub mod reverb;

use chorus::Chorus;
use delay::StereoDelay;
use drive::Drive;
use eq::Eq3Band;
use reverb::Fdn8Reverb;

/// All five post-mix effects in series.
///
/// Call [`FxChain::process`] once per sample after summing voices.
/// Setters push parameter updates into the chain so the audio path
/// reads only the currently-set values.
pub struct FxChain {
    // ── EQ ────────────────────────────────────────────────────────────────
    eq: Eq3Band,
    eq_enabled: bool,

    // ── Drive ─────────────────────────────────────────────────────────────
    drive: Drive,
    drive_enabled: bool,

    // ── Chorus ────────────────────────────────────────────────────────────
    chorus: Chorus,
    chorus_enabled: bool,

    // ── Delay ─────────────────────────────────────────────────────────────
    delay: StereoDelay,
    delay_enabled: bool,

    // ── Reverb ────────────────────────────────────────────────────────────
    reverb: Fdn8Reverb,
    reverb_enabled: bool,
}

impl FxChain {
    /// Construct with default settings. Allocates all delay buffers.
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            eq: Eq3Band::new(sample_rate_hz),
            eq_enabled: false,
            drive: Drive::default(),
            drive_enabled: false,
            chorus: Chorus::new(sample_rate_hz),
            chorus_enabled: false,
            delay: StereoDelay::new(sample_rate_hz),
            delay_enabled: false,
            reverb: Fdn8Reverb::new(sample_rate_hz),
            reverb_enabled: false,
        }
    }

    // ── EQ setters ─────────────────────────────────────────────────────────

    /// Toggle the EQ stage on or off.
    pub fn set_eq_enabled(&mut self, enabled: bool) {
        self.eq_enabled = enabled;
    }

    /// Update low-shelf parameters and recompute coefficients.
    pub fn set_eq_low(&mut self, freq_hz: f32, gain_db: f32) {
        self.eq.set_low(freq_hz, gain_db);
    }

    /// Update mid-peak parameters and recompute coefficients.
    pub fn set_eq_mid(&mut self, freq_hz: f32, gain_db: f32, q: f32) {
        self.eq.set_mid(freq_hz, gain_db, q);
    }

    /// Update high-shelf parameters and recompute coefficients.
    pub fn set_eq_high(&mut self, freq_hz: f32, gain_db: f32) {
        self.eq.set_high(freq_hz, gain_db);
    }

    // ── Drive setters ──────────────────────────────────────────────────────

    /// Toggle the drive stage on or off.
    pub fn set_drive_enabled(&mut self, enabled: bool) {
        self.drive_enabled = enabled;
    }

    /// Update drive amount (1–20) and asymmetry (-1..1).
    pub fn set_drive_params(&mut self, drive: f32, asymmetry: f32) {
        self.drive.set_params(drive, asymmetry);
    }

    // ── Chorus setters ─────────────────────────────────────────────────────

    /// Toggle the chorus stage.
    pub fn set_chorus_enabled(&mut self, enabled: bool) {
        self.chorus_enabled = enabled;
    }

    /// Update chorus parameters.
    pub fn set_chorus_params(&mut self, rate_hz: f32, depth_ms: f32, mix: f32, spread: f32) {
        self.chorus.set_params(rate_hz, depth_ms, mix, spread);
    }

    // ── Delay setters ──────────────────────────────────────────────────────

    /// Toggle the delay stage.
    pub fn set_delay_enabled(&mut self, enabled: bool) {
        self.delay_enabled = enabled;
    }

    /// Update delay parameters.
    pub fn set_delay_params(&mut self, time_secs: f32, feedback: f32, mix: f32, lowcut_hz: f32, ping_pong: bool) {
        self.delay.set_params(time_secs, feedback, mix, lowcut_hz, ping_pong);
    }

    // ── Reverb setters ─────────────────────────────────────────────────────

    /// Toggle the reverb stage.
    pub fn set_reverb_enabled(&mut self, enabled: bool) {
        self.reverb_enabled = enabled;
    }

    /// Update reverb parameters.
    pub fn set_reverb_params(&mut self, predelay_ms: f32, decay_secs: f32, size: f32, damping: f32, mix: f32) {
        self.reverb.set_params(predelay_ms, decay_secs, size, damping, mix);
    }

    // ── Audio path ─────────────────────────────────────────────────────────

    /// Process one stereo sample through the enabled effects.
    ///
    /// Called from the audio thread — zero allocations, no locks.
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (mut l, mut r) = (left, right);

        if self.eq_enabled {
            (l, r) = self.eq.process(l, r);
        }

        if self.drive_enabled {
            (l, r) = self.drive.process(l, r);
        }

        if self.chorus_enabled {
            (l, r) = self.chorus.process(l, r);
        }

        if self.delay_enabled {
            (l, r) = self.delay.process(l, r);
        }

        if self.reverb_enabled {
            (l, r) = self.reverb.process(l, r);
        }

        (l, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_disabled_is_transparent() {
        let mut fx = FxChain::new(48_000.0);
        let (l, r) = fx.process(0.5, -0.3);
        assert_eq!(l, 0.5);
        assert_eq!(r, -0.3);
    }

    #[test]
    fn enabled_chain_does_not_crash() {
        let mut fx = FxChain::new(48_000.0);
        fx.set_eq_enabled(true);
        fx.set_drive_enabled(true);
        fx.set_chorus_enabled(true);
        fx.set_delay_enabled(true);
        fx.set_reverb_enabled(true);

        fx.set_eq_low(200.0, 3.0);
        fx.set_eq_mid(1000.0, -2.0, 0.7);
        fx.set_eq_high(8000.0, 4.0);
        fx.set_drive_params(3.0, 0.1);
        fx.set_chorus_params(0.5, 3.0, 0.4, 0.5);
        fx.set_delay_params(0.375, 0.4, 0.25, 200.0, false);
        fx.set_reverb_params(10.0, 2.0, 0.7, 0.5, 0.25);

        for _ in 0..48_000 {
            let (l, r) = fx.process(0.5, 0.5);
            assert!(l.is_finite(), "FxChain produced non-finite L");
            assert!(r.is_finite(), "FxChain produced non-finite R");
        }
    }
}
