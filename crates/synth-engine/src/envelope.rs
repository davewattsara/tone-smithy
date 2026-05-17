//! Amplitude envelope generator.
//!
//! M1 ships a single linear-segment ADSR. Curve shaping (exponential
//! attack/decay/release) is in v1 scope but lives in M5 alongside Env2;
//! the linear form is the right starting point because it makes the
//! state machine and time-constant logic visible without curve maths
//! getting in the way.

/// ADSR envelope phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Not running. `next()` returns 0.0.
    Idle,
    /// Rising from 0 to 1.0 over `attack_secs`.
    Attack,
    /// Falling from 1.0 to `sustain` over `decay_secs`.
    Decay,
    /// Held at `sustain` until `note_off` is called.
    Sustain,
    /// Falling from current level to 0 over `release_secs`.
    Release,
}

/// Linear-segment ADSR envelope.
///
/// All times are in seconds; level outputs are linear amplitude in 0..=1.
/// Time constants are converted to per-sample step sizes at construction
/// and recomputed whenever the corresponding time changes.
pub struct Adsr {
    sample_rate_hz: f32,

    attack_secs: f32,
    decay_secs: f32,
    sustain_level: f32,
    release_secs: f32,

    phase: Phase,

    /// Current envelope output, 0..=1.
    level: f32,

    /// Per-sample increment used by the current phase. Recomputed on
    /// phase transitions and when the relevant time setter is called.
    step: f32,

    /// The level the release phase started from. Captured at `note_off`
    /// so a short release scales correctly even if the envelope had not
    /// yet reached the sustain level.
    release_start_level: f32,
}

impl Adsr {
    /// Creates an idle envelope with reasonable defaults
    /// (10 ms / 200 ms / 0.8 / 200 ms).
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        let mut adsr = Self {
            sample_rate_hz,
            attack_secs: 0.010,
            decay_secs: 0.200,
            sustain_level: 0.8,
            release_secs: 0.200,
            phase: Phase::Idle,
            level: 0.0,
            step: 0.0,
            release_start_level: 0.0,
        };
        // Pre-compute the attack step so the first `note_on` is responsive
        // even before any setter has been called.
        adsr.recompute_step();
        adsr
    }

    /// Sets the attack time in seconds. Values below one sample are
    /// clamped up so the envelope still rises monotonically.
    pub fn set_attack_secs(&mut self, attack_secs: f32) {
        self.attack_secs = attack_secs.max(1.0 / self.sample_rate_hz);
        if self.phase == Phase::Attack {
            self.recompute_step();
        }
    }

    /// Sets the decay time in seconds.
    pub fn set_decay_secs(&mut self, decay_secs: f32) {
        self.decay_secs = decay_secs.max(1.0 / self.sample_rate_hz);
        if self.phase == Phase::Decay {
            self.recompute_step();
        }
    }

    /// Sets the sustain level, clamped to 0..=1.
    pub fn set_sustain_level(&mut self, sustain_level: f32) {
        self.sustain_level = sustain_level.clamp(0.0, 1.0);
    }

    /// Sets the release time in seconds.
    pub fn set_release_secs(&mut self, release_secs: f32) {
        self.release_secs = release_secs.max(1.0 / self.sample_rate_hz);
        if self.phase == Phase::Release {
            self.recompute_step();
        }
    }

    /// Begins the attack phase from the current level. Re-triggering
    /// during a running envelope picks up from wherever the level is —
    /// this is the simple "free-run" behaviour; legato/voice-stealing
    /// strategies belong in the voice manager (M3).
    pub fn note_on(&mut self) {
        self.phase = Phase::Attack;
        self.recompute_step();
    }

    /// Begins the release phase from the current level.
    pub fn note_off(&mut self) {
        self.release_start_level = self.level;
        self.phase = Phase::Release;
        self.recompute_step();
    }

    /// Returns true once the envelope has fully released and is back
    /// at idle. The voice manager uses this to free voices.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Idle
    }

    /// Advances the envelope by one sample and returns the new level.
    pub fn next_sample(&mut self) -> f32 {
        match self.phase {
            Phase::Idle => 0.0,
            Phase::Attack => {
                self.level += self.step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.phase = Phase::Decay;
                    self.recompute_step();
                }
                self.level
            }
            Phase::Decay => {
                self.level -= self.step;
                if self.level <= self.sustain_level {
                    self.level = self.sustain_level;
                    self.phase = Phase::Sustain;
                }
                self.level
            }
            Phase::Sustain => self.sustain_level,
            Phase::Release => {
                self.level -= self.step;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.phase = Phase::Idle;
                }
                self.level
            }
        }
    }

    /// Recomputes `step` for the current phase. Called whenever a time
    /// setter changes the relevant time or the phase transitions.
    fn recompute_step(&mut self) {
        self.step = match self.phase {
            Phase::Idle | Phase::Sustain => 0.0,
            Phase::Attack => {
                let remaining = (1.0 - self.level).max(0.0);
                let frames = (self.attack_secs * self.sample_rate_hz).max(1.0);
                remaining / frames
            }
            Phase::Decay => {
                let remaining = (1.0 - self.sustain_level).max(0.0);
                let frames = (self.decay_secs * self.sample_rate_hz).max(1.0);
                remaining / frames
            }
            Phase::Release => {
                let frames = (self.release_secs * self.sample_rate_hz).max(1.0);
                self.release_start_level / frames
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_envelope_outputs_silence() {
        let mut env = Adsr::new(48_000.0);
        for _ in 0..1000 {
            assert_eq!(env.next_sample(), 0.0);
        }
    }

    #[test]
    fn attack_reaches_one_in_about_attack_time() {
        let sample_rate = 48_000.0;
        let mut env = Adsr::new(sample_rate);
        env.set_attack_secs(0.010);
        env.note_on();

        // Run 10 ms of samples; level should be at the peak (1.0) by then.
        let samples = (0.010 * sample_rate) as usize;
        for _ in 0..samples {
            env.next_sample();
        }
        // Allow a tiny tolerance — the loop runs exactly `attack_secs * sr`
        // iterations, and a one-sample remainder either way is fine.
        let level = env.next_sample();
        assert!(level >= 0.99, "expected near 1.0 after attack, got {level}");
    }

    #[test]
    fn release_runs_to_silence_and_returns_to_idle() {
        let sample_rate = 48_000.0;
        let mut env = Adsr::new(sample_rate);
        env.set_attack_secs(0.001);
        env.set_decay_secs(0.001);
        env.set_sustain_level(0.5);
        env.set_release_secs(0.010);
        env.note_on();
        // Run long enough to be solidly in sustain.
        for _ in 0..1000 {
            env.next_sample();
        }
        env.note_off();
        // Plenty of headroom past the release time.
        for _ in 0..2000 {
            env.next_sample();
        }
        assert!(env.is_idle());
    }
}
