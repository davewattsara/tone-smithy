//! A single synth voice.
//!
//! A voice owns the subtractive slot's four oscillators — three main
//! [`UnisonOscillator`] banks (each one a 1..=7 voice unison sharing
//! a waveform), plus a dedicated sub oscillator that is always a sine
//! an octave below the held pitch — one filter (per channel), one amp
//! envelope, two LFOs, and one modulation envelope (Env2). Smoothed
//! parameters live in the engine's [`ParameterTree`]; the voice
//! consumes the current per-sample values as a [`SampleParams`]
//! reference passed to [`Voice::next_sample`].
//!
//! Signal flow per sample: each main oscillator bank produces a
//! stereo pair (its unison voices already mixed with the per-osc pan
//! as the spread centre); the sub contributes one mono sample
//! equal-power panned; the per-channel sums get the slot headroom
//! scale and feed the per-channel filter, then the envelope. The
//! filter sits *after* the per-osc mix so LP cutoff sweeps act on
//! the whole slot.
//!
//! LFOs and Env2 are block-rate: advance them once per inner block
//! via [`Voice::advance_modulators`] before the per-sample loop.
//! Their most recent outputs are available via [`Voice::lfo1_out`],
//! [`Voice::lfo2_out`], and [`Voice::env2_out`].
//!
//! [`ParameterTree`]: crate::params::ParameterTree
//! [`SampleParams`]: crate::params::SampleParams
//! [`UnisonOscillator`]: crate::oscillator::UnisonOscillator

use crate::MAIN_OSCILLATOR_COUNT;
use crate::envelope::Adsr;
use crate::filter::{FilterMode, StateVariableFilter};
use crate::lfo::{Lfo, LfoShape};
use crate::mod_env::ModEnv;
use crate::mod_matrix::DestOffsets;
use crate::oscillator::{Oscillator, UnisonOscillator, Waveform};
use crate::panning::equal_power_pan;
use crate::params::SampleParams;

/// LFO1 and LFO2 use different seeds so their S&H and SmoothRandom
/// sequences are independent from the very first note.
const LFO1_SEED: u32 = 0x1234_5678;
const LFO2_SEED: u32 = 0x9ABC_DEF0;

/// Headroom scale applied to each channel's slot sum. Sized so the
/// worst-case in-phase sum of four unit-level oscillators (3 main
/// banks + sub, each summing to ≤ 1 internally) cannot exceed unity
/// per channel even before the equal-power pan attenuation.
/// `1 / (MAIN_OSCILLATOR_COUNT + 1)`.
const SLOT_MIX_SCALE: f32 = 1.0 / 4.0;

/// One synth voice: three unison main oscillator banks + a sub
/// oscillator, mixed through per-osc level/pan into a stereo slot
/// sum, fed through one filter (per channel), gated by one amp
/// envelope, and accompanied by two LFOs and a modulation envelope.
pub struct Voice {
    main_oscillators: [UnisonOscillator; MAIN_OSCILLATOR_COUNT],
    sub_oscillator: Oscillator,
    filter_l: StateVariableFilter,
    filter_r: StateVariableFilter,
    amp_envelope: Adsr,
    lfo1: Lfo,
    lfo2: Lfo,
    mod_env: ModEnv,

    /// MIDI note currently being held by the voice, if any. Used so
    /// `note_off` only releases the matching note.
    held_note_midi: Option<u8>,

    /// Linear velocity scale applied to the amp envelope output, 0..=1.
    /// Set at `note_on` from the MIDI velocity byte. Allows soft notes
    /// to be quieter than hard ones without changing the envelope shape.
    velocity_scale: f32,

    /// Most recent output of LFO1, set by `advance_modulators`. Consumed
    /// by the mod matrix and exposed to the UI via the snapshot.
    lfo1_out: f32,
    /// Most recent output of LFO2.
    lfo2_out: f32,
    /// Most recent output of Env2 (the modulation envelope).
    env2_out: f32,

    /// Modulation offsets computed by the mod matrix once per block.
    /// Applied inside [`Voice::next_sample`]; cleared to zero at init.
    pub mod_offsets: DestOffsets,
}

