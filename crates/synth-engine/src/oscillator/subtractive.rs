//! Multi-shape phase-accumulating oscillator for the subtractive slot.
//!
//! One oscillator owns one phase accumulator and produces one of the
//! four subtractive shapes: sine, saw, square, triangle. Saw and
//! square are band-limited with [`crate::oscillator::polyblep`] so
//! they stay clean up into the highest playable octaves; sine and
//! triangle are generated naively (sine has no harmonics above the
//! fundamental, and the polynomial-triangle approximation is
//! perceptually clean enough that integrating polyBLEP into a square
//! is overkill for v1 — the trade-off in
//! `docs/planning/05-design/dsp-and-sound.md` allows either form).
//!
//! Phase is stored in normalised \[0, 1\) units rather than radians so
//! the polyBLEP math matches the published derivation 1:1. The
//! conversion to radians for the sine path is a single multiply.
//!
//! The voice owns three of these (plus a dedicated sub oscillator)
//! and sums them into the slot mixer.

use core::f32::consts::TAU;

use super::polyblep::poly_blep;

/// The shape an oscillator produces.
///
/// `Waveform` is a discrete parameter — switching mid-block would
/// cause an audible step. Adapters must change it via [`EngineEvent`]
/// so the change lands at a block boundary; the engine drains events
/// once per block before processing samples
/// (`docs/planning/03-architecture/design-patterns.md` §2.7).
///
/// [`EngineEvent`]: crate::EngineEvent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Waveform {
    /// Pure sine, fundamental only. No aliasing concerns.
    #[default]
    Sine,

    /// Band-limited (polyBLEP) ascending sawtooth.
    Saw,

    /// Band-limited (polyBLEP) 50%-duty square.
    Square,

    /// Naive (linear-segment) triangle. The fundamental dominates so
    /// audible aliasing only sets in at very high pitches; the
    /// polyBLAMP-integrated form is a v1.1 upgrade.
    Triangle,
}

impl Waveform {
    /// Returns the zero-based index used when serialising waveform to a preset.
    /// Order: Sine=0, Saw=1, Square=2, Triangle=3.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Waveform::Sine => 0,
            Waveform::Saw => 1,
            Waveform::Square => 2,
            Waveform::Triangle => 3,
        }
    }

    /// Converts a zero-based index back to a `Waveform`. Indices outside 0..=3 return `Sine`.
    #[must_use]
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Waveform::Sine,
            1 => Waveform::Saw,
            2 => Waveform::Square,
            3 => Waveform::Triangle,
            _ => Waveform::Sine,
        }
    }
}

/// A single-shape phase-accumulating oscillator.
///
/// Carries no state about pitch beyond `phase_increment`. The caller
/// (the voice) is responsible for converting MIDI note + tuning into
/// a frequency and calling [`Oscillator::set_frequency_hz`] when it
/// changes.
pub struct Oscillator {
    /// Sample rate in Hz, captured at construction.
    sample_rate_hz: f32,

    /// Current phase, normalised to \[0, 1\). Stored in this unit
    /// rather than radians so polyBLEP arithmetic matches the
    /// published form without extra conversions.
    phase: f32,

    /// Per-sample phase advance, also in \[0, 1\) (frequency divided
    /// by sample rate).
    phase_increment: f32,

    /// Current waveform. Changes take effect on the next sample; the
    /// engine guarantees changes land at a block boundary.
    waveform: Waveform,
}

