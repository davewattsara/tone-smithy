//! Low-Frequency Oscillator.
//!
//! Produces a bipolar (-1..=1) modulation signal at sub-audio rates.
//! The LFO is advanced once per inner block (not per sample) — at typical
//! LFO rates (≤ 20 Hz) the 64-sample block rate gives >750 Hz update
//! resolution, far above any audible staircase threshold.
//!
//! BPM sync is not handled inside this struct. The engine converts
//! `bpm / (60 × division_beats)` to Hz and calls [`Lfo::set_rate_hz`];
//! the LFO only knows Hz.

use std::f32::consts::TAU;

/// Waveform shape of the LFO output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfoShape {
    /// Smooth sinusoid. Starts at 0, rises first.
    #[default]
    Sine,
    /// Linear triangle wave. Starts at 0, rises to +1 at ¼-cycle.
    Triangle,
    /// Rising sawtooth. Starts at -1, ramps to +1.
    SawUp,
    /// Falling sawtooth. Starts at +1, ramps to -1.
    SawDown,
    /// Two-state square wave. High (+1) for the first half of each cycle.
    Square,
    /// Sample-and-hold: random value locked until the next period boundary.
    SampleAndHold,
    /// Smooth random: linearly interpolates between consecutive random
    /// values, giving a wandering signal with no abrupt jumps.
    SmoothRandom,
}

/// BPM sync division for an LFO.
///
/// Represents the period of the LFO in bars (assuming 4/4 time). Index
/// ordering matches the UI selector: shortest period first.
///
/// Rate formula: `rate_hz = bpm / 60 / (4 × multiplier_bars)`, clamped to
/// the LFO's `[0.01, 20]` Hz range by [`Lfo::set_rate_hz`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncDivision {
    /// 1/32 of a bar — fastest sync option.
    ThirtySecond,
    /// 1/16 of a bar.
    Sixteenth,
    /// 1/8 of a bar.
    Eighth,
    /// 1/4 of a bar (one beat in 4/4).
    Quarter,
    /// 1/2 of a bar.
    Half,
    /// 1 bar.
    #[default]
    One,
    /// 2 bars.
    Two,
    /// 4 bars — slowest sync option.
    Four,
}

impl SyncDivision {
    /// Period in bars for this division. Used in the rate formula.
    #[must_use]
    pub fn multiplier_bars(self) -> f32 {
        match self {
            Self::ThirtySecond => 1.0 / 32.0,
            Self::Sixteenth => 1.0 / 16.0,
            Self::Eighth => 1.0 / 8.0,
            Self::Quarter => 1.0 / 4.0,
            Self::Half => 1.0 / 2.0,
            Self::One => 1.0,
            Self::Two => 2.0,
            Self::Four => 4.0,
        }
    }

    /// Returns the `SyncDivision` for a zero-based index.
    /// Indices beyond the last variant clamp to `Four`.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::ThirtySecond,
            1 => Self::Sixteenth,
            2 => Self::Eighth,
            3 => Self::Quarter,
            4 => Self::Half,
            5 => Self::One,
            6 => Self::Two,
            _ => Self::Four,
        }
    }

    /// Zero-based index of this division, inverse of [`from_index`].
    ///
    /// [`from_index`]: Self::from_index
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::ThirtySecond => 0,
            Self::Sixteenth => 1,
            Self::Eighth => 2,
            Self::Quarter => 3,
            Self::Half => 4,
            Self::One => 5,
            Self::Two => 6,
            Self::Four => 7,
        }
    }
}

impl LfoShape {
    /// Returns the `LfoShape` corresponding to a zero-based index.
    /// Indices beyond the last valid variant clamp to `SmoothRandom`.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Sine,
            1 => Self::Triangle,
            2 => Self::SawUp,
            3 => Self::SawDown,
            4 => Self::Square,
            5 => Self::SampleAndHold,
            _ => Self::SmoothRandom,
        }
    }

    /// Zero-based index of this shape, inverse of [`from_index`].
    ///
    /// [`from_index`]: Self::from_index
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Sine => 0,
            Self::Triangle => 1,
            Self::SawUp => 2,
            Self::SawDown => 3,
            Self::Square => 4,
            Self::SampleAndHold => 5,
            Self::SmoothRandom => 6,
        }
    }
}

