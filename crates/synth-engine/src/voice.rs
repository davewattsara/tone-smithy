//! A single synth voice.
//!
//! A voice owns the subtractive slot's four oscillators (three main
//! oscillators sharing a waveform, plus a dedicated sub that is always
//! a sine an octave below the held pitch), one filter, and one amp
//! envelope. Smoothed parameters (pitch offset, filter cutoff,
//! resonance, per-osc level / detune / pan, …) live in the engine's
//! [`ParameterTree`] — the voice is a pure consumer that takes the
//! current per-sample values as a [`SampleParams`] reference passed
//! to [`Voice::next_sample`]. The engine owns a single voice for M2;
//! a polyphonic voice manager joins at M3.
//!
//! Signal flow per sample: each oscillator generates a sample,
//! scaled by its level and split L/R by an equal-power pan; the
//! per-channel sums are scaled by the slot headroom factor and fed
//! through the filter, then gated by the amp envelope. The filter
//! sits *after* the per-osc mix so that LP cutoff sweeps act on the
//! whole slot (the analog norm) rather than per oscillator.
//!
//! [`ParameterTree`]: crate::params::ParameterTree
//! [`SampleParams`]: crate::params::SampleParams

use crate::MAIN_OSCILLATOR_COUNT;
use crate::envelope::Adsr;
use crate::filter::{FilterMode, StateVariableFilter};
use crate::oscillator::{Oscillator, Waveform};
use crate::params::SampleParams;

/// Headroom scale applied to each channel's slot sum. Sized so the
/// worst-case in-phase sum of four unit-level oscillators (3 mains +
/// sub) cannot exceed unity per channel even before the equal-power
/// pan attenuation. `1 / (MAIN_OSCILLATOR_COUNT + 1)`.
const SLOT_MIX_SCALE: f32 = 1.0 / 4.0;

/// One synth voice: three main oscillators + a sub oscillator, mixed
/// through per-osc level/pan into a stereo slot sum, fed through one
/// filter (per channel), gated by one amp envelope.
pub struct Voice {
    main_oscillators: [Oscillator; MAIN_OSCILLATOR_COUNT],
    sub_oscillator: Oscillator,
    filter_l: StateVariableFilter,
    filter_r: StateVariableFilter,
    amp_envelope: Adsr,

    /// MIDI note currently being held by the voice, if any. Used so
    /// `note_off` only releases the matching note.
    held_note_midi: Option<u8>,
}