impl Voice {
    /// Creates a silent, idle voice at the given sample rate. All
    /// three main oscillator banks default to one voice each
    /// ([`Waveform::Sine`]); the sub oscillator is fixed as a sine
    /// and never changes shape. The filter defaults to a wide-open
    /// low-pass.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            main_oscillators: [(); MAIN_OSCILLATOR_COUNT].map(|()| UnisonOscillator::new(sample_rate_hz)),
            sub_oscillator: Oscillator::new(sample_rate_hz),
            filter_l: StateVariableFilter::new(sample_rate_hz),
            filter_r: StateVariableFilter::new(sample_rate_hz),
            amp_envelope: Adsr::new(sample_rate_hz),
            lfo1: Lfo::new(sample_rate_hz, LFO1_SEED),
            lfo2: Lfo::new(sample_rate_hz, LFO2_SEED),
            mod_env: ModEnv::new(sample_rate_hz),
            held_note_midi: None,
            velocity_scale: 1.0,
            lfo1_out: 0.0,
            lfo2_out: 0.0,
            env2_out: 0.0,
            mod_offsets: DestOffsets::default(),
        }
    }

    /// Triggers a note. The oscillator phases are only reset when the
    /// envelope was idle (first note from silence); on retrigger the
    /// phases continue uninterrupted so there is no discontinuity in
    /// the waveform output while the envelope level is non-zero. The
    /// unison banks have their internal phases pseudo-randomised on
    /// the same idle condition so multiple held notes (M3) don't
    /// comb-filter at attack. Both channel filter states reset on the
    /// same idle condition so a fresh note never inherits a ringing
    /// tail. The caller (the engine) is responsible for snapping any
    /// per-voice smoothed parameters before calling this so the first
    /// sample plays exactly at the target value.
    pub fn note_on(&mut self, note_midi: u8, velocity: u8) {
        self.held_note_midi = Some(note_midi);
        self.velocity_scale = f32::from(velocity) / 127.0;
        if self.amp_envelope.is_idle() {
            for bank in &mut self.main_oscillators {
                bank.randomize_phases();
            }
            self.sub_oscillator.reset_phase();
            self.filter_l.reset();
            self.filter_r.reset();
        }
        self.amp_envelope.note_on();
        self.lfo1.note_on();
        self.lfo2.note_on();
        self.mod_env.note_on();
    }

    /// Releases the held note. Ignored if a different note is currently
    /// held or if the voice is already idle — this matches what
    /// polyphonic hardware does and avoids drop-outs from out-of-order
    /// note-off events.
    pub fn note_off(&mut self, note_midi: u8) {
        if self.held_note_midi == Some(note_midi) {
            self.amp_envelope.note_off();
            self.mod_env.note_off();
            self.held_note_midi = None;
        }
    }

    /// Sets the amp envelope attack time in seconds.
    pub fn set_attack_secs(&mut self, attack_secs: f32) {
        self.amp_envelope.set_attack_secs(attack_secs);
    }

    /// Sets the amp envelope decay time in seconds.
    pub fn set_decay_secs(&mut self, decay_secs: f32) {
        self.amp_envelope.set_decay_secs(decay_secs);
    }

    /// Sets the amp envelope sustain level, 0..=1.
    pub fn set_sustain_level(&mut self, sustain_level: f32) {
        self.amp_envelope.set_sustain_level(sustain_level);
    }

    /// Sets the amp envelope release time in seconds.
    pub fn set_release_secs(&mut self, release_secs: f32) {
        self.amp_envelope.set_release_secs(release_secs);
    }

    /// Sets the LFO1 rate in Hz. Clamped to `[0.01, 20.0]` inside `Lfo`.
    pub fn set_lfo1_rate_hz(&mut self, rate_hz: f32) {
        self.lfo1.set_rate_hz(rate_hz);
    }

    /// Sets the LFO1 waveform shape.
    pub fn set_lfo1_shape(&mut self, shape: LfoShape) {
        self.lfo1.set_shape(shape);
    }

    /// Enables or disables LFO1 phase reset on note-on.
    pub fn set_lfo1_reset_on_note_on(&mut self, reset: bool) {
        self.lfo1.set_reset_on_note_on(reset);
    }

    /// Sets the LFO2 rate in Hz.
    pub fn set_lfo2_rate_hz(&mut self, rate_hz: f32) {
        self.lfo2.set_rate_hz(rate_hz);
    }

    /// Sets the LFO2 waveform shape.
    pub fn set_lfo2_shape(&mut self, shape: LfoShape) {
        self.lfo2.set_shape(shape);
    }

    /// Enables or disables LFO2 phase reset on note-on.
    pub fn set_lfo2_reset_on_note_on(&mut self, reset: bool) {
        self.lfo2.set_reset_on_note_on(reset);
    }

    /// Sets the Env2 attack time in seconds.
    pub fn set_env2_attack_secs(&mut self, secs: f32) {
        self.mod_env.set_attack_secs(secs);
    }

    /// Sets the Env2 decay time in seconds.
    pub fn set_env2_decay_secs(&mut self, secs: f32) {
        self.mod_env.set_decay_secs(secs);
    }

    /// Sets the Env2 sustain level, clamped to `[0, 1]`.
    pub fn set_env2_sustain_level(&mut self, level: f32) {
        self.mod_env.set_sustain_level(level);
    }

    /// Sets the Env2 release time in seconds.
    pub fn set_env2_release_secs(&mut self, secs: f32) {
        self.mod_env.set_release_secs(secs);
    }

    /// Sets the Env2 Attack stage curve, `[-1, +1]`.
    pub fn set_env2_attack_curve(&mut self, curve: f32) {
        self.mod_env.set_attack_curve(curve);
    }

    /// Sets the Env2 Decay stage curve, `[-1, +1]`.
    pub fn set_env2_decay_curve(&mut self, curve: f32) {
        self.mod_env.set_decay_curve(curve);
    }

    /// Sets the Env2 Release stage curve, `[-1, +1]`.
    pub fn set_env2_release_curve(&mut self, curve: f32) {
        self.mod_env.set_release_curve(curve);
    }

    /// Advances LFO1, LFO2, and Env2 by `block_size` samples and caches
    /// their outputs. Call once per inner block, before the per-sample loop.
    pub fn advance_modulators(&mut self, block_size: usize) {
        self.lfo1_out = self.lfo1.advance(block_size);
        self.lfo2_out = self.lfo2.advance(block_size);
        self.env2_out = self.mod_env.advance(block_size);
    }

    /// Most recent LFO1 output from `advance_modulators`.
    #[must_use]
    pub fn lfo1_out(&self) -> f32 {
        self.lfo1_out
    }

    /// Most recent LFO2 output from `advance_modulators`.
    #[must_use]
    pub fn lfo2_out(&self) -> f32 {
        self.lfo2_out
    }

    /// Most recent Env2 output from `advance_modulators`.
    #[must_use]
    pub fn env2_out(&self) -> f32 {
        self.env2_out
    }

    /// Sets the waveform on every voice of all three main oscillator
    /// banks. The sub oscillator is unaffected — it is always a sine
    /// per `docs/planning/05-design/dsp-and-sound.md`. The discrete-
    /// parameter-at-block-boundary rule is enforced by the engine
    /// draining events before processing.
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        for bank in &mut self.main_oscillators {
            bank.set_waveform(waveform);
        }
    }

    /// Sets the filter output mode on both channel filters. The
    /// integrator state is preserved on each so mode flips are
    /// click-free.
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_l.set_mode(mode);
        self.filter_r.set_mode(mode);
    }

    /// Returns true if the voice is fully idle: both the amp envelope
    /// and Env2 have completed. Env2 may still be releasing after the
    /// amp goes silent, keeping the voice alive for M6 modulation use.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.amp_envelope.is_idle() && self.mod_env.is_idle()
    }

    /// Returns true when the amp envelope has fully released and the
    /// voice is producing no audio. Env2 may still be running.
    /// Use this for the UI voice counter — voices that are silent but
    /// still cleaning up their Env2 should not show as "active".
    #[must_use]
    pub fn is_amp_silent(&self) -> bool {
        self.amp_envelope.is_idle()
    }

    /// Returns the MIDI note the voice is currently holding (i.e.
    /// the most recent `note_on` not yet matched by a `note_off`).
    /// Used by the voice manager to route incoming note-offs.
    #[must_use]
    pub fn held_note(&self) -> Option<u8> {
        self.held_note_midi
    }

    /// Returns true while the amp envelope is in its release phase.
    /// Voice-stealing prefers releasing voices over still-attacking
    /// ones.
    #[must_use]
    pub fn is_releasing(&self) -> bool {
        self.amp_envelope.is_releasing()
    }

    /// Returns the current amp-envelope level, 0..=1. Voice-stealing
    /// uses this as the tiebreaker when no voice is in release.
    #[must_use]
    pub fn envelope_level(&self) -> f32 {
        self.amp_envelope.current_level()
    }

    /// Returns the amp-envelope level sampled at the start of the current
    /// block, before `next_sample` advances it. Used by the mod matrix to
    /// build `ModSources::amp_env`.
    #[must_use]
    pub fn amp_env_level(&self) -> f32 {
        self.amp_envelope.current_level()
    }

    /// Returns the velocity scale captured at the last `note_on`, 0..=1.
    #[must_use]
    pub fn velocity_scale(&self) -> f32 {
        self.velocity_scale
    }

    /// Produces one stereo frame as `(left, right)`. Reads every
    /// per-sample smoothed parameter from `params`; the voice itself
    /// is stateless with respect to parameter sources.
    pub fn next_sample(&mut self, params: &SampleParams) -> (f32, f32) {
        self.update_voice_counts_and_frequencies(params);
        let env =
            (self.amp_envelope.next_sample() * self.velocity_scale * (1.0 + self.mod_offsets.volume)).clamp(0.0, 1.0);

        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for (i, bank) in self.main_oscillators.iter_mut().enumerate() {
            let level = params.osc_main_levels[i];
            let (l, r) = bank.next_sample_stereo(params.osc_main_unison_spreads[i], params.osc_main_pans[i]);
            sum_l += l * level;
            sum_r += r * level;
        }
        let sub = self.sub_oscillator.next_sample();
        let (sub_pl, sub_pr) = equal_power_pan(params.sub_pan);
        sum_l += sub * params.sub_level * sub_pl;
        sum_r += sub * params.sub_level * sub_pr;

        let mixed_l = sum_l * SLOT_MIX_SCALE;
        let mixed_r = sum_r * SLOT_MIX_SCALE;

        self.filter_l
            .set_params(params.filter_cutoff_hz, params.filter_resonance);
        self.filter_r
            .set_params(params.filter_cutoff_hz, params.filter_resonance);
        let filtered_l = self.filter_l.next_sample(mixed_l);
        let filtered_r = self.filter_r.next_sample(mixed_r);

        (filtered_l * env, filtered_r * env)
    }

    /// Per sample: clamp unison voice counts to `1..=MAX_UNISON_VOICES`,
    /// then re-derive each oscillator's frequencies. Voice-count
    /// changes are detected inside the unison bank so newly active
    /// voices get fresh phases.
    fn update_voice_counts_and_frequencies(&mut self, params: &SampleParams) {
        // Voice count is meaningful even when no note is held — the
        // bank caches the count for the next frequency update.
        for (i, bank) in self.main_oscillators.iter_mut().enumerate() {
            let count = round_voice_count(params.osc_main_unison_voices[i]);
            bank.set_voice_count(count);
        }
        if let Some(note) = self.held_note_midi {
            let base_semis = f32::from(note) + params.pitch_offset_semis + params.pitch_bend_semis;
            for (i, bank) in self.main_oscillators.iter_mut().enumerate() {
                let detune_semis = params.osc_main_detune_cents[i] * (1.0 / 100.0);
                let semis = base_semis + detune_semis;
                let osc_base_hz = 440.0 * 2.0_f32.powf((semis - 69.0) / 12.0);
                bank.set_base_frequency(osc_base_hz, params.osc_main_unison_detune_cents[i]);
            }
            // Sub: one octave below the base, no detune.
            let sub_hz = 440.0 * 2.0_f32.powf((base_semis - 81.0) / 12.0);
            self.sub_oscillator.set_frequency_hz(sub_hz);
        }
    }
}