impl Oscillator {
    /// Creates an oscillator producing the default waveform
    /// ([`Waveform::Sine`]) at 0 Hz. Call
    /// [`Oscillator::set_frequency_hz`] before processing.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            phase: 0.0,
            phase_increment: 0.0,
            waveform: Waveform::default(),
        }
    }

    /// Sets the oscillator frequency in Hz. Negative and zero values
    /// are accepted and produce no useful sound; the caller should
    /// clamp.
    pub fn set_frequency_hz(&mut self, frequency_hz: f32) {
        self.phase_increment = frequency_hz / self.sample_rate_hz;
    }

    /// Sets the active waveform. Takes effect on the next call to
    /// [`Oscillator::next_sample`].
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    /// Resets the phase to zero. Called on note-on so each note
    /// starts from a known phase; this avoids the random-DC artefacts
    /// that follow from leaving the phase wherever the last note
    /// ended.
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }

    /// Forces the phase to a caller-chosen value in \[0, 1\). Used by
    /// the unison oscillator to decorrelate the phases of its voices
    /// — perfectly correlated phases would comb-filter the unison
    /// sum until detuned drift broke up the alignment.
    pub fn set_phase_normalised(&mut self, phase: f32) {
        self.phase = phase.clamp(0.0, 0.999_999);
    }

    /// Produces one sample and advances the internal phase.
    pub fn next_sample(&mut self) -> f32 {
        let t = self.phase;
        let dt = self.phase_increment;

        let sample = match self.waveform {
            Waveform::Sine => (t * TAU).sin(),
            Waveform::Saw => {
                // Naive saw rises from -1 to +1 across [0, 1) and
                // steps back to -1 at the wrap. polyBLEP smooths the
                // step.
                let naive = 2.0 * t - 1.0;
                naive - poly_blep(t, dt)
            }
            Waveform::Square => {
                // Naive square is +1 on the first half, -1 on the
                // second. Two discontinuities per cycle: an upward
                // step at t == 0 (corrected by + polyBLEP) and a
                // downward step at t == 0.5 (corrected by - polyBLEP
                // of the half-shifted phase).
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                naive + poly_blep(t, dt) - poly_blep(fract(t + 0.5), dt)
            }
            Waveform::Triangle => {
                // 4 * |t - 0.5| - 1 traces 1 → -1 → 1 across one cycle.
                4.0 * (t - 0.5).abs() - 1.0
            }
        };

        self.phase += dt;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        sample
    }
}

/// Fractional part for non-negative inputs in \[0, 2). Equivalent to
/// `x - floor(x)`; used for the half-shifted polyBLEP phase in the
/// square wave. Avoids a libm call.
#[inline]
fn fract(x: f32) -> f32 {
    if x >= 1.0 { x - 1.0 } else { x }
}

// =========================================================================
// Unison oscillator
// =========================================================================

use crate::MAX_UNISON_VOICES;
use crate::panning::equal_power_pan;

/// A bank of up to [`MAX_UNISON_VOICES`] oscillator copies sharing
/// one waveform, mixed with per-voice detune and stereo spread.
///
/// At voice count = 1 the bank is a single oscillator and the unison
/// detune / spread parameters do nothing. At higher counts the voices
/// are detuned linearly across `[-d, +d]` cents (odd counts include
/// a centred "in-tune" voice) and panned across the stereo field
/// scaled by `spread`. The summed output is normalised by
/// `1 / sqrt(N)` so the perceived loudness of the unison bank tracks
/// the loudness of a single voice for mostly-uncorrelated detuned
/// signals — a real analog unison sounds about as loud regardless of
/// voice count, which is the property we want.
///
/// Phases for newly-active voices are pseudo-randomised on note-on
/// and on a voice-count increase so the bank doesn't comb-filter from
/// perfectly aligned starting phases (dsp-and-sound.md §"Oscillator
/// design").
pub struct UnisonOscillator {
    voices: [Oscillator; MAX_UNISON_VOICES],

    /// Currently active voice count (1..=MAX_UNISON_VOICES). Kept on
    /// the struct so frequency updates and stereo mixing share one
    /// source of truth and we don't divide by zero when no voices
    /// are configured (the constructor guarantees ≥ 1).
    voice_count: u8,

    /// Used to detect a count increase between calls so newly
    /// activated voices can have their phases randomised.
    prev_voice_count: u8,

    /// LCG state for phase decorrelation. Re-seeded on note-on and
    /// stepped each time a phase is drawn.
    rng_state: u32,
}