impl Voice {
    /// Creates a silent, idle voice at the given sample rate. All
    /// three main oscillators default to [`Waveform::Sine`]; the sub
    /// oscillator is fixed as a sine and is never changed. The filter
    /// defaults to a wide-open low-pass.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            main_oscillators: [(); MAIN_OSCILLATOR_COUNT].map(|()| Oscillator::new(sample_rate_hz)),
            sub_oscillator: Oscillator::new(sample_rate_hz),
            filter_l: StateVariableFilter::new(sample_rate_hz),
            filter_r: StateVariableFilter::new(sample_rate_hz),
            amp_envelope: Adsr::new(sample_rate_hz),
            held_note_midi: None,
        }
    }

    /// Triggers a note. The oscillator phase is only reset when the
    /// envelope was idle (first note from silence); on retrigger the
    /// phase continues uninterrupted so there is no discontinuity in
    /// the waveform output while the envelope level is non-zero. Both
    /// channel filter states reset on the same idle condition so a
    /// fresh note never inherits a ringing tail. The caller (the
    /// engine) is responsible for snapping any per-voice smoothed
    /// parameters before calling this so the first sample plays
    /// exactly at the target value.
    pub fn note_on(&mut self, note_midi: u8) {
        self.held_note_midi = Some(note_midi);
        if self.amp_envelope.is_idle() {
            for osc in &mut self.main_oscillators {
                osc.reset_phase();
            }
            self.sub_oscillator.reset_phase();
            self.filter_l.reset();
            self.filter_r.reset();
        }
        self.amp_envelope.note_on();
    }

    /// Releases the held note. Ignored if a different note is currently
    /// held or if the voice is already idle — this matches what
    /// polyphonic hardware does and avoids drop-outs from out-of-order
    /// note-off events.
    pub fn note_off(&mut self, note_midi: u8) {
        if self.held_note_midi == Some(note_midi) {
            self.amp_envelope.note_off();
            self.held_note_midi = None;
        }
    }

    /// Sets the amp envelope release time in seconds.
    pub fn set_release_secs(&mut self, release_secs: f32) {
        self.amp_envelope.set_release_secs(release_secs);
    }

    /// Sets the waveform on all three main oscillators. The sub
    /// oscillator is unaffected — it is always a sine per
    /// `docs/planning/05-design/dsp-and-sound.md`. The discrete-
    /// parameter-at-block-boundary rule is enforced by the engine
    /// draining events before processing.
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        for osc in &mut self.main_oscillators {
            osc.set_waveform(waveform);
        }
    }

    /// Sets the filter output mode on both channel filters. The
    /// integrator state is preserved on each so mode flips are
    /// click-free.
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_l.set_mode(mode);
        self.filter_r.set_mode(mode);
    }

    /// Returns true if the voice is fully idle (amp envelope at zero
    /// and no note held).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.amp_envelope.is_idle()
    }

    /// Produces one stereo frame as `(left, right)`. Reads every
    /// per-sample smoothed parameter from `params` so the engine has
    /// a single point of fan-out; the voice itself is stateless with
    /// respect to parameter sources.
    pub fn next_sample(&mut self, params: &SampleParams) -> (f32, f32) {
        self.update_frequencies(params);
        let env = self.amp_envelope.next_sample();

        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for (i, osc) in self.main_oscillators.iter_mut().enumerate() {
            let s = osc.next_sample();
            let (pl, pr) = equal_power_pan(params.osc_main_pans[i]);
            let level = params.osc_main_levels[i];
            sum_l += s * level * pl;
            sum_r += s * level * pr;
        }
        let sub = self.sub_oscillator.next_sample();
        let (sub_pl, sub_pr) = equal_power_pan(params.sub_pan);
        sum_l += sub * params.sub_level * sub_pl;
        sum_r += sub * params.sub_level * sub_pr;

        let mixed_l = sum_l * SLOT_MIX_SCALE;
        let mixed_r = sum_r * SLOT_MIX_SCALE;

        // The same filter parameters drive both channels — independent
        // L/R filters would let cutoff modulation differ across the
        // stereo field, which is not what we want for v1. Sharing the
        // params means the per-channel filters track each other; the
        // integrator state still has to be per-channel because the
        // inputs differ.
        self.filter_l
            .set_params(params.filter_cutoff_hz, params.filter_resonance);
        self.filter_r
            .set_params(params.filter_cutoff_hz, params.filter_resonance);
        let filtered_l = self.filter_l.next_sample(mixed_l);
        let filtered_r = self.filter_r.next_sample(mixed_r);

        (filtered_l * env, filtered_r * env)
    }

    /// Re-derives oscillator frequencies from the held note plus the
    /// current pitch offset and per-oscillator detune. The three main
    /// oscillators each track the held pitch shifted by their own
    /// detune; the sub oscillator runs an octave below the base
    /// (un-detuned) pitch. When no note is held (release tail)
    /// frequencies are left unchanged so each oscillator keeps cycling
    /// at its last correct pitch — stopping mid-cycle causes a timbral
    /// discontinuity that sounds like a click at note end.
    fn update_frequencies(&mut self, params: &SampleParams) {
        if let Some(note) = self.held_note_midi {
            let base_semis = f32::from(note) + params.pitch_offset_semis;
            for (i, osc) in self.main_oscillators.iter_mut().enumerate() {
                let detune_semis = params.osc_main_detune_cents[i] * (1.0 / 100.0);
                let semis = base_semis + detune_semis;
                let hz = 440.0 * 2.0_f32.powf((semis - 69.0) / 12.0);
                osc.set_frequency_hz(hz);
            }
            // Sub: one octave below the base, no detune.
            let sub_hz = 440.0 * 2.0_f32.powf((base_semis - 81.0) / 12.0);
            self.sub_oscillator.set_frequency_hz(sub_hz);
        }
    }
}

