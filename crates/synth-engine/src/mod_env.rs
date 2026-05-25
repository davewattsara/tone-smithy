//! Modulation envelope (Env2).
//!
//! A block-rate ADSR with per-stage curve shaping. Unlike the amplitude
//! envelope (`Adsr`), which advances per sample, this envelope:
//!
//! - Is advanced **once per inner block** (e.g. every 64 samples).
//! - Has a curve parameter per timed stage in `[-1, +1]`:
//!   - `0` → linear
//!   - positive → fast initial change, slow tail ("snappy")
//!   - negative → slow initial change, fast tail ("smooth")
//! - Outputs unipolar `[0, 1]`. The mod matrix (M6) scales and inverts.
//!
//! The curve formula is `y = x^(2^(−curve))`, where `x` is the normalised
//! stage progress in `[0, 1]`.

/// Stage of the modulation envelope state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModEnvStage {
    /// Not running; output is 0.0.
    Idle,
    /// Rising toward 1.0 over `attack_secs`.
    Attack,
    /// Falling from 1.0 toward `sustain_level` over `decay_secs`.
    Decay,
    /// Held at `sustain_level` until `note_off`.
    Sustain,
    /// Falling toward 0.0 over `release_secs`; transitions to `Idle`.
    Release,
}

/// Modulation envelope with per-stage curve shaping.
///
/// Call [`advance`] once per inner block. Drive the note lifecycle via
/// [`note_on`] and [`note_off`]. Poll [`is_idle`] to know when the voice
/// manager may free the voice.
///
/// [`advance`]: ModEnv::advance
/// [`note_on`]: ModEnv::note_on
/// [`note_off`]: ModEnv::note_off
/// [`is_idle`]: ModEnv::is_idle
pub struct ModEnv {
    sample_rate_hz: f32,

    stage: ModEnvStage,

    /// Normalised progress through the current timed stage, `[0, 1)`.
    progress: f32,

    /// Output level at the start of the current stage. For Attack this is
    /// the level at `note_on` (legato-safe retrigger); for Release it is
    /// the level at `note_off`.
    stage_start_level: f32,

    attack_secs: f32,
    decay_secs: f32,
    sustain_level: f32,
    release_secs: f32,

    /// Curve for the Attack stage, clamped to `[-1, +1]`.
    attack_curve: f32,
    /// Curve for the Decay stage, clamped to `[-1, +1]`.
    decay_curve: f32,
    /// Curve for the Release stage, clamped to `[-1, +1]`.
    release_curve: f32,
}