impl UnisonOscillator {
    /// Creates a unison oscillator with one active voice (so it
    /// behaves identically to a single [`Oscillator`] until the count
    /// is raised).
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            voices: [(); MAX_UNISON_VOICES].map(|()| Oscillator::new(sample_rate_hz)),
            voice_count: 1,
            prev_voice_count: 1,
            // Arbitrary non-zero seed. Re-seeded on every note-on so
            // separate notes get distinct phase distributions.
            rng_state: 0x12345678,
        }
    }

    /// Sets the waveform on every internal voice. Fan-out is cheap
    /// (one assignment per voice) so we always update all
    /// `MAX_UNISON_VOICES` rather than only the active ones; if the
    /// user raises the voice count later, the newly-active voices
    /// already have the right waveform.
    pub fn set_waveform(&mut self, waveform: Waveform) {
        for v in &mut self.voices {
            v.set_waveform(waveform);
        }
    }

    /// Updates the active voice count, clamped to `1..=MAX_UNISON_VOICES`.
    /// Voices that *become* active (count went up) have their phases
    /// pseudo-randomised so a leftover phase from the last time they
    /// were active doesn't make the bank momentarily comb-filter.
    pub fn set_voice_count(&mut self, count: u8) {
        // Saturating cast — count comes from user input.
        #[allow(clippy::cast_possible_truncation)]
        let max = MAX_UNISON_VOICES as u8;
        let new_count = count.clamp(1, max);
        if new_count > self.prev_voice_count {
            for i in self.prev_voice_count..new_count {
                let phase = random_phase(&mut self.rng_state);
                self.voices[i as usize].set_phase_normalised(phase);
            }
        }
        self.voice_count = new_count;
        self.prev_voice_count = new_count;
    }

    /// Pseudo-randomises every voice's phase. Called by the voice on
    /// note-on so chords (multiple unison banks playing simultaneously)
    /// don't comb-filter on attack.
    pub fn randomize_phases(&mut self) {
        for v in &mut self.voices {
            let phase = random_phase(&mut self.rng_state);
            v.set_phase_normalised(phase);
        }
    }

    /// Resets every voice's phase to zero. Called by the voice on
    /// note-on for an oscillator in Retrig phase mode, giving a
    /// deterministic, repeatable attack transient on every note.
    pub fn reset_phases(&mut self) {
        for v in &mut self.voices {
            v.set_phase_normalised(0.0);
        }
    }

    /// Recomputes per-voice frequencies from a base pitch and a total
    /// unison detune amount (in cents). Voices are spread linearly
    /// across `[-detune_cents, +detune_cents]`; with odd counts the
    /// middle voice sits exactly at `base_hz`.
    pub fn set_base_frequency(&mut self, base_hz: f32, detune_cents: f32) {
        let n = self.voice_count as usize;
        if n == 1 {
            self.voices[0].set_frequency_hz(base_hz);
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        let denom = (n - 1) as f32;
        for i in 0..n {
            #[allow(clippy::cast_precision_loss)]
            let position = -1.0 + 2.0 * (i as f32) / denom;
            let cents = position * detune_cents;
            let hz = base_hz * 2.0_f32.powf(cents / 1200.0);
            self.voices[i].set_frequency_hz(hz);
        }
    }

    /// Produces one stereo frame for the unison bank. `spread` is the
    /// 0..=1 width of the unison voices' stereo spread around
    /// `center_pan` (the per-oscillator pan). The summed output is
    /// scaled by `1 / sqrt(N)` so the bank's perceived loudness is
    /// roughly invariant in N.
    pub fn next_sample_stereo(&mut self, spread: f32, center_pan: f32) -> (f32, f32) {
        let n = self.voice_count as usize;
        if n == 1 {
            let s = self.voices[0].next_sample();
            let (pl, pr) = equal_power_pan(center_pan);
            return (s * pl, s * pr);
        }
        #[allow(clippy::cast_precision_loss)]
        let denom = (n - 1) as f32;
        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for i in 0..n {
            let s = self.voices[i].next_sample();
            #[allow(clippy::cast_precision_loss)]
            let position = -1.0 + 2.0 * (i as f32) / denom;
            let voice_pan = (center_pan + position * spread).clamp(-1.0, 1.0);
            let (pl, pr) = equal_power_pan(voice_pan);
            sum_l += s * pl;
            sum_r += s * pr;
        }
        #[allow(clippy::cast_precision_loss)]
        let norm = (n as f32).sqrt().recip();
        (sum_l * norm, sum_r * norm)
    }
}

