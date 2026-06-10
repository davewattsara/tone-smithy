//! Topology-preserving-transform state-variable filter (TPT SVF).
//!
//! Implements the trapezoidal-integrator SVF from Andrew Simper's
//! "Linear Trapezoidal State Variable Filter (SVF) in state increment
//! form" (a derivation of Zavalishin's "The Art of VA Filter Design").
//! The same integrator state produces all four output taps — low-pass,
//! band-pass, high-pass, and notch — so switching modes is just a
//! match on the output stage and does not need to reset state.
//!
//! Why this filter rather than a biquad: TPT preserves the analog
//! prototype's behaviour at high resonance, stays stable up to (and
//! musically through) self-oscillation, and lets cutoff modulate at
//! audio rates without the dezippering grief you get with a direct-
//! form biquad whose coefficients aren't a smooth function of cutoff
//! near Nyquist. See dsp-and-sound.md §"Filter design".

use core::f32::consts::PI;

use super::FilterSlope;

/// Lowest cutoff the filter accepts, in Hz. Below this the prewarp
/// `tan(π·fc/fs)` is essentially zero and the filter passes input
/// straight through, which is the right musical answer; pinning to a
/// floor avoids divide-by-near-zero artefacts in the coefficient
/// recompute.
const MIN_CUTOFF_HZ: f32 = 20.0;

/// Headroom below Nyquist where the prewarp `tan` blows up. Keeping
/// 1 % of fs on the high end is a standard musically-transparent
/// cap.
const NYQUIST_HEADROOM: f32 = 0.49;

/// Minimum filter Q. Heavily damped (no resonant peak at all) — the
/// filter behaves like a clean two-pole shelf.
const MIN_Q: f32 = 0.5;

/// Maximum filter Q. Very ringy and audibly self-oscillating in the
/// LP / BP taps; chosen so resonance = 1 is dramatic but the
/// integrator state cannot diverge for a clipped input.
const MAX_Q: f32 = 25.0;

/// Which output tap the filter exposes.
///
/// The integrator state is the same across modes; only the output
/// linear combination differs, so changes are click-free (no need to
/// re-warm state on a mode flip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    /// 2-pole low-pass. The bread-and-butter mode for subtractive
    /// patches.
    #[default]
    LowPass,

    /// 2-pole high-pass. Useful for thinning pads and saws.
    HighPass,

    /// 2-pole band-pass. Narrowing toward a sine as resonance climbs.
    BandPass,

    /// Notch (LP + HP). Removes a narrow band centred on the cutoff.
    Notch,
}

impl FilterMode {
    /// Returns the zero-based index used when serialising filter mode to a preset.
    /// Order: LowPass=0, HighPass=1, BandPass=2, Notch=3.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            FilterMode::LowPass => 0,
            FilterMode::HighPass => 1,
            FilterMode::BandPass => 2,
            FilterMode::Notch => 3,
        }
    }

    /// Converts a zero-based index back to a `FilterMode`. Indices outside 0..=3 return `LowPass`.
    #[must_use]
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => FilterMode::LowPass,
            1 => FilterMode::HighPass,
            2 => FilterMode::BandPass,
            3 => FilterMode::Notch,
            _ => FilterMode::LowPass,
        }
    }
}

/// A single TPT state-variable filter instance.
///
/// Owns the integrator state and the precomputed coefficients. A
/// typical use pattern from the audio thread:
///
/// 1. Call [`set_params`](Self::set_params) once per sample with the
///    smoothed cutoff and resonance values pulled from the parameter
///    tree.
/// 2. Call [`next_sample`](Self::next_sample) with the input sample
///    to read the output tap selected by the current mode.
pub struct StateVariableFilter {
    sample_rate_hz: f32,
    mode: FilterMode,
    slope: FilterSlope,

    // Cached parameter inputs. Stored so we can decide cheaply whether
    // coefficients actually need to be recomputed on a `set_params`
    // call — the smoother delivers a slightly different value every
    // sample, so a strict equality check would always trigger; we
    // recompute unconditionally instead.
    cutoff_hz: f32,
    resonance: f32,

    // Precomputed coefficients. See `recompute_coefficients` for the
    // derivation (Simper §3, the "state increment form").
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,

    // Second-stage coefficients for the 24 dB/oct cascade. Same cutoff
    // (so `g` is shared) but a damped resonance so cascading does not
    // square the resonant peak into instability.
    k2: f32,
    a1_2: f32,
    a2_2: f32,
    a3_2: f32,

