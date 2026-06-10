//! A single synth voice.
//!
//! A voice owns two [`Slot`] lanes — each independently subtractive or
//! FM, mixed before the per-voice filter — plus one filter (per channel),
//! one amp envelope, two LFOs, and one modulation envelope (Env2).
//! Smoothed parameters live in the engine's [`ParameterTree`]; the voice
//! consumes the current per-sample values as a [`SampleParams`]
//! reference passed to [`Voice::next_sample`].
//!
//! Signal flow per sample: each slot produces a stereo pair scaled by
//! its mix level; both slot outputs are summed, the slot headroom
//! scale is applied, the per-channel filter runs, then the amp envelope
//! gates the result. The filter sits *after* the slot mix so a cutoff
//! sweep acts on the whole voice, not just one slot.
//!
//! M7.0 holds slot 1's mix level at zero so the audible behaviour
//! matches the pre-M7 single-slot voice. Slot 1 becomes audible once
//! the parameter bus surface (M7.3) and FM bank (M7.1/M7.2) land.
//!
//! LFOs and Env2 are block-rate: advance them once per inner block
//! via [`Voice::advance_modulators`] before the per-sample loop.
//! Their most recent outputs are available via [`Voice::lfo1_out`],
//! [`Voice::lfo2_out`], and [`Voice::env2_out`].
//!
//! [`ParameterTree`]: crate::params::ParameterTree
//! [`SampleParams`]: crate::params::SampleParams
//! [`Slot`]: crate::slot::Slot

use crate::envelope::Adsr;
use crate::filter::{FilterMode, FilterRouting, StateVariableFilter};
use crate::lfo::{Lfo, LfoShape};
use crate::mod_env::ModEnv;
use crate::mod_matrix::DestOffsets;
use crate::oscillator::Waveform;
use crate::params::SampleParams;
use crate::slot::{Slot, SlotMode};
use crate::smoothing::SmoothedParam;

/// LFO1 and LFO2 use different seeds so their S&H and SmoothRandom
/// sequences are independent from the very first note.
const LFO1_SEED: u32 = 0x1234_5678;
const LFO2_SEED: u32 = 0x9ABC_DEF0;

/// Headroom scale applied to the summed slot output before the filter.
/// Sized so the worst-case in-phase sum of one slot's four unit-level
/// oscillators (3 main banks + sub, each summing to ≤ 1 internally)
/// cannot exceed unity per channel. The same scale is reused for the
/// two-slot sum so a single audible slot keeps its pre-M7 headroom;
/// when slot 1 also runs (M7.3+) callers will need to mind the combined
/// peak via the per-slot level controls.
const SLOT_MIX_SCALE: f32 = 1.0 / 4.0;

/// One synth voice: two slots ([`Slot`]) mixed before a per-channel
/// filter, gated by one amp envelope, accompanied by two LFOs and a
/// modulation envelope (Env2).
pub struct Voice {
    /// Two synthesis slots. Slot 0 carries the existing subtractive
    /// behaviour at unit mix level. Slot 1 starts silent (level 0) in
    /// M7.0 and becomes audible once the parameter bus surface lands.
    slots: [Slot; 2],
    filter_l: StateVariableFilter,
    filter_r: StateVariableFilter,
    filter2_l: StateVariableFilter,
    filter2_r: StateVariableFilter,
    filter_routing: FilterRouting,
    amp_envelope: Adsr,
    lfo1: Lfo,
    lfo2: Lfo,
    mod_env: ModEnv,
    mod_env3: ModEnv,

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
    /// Most recent output of Env3 (the second modulation envelope).
    env3_out: f32,

    /// Modulation offsets computed by the mod matrix once per block.
    /// Applied inside [`Voice::next_sample`]; cleared to zero at init.
    pub mod_offsets: DestOffsets,

    /// Per-sample smoother for the mod-matrix *volume* offset. The matrix
    /// recomputes [`mod_offsets`](Self::mod_offsets) only once per block,
    /// so a volume routing (e.g. an LFO→Volume tremolo) would step the
    /// gain at block boundaries and crackle. Smoothing the volume offset
    /// turns those steps into per-sample ramps.
    volume_mod: SmoothedParam,
}

