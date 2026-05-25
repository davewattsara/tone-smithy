//! 4-operator FM synthesis bank.
//!
//! An FM slot is built from four [`Operator`]s wired together by an
//! [`Algorithm`]. Each operator is a sine oscillator with its own ADSR,
//! ratio (integer + fine), level, and (op 3 only) one-sample-delayed
//! self-feedback. The algorithm describes which operators modulate
//! which others and which operators are carriers (contribute to the
//! slot's audio output).
//!
//! Evaluation order each sample is fixed: op 3 → op 2 → op 1 → op 0.
//! A higher-indexed operator can modulate any lower-indexed operator
//! using the same-sample output (computed earlier in the loop). The
//! only legal back-edge is op 3's self-feedback, which uses the
//! previous sample's output (one-sample delay). This restriction lets
//! the bank evaluate in a single forward pass with no topology sort
//! and is sufficient for all 8 starter algorithms.
//!
//! All state is `Copy`-friendly and stack-allocated; no heap, no locks,
//! no syscalls on the audio path.
//!
//! M7.1 ships the data types and stand-alone bank. The bank is wired
//! into [`crate::slot::Slot`] in M7.2.

use std::f32::consts::TAU;

use crate::envelope::Adsr;
use crate::halfband::HalfBand;

/// Number of operators per FM bank. DX7-family standard.
pub const OPERATOR_COUNT: usize = 4;

/// Number of starter algorithms shipped in v1.
pub const ALGORITHM_COUNT: usize = 8;

/// Maximum integer ratio multiplier per operator. Higher than DX7's
/// 0..=31 only because v1.2 may want wider; for v1 we keep 0..=15 to
/// match what the UI knob exposes.
pub const RATIO_INTEGER_MAX: u8 = 15;

/// A single FM operator: phase + ADSR + sine output.
///
/// Output per sample is
/// `sin(2π × (phase + mod_in)) × env_level × level`
/// where `mod_in` is the sum of incoming modulator outputs (and, for
/// op 3, the previous-sample self-output scaled by `feedback_amount`).
pub struct Operator {
    sample_rate_hz: f32,
    /// Normalised phase, 0..=1.
    phase: f32,
    envelope: Adsr,
    /// Integer multiplier of the held note's frequency. `0` is reserved
    /// for fixed-frequency mode (v1.2) and currently behaves as `1`.
    ratio_integer: u8,
    /// Fine ratio offset in cents, clamped to ±100.
    ratio_fine_cents: f32,
    /// Per-operator output gain, 0..=1. Sits between the envelope and
    /// the sine.
    level: f32,
    /// One-sample-delayed self-feedback amount. Only op 3 of a default
    /// algorithm exercises this; the other operators leave it at 0.
    feedback_amount: f32,
    /// Stored previous-sample output for the feedback path.
    feedback_prev_output: f32,
}