    // Trapezoidal-integrator state. The `_2` pair is the second cascade
    // stage, used only when `slope` is 24 dB/oct.
    ic1eq: f32,
    ic2eq: f32,
    ic1eq_2: f32,
    ic2eq_2: f32,
}

impl StateVariableFilter {
    /// Creates a low-pass filter sitting wide open (cutoff just below
    /// Nyquist, resonance 0) at the given sample rate. Call
    /// [`set_params`](Self::set_params) before processing to set the
    /// musical cutoff.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        let mut svf = Self {
            sample_rate_hz,
            mode: FilterMode::default(),
            slope: FilterSlope::default(),
            cutoff_hz: NYQUIST_HEADROOM * sample_rate_hz,
            resonance: 0.0,
            g: 0.0,
            k: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            k2: 0.0,
            a1_2: 0.0,
            a2_2: 0.0,
            a3_2: 0.0,
            ic1eq: 0.0,
            ic2eq: 0.0,
            ic1eq_2: 0.0,
            ic2eq_2: 0.0,
        };
        svf.recompute_coefficients();
        svf
    }

    /// Selects which output tap the filter returns from
    /// [`next_sample`](Self::next_sample). Discrete; safe to call mid-
    /// stream because all four taps share the same integrator state.
    pub fn set_mode(&mut self, mode: FilterMode) {
        self.mode = mode;
    }

    /// Returns the currently selected output tap.
    #[must_use]
    pub fn mode(&self) -> FilterMode {
        self.mode
    }

    /// Sets the roll-off slope. 24 dB/oct engages a second cascaded
    /// stage; switching is click-free because the second stage's
    /// integrators stay warm and simply stop being read at 12 dB/oct.
    pub fn set_slope(&mut self, slope: FilterSlope) {
        self.slope = slope;
    }

    /// Returns the current slope.
    #[must_use]
    pub fn slope(&self) -> FilterSlope {
        self.slope
    }

    /// Updates cutoff and resonance and recomputes the filter
    /// coefficients. Designed to be called once per sample with the
    /// smoothed values from the parameter tree.
    pub fn set_params(&mut self, cutoff_hz: f32, resonance: f32) {
        self.cutoff_hz = cutoff_hz;
        self.resonance = resonance;
        self.recompute_coefficients();
    }

    /// Clears the integrator state so the next sample is computed
    /// from a quiescent filter. The engine calls this when a voice
    /// becomes idle so the next note-from-silence does not inherit a
    /// ringing tail.
    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
        self.ic1eq_2 = 0.0;
        self.ic2eq_2 = 0.0;
    }

    /// Processes one sample. Returns the selected output tap. At
    /// 24 dB/oct the output of the first stage is fed through a second
    /// cascaded stage for a 4-pole roll-off.
    pub fn next_sample(&mut self, input: f32) -> f32 {
        let out1 = Self::process_stage(
            input,
            self.a1,
            self.a2,
            self.a3,
            self.k,
            self.mode,
            &mut self.ic1eq,
            &mut self.ic2eq,
        );
        match self.slope {
            FilterSlope::TwelveDbOct => out1,
            FilterSlope::TwentyFourDbOct => Self::process_stage(
                out1,
                self.a1_2,
                self.a2_2,
                self.a3_2,
                self.k2,
                self.mode,
                &mut self.ic1eq_2,
                &mut self.ic2eq_2,
            ),
        }
    }

    /// One TPT SVF stage in state-increment form (Simper). `v1` is the
    /// band-pass output and `v2` the low-pass; the HP / notch taps are
    /// derived from these and the input. Factored out so the 24 dB/oct
    /// cascade can reuse it with a second set of coefficients and state.
    #[allow(clippy::too_many_arguments)]
    fn process_stage(
        input: f32,
        a1: f32,
        a2: f32,
        a3: f32,
        k: f32,
        mode: FilterMode,
        ic1eq: &mut f32,
        ic2eq: &mut f32,
    ) -> f32 {
        let v3 = input - *ic2eq;
        let v1 = a1 * *ic1eq + a2 * v3;
        let v2 = *ic2eq + a2 * *ic1eq + a3 * v3;
        *ic1eq = 2.0 * v1 - *ic1eq;
        *ic2eq = 2.0 * v2 - *ic2eq;

        match mode {
            FilterMode::LowPass => v2,
            FilterMode::BandPass => v1,
            FilterMode::HighPass => input - k * v1 - v2,
            FilterMode::Notch => input - k * v1,
        }
    }

    fn recompute_coefficients(&mut self) {
        let fc = self
            .cutoff_hz
            .clamp(MIN_CUTOFF_HZ, NYQUIST_HEADROOM * self.sample_rate_hz);
        let q = resonance_to_q(self.resonance);
        self.g = (PI * fc / self.sample_rate_hz).tan();
        self.k = 1.0 / q;
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;

        // Second cascade stage shares the cutoff (`g`) but runs at a
        // reduced resonance so two stacked resonant peaks don't multiply
        // into a runaway gain at high Q.
        let q2 = resonance_to_q(self.resonance * 0.5);
        self.k2 = 1.0 / q2;
        self.a1_2 = 1.0 / (1.0 + self.g * (self.g + self.k2));
        self.a2_2 = self.g * self.a1_2;
        self.a3_2 = self.g * self.a2_2;
    }
}