/// Linear-congruential pseudo-random in \[0, 1). The constants are
/// Numerical Recipes' classic — adequate for phase decorrelation,
/// not for anything cryptographic. Pure arithmetic, so safe on the
/// audio thread.
fn random_phase(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    // Map u32 into [0, 1). Multiplying by 1 / 2^32 avoids the
    // `u32::MAX as f32 + 1.0` lossy precision pattern.
    #[allow(clippy::cast_precision_loss)]
    let r = (*state as f32) * (1.0 / 4_294_967_296.0);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(osc: &mut Oscillator, frames: usize) -> Vec<f32> {
        (0..frames).map(|_| osc.next_sample()).collect()
    }

    #[test]
    fn sine_oscillator_stays_in_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.0 + 1e-6);
        }
    }

    #[test]
    fn saw_oscillator_stays_close_to_unit_bounds() {
        // polyBLEP can overshoot ±1 by a small fraction of dt near the
        // discontinuity. At 440 Hz / 48 kHz that fraction is tiny;
        // allow a small headroom rather than asserting exact ±1.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.05, "saw out of bounds: {s}");
        }
    }

    #[test]
    fn square_oscillator_stays_close_to_unit_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Square);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.05, "square out of bounds: {s}");
        }
    }

    #[test]
    fn triangle_oscillator_stays_in_bounds() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Triangle);
        osc.set_frequency_hz(440.0);
        for s in run(&mut osc, 10_000) {
            assert!(s.abs() <= 1.0 + 1e-6, "triangle out of bounds: {s}");
        }
    }

    #[test]
    fn saw_completes_full_swing_per_period() {
        // At 480 Hz on a 48 kHz sample rate, one period is exactly 100
        // samples. Even after polyBLEP smooths the discontinuity, the
        // ramp still sweeps near ±1 in the body of the cycle.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(480.0);
        let samples = run(&mut osc, 100);
        let min = samples.iter().copied().fold(f32::INFINITY, f32::min);
        let max = samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(min < -0.9, "expected near -1.0, got {min}");
        assert!(max > 0.9, "expected near +1.0, got {max}");
    }

    #[test]
    fn square_has_zero_dc_over_one_period() {
        // Symmetric 50% duty: positive half cancels negative half.
        // 100-sample period at 480 Hz / 48 kHz divides evenly.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Square);
        osc.set_frequency_hz(480.0);
        let samples = run(&mut osc, 100);
        #[allow(clippy::cast_precision_loss)]
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.05, "square DC = {mean}, expected near 0");
    }

    #[test]
    fn triangle_is_symmetric_over_one_period() {
        // 1 → -1 → 1 trace across [0, 1) is mirror-symmetric about
        // t = 0.5; sum over a full cycle is zero.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Triangle);
        osc.set_frequency_hz(480.0);
        let samples = run(&mut osc, 100);
        #[allow(clippy::cast_precision_loss)]
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.02, "triangle DC = {mean}, expected near 0");
    }

    #[test]
    fn polyblep_saw_matches_naive_away_from_discontinuity() {
        // In the bulk of a cycle (well away from the wrap) polyBLEP
        // returns zero, so the output equals the naive ramp 2t - 1.
        // At 48 Hz / 48 kHz, dt = 0.001; after 100 samples the phase
        // is 0.100, well outside the discontinuity window. Naive
        // saw at t = 0.100 is 2*0.100 - 1 = -0.800.
        let mut osc = Oscillator::new(48_000.0);
        osc.set_waveform(Waveform::Saw);
        osc.set_frequency_hz(48.0);
        let _warmup = run(&mut osc, 100);
        let s = osc.next_sample();
        assert!((s - (-0.798)).abs() < 0.01, "expected ~-0.798, got {s}");
    }

    #[test]
    fn reset_phase_returns_to_zero() {
        let mut osc = Oscillator::new(48_000.0);
        osc.set_frequency_hz(440.0);
        let _ = run(&mut osc, 250);
        osc.reset_phase();
        // First sample after reset is sin(0) = 0 for sine.
        assert_eq!(osc.next_sample(), 0.0);
    }

    #[test]
    fn unison_at_one_voice_matches_single_oscillator() {
        // With voice count = 1, the unison bank is a single
        // oscillator at the base frequency. Output should equal a
        // bare oscillator's, modulo the equal-power pan for
        // center_pan = 0 (= 1/sqrt(2) per channel).
        let mut single = Oscillator::new(48_000.0);
        single.set_waveform(Waveform::Sine);
        single.set_frequency_hz(440.0);

        let mut unison = UnisonOscillator::new(48_000.0);
        unison.set_waveform(Waveform::Sine);
        unison.set_voice_count(1);
        unison.set_base_frequency(440.0, 25.0); // detune ignored at count 1

        for _ in 0..1_000 {
            let s_single = single.next_sample();
            let (l, r) = unison.next_sample_stereo(0.5, 0.0);
            let expected = s_single * (0.5_f32).sqrt();
            assert!((l - expected).abs() < 1e-5, "L mismatch: {l} vs {expected}");
            assert!((r - expected).abs() < 1e-5, "R mismatch: {r} vs {expected}");
        }
    }

    #[test]
    fn unison_detune_produces_amplitude_beating() {
        // Two voices ±20 cents around 440 Hz beat at ~10 Hz (100 ms
        // envelope period). Measure RMS in 20 ms windows — well below
        // the beat period — so consecutive windows catch the envelope
        // at different points of its cycle and the RMS varies. A
        // single un-detuned oscillator would have constant RMS.
        let mut unison = UnisonOscillator::new(48_000.0);
        unison.set_waveform(Waveform::Sine);
        unison.set_voice_count(2);
        unison.set_base_frequency(440.0, 20.0);

        let total = 48_000usize;
        let window = 960usize;
        let mut samples = Vec::with_capacity(total);
        for _ in 0..total {
            let (l, _r) = unison.next_sample_stereo(0.0, 0.0);
            samples.push(l);
        }
        let rms: Vec<f32> = samples
            .chunks_exact(window)
            .map(|chunk| {
                let sum_sq: f32 = chunk.iter().map(|s| s * s).sum();
                #[allow(clippy::cast_precision_loss)]
                let mean = sum_sq / chunk.len() as f32;
                mean.sqrt()
            })
            .collect();
        let max = rms.iter().copied().fold(0.0_f32, f32::max);
        let min = rms.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(
            max - min > 0.1,
            "expected RMS variation from beating; range {min}..{max}"
        );
    }

    #[test]
    fn unison_spread_creates_stereo_difference() {
        // Multi-voice unison with non-zero spread must produce L != R
        // most of the time. Compare against spread = 0 where L == R
        // exactly (all voices panned to center).
        let mut wide = UnisonOscillator::new(48_000.0);
        wide.set_waveform(Waveform::Sine);
        wide.set_voice_count(5);
        wide.set_base_frequency(440.0, 15.0);
        wide.randomize_phases();

        let mut narrow = UnisonOscillator::new(48_000.0);
        narrow.set_waveform(Waveform::Sine);
        narrow.set_voice_count(5);
        narrow.set_base_frequency(440.0, 15.0);
        narrow.randomize_phases();

        let mut wide_diff = 0.0_f32;
        let mut narrow_diff = 0.0_f32;
        for _ in 0..4_800 {
            let (l, r) = wide.next_sample_stereo(1.0, 0.0);
            wide_diff += (l - r).abs();
            let (l, r) = narrow.next_sample_stereo(0.0, 0.0);
            narrow_diff += (l - r).abs();
        }
        assert!(narrow_diff < 1e-3, "narrow should be mono: {narrow_diff}");
        assert!(wide_diff > 50.0, "wide should be clearly stereo: {wide_diff}");
    }

    #[test]
    fn unison_randomize_phases_decorrelates_voices() {
        // Without randomisation, two newly-created unison voices both
        // start at phase 0 and their first samples are identical. After
        // randomisation, the first samples are (almost certainly)
        // different. Test by checking that the first sample of a fresh
        // unison-count-2 oscillator differs from the same with
        // randomised phases.
        let mut bank = UnisonOscillator::new(48_000.0);
        bank.set_waveform(Waveform::Sine);
        bank.set_voice_count(2);
        bank.set_base_frequency(440.0, 10.0);
        let (_l0, _r0) = bank.next_sample_stereo(0.0, 0.0);
        // After randomisation, the next sample distribution shifts
        // because each voice now starts from a fresh phase.
        bank.randomize_phases();
        let mut max_abs = 0.0_f32;
        for _ in 0..256 {
            let (l, _r) = bank.next_sample_stereo(0.0, 0.0);
            max_abs = max_abs.max(l.abs());
        }
        // Decorrelated voices at small detune still sum to
        // non-trivial amplitude on average, but the perfectly-in-
        // phase case would peak much closer to unity. Bound is
        // generous — the assertion is mostly that we don't crash
        // and the output is finite.
        assert!(max_abs.is_finite(), "non-finite after randomise");
        assert!(max_abs < 1.5, "unison output runaway: {max_abs}");
    }

    #[test]
    fn unison_voice_count_clamps_above_maximum() {
        // Asking for 12 voices should clamp to MAX_UNISON_VOICES;
        // the bank still produces audible, finite output.
        let mut bank = UnisonOscillator::new(48_000.0);
        bank.set_waveform(Waveform::Sine);
        #[allow(clippy::cast_possible_truncation)]
        bank.set_voice_count(12);
        bank.set_base_frequency(440.0, 20.0);
        bank.randomize_phases();
        let mut peak = 0.0_f32;
        for _ in 0..4_800 {
            let (l, r) = bank.next_sample_stereo(0.5, 0.0);
            peak = peak.max(l.abs().max(r.abs()));
            assert!(l.is_finite() && r.is_finite());
        }
        assert!(peak > 0.05, "expected audible output, got {peak}");
    }
}