/// Equal-power pan. `pan` is in [-1, 1]: -1 is full left, +1 is full
/// right, 0 is centred (each channel at `1 / sqrt(2)`). Two sqrts per
/// call, no transcendentals. Out-of-range inputs are clamped.
#[inline]
fn equal_power_pan(pan: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    let l = ((1.0 - p) * 0.5).sqrt();
    let r = ((1.0 + p) * 0.5).sqrt();
    (l, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamSnapshot;

    /// Open-filter sample params derived from `ParamSnapshot::default`,
    /// with the filter forced wide open so the oscillator-only voice
    /// tests stay focused on the generators. Mirrors the defaults the
    /// real engine seeds the tree with — when defaults change, tests
    /// pick up the change automatically.
    fn default_sample_params() -> SampleParams {
        let snap = ParamSnapshot::default();
        SampleParams {
            pitch_offset_semis: snap.pitch_offset_semis,
            filter_cutoff_hz: 22_000.0,
            filter_resonance: 0.0,
            osc_main_levels: snap.osc_main_levels,
            sub_level: snap.sub_level,
            osc_main_detune_cents: snap.osc_main_detune_cents,
            osc_main_pans: snap.osc_main_pans,
            sub_pan: snap.sub_pan,
        }
    }

    #[test]
    fn fresh_voice_is_idle_and_silent() {
        let mut voice = Voice::new(48_000.0);
        assert!(voice.is_idle());
        let params = default_sample_params();
        for _ in 0..256 {
            assert_eq!(voice.next_sample(&params), (0.0, 0.0));
        }
    }

    #[test]
    fn note_off_for_unrelated_note_is_ignored() {
        let mut voice = Voice::new(48_000.0);
        voice.note_on(60);
        voice.note_off(72);
        assert!(!voice.is_idle(), "voice should still be running");
    }

    #[test]
    fn retrigger_during_release_produces_no_output_discontinuity() {
        let sample_rate = 48_000.0;
        let mut voice = Voice::new(sample_rate);
        let params = default_sample_params();

        voice.note_on(60);
        for _ in 0..4_800 {
            voice.next_sample(&params);
        }
        voice.note_off(60);

        let mut last = (0.0_f32, 0.0_f32);
        for _ in 0..480 {
            last = voice.next_sample(&params);
        }

        voice.note_on(62);
        let first = voice.next_sample(&params);

        let jump_l = (first.0 - last.0).abs();
        let jump_r = (first.1 - last.1).abs();
        assert!(jump_l < 0.05, "L jumped by {jump_l:.4} on retrigger");
        assert!(jump_r < 0.05, "R jumped by {jump_r:.4} on retrigger");
    }

    #[test]
    fn four_in_phase_sines_stay_within_per_channel_unity() {
        // With equal-power center pans and unit levels, four in-phase
        // sines on each channel peak at 4 * 0.707 = 2.83 before the
        // slot scale of 0.25 brings them to ~0.707 per channel. The
        // assert just checks the headroom safety bound.
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69);
        let params = default_sample_params();
        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        for _ in 0..48_000 {
            let (l, r) = voice.next_sample(&params);
            peak_l = peak_l.max(l.abs());
            peak_r = peak_r.max(r.abs());
        }
        assert!(peak_l <= 1.0 + 1e-3, "L peak exceeded unity: {peak_l}");
        assert!(peak_r <= 1.0 + 1e-3, "R peak exceeded unity: {peak_r}");
    }

    #[test]
    fn hard_pan_routes_signal_to_one_channel() {
        // Mute every oscillator except osc1, pan it hard left. The R
        // channel must be silent.
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69);
        let mut params = default_sample_params();
        params.osc_main_levels = [1.0, 0.0, 0.0];
        params.sub_level = 0.0;
        params.osc_main_pans = [-1.0, 0.0, 0.0];

        // Let the envelope leave attack.
        for _ in 0..4_800 {
            voice.next_sample(&params);
        }

        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        for _ in 0..4_800 {
            let (l, r) = voice.next_sample(&params);
            peak_l = peak_l.max(l.abs());
            peak_r = peak_r.max(r.abs());
        }
        assert!(peak_l > 0.05, "expected audible left, got {peak_l}");
        assert!(peak_r < 1e-4, "expected silent right, got {peak_r}");
    }

    #[test]
    fn mutes_all_silence_the_voice() {
        // With every oscillator at level 0 the voice's output must be
        // silent regardless of the held note or the filter setting.
        let mut voice = Voice::new(48_000.0);
        voice.set_main_waveform(Waveform::Saw);
        voice.note_on(60);
        let mut params = default_sample_params();
        params.osc_main_levels = [0.0; MAIN_OSCILLATOR_COUNT];
        params.sub_level = 0.0;

        for _ in 0..4_800 {
            let (l, r) = voice.next_sample(&params);
            assert_eq!(l, 0.0, "expected silent L with all levels 0");
            assert_eq!(r, 0.0, "expected silent R with all levels 0");
        }
    }

    #[test]
    fn detune_shifts_oscillator_pitch() {
        // Solo osc1 (mute the others), detune it +1200 cents = 1
        // octave up. The output sine should be at 880 Hz when note=69
        // (440 Hz base). Count zero crossings to verify.
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69);
        let mut params = default_sample_params();
        params.osc_main_levels = [1.0, 0.0, 0.0];
        params.sub_level = 0.0;
        params.osc_main_pans = [0.0, 0.0, 0.0];
        params.osc_main_detune_cents = [1200.0, 0.0, 0.0];

        // Settle envelope.
        for _ in 0..4_800 {
            voice.next_sample(&params);
        }

        let mut prev = voice.next_sample(&params).0;
        let mut crossings = 0;
        for _ in 0..48_000 {
            let s = voice.next_sample(&params).0;
            if (prev <= 0.0 && s > 0.0) || (prev >= 0.0 && s < 0.0) {
                crossings += 1;
            }
            prev = s;
        }
        // 880 Hz sine has ~1760 zero crossings per second; tolerate a
        // ±20 margin for envelope drift and float jitter.
        assert!(
            (1700..=1820).contains(&crossings),
            "expected ~1760 zero crossings at 880 Hz, got {crossings}"
        );
    }

    #[test]
    fn closed_low_pass_silences_the_voice() {
        let mut voice = Voice::new(48_000.0);
        voice.set_main_waveform(Waveform::Saw);
        voice.set_filter_mode(FilterMode::LowPass);
        voice.note_on(69);
        let mut params = default_sample_params();
        params.filter_cutoff_hz = 30.0;
        for _ in 0..4_800 {
            voice.next_sample(&params);
        }
        let mut peak = 0.0_f32;
        for _ in 0..4_800 {
            let (l, r) = voice.next_sample(&params);
            peak = peak.max(l.abs().max(r.abs()));
        }
        assert!(peak < 0.05, "expected LP to silence saw, peak {peak}");
    }

    #[test]
    fn equal_power_pan_lookup_is_unit_power() {
        // L² + R² = 1 for all pan positions in [-1, 1].
        for i in -100..=100 {
            #[allow(clippy::cast_precision_loss)]
            let p = (i as f32) / 100.0;
            let (l, r) = equal_power_pan(p);
            let power = l * l + r * r;
            assert!((power - 1.0).abs() < 1e-6, "pan {p}: L²+R² = {power}");
        }
    }
}