/// Maps the user-facing resonance knob (0..=1) onto a musically
/// useful Q range. Linear in Q for simplicity; the perceptual taper
/// from a real "resonance" control can be added later (the matrix
/// will scale this with key tracking and velocity anyway).
fn resonance_to_q(resonance: f32) -> f32 {
    let r = resonance.clamp(0.0, 1.0);
    MIN_Q + r * (MAX_Q - MIN_Q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f32::consts::TAU;

    const SAMPLE_RATE_HZ: f32 = 48_000.0;

    /// Run a fresh sine of `signal_hz` through the filter at the given
    /// settings and return the peak |sample| of the steady-state tail
    /// (warmup discarded).
    fn measure_peak(mode: FilterMode, cutoff_hz: f32, resonance: f32, signal_hz: f32) -> f32 {
        measure_peak_slope(mode, FilterSlope::TwelveDbOct, cutoff_hz, resonance, signal_hz)
    }

    fn measure_peak_slope(mode: FilterMode, slope: FilterSlope, cutoff_hz: f32, resonance: f32, signal_hz: f32) -> f32 {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE_HZ);
        filter.set_mode(mode);
        filter.set_slope(slope);
        filter.set_params(cutoff_hz, resonance);

        // Warm the filter up so the impulse-response transient is past.
        let warmup_frames = 8_192;
        let measure_frames = 8_192;
        let mut phase = 0.0_f32;
        let dphase = TAU * signal_hz / SAMPLE_RATE_HZ;
        for _ in 0..warmup_frames {
            let _ = filter.next_sample(phase.sin());
            phase += dphase;
        }
        let mut peak = 0.0_f32;
        for _ in 0..measure_frames {
            let s = filter.next_sample(phase.sin());
            peak = peak.max(s.abs());
            phase += dphase;
        }
        peak
    }

    #[test]
    fn low_pass_passes_signal_well_below_cutoff() {
        let peak = measure_peak(FilterMode::LowPass, 8_000.0, 0.0, 200.0);
        // 200 Hz is two octaves below 8 kHz; LP at flat Q should
        // pass it almost untouched.
        assert!(peak > 0.9, "expected near-unity pass, got {peak}");
    }

    #[test]
    fn low_pass_attenuates_signal_well_above_cutoff() {
        let peak = measure_peak(FilterMode::LowPass, 200.0, 0.0, 8_000.0);
        // 8 kHz is over five octaves above 200 Hz cutoff; 12 dB/oct
        // gives ~ -60 dB attenuation, well under 0.01.
        assert!(peak < 0.05, "expected strong attenuation, got {peak}");
    }

    #[test]
    fn high_pass_attenuates_signal_well_below_cutoff() {
        let peak = measure_peak(FilterMode::HighPass, 8_000.0, 0.0, 200.0);
        assert!(peak < 0.05, "expected strong attenuation, got {peak}");
    }

    #[test]
    fn high_pass_passes_signal_well_above_cutoff() {
        let peak = measure_peak(FilterMode::HighPass, 200.0, 0.0, 8_000.0);
        assert!(peak > 0.9, "expected near-unity pass, got {peak}");
    }

    #[test]
    fn band_pass_passes_signal_at_cutoff() {
        // BP centred on the input frequency at low Q is close to
        // unity, but the gain depends on Q (~0.5 at Q=0.5). What we
        // can robustly say: the level at the centre is meaningfully
        // higher than the same input fed through a BP centred two
        // octaves away.
        let peak_on = measure_peak(FilterMode::BandPass, 1_000.0, 0.5, 1_000.0);
        let peak_off = measure_peak(FilterMode::BandPass, 1_000.0, 0.5, 4_000.0);
        assert!(
            peak_on > peak_off * 2.0,
            "expected BP centre to ring louder than out-of-band: on {peak_on}, off {peak_off}"
        );
    }

    #[test]
    fn notch_attenuates_signal_at_cutoff() {
        // Notch at signal frequency should null. Use low Q (broad
        // notch) and tolerate some leakage from the SVF's analog-style
        // skirt.
        let peak_on = measure_peak(FilterMode::Notch, 1_000.0, 0.5, 1_000.0);
        let peak_far = measure_peak(FilterMode::Notch, 100.0, 0.5, 1_000.0);
        assert!(
            peak_on < peak_far * 0.5,
            "expected notch null to be quieter than far-cutoff: on {peak_on}, far {peak_far}"
        );
    }

    #[test]
    fn resonance_amplifies_at_cutoff() {
        let low_q = measure_peak(FilterMode::LowPass, 1_000.0, 0.0, 1_000.0);
        let high_q = measure_peak(FilterMode::LowPass, 1_000.0, 1.0, 1_000.0);
        // High Q should peak well above the low-Q response at the
        // corner. Margin of 3x covers process-rate variation.
        assert!(
            high_q > low_q * 3.0,
            "expected high-Q peak well above low-Q: low {low_q}, high {high_q}"
        );
    }

    #[test]
    fn extreme_resonance_does_not_diverge() {
        // Bounded input plus the clamped Q means the integrator state
        // must stay finite over a long run.
        let mut filter = StateVariableFilter::new(SAMPLE_RATE_HZ);
        filter.set_mode(FilterMode::LowPass);
        filter.set_params(2_000.0, 1.0);
        let mut phase = 0.0_f32;
        let dphase = TAU * 2_000.0 / SAMPLE_RATE_HZ;
        for _ in 0..SAMPLE_RATE_HZ as usize {
            let s = filter.next_sample(phase.sin());
            assert!(s.is_finite(), "filter diverged: {s}");
            phase += dphase;
        }
    }

    #[test]
    fn twenty_four_db_rolls_off_steeper_than_twelve() {
        // One octave above a low-pass corner: the 4-pole cascade should
        // attenuate the signal noticeably more than the 2-pole stage.
        let twelve = measure_peak_slope(FilterMode::LowPass, FilterSlope::TwelveDbOct, 1_000.0, 0.0, 2_000.0);
        let twenty_four = measure_peak_slope(FilterMode::LowPass, FilterSlope::TwentyFourDbOct, 1_000.0, 0.0, 2_000.0);
        assert!(
            twenty_four < twelve * 0.7,
            "expected 24 dB/oct to attenuate more than 12 dB/oct: 12 {twelve}, 24 {twenty_four}"
        );
    }

    #[test]
    fn twenty_four_db_high_resonance_does_not_diverge() {
        // The damped second stage must keep the cascade bounded even at
        // max resonance with the input sitting on the corner.
        let mut filter = StateVariableFilter::new(SAMPLE_RATE_HZ);
        filter.set_mode(FilterMode::LowPass);
        filter.set_slope(FilterSlope::TwentyFourDbOct);
        filter.set_params(2_000.0, 1.0);
        let mut phase = 0.0_f32;
        let dphase = TAU * 2_000.0 / SAMPLE_RATE_HZ;
        for _ in 0..SAMPLE_RATE_HZ as usize {
            let s = filter.next_sample(phase.sin());
            assert!(s.is_finite(), "24 dB/oct filter diverged: {s}");
            phase += dphase;
        }
    }

    #[test]
    fn reset_returns_filter_to_silence() {
        let mut filter = StateVariableFilter::new(SAMPLE_RATE_HZ);
        filter.set_mode(FilterMode::LowPass);
        filter.set_params(1_000.0, 0.5);
        for _ in 0..1_000 {
            let _ = filter.next_sample(1.0);
        }
        filter.reset();
        // First sample after reset with zero input: integrators are
        // zero, so output is also zero.
        let s = filter.next_sample(0.0);
        assert_eq!(s, 0.0);
    }

    #[test]
    fn mode_change_does_not_reset_state() {
        // After warming up, switching modes should keep the integrator
        // state — the next output should NOT jump to zero.
        let mut filter = StateVariableFilter::new(SAMPLE_RATE_HZ);
        filter.set_mode(FilterMode::LowPass);
        filter.set_params(500.0, 0.5);
        let mut phase = 0.0_f32;
        let dphase = TAU * 500.0 / SAMPLE_RATE_HZ;
        for _ in 0..4_096 {
            let _ = filter.next_sample(phase.sin());
            phase += dphase;
        }
        filter.set_mode(FilterMode::BandPass);
        // First BP sample with state intact should be non-zero (BP
        // passes at the centre frequency we've been warming with).
        let s = filter.next_sample(phase.sin());
        assert!(s.abs() > 0.01, "expected non-zero post-mode-change, got {s}");
    }
}