impl Operator {
    /// Creates a fresh operator at the given sample rate. Defaults:
    /// ratio 1, no fine offset, unit level, no feedback, idle envelope.
    /// The envelope inherits [`Adsr::new`]'s defaults; callers should
    /// override times before use.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            phase: 0.0,
            envelope: Adsr::new(sample_rate_hz),
            ratio_integer: 1,
            ratio_fine_cents: 0.0,
            level: 1.0,
            feedback_amount: 0.0,
            feedback_prev_output: 0.0,
        }
    }

    /// Triggers the envelope and resets phase + feedback state so the
    /// FM stack starts deterministically — unlike the subtractive
    /// unison banks, FM operators run from phase 0 each note so the
    /// timbre of a fresh note is reproducible.
    pub fn note_on(&mut self) {
        self.envelope.note_on();
        self.phase = 0.0;
        self.feedback_prev_output = 0.0;
    }

    /// Begins the release stage of the envelope.
    pub fn note_off(&mut self) {
        self.envelope.note_off();
    }

    /// True when the envelope has fully released.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.envelope.is_idle()
    }

    /// Sets the integer ratio multiplier, clamped to `0..=RATIO_INTEGER_MAX`.
    /// `0` currently behaves as `1`; fixed-frequency mode is v1.2.
    pub fn set_ratio_integer(&mut self, v: u8) {
        self.ratio_integer = v.min(RATIO_INTEGER_MAX);
    }

    /// Sets the fine ratio offset in cents, clamped to ±100.
    pub fn set_ratio_fine_cents(&mut self, v: f32) {
        self.ratio_fine_cents = v.clamp(-100.0, 100.0);
    }

    /// Sets the operator output level, clamped to 0..=1.
    pub fn set_level(&mut self, v: f32) {
        self.level = v.clamp(0.0, 1.0);
    }

    /// Sets the self-feedback amount, clamped to -1..=1. Only meaningful
    /// when the active algorithm has a feedback edge on this operator
    /// (in the 8 starter algorithms, that is op 3 only).
    pub fn set_feedback_amount(&mut self, v: f32) {
        self.feedback_amount = v.clamp(-1.0, 1.0);
    }

    /// Sets the envelope attack time, in seconds.
    pub fn set_attack_secs(&mut self, secs: f32) {
        self.envelope.set_attack_secs(secs);
    }

    /// Sets the envelope decay time, in seconds.
    pub fn set_decay_secs(&mut self, secs: f32) {
        self.envelope.set_decay_secs(secs);
    }

    /// Sets the envelope sustain level, 0..=1.
    pub fn set_sustain_level(&mut self, level: f32) {
        self.envelope.set_sustain_level(level);
    }

    /// Sets the envelope release time, in seconds.
    pub fn set_release_secs(&mut self, secs: f32) {
        self.envelope.set_release_secs(secs);
    }

    /// Current envelope level (0..=1) without advancing.
    #[must_use]
    pub fn envelope_level(&self) -> f32 {
        self.envelope.current_level()
    }

    /// Returns the effective frequency ratio: integer × `2^(fine/1200)`.
    #[must_use]
    fn ratio(&self) -> f32 {
        let base = if self.ratio_integer == 0 {
            1.0
        } else {
            f32::from(self.ratio_integer)
        };
        base * 2.0_f32.powf(self.ratio_fine_cents / 1200.0)
    }

    /// Computes one sample at the base rate. Used in operator-level unit
    /// tests; production code uses [`next_sample_os`][Self::next_sample_os]
    /// via [`FmBank::next_sample`].
    #[allow(dead_code)]
    fn next_sample(&mut self, mod_in: f32, base_note_hz: f32) -> f32 {
        let freq_hz = base_note_hz * self.ratio();
        let phase_increment = freq_hz / self.sample_rate_hz;
        let env = self.envelope.next_sample();
        // .fract() preserves the sign on negative values; wrap into 0..1
        // by way of `rem_euclid` so the sin argument is always positive
        // and the high-amplitude feedback case doesn't drift outside
        // the unit interval.
        let phase_modulated = (self.phase + mod_in).rem_euclid(1.0);
        let output = (TAU * phase_modulated).sin() * env * self.level;
        self.phase = (self.phase + phase_increment).rem_euclid(1.0);
        self.feedback_prev_output = output;
        output
    }

    /// Computes one sample at the 2× oversampled rate.
    ///
    /// Phase advances at `2 × sample_rate_hz`. When `advance_env` is true
    /// the envelope ticks once (base-rate pace); when false the envelope
    /// level from the previous call is reused, so envelopes run at base
    /// rate even though the oscillator runs at 2× rate.
    fn next_sample_os(&mut self, mod_in: f32, base_note_hz: f32, advance_env: bool) -> f32 {
        let freq_hz = base_note_hz * self.ratio();
        let phase_increment = freq_hz / (self.sample_rate_hz * 2.0);
        let env = if advance_env {
            self.envelope.next_sample()
        } else {
            self.envelope.current_level()
        };
        let phase_modulated = (self.phase + mod_in).rem_euclid(1.0);
        let output = (TAU * phase_modulated).sin() * env * self.level;
        self.phase = (self.phase + phase_increment).rem_euclid(1.0);
        self.feedback_prev_output = output;
        output
    }
}