impl Voice {
    /// Creates a silent, idle voice at the given sample rate. Both
    /// slots default to subtractive mode; slot 0 carries the existing
    /// behaviour at mix level 1.0 and slot 1 is silent (mix level 0.0)
    /// until M7.3 surfaces per-slot mixing on the parameter bus. The
    /// filter defaults to a wide-open low-pass.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            slots: [
                Slot::new(sample_rate_hz, 1.0, SlotMode::Subtractive),
                Slot::new(sample_rate_hz, 0.0, SlotMode::Fm),
            ],
            filter_l: StateVariableFilter::new(sample_rate_hz),
            filter_r: StateVariableFilter::new(sample_rate_hz),
            filter2_l: StateVariableFilter::new(sample_rate_hz),
            filter2_r: StateVariableFilter::new(sample_rate_hz),
            filter_routing: FilterRouting::Serial,
            amp_envelope: Adsr::new(sample_rate_hz),
            lfo1: Lfo::new(sample_rate_hz, LFO1_SEED),
            lfo2: Lfo::new(sample_rate_hz, LFO2_SEED),
            mod_env: ModEnv::new(sample_rate_hz),
            mod_env3: ModEnv::new(sample_rate_hz),
            held_note_midi: None,
            velocity_scale: 1.0,
            lfo1_out: 0.0,
            lfo2_out: 0.0,
            env2_out: 0.0,
            env3_out: 0.0,
            mod_offsets: DestOffsets::default(),
            volume_mod: SmoothedParam::new(0.0, sample_rate_hz),
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
        let is_first_note = self.amp_envelope.is_idle();
        for slot in &mut self.slots {
            slot.note_on(is_first_note);
        }
        if is_first_note {
            self.filter_l.reset();
            self.filter_r.reset();
            self.filter2_l.reset();
            self.filter2_r.reset();
        }
        self.amp_envelope.note_on();
        self.lfo1.note_on();
        self.lfo2.note_on();
        self.mod_env.note_on();
        self.mod_env3.note_on();
    }

    /// Releases the held note. Ignored if a different note is currently
    /// held or if the voice is already idle — this matches what
    /// polyphonic hardware does and avoids drop-outs from out-of-order
    /// note-off events.
    pub fn note_off(&mut self, note_midi: u8) {
        if self.held_note_midi == Some(note_midi) {
            for slot in &mut self.slots {
                slot.note_off();
            }
            self.amp_envelope.note_off();
            self.mod_env.note_off();
            self.mod_env3.note_off();
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

    /// Sets the Env3 attack time in seconds.
    pub fn set_env3_attack_secs(&mut self, secs: f32) {
        self.mod_env3.set_attack_secs(secs);
    }

    /// Sets the Env3 decay time in seconds.
    pub fn set_env3_decay_secs(&mut self, secs: f32) {
        self.mod_env3.set_decay_secs(secs);
    }

    /// Sets the Env3 sustain level, clamped to `[0, 1]`.
    pub fn set_env3_sustain_level(&mut self, level: f32) {
        self.mod_env3.set_sustain_level(level);
    }

    /// Sets the Env3 release time in seconds.
    pub fn set_env3_release_secs(&mut self, secs: f32) {
        self.mod_env3.set_release_secs(secs);
    }

    /// Sets the Env3 Attack stage curve, `[-1, +1]`.
    pub fn set_env3_attack_curve(&mut self, curve: f32) {
        self.mod_env3.set_attack_curve(curve);
    }

    /// Sets the Env3 Decay stage curve, `[-1, +1]`.
    pub fn set_env3_decay_curve(&mut self, curve: f32) {
        self.mod_env3.set_decay_curve(curve);
    }

    /// Sets the Env3 Release stage curve, `[-1, +1]`.
    pub fn set_env3_release_curve(&mut self, curve: f32) {
        self.mod_env3.set_release_curve(curve);
    }

    /// Sets the mix level for slot `slot`, clamped to 0..=1.
    pub fn set_slot_level(&mut self, slot: usize, level: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.level = level.clamp(0.0, 1.0);
        }
    }

    /// Sets the mix pan for slot `slot`, clamped to -1..=1.
    pub fn set_slot_pan(&mut self, slot: usize, pan: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.pan = pan.clamp(-1.0, 1.0);
        }
    }

    /// Sets the FM algorithm for slot `slot`.
    pub fn set_fm_algorithm(&mut self, slot: usize, index: u8) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.fm.set_algorithm(index);
        }
    }

    /// Sets an FM operator's integer ratio.
    pub fn set_fm_op_ratio_integer(&mut self, slot: usize, op: usize, v: u8) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_ratio_integer(v);
            }
        }
    }

    /// Sets an FM operator's fine ratio in cents.
    pub fn set_fm_op_ratio_fine(&mut self, slot: usize, op: usize, v: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_ratio_fine_cents(v);
            }
        }
    }

    /// Sets an FM operator's output level.
    pub fn set_fm_op_level(&mut self, slot: usize, op: usize, v: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_level(v);
            }
        }
    }

    /// Sets an FM operator's envelope attack time in seconds.
    pub fn set_fm_op_attack_secs(&mut self, slot: usize, op: usize, v: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_attack_secs(v);
            }
        }
    }

    /// Sets an FM operator's envelope decay time in seconds.
    pub fn set_fm_op_decay_secs(&mut self, slot: usize, op: usize, v: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_decay_secs(v);
            }
        }
    }

    /// Sets an FM operator's envelope sustain level.
    pub fn set_fm_op_sustain_level(&mut self, slot: usize, op: usize, v: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_sustain_level(v);
            }
        }
    }

    /// Sets an FM operator's envelope release time in seconds.
    pub fn set_fm_op_release_secs(&mut self, slot: usize, op: usize, v: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_release_secs(v);
            }
        }
    }

    /// Sets an FM operator's self-feedback amount.
    pub fn set_fm_op_feedback(&mut self, slot: usize, op: usize, v: f32) {
        if let Some(s) = self.slots.get_mut(slot) {
            if let Some(operator) = s.fm.operator_mut(op) {
                operator.set_feedback_amount(v);
            }
        }
    }

    /// Advances LFO1, LFO2, Env2, and Env3 by `block_size` samples and
    /// caches their outputs. Call once per inner block, before the
    /// per-sample loop.
    pub fn advance_modulators(&mut self, block_size: usize) {
        self.lfo1_out = self.lfo1.advance(block_size);
        self.lfo2_out = self.lfo2.advance(block_size);
        self.env2_out = self.mod_env.advance(block_size);
        self.env3_out = self.mod_env3.advance(block_size);
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

    /// Most recent Env3 output from `advance_modulators`.
    #[must_use]
    pub fn env3_out(&self) -> f32 {
        self.env3_out
    }

    /// Sets the subtractive waveform on every main oscillator bank of
    /// both slots. The sub oscillator is unaffected — it is always a
    /// sine per `docs/planning/05-design/dsp-and-sound.md`. FM bank
    /// operators are always sine and ignore this setter. The discrete-
    /// parameter-at-block-boundary rule is enforced by the engine
    /// draining events before processing.
    ///
    /// Applied to both slots in M7.0 so the global `SetOscillatorWaveform`
    /// event keeps its previous voice-wide effect. Per-slot waveform
    /// control arrives with the parameter bus expansion in M7.3.
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        for slot in &mut self.slots {
            slot.set_main_waveform(waveform);
        }
    }

    /// Sets the output mode on both filter-2 channels. Click-free, like
    /// [`set_filter_mode`](Self::set_filter_mode).
    pub fn set_filter2_mode(&mut self, mode: FilterMode) {
        self.filter2_l.set_mode(mode);
        self.filter2_r.set_mode(mode);
    }

    /// Sets how filter 1 and filter 2 are connected.
    pub fn set_filter_routing(&mut self, routing: FilterRouting) {
        self.filter_routing = routing;
    }

    /// Sets the filter output mode on both channel filters. The
    /// integrator state is preserved on each so mode flips are
    /// click-free.
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_l.set_mode(mode);
        self.filter_r.set_mode(mode);
    }

    /// Returns true if the voice is fully idle: both the amp envelope
    /// and both mod envelopes have completed. Env2/Env3 may still be
    /// releasing after the amp goes silent, keeping the voice alive so
    /// their modulation finishes.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.amp_envelope.is_idle() && self.mod_env.is_idle() && self.mod_env3.is_idle()
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
    /// is stateless with respect to parameter sources. Each slot
    /// internally short-circuits to silence if its mix level is zero,
    /// so a single-slot patch costs the same as the pre-M7 voice plus
    /// one comparison per sample.
    pub fn next_sample(&mut self, params: &SampleParams) -> (f32, f32) {
        // Smooth the once-per-block volume offset to a per-sample ramp so
        // amplitude modulation (e.g. an LFO→Volume tremolo) doesn't step
        // the gain at block boundaries and crackle.
        self.volume_mod.set_target(self.mod_offsets.volume);
        let volume_mod = self.volume_mod.next_sample();
        let env = (self.amp_envelope.next_sample() * self.velocity_scale * (1.0 + volume_mod)).clamp(0.0, 1.0);

        let held = self.held_note_midi;
        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for slot in &mut self.slots {
            let (l, r) = slot.next_sample(params, held);
            sum_l += l;
            sum_r += r;
        }

        let mixed_l = sum_l * SLOT_MIX_SCALE;
        let mixed_r = sum_r * SLOT_MIX_SCALE;

        self.filter_l
            .set_params(params.filter_cutoff_hz, params.filter_resonance);
        self.filter_r
            .set_params(params.filter_cutoff_hz, params.filter_resonance);
        self.filter2_l
            .set_params(params.filter2_cutoff_hz, params.filter2_resonance);
        self.filter2_r
            .set_params(params.filter2_cutoff_hz, params.filter2_resonance);

        let (filtered_l, filtered_r) = match self.filter_routing {
            FilterRouting::Serial => {
                let f1_l = self.filter_l.next_sample(mixed_l);
                let f1_r = self.filter_r.next_sample(mixed_r);
                (self.filter2_l.next_sample(f1_l), self.filter2_r.next_sample(f1_r))
            }
            FilterRouting::Parallel => {
                // Each filter sees the slot mix; outputs are averaged so a
                // parallel patch keeps roughly the same level as serial.
                let f1_l = self.filter_l.next_sample(mixed_l);
                let f1_r = self.filter_r.next_sample(mixed_r);
                let f2_l = self.filter2_l.next_sample(mixed_l);
                let f2_r = self.filter2_r.next_sample(mixed_r);
                ((f1_l + f2_l) * 0.5, (f1_r + f2_r) * 0.5)
            }
        };

        (filtered_l * env, filtered_r * env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAIN_OSCILLATOR_COUNT, ParamSnapshot};

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
            filter2_cutoff_hz: 22_000.0,
            filter2_resonance: 0.0,
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

    #[test]
    fn fm_slot_routes_audio_through_voice_filter_and_amp() {
        // Smoke test: configure slot 1 in FM mode at unit level, snap
        // the operator envelopes and the amp envelope so we are audible
        // within a small window, play a note, confirm the voice produces
        // bounded non-zero stereo audio.
        let mut voice = Voice::new(48_000.0);
        // Silence slot 0 (Sub) so we measure only the FM contribution.
        voice.slots[0].level = 0.0;
        // Slot 1 is FM by construction; bring it to unit level.
        voice.slots[1].level = 1.0;
        for i in 0..crate::fm::OPERATOR_COUNT {
            let op = voice.slots[1].fm.operator_mut(i).unwrap();
            op.set_attack_secs(0.001);
            op.set_decay_secs(0.001);
            op.set_sustain_level(1.0);
        }
        // Snap amp envelope so we don't wait for its attack.
        voice.set_attack_secs(0.001);
        voice.set_decay_secs(0.001);
        voice.set_sustain_level(1.0);
        voice.note_on(60, 100);

        let params = default_sample_params();
        // Settle envelopes.
        for _ in 0..256 {
            voice.next_sample(&params);
        }
        let mut peak = 0.0_f32;
        for _ in 0..4096 {
            let (l, r) = voice.next_sample(&params);
            assert!(l.is_finite() && r.is_finite(), "non-finite FM output");
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 0.001, "FM-only voice should produce audio, peak={peak}");
        assert!(peak < 2.0, "FM voice output should stay bounded, peak={peak}");
    }
}