impl ModEnv {
    /// Creates an idle envelope. Defaults: 10 ms attack / 200 ms decay /
    /// 0.8 sustain / 200 ms release, all stages linear (curve = 0).
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            stage: ModEnvStage::Idle,
            progress: 0.0,
            stage_start_level: 0.0,
            attack_secs: 0.010,
            decay_secs: 0.200,
            sustain_level: 0.8,
            release_secs: 0.200,
            attack_curve: 0.0,
            decay_curve: 0.0,
            release_curve: 0.0,
        }
    }

    /// Sets the attack time in seconds. Values below one sample are clamped up.
    pub fn set_attack_secs(&mut self, secs: f32) {
        self.attack_secs = secs.max(1.0 / self.sample_rate_hz);
    }

    /// Sets the decay time in seconds.
    pub fn set_decay_secs(&mut self, secs: f32) {
        self.decay_secs = secs.max(1.0 / self.sample_rate_hz);
    }

    /// Sets the sustain level, clamped to `[0, 1]`.
    pub fn set_sustain_level(&mut self, level: f32) {
        self.sustain_level = level.clamp(0.0, 1.0);
    }

    /// Sets the release time in seconds.
    pub fn set_release_secs(&mut self, secs: f32) {
        self.release_secs = secs.max(1.0 / self.sample_rate_hz);
    }

    /// Sets the Attack stage curve, clamped to `[-1, +1]`.
    pub fn set_attack_curve(&mut self, curve: f32) {
        self.attack_curve = curve.clamp(-1.0, 1.0);
    }

    /// Sets the Decay stage curve, clamped to `[-1, +1]`.
    pub fn set_decay_curve(&mut self, curve: f32) {
        self.decay_curve = curve.clamp(-1.0, 1.0);
    }

    /// Sets the Release stage curve, clamped to `[-1, +1]`.
    pub fn set_release_curve(&mut self, curve: f32) {
        self.release_curve = curve.clamp(-1.0, 1.0);
    }

    /// Starts (or restarts) the Attack phase from the current output level.
    /// Legato-safe: retriggers mid-release without a level jump.
    pub fn note_on(&mut self) {
        self.stage_start_level = self.output();
        self.progress = 0.0;
        self.stage = ModEnvStage::Attack;
    }

    /// Enters the Release phase from the current output level. No-op if already idle.
    pub fn note_off(&mut self) {
        if self.stage == ModEnvStage::Idle {
            return;
        }
        self.stage_start_level = self.output();
        self.progress = 0.0;
        self.stage = ModEnvStage::Release;
    }

    /// Returns `true` when the envelope has fully released and output is 0.
    /// The voice manager AND-gates this with the amp envelope's `is_idle`.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.stage == ModEnvStage::Idle
    }

    /// Advances the envelope by `block_size` samples and returns the current
    /// output. Call once per inner block (same cadence as the LFOs).
    pub fn advance(&mut self, block_size: usize) -> f32 {
        let block = block_size as f32;
        match self.stage {
            ModEnvStage::Idle | ModEnvStage::Sustain => {}
            ModEnvStage::Attack => {
                let total = (self.attack_secs * self.sample_rate_hz).max(1.0);
                self.progress += block / total;
                if self.progress >= 1.0 {
                    self.stage_start_level = 1.0;
                    self.progress = 0.0;
                    self.stage = ModEnvStage::Decay;
                }
            }
            ModEnvStage::Decay => {
                let total = (self.decay_secs * self.sample_rate_hz).max(1.0);
                self.progress += block / total;
                if self.progress >= 1.0 {
                    self.progress = 0.0;
                    self.stage = ModEnvStage::Sustain;
                }
            }
            ModEnvStage::Release => {
                let total = (self.release_secs * self.sample_rate_hz).max(1.0);
                self.progress += block / total;
                if self.progress >= 1.0 {
                    self.stage_start_level = 0.0;
                    self.progress = 0.0;
                    self.stage = ModEnvStage::Idle;
                }
            }
        }
        self.output()
    }

    /// Returns the current output without advancing the envelope.
    #[must_use]
    pub fn current_output(&self) -> f32 {
        self.output()
    }

    fn output(&self) -> f32 {
        match self.stage {
            ModEnvStage::Idle => 0.0,
            ModEnvStage::Attack => {
                let shaped = apply_curve(self.progress, self.attack_curve);
                self.stage_start_level + (1.0 - self.stage_start_level) * shaped
            }
            ModEnvStage::Decay => {
                let shaped = apply_curve(self.progress, self.decay_curve);
                1.0 + (self.sustain_level - 1.0) * shaped
            }
            ModEnvStage::Sustain => self.sustain_level,
            ModEnvStage::Release => {
                let shaped = apply_curve(self.progress, self.release_curve);
                self.stage_start_level * (1.0 - shaped)
            }
        }
    }
}