/// Low-frequency oscillator.
///
/// Phase runs in `[0, 1)`. `advance` steps it by `rate_hz × block_size /
/// sample_rate_hz` each inner block and returns the current output.
pub struct Lfo {
    sample_rate_hz: f32,

    /// Current phase, normalised to `[0, 1)`.
    phase: f32,

    /// Oscillation rate in Hz. Clamped to `[0.01, 20.0]`.
    rate_hz: f32,

    shape: LfoShape,

    /// If true, `note_on` resets the phase to 0.0.
    reset_on_note_on: bool,

    /// xorshift32 state. Must never be 0.
    rng: u32,

    /// Current held value for S&H / start value for SmoothRandom interpolation.
    held_value: f32,

    /// Next target value for SmoothRandom, drawn at each period wrap.
    next_value: f32,
}

impl Lfo {
    /// Creates an LFO at 1 Hz sine, phase at 0. `seed` must be non-zero;
    /// passing different seeds for LFO1 vs LFO2 gives independent random
    /// sequences for S&H and SmoothRandom.
    #[must_use]
    pub fn new(sample_rate_hz: f32, seed: u32) -> Self {
        assert!(seed != 0, "xorshift32 seed must be non-zero");
        let mut lfo = Self {
            sample_rate_hz,
            phase: 0.0,
            rate_hz: 1.0,
            shape: LfoShape::Sine,
            reset_on_note_on: false,
            rng: seed,
            held_value: 0.0,
            next_value: 0.0,
        };
        // Prime the random state so S&H starts with a valid value.
        lfo.held_value = lfo.rand_bipolar();
        lfo.next_value = lfo.rand_bipolar();
        lfo
    }

    /// Advances the LFO by `block_size` samples and returns the new output.
    pub fn advance(&mut self, block_size: usize) -> f32 {
        let phase_inc = self.rate_hz * block_size as f32 / self.sample_rate_hz;
        let new_phase = self.phase + phase_inc;
        let wrapped = new_phase >= 1.0;
        self.phase = new_phase % 1.0;

        if wrapped {
            // Roll new random targets on each period boundary.
            match self.shape {
                LfoShape::SampleAndHold => {
                    self.held_value = self.rand_bipolar();
                }
                LfoShape::SmoothRandom => {
                    self.held_value = self.next_value;
                    self.next_value = self.rand_bipolar();
                }
                _ => {}
            }
        }

        self.output()
    }

    /// Resets the phase to 0 if `reset_on_note_on` is set. Call from
    /// the voice's `note_on` handler.
    pub fn note_on(&mut self) {
        if self.reset_on_note_on {
            self.phase = 0.0;
        }
    }

    /// Sets the oscillation rate in Hz, clamped to `[0.01, 20.0]`.
    pub fn set_rate_hz(&mut self, rate_hz: f32) {
        self.rate_hz = rate_hz.clamp(0.01, 20.0);
    }

    /// Sets the waveform shape.
    pub fn set_shape(&mut self, shape: LfoShape) {
        self.shape = shape;
    }

    /// Enables or disables phase reset on note-on.
    pub fn set_reset_on_note_on(&mut self, reset: bool) {
        self.reset_on_note_on = reset;
    }

    /// Returns the current LFO output value without advancing the phase.
    #[must_use]
    pub fn current_output(&self) -> f32 {
        self.output()
    }