/// Rounds an `f32` voice-count parameter to the nearest valid `u8` in
/// `1..=MAX_UNISON_VOICES`. The unison bank clamps internally too, but
/// rounding here keeps `SampleParams`-side and bank-side semantics
/// aligned.
fn round_voice_count(v: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rounded = v.round().max(1.0) as u32;
    rounded.min(u8::MAX as u32) as u8
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
            osc_main_unison_voices: snap.osc_main_unison_voices,
            osc_main_unison_detune_cents: snap.osc_main_unison_detune_cents,
            osc_main_unison_spreads: snap.osc_main_unison_spreads,
            pitch_bend_semis: snap.pitch_bend_semis,
            master_volume: 1.0,
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
        voice.note_on(60, 100);
        voice.note_off(72);
        assert!(!voice.is_idle(), "voice should still be running");
    }

    #[test]
    fn retrigger_during_release_produces_no_output_discontinuity() {
        let sample_rate = 48_000.0;
        let mut voice = Voice::new(sample_rate);
        let params = default_sample_params();

        voice.note_on(60, 100);
        for _ in 0..4_800 {
            voice.next_sample(&params);
        }
        voice.note_off(60);

        let mut last = (0.0_f32, 0.0_f32);
        for _ in 0..480 {
            last = voice.next_sample(&params);
        }

        voice.note_on(62, 100);
        let first = voice.next_sample(&params);

        let jump_l = (first.0 - last.0).abs();
        let jump_r = (first.1 - last.1).abs();
        assert!(jump_l < 0.05, "L jumped by {jump_l:.4} on retrigger");
        assert!(jump_r < 0.05, "R jumped by {jump_r:.4} on retrigger");
    }

    #[test]
    fn four_in_phase_sines_stay_within_per_channel_unity() {
        // Default voice count = 1, so each main bank is a single
        // sine; with center pans and unit levels the per-channel peak
        // sits around 0.707 thanks to the equal-power center pan.
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69, 100);
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
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69, 100);
        let mut params = default_sample_params();
        params.osc_main_levels = [1.0, 0.0, 0.0];
        params.sub_level = 0.0;
        params.osc_main_pans = [-1.0, 0.0, 0.0];

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
        let mut voice = Voice::new(48_000.0);
        voice.set_main_waveform(Waveform::Saw);
        voice.note_on(60, 100);
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
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69, 100);
        let mut params = default_sample_params();
        params.osc_main_levels = [1.0, 0.0, 0.0];
        params.sub_level = 0.0;
        params.osc_main_pans = [0.0, 0.0, 0.0];
        params.osc_main_detune_cents = [1200.0, 0.0, 0.0];

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
        voice.note_on(69, 100);
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
    fn unison_widens_stereo_field_compared_to_single_voice() {
        // Solo osc1, full spread, 5 unison voices vs 1 unison voice.
        // The 5-voice case should produce a meaningfully larger L vs R
        // difference (= wider stereo) than the 1-voice case.
        fn measure_stereo_diff(voice_count: f32) -> f32 {
            let mut voice = Voice::new(48_000.0);
            voice.note_on(69, 100);
            let mut params = default_sample_params();
            params.osc_main_levels = [1.0, 0.0, 0.0];
            params.sub_level = 0.0;
            params.osc_main_unison_voices = [voice_count, 1.0, 1.0];
            params.osc_main_unison_detune_cents = [25.0, 10.0, 10.0];
            params.osc_main_unison_spreads = [1.0, 0.5, 0.5];

            // Settle.
            for _ in 0..4_800 {
                voice.next_sample(&params);
            }
            let mut diff = 0.0_f32;
            for _ in 0..4_800 {
                let (l, r) = voice.next_sample(&params);
                diff += (l - r).abs();
            }
            diff
        }
        let one = measure_stereo_diff(1.0);
        let five = measure_stereo_diff(5.0);
        assert!(one < 1.0, "1 voice should be near-mono: {one}");
        assert!(five > 5.0, "5 voices should be clearly stereo: {five}");
    }

    #[test]
    fn unison_voice_count_param_clamps_to_valid_range() {
        // Passing a wildly out-of-range voice count should not crash
        // or produce non-finite output.
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69, 100);
        let mut params = default_sample_params();
        params.osc_main_unison_voices = [-3.0, 99.0, 3.6];

        for _ in 0..4_800 {
            let (l, r) = voice.next_sample(&params);
            assert!(l.is_finite() && r.is_finite(), "non-finite output");
        }
    }
}