/// Applies a curve to normalised progress `x ∈ [0, 1]`.
///
/// `y = x^(2^(−curve))`:
/// - `curve = 0` → exponent 1.0 → linear
/// - `curve > 0` → exponent < 1.0 → concave up → fast initial change
/// - `curve < 0` → exponent > 1.0 → concave down → slow initial change
fn apply_curve(x: f32, curve: f32) -> f32 {
    let exponent = 2.0_f32.powf(-curve);
    x.powf(exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 64;

    fn env() -> ModEnv {
        ModEnv::new(SR)
    }

    /// Advance `e` until `stage` changes away from `target_stage`, returning the
    /// final output. Panics if the stage doesn't change within `max_blocks`.
    fn advance_until_stage_changes(e: &mut ModEnv, from_stage: ModEnvStage, max_blocks: usize) -> f32 {
        for _ in 0..max_blocks {
            let out = e.advance(BLOCK);
            if e.stage != from_stage {
                return out;
            }
        }
        panic!("stage {:?} did not change within {max_blocks} blocks", from_stage);
    }

    #[test]
    fn idle_outputs_zero_and_is_idle() {
        let e = env();
        assert!(e.is_idle());
        assert_eq!(e.current_output(), 0.0);
    }

    #[test]
    fn attack_rises_to_one() {
        let mut e = env();
        e.set_attack_secs(0.1);
        e.note_on();
        // Run well past the attack time.
        let blocks = (SR as usize / BLOCK) + 10;
        let mut out = 0.0;
        for _ in 0..blocks {
            out = e.advance(BLOCK);
            if e.stage != ModEnvStage::Attack {
                break;
            }
        }
        // After attack completes output (first Decay block) must be at 1.0 or just below.
        assert!(out >= 0.99, "expected ~1.0 after attack, got {out}");
    }

    #[test]
    fn decay_falls_to_sustain_level() {
        let mut e = env();
        e.set_attack_secs(0.001);
        e.set_decay_secs(0.1);
        e.set_sustain_level(0.5);
        e.note_on();
        // Rush through attack then run past decay.
        advance_until_stage_changes(&mut e, ModEnvStage::Attack, 1000);
        let blocks = (SR as usize / BLOCK) + 10;
        let mut out = 1.0;
        for _ in 0..blocks {
            out = e.advance(BLOCK);
            if e.stage == ModEnvStage::Sustain {
                break;
            }
        }
        assert!((out - 0.5).abs() < 0.05, "expected ~0.5 at sustain, got {out}");
    }

    #[test]
    fn sustain_holds_level() {
        let mut e = env();
        e.set_attack_secs(0.001);
        e.set_decay_secs(0.001);
        e.set_sustain_level(0.6);
        e.note_on();
        // Burn through A and D.
        for _ in 0..1000 {
            e.advance(BLOCK);
            if e.stage == ModEnvStage::Sustain {
                break;
            }
        }
        assert_eq!(e.stage, ModEnvStage::Sustain);
        for _ in 0..100 {
            let out = e.advance(BLOCK);
            assert!((out - 0.6).abs() < 1e-6, "sustain should be steady at 0.6, got {out}");
        }
    }

    #[test]
    fn release_reaches_idle() {
        let mut e = env();
        e.set_attack_secs(0.001);
        e.set_decay_secs(0.001);
        e.set_sustain_level(0.8);
        e.set_release_secs(0.1);
        e.note_on();
        for _ in 0..1000 {
            e.advance(BLOCK);
            if e.stage == ModEnvStage::Sustain {
                break;
            }
        }
        e.note_off();
        let blocks = (SR as usize / BLOCK) + 10;
        for _ in 0..blocks {
            e.advance(BLOCK);
            if e.is_idle() {
                break;
            }
        }
        assert!(e.is_idle(), "envelope should be idle after release");
        assert_eq!(e.current_output(), 0.0);
    }

    #[test]
    fn note_off_before_attack_completes_releases_from_current_level() {
        let mut e = env();
        e.set_attack_secs(1.0);
        e.note_on();
        // Advance about half the attack.
        let half_blocks = (0.5 * SR) as usize / BLOCK;
        let mut mid_level = 0.0;
        for _ in 0..half_blocks {
            mid_level = e.advance(BLOCK);
        }
        assert!(mid_level > 0.1 && mid_level < 0.9, "should be mid-attack");
        let level_at_release = e.current_output();
        e.note_off();
        // Release should start from that mid level.
        assert_eq!(e.stage, ModEnvStage::Release);
        let first_release = e.advance(BLOCK);
        // Should be just below the captured level (one block into release).
        assert!(
            first_release <= level_at_release,
            "release should start at or below mid-attack level"
        );
    }

    #[test]
    fn legato_retrigger_continues_from_current_level() {
        let mut e = env();
        e.set_attack_secs(0.001);
        e.set_decay_secs(0.001);
        e.set_sustain_level(0.8);
        e.set_release_secs(1.0);
        e.note_on();
        for _ in 0..1000 {
            e.advance(BLOCK);
            if e.stage == ModEnvStage::Sustain {
                break;
            }
        }
        e.note_off();
        // Run a few blocks into the release.
        for _ in 0..10 {
            e.advance(BLOCK);
        }
        let level_before = e.current_output();
        assert!(level_before > 0.5, "should be in mid-release");
        // Retrigger — attack should resume from mid-release level, not from zero.
        e.note_on();
        assert_eq!(e.stage, ModEnvStage::Attack);
        let first_attack = e.advance(BLOCK);
        assert!(
            first_attack >= level_before,
            "attack should rise from {level_before}, not jump down; got {first_attack}"
        );
    }

    #[test]
    fn note_off_while_idle_is_noop() {
        let mut e = env();
        e.note_off();
        assert!(e.is_idle());
        assert_eq!(e.current_output(), 0.0);
    }

    #[test]
    fn positive_attack_curve_reaches_midpoint_faster_than_linear() {
        let mut linear = env();
        linear.set_attack_secs(1.0);
        linear.set_attack_curve(0.0);
        linear.note_on();

        let mut curved = env();
        curved.set_attack_secs(1.0);
        curved.set_attack_curve(1.0);
        curved.note_on();

        // Advance to ~25% of the attack time.
        let quarter_blocks = (0.25 * SR) as usize / BLOCK;
        let mut lin_out = 0.0;
        let mut cur_out = 0.0;
        for _ in 0..quarter_blocks {
            lin_out = linear.advance(BLOCK);
            cur_out = curved.advance(BLOCK);
        }
        assert!(
            cur_out > lin_out,
            "positive curve should be ahead of linear at 25% of attack; linear={lin_out}, curved={cur_out}"
        );
    }

    #[test]
    fn negative_attack_curve_reaches_midpoint_slower_than_linear() {
        let mut linear = env();
        linear.set_attack_secs(1.0);
        linear.set_attack_curve(0.0);
        linear.note_on();

        let mut curved = env();
        curved.set_attack_secs(1.0);
        curved.set_attack_curve(-1.0);
        curved.note_on();

        let quarter_blocks = (0.25 * SR) as usize / BLOCK;
        let mut lin_out = 0.0;
        let mut cur_out = 0.0;
        for _ in 0..quarter_blocks {
            lin_out = linear.advance(BLOCK);
            cur_out = curved.advance(BLOCK);
        }
        assert!(
            cur_out < lin_out,
            "negative curve should be behind linear at 25% of attack; linear={lin_out}, curved={cur_out}"
        );
    }

    #[test]
    fn output_stays_in_zero_to_one() {
        let mut e = env();
        e.set_attack_secs(0.05);
        e.set_decay_secs(0.05);
        e.set_sustain_level(0.5);
        e.set_release_secs(0.05);
        e.set_attack_curve(1.0);
        e.set_decay_curve(-1.0);
        e.set_release_curve(1.0);
        e.note_on();
        for i in 0..10_000 {
            if i == 2000 {
                e.note_off();
            }
            let out = e.advance(BLOCK);
            assert!(
                (0.0..=1.0).contains(&out),
                "output {out} out of [0, 1] range at block {i}"
            );
        }
    }
}