    /// Computes the output for the current `phase`.
    fn output(&self) -> f32 {
        match self.shape {
            LfoShape::Sine => (self.phase * TAU).sin(),
            LfoShape::Triangle => {
                // Starts at 0, rises to +1 at phase=0.25, back to 0 at 0.5,
                // falls to -1 at 0.75, returns to 0 at 1.0.
                let p = self.phase;
                if p < 0.25 {
                    4.0 * p
                } else if p < 0.75 {
                    2.0 - 4.0 * p
                } else {
                    4.0 * p - 4.0
                }
            }
            LfoShape::SawUp => 2.0 * self.phase - 1.0,
            LfoShape::SawDown => 1.0 - 2.0 * self.phase,
            LfoShape::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoShape::SampleAndHold => self.held_value,
            LfoShape::SmoothRandom => self.held_value + (self.next_value - self.held_value) * self.phase,
        }
    }

    /// xorshift32 PRNG. Returns a value in `[-1.0, 1.0]`.
    fn rand_bipolar(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        // u32::MAX as f32 is exact (2^32-1 rounds to 2^32 in f32, but the
        // division still produces values well within [-1,1]).
        self.rng as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;
    const BLOCK: usize = 64;

    fn lfo(shape: LfoShape) -> Lfo {
        let mut l = Lfo::new(SR, 0xDEAD_BEEF);
        l.set_shape(shape);
        l.set_rate_hz(1.0);
        l
    }

    /// Advances the LFO until its phase is approximately `target_phase`
    /// (within one block's resolution) and returns the output.
    fn advance_to_phase(l: &mut Lfo, target_phase: f32) -> f32 {
        // Each block advances by rate_hz * BLOCK / SR seconds of phase.
        let phase_per_block = l.rate_hz * BLOCK as f32 / SR;
        let blocks = (target_phase / phase_per_block).round() as usize;
        let mut out = 0.0;
        for _ in 0..blocks {
            out = l.advance(BLOCK);
        }
        out
    }

    #[test]
    fn sine_starts_near_zero() {
        let mut l = lfo(LfoShape::Sine);
        let out = l.advance(BLOCK);
        // After one block at 1 Hz, phase ≈ 64/48000 ≈ 0.00133;
        // sin(2π · 0.00133) ≈ 0.0084. Very close to zero.
        assert!(out.abs() < 0.02, "sine near zero at start, got {out}");
    }

    #[test]
    fn sine_peaks_near_positive_one_at_quarter_cycle() {
        let mut l = lfo(LfoShape::Sine);
        let out = advance_to_phase(&mut l, 0.25);
        assert!(out > 0.95, "sine should be near +1 at quarter cycle, got {out}");
    }

    #[test]
    fn sine_troughs_near_negative_one_at_three_quarter_cycle() {
        let mut l = lfo(LfoShape::Sine);
        let out = advance_to_phase(&mut l, 0.75);
        assert!(out < -0.95, "sine should be near -1 at 3/4 cycle, got {out}");
    }

    #[test]
    fn triangle_zero_at_start() {
        let l = lfo(LfoShape::Triangle);
        // phase=0 exactly → output(0) = 4*0 = 0
        assert_eq!(l.output(), 0.0);
    }

    #[test]
    fn triangle_peaks_positive_at_quarter_cycle() {
        let mut l = lfo(LfoShape::Triangle);
        let out = advance_to_phase(&mut l, 0.25);
        assert!(out > 0.9, "triangle near +1 at 1/4 cycle, got {out}");
    }

    #[test]
    fn triangle_troughs_negative_at_three_quarter_cycle() {
        let mut l = lfo(LfoShape::Triangle);
        let out = advance_to_phase(&mut l, 0.75);
        assert!(out < -0.9, "triangle near -1 at 3/4 cycle, got {out}");
    }

    #[test]
    fn saw_up_rises_monotonically() {
        let mut l = lfo(LfoShape::SawUp);
        let mut prev = l.advance(BLOCK);
        let half_cycle_blocks = (0.5 * SR / BLOCK as f32) as usize;
        for _ in 0..half_cycle_blocks {
            let cur = l.advance(BLOCK);
            assert!(
                cur >= prev - 0.01,
                "saw_up should be non-decreasing, got {cur} after {prev}"
            );
            prev = cur;
        }
    }

    #[test]
    fn saw_down_falls_monotonically() {
        let mut l = lfo(LfoShape::SawDown);
        let mut prev = l.advance(BLOCK);
        let half_cycle_blocks = (0.5 * SR / BLOCK as f32) as usize;
        for _ in 0..half_cycle_blocks {
            let cur = l.advance(BLOCK);
            assert!(
                cur <= prev + 0.01,
                "saw_down should be non-increasing, got {cur} after {prev}"
            );
            prev = cur;
        }
    }

    #[test]
    fn square_is_positive_at_start() {
        let l = lfo(LfoShape::Square);
        assert_eq!(l.output(), 1.0, "square should start high");
    }

    #[test]
    fn square_is_negative_in_second_half() {
        let mut l = lfo(LfoShape::Square);
        // Advance clearly past the midpoint (60% of a cycle) to avoid
        // landing exactly on the phase=0.5 boundary in floating point.
        let out = advance_to_phase(&mut l, 0.6);
        assert_eq!(out, -1.0, "square should be -1 in second half");
    }

    #[test]
    fn all_shapes_stay_in_minus_one_to_one() {
        use LfoShape::*;
        for shape in [Sine, Triangle, SawUp, SawDown, Square, SampleAndHold, SmoothRandom] {
            let mut l = lfo(shape);
            for _ in 0..(SR as usize / BLOCK * 3) {
                let out = l.advance(BLOCK);
                assert!((-1.0..=1.0).contains(&out), "shape {shape:?} output {out} out of range");
            }
        }
    }

    #[test]
    fn phase_reset_on_note_on_returns_to_start() {
        let mut l = lfo(LfoShape::Sine);
        l.set_reset_on_note_on(true);
        // Advance well into the cycle.
        for _ in 0..100 {
            l.advance(BLOCK);
        }
        l.note_on();
        // After reset, the sine should be back near zero (phase ≈ 0).
        assert_eq!(l.phase, 0.0, "phase should reset to 0 on note_on");
    }

    #[test]
    fn no_phase_reset_when_disabled() {
        let mut l = lfo(LfoShape::Sine);
        l.set_reset_on_note_on(false);
        for _ in 0..100 {
            l.advance(BLOCK);
        }
        let phase_before = l.phase;
        l.note_on();
        assert_eq!(l.phase, phase_before, "phase should not change if reset disabled");
    }

    #[test]
    fn rate_2hz_completes_two_cycles_in_one_second() {
        // Run for slightly more than 1 second so both cycle completions are
        // captured even if one falls near a block boundary.
        let mut l = lfo(LfoShape::Sine);
        l.set_rate_hz(2.0);
        let blocks = (SR as usize / BLOCK) + 10;
        let mut zero_crossings = 0usize;
        let mut prev = 0.0_f32;
        for _ in 0..blocks {
            let cur = l.advance(BLOCK);
            if prev < 0.0 && cur >= 0.0 {
                zero_crossings += 1;
            }
            prev = cur;
        }
        // Over slightly more than 1 s, a 2 Hz sine crosses 0 from below twice.
        assert_eq!(
            zero_crossings, 2,
            "expected 2 positive zero crossings at 2 Hz, got {zero_crossings}"
        );
    }

    #[test]
    fn saw_up_starts_at_minus_one_and_is_positive_near_end_of_cycle() {
        let mut l = lfo(LfoShape::SawUp);
        // At phase=0 the formula gives 2*0-1 = -1.
        assert!((l.output() - (-1.0)).abs() < 1e-6, "saw_up at phase=0 should be -1");
        // Advance to ~90 % of a cycle; output = 2*0.9-1 = 0.8.
        let near_end = advance_to_phase(&mut l, 0.9);
        assert!(
            near_end > 0.5,
            "saw_up near end of cycle should be > 0.5, got {near_end}"
        );
    }
}