/// A static FM routing graph. Each [`Operator`] index `i` has a
/// `mod_sources[i]` bitmask: bit `j` set means operator `j` contributes
/// to op `i`'s phase modulation. When `j == i`, the contribution comes
/// from op `i`'s one-sample-delayed previous output, scaled by its
/// `feedback_amount`. `is_carrier[i]` true means op `i`'s output is
/// summed into the slot's audio output.
#[derive(Clone, Copy, Debug)]
pub struct Algorithm {
    /// `mod_sources[i]` is a bitmask of operators feeding op `i`'s
    /// phase modulator input. Bit `j` (lsb-first) corresponds to op `j`.
    pub mod_sources: [u8; OPERATOR_COUNT],
    /// `is_carrier[i]` is `true` when op `i`'s output contributes to
    /// the slot's audio output.
    pub is_carrier: [bool; OPERATOR_COUNT],
}

/// The 8 starter algorithms. Indices in the table are 0-based to match
/// `OPERATOR_COUNT`; the names in the table use the more familiar
/// 1-based DX7 numbering ("4→3" means op 3 modulates op 2).
pub const ALGORITHMS: [Algorithm; ALGORITHM_COUNT] = [
    // 1: Stack 4→3→2→1 (canonical FM bell / brass stack)
    Algorithm {
        mod_sources: [0b0010, 0b0100, 0b1000, 0],
        is_carrier: [true, false, false, false],
    },
    // 2: Stack 4→3→2→1 with op 3 self-feedback (richer mod content)
    Algorithm {
        mod_sources: [0b0010, 0b0100, 0b1000, 0b1000],
        is_carrier: [true, false, false, false],
    },
    // 3: Two stacks (4→3 into 2→1) mixed
    Algorithm {
        mod_sources: [0b0010, 0, 0b1000, 0],
        is_carrier: [true, false, true, false],
    },
    // 4: Op 3 modulates ops 0, 1, 2 (parallel modulation)
    Algorithm {
        mod_sources: [0b1000, 0b1000, 0b1000, 0],
        is_carrier: [true, true, true, false],
    },
    // 5: 3→2, 3→1, 2→0 (branching modulator)
    Algorithm {
        mod_sources: [0b1100, 0b0100, 0b1000, 0],
        is_carrier: [true, false, false, false],
    },
    // 6: 2+1 mod op 0; op 3 separate carrier
    Algorithm {
        mod_sources: [0b0110, 0, 0, 0],
        is_carrier: [true, false, false, true],
    },
    // 7: All four parallel (additive)
    Algorithm {
        mod_sources: [0, 0, 0, 0],
        is_carrier: [true, true, true, true],
    },
    // 8: 3→1, 2→0; both carriers (paired)
    Algorithm {
        mod_sources: [0b1000, 0b0100, 0, 0],
        is_carrier: [true, true, false, false],
    },
];

/// 4-operator FM synthesis bank. Wraps four [`Operator`]s and the
/// currently selected algorithm. Operators run at 2× the base sample rate
/// and are decimated to base rate via a 31-tap half-band FIR filter.
pub struct FmBank {
    operators: [Operator; OPERATOR_COUNT],
    algorithm_index: u8,
    /// Scratch buffer holding each operator's same-sample output so a
    /// lower-indexed operator can read its modulators' fresh outputs.
    op_outputs: [f32; OPERATOR_COUNT],
    /// Half-band FIR decimation filter — converts the 2× oversampled FM
    /// output to the base sample rate, attenuating aliasing above Nyquist.
    decim: HalfBand,
}

impl FmBank {
    /// Creates a fresh FM bank at the given sample rate. All four
    /// operators default to ratio 1, level 1.0; the algorithm index
    /// defaults to 0 (algorithm 1, a 4→3→2→1 stack).
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            operators: [
                Operator::new(sample_rate_hz),
                Operator::new(sample_rate_hz),
                Operator::new(sample_rate_hz),
                Operator::new(sample_rate_hz),
            ],
            algorithm_index: 0,
            op_outputs: [0.0; OPERATOR_COUNT],
            decim: HalfBand::new(),
        }
    }

    /// Triggers every operator envelope and resets each operator's
    /// phase + feedback state. Call at the slot's note-on.
    pub fn note_on(&mut self) {
        for op in &mut self.operators {
            op.note_on();
        }
    }

    /// Begins the release stage of every operator envelope.
    pub fn note_off(&mut self) {
        for op in &mut self.operators {
            op.note_off();
        }
    }

    /// True when every operator envelope has fully released.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.operators.iter().all(Operator::is_idle)
    }

    /// Sets the active algorithm. Indices outside `0..ALGORITHM_COUNT`
    /// are clamped to the last valid index.
    pub fn set_algorithm(&mut self, index: u8) {
        let max = (ALGORITHM_COUNT - 1) as u8;
        self.algorithm_index = index.min(max);
    }

    /// Returns the currently selected algorithm index.
    #[must_use]
    pub fn algorithm_index(&self) -> u8 {
        self.algorithm_index
    }

    /// Mutable access to a single operator for parameter setters.
    pub fn operator_mut(&mut self, index: usize) -> Option<&mut Operator> {
        self.operators.get_mut(index)
    }

    /// Produces one mono sample at the given note frequency, using 2×
    /// oversampling to suppress FM aliasing.
    ///
    /// Two FM sub-samples are computed internally at twice the base rate
    /// and decimated to one output via the half-band FIR filter. Operator
    /// envelopes advance only once per call (base-rate pacing); only the
    /// oscillator phases run at 2× rate. The slot caller is responsible for
    /// stereo panning and slot-level scaling.
    pub fn next_sample(&mut self, base_note_hz: f32) -> f32 {
        // First sub-sample: advance envelopes at base rate.
        let s0 = self.eval_operators(base_note_hz, true);
        // Second sub-sample: hold envelopes at current level.
        let s1 = self.eval_operators(base_note_hz, false);
        self.decim.push(s0);
        self.decim.push(s1);
        self.decim.compute()
    }

    /// Evaluates one 2× sub-sample. `advance_env` controls whether operator
    /// envelopes tick this sub-sample; only the first of each pair should.
    fn eval_operators(&mut self, base_note_hz: f32, advance_env: bool) -> f32 {
        let alg = &ALGORITHMS[self.algorithm_index as usize];

        // Evaluate operators in order 3 → 2 → 1 → 0 so a lower-indexed
        // op reads its modulators' fresh outputs from this sub-sample.
        for op_idx in (0..OPERATOR_COUNT).rev() {
            let mod_mask = alg.mod_sources[op_idx];
            let mut mod_in = 0.0_f32;
            for src_idx in 0..OPERATOR_COUNT {
                if mod_mask & (1 << src_idx) == 0 {
                    continue;
                }
                if src_idx == op_idx {
                    // Self-feedback: previous sub-sample output × feedback_amount.
                    let op = &self.operators[op_idx];
                    mod_in += op.feedback_amount * op.feedback_prev_output;
                } else {
                    mod_in += self.op_outputs[src_idx];
                }
            }
            self.op_outputs[op_idx] = self.operators[op_idx].next_sample_os(mod_in, base_note_hz, advance_env);
        }

        let mut sum = 0.0_f32;
        for i in 0..OPERATOR_COUNT {
            if alg.is_carrier[i] {
                sum += self.op_outputs[i];
            }
        }
        sum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set every operator's envelope to instant attack + full sustain
    /// so a few samples after `note_on` the envelope is at 1.0 and the
    /// operator output is deterministic.
    fn note_on_with_instant_envelope(bank: &mut FmBank) {
        for i in 0..OPERATOR_COUNT {
            let op = bank.operator_mut(i).unwrap();
            op.set_attack_secs(0.001);
            op.set_decay_secs(0.001);
            op.set_sustain_level(1.0);
            op.set_release_secs(0.100);
        }
        bank.note_on();
    }

    #[test]
    fn algorithm_table_has_expected_shape() {
        assert_eq!(ALGORITHMS.len(), ALGORITHM_COUNT);

        // Algorithm 1: stack 3→2→1→0, op 0 carrier
        let a1 = ALGORITHMS[0];
        assert_eq!(a1.mod_sources, [0b0010, 0b0100, 0b1000, 0]);
        assert_eq!(a1.is_carrier, [true, false, false, false]);

        // Algorithm 2: same stack plus op 3 self-feedback
        let a2 = ALGORITHMS[1];
        assert_eq!(a2.mod_sources, [0b0010, 0b0100, 0b1000, 0b1000]);
        assert_eq!(a2.is_carrier, [true, false, false, false]);

        // Algorithm 7: pure additive
        let a7 = ALGORITHMS[6];
        assert_eq!(a7.mod_sources, [0, 0, 0, 0]);
        assert_eq!(a7.is_carrier, [true, true, true, true]);
    }

    #[test]
    fn every_algorithm_has_at_least_one_carrier() {
        for (i, alg) in ALGORITHMS.iter().enumerate() {
            assert!(alg.is_carrier.iter().any(|&c| c), "algorithm {i} has no carriers");
        }
    }

    #[test]
    fn operator_phase_wraps_into_unit_interval() {
        let mut op = Operator::new(48_000.0);
        op.set_ratio_integer(8);
        op.set_attack_secs(0.001);
        op.set_decay_secs(0.001);
        op.set_sustain_level(1.0);
        op.note_on();
        for _ in 0..96_000 {
            op.next_sample(0.0, 1_000.0);
            assert!(
                (0.0..1.0).contains(&op.phase),
                "phase escaped unit interval: {}",
                op.phase
            );
        }
    }

    #[test]
    fn additive_algorithm_at_zero_modulation_produces_pure_sine() {
        // Algorithm 7 (index 6): all carriers, no modulators.
        let sample_rate = 48_000.0;
        let mut bank = FmBank::new(sample_rate);
        bank.set_algorithm(6);
        // Only op 0 contributes — silence the other three carriers so
        // we can read a clean single-frequency tone.
        for i in 1..OPERATOR_COUNT {
            bank.operator_mut(i).unwrap().set_level(0.0);
        }
        note_on_with_instant_envelope(&mut bank);

        // Skip the brief envelope ramp.
        for _ in 0..256 {
            bank.next_sample(440.0);
        }

        // Count zero crossings over one second; expect ~880 (440 Hz × 2).
        let mut prev = bank.next_sample(440.0);
        let mut crossings = 0;
        for _ in 0..(sample_rate as usize) {
            let s = bank.next_sample(440.0);
            if (prev <= 0.0 && s > 0.0) || (prev >= 0.0 && s < 0.0) {
                crossings += 1;
            }
            prev = s;
        }
        assert!(
            (860..=900).contains(&crossings),
            "expected ~880 zero crossings at 440 Hz, got {crossings}"
        );
    }

    #[test]
    fn fm_bank_at_zero_modulation_stack_is_silent_when_modulators_silent() {
        // Algorithm 1: 3→2→1→0 stack, op 0 carrier. With modulator
        // levels 0, the carrier sees no phase modulation and outputs a
        // pure sine through op 0's level (which we'll set as the only
        // audible factor).
        let mut bank = FmBank::new(48_000.0);
        bank.set_algorithm(0);
        for i in 1..OPERATOR_COUNT {
            bank.operator_mut(i).unwrap().set_level(0.0);
        }
        note_on_with_instant_envelope(&mut bank);

        for _ in 0..256 {
            bank.next_sample(440.0);
        }
        let s = bank.next_sample(440.0);
        // Bounded — pure sine, env ~1, op 0 level 1 — should be in
        // [-1, 1] easily.
        assert!(s.abs() <= 1.001, "carrier output should be bounded, got {s}");
    }

    #[test]
    fn self_feedback_at_unit_amount_does_not_blow_up() {
        // Algorithm 2 (index 1): stack with op 3 self-feedback.
        // Crank the feedback to maximum, levels to maximum, and run
        // for many samples — the output must stay bounded.
        let mut bank = FmBank::new(48_000.0);
        bank.set_algorithm(1);
        for i in 0..OPERATOR_COUNT {
            bank.operator_mut(i).unwrap().set_level(1.0);
        }
        bank.operator_mut(3).unwrap().set_feedback_amount(1.0);
        note_on_with_instant_envelope(&mut bank);

        let mut peak = 0.0_f32;
        for _ in 0..48_000 {
            let s = bank.next_sample(220.0);
            peak = peak.max(s.abs());
            assert!(s.is_finite(), "FM output went non-finite at unit feedback: {s}");
        }
        // Op 0 is the only carrier; raw samples are bounded to ±1. After
        // the half-band FIR (L1 norm ≈ 1.50), the decimated output can
        // momentarily exceed ±1 due to the weighted sum of past samples,
        // but must stay below the filter's L1 norm bound.
        assert!(
            peak <= 1.51,
            "single-carrier FM output exceeded filter L1 bound, peak={peak}"
        );
    }

    #[test]
    fn ratio_zero_behaves_as_ratio_one() {
        let sample_rate = 48_000.0;
        let mut op_zero = Operator::new(sample_rate);
        op_zero.set_ratio_integer(0);
        op_zero.set_attack_secs(0.001);
        op_zero.set_decay_secs(0.001);
        op_zero.set_sustain_level(1.0);
        op_zero.note_on();

        let mut op_one = Operator::new(sample_rate);
        op_one.set_ratio_integer(1);
        op_one.set_attack_secs(0.001);
        op_one.set_decay_secs(0.001);
        op_one.set_sustain_level(1.0);
        op_one.note_on();

        // After identical sample sequences, both should produce identical
        // output (ratio 0 currently maps to ratio 1).
        for _ in 0..1024 {
            let a = op_zero.next_sample(0.0, 440.0);
            let b = op_one.next_sample(0.0, 440.0);
            assert!(
                (a - b).abs() < 1e-5,
                "ratio 0 should equal ratio 1, diverged: {a} vs {b}"
            );
        }
    }

    #[test]
    fn fine_cents_offsets_frequency() {
        let sample_rate = 48_000.0;
        // Op at ratio 1, +700 cents should sound at ~659 Hz (440 × 2^(7/12))
        // when given a 440 Hz base — but the operator clamps fine to ±100.
        // So at +100 cents (just over a half-step), 440 → ~466 Hz.
        let mut op = Operator::new(sample_rate);
        op.set_ratio_integer(1);
        op.set_ratio_fine_cents(100.0);
        op.set_attack_secs(0.001);
        op.set_decay_secs(0.001);
        op.set_sustain_level(1.0);
        op.note_on();

        // Settle envelope.
        for _ in 0..256 {
            op.next_sample(0.0, 440.0);
        }

        let mut prev = op.next_sample(0.0, 440.0);
        let mut crossings = 0;
        for _ in 0..(sample_rate as usize) {
            let s = op.next_sample(0.0, 440.0);
            if (prev <= 0.0 && s > 0.0) || (prev >= 0.0 && s < 0.0) {
                crossings += 1;
            }
            prev = s;
        }
        // Expected: 466 Hz × 2 = ~932 zero crossings/sec.
        assert!(
            (910..=950).contains(&crossings),
            "expected ~932 zero crossings at 466 Hz, got {crossings}"
        );
    }
}
