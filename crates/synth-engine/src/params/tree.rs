//! The engine's typed parameter tree.
//!
//! The tree is the single source of truth for sound-affecting state, per
//! `docs/planning/03-architecture/design-patterns.md` §1.3. The UI never
//! mutates parameters directly — every change arrives as an
//! [`EngineEvent`] and is applied here. Each block the engine reads the
//! tree to produce an immutable [`ParamSnapshot`] for the UI.
//!
//! Continuous parameters fall into two flavours:
//!
//! - **Smoothed** — read every sample by the audio path, so a UI step
//!   would cause a click. The tree owns a [`SmoothedParam`] and the
//!   audio thread advances it per sample (§2.6).
//! - **Stepped** — read only on edge transitions (e.g. amp release time
//!   is sampled when the envelope enters its release phase), so an
//!   instantaneous change has no audible artefact. Stored as a plain
//!   `f32` and pushed to the consuming DSP component on the event.
//!
//! Discrete parameters (waveform, filter mode, FM algorithm) have their
//! own typed event variants and live as enum fields on the tree;
//! changes take effect at the next block boundary per §2.7.
//!
//! [`EngineEvent`]: crate::EngineEvent

use crate::MAIN_OSCILLATOR_COUNT;
use crate::filter::{FilterMode, FilterRouting, FilterSlope};
use crate::fm::OPERATOR_COUNT;
use crate::lfo::SyncDivision;
use crate::mod_matrix::{MOD_MATRIX_SLOTS, ModDest, ModSource};
use crate::oscillator::Waveform;
use crate::seq::SEQ_MAX_STEPS;
use crate::smoothing::SmoothedParam;

use super::ids::ParamId;
use super::snapshot::{ParamSnapshot, SampleParams};

// ── Default constants ─────────────────────────────────────────────────────────

/// Default amp envelope attack time, in seconds.
pub(super) const DEFAULT_AMP_ATTACK_SECS: f32 = 0.010;

/// Default amp envelope decay time, in seconds.
pub(super) const DEFAULT_AMP_DECAY_SECS: f32 = 0.200;

/// Default amp envelope sustain level, on the 0..=1 scale.
pub(super) const DEFAULT_AMP_SUSTAIN_LEVEL: f32 = 0.8;

/// Default amp envelope release time, in seconds.
pub(super) const DEFAULT_AMP_RELEASE_SECS: f32 = 0.200;

/// Default master output volume, on the 0..=1 scale. Leaves headroom
/// for the polyphony summing that accumulates before M8's limiter.
pub(super) const DEFAULT_MASTER_VOLUME: f32 = 0.8;

/// Default filter cutoff frequency, in Hz. Sits well above the
/// fundamental of every playable MIDI note, so a fresh patch is
/// effectively wide open until the user turns the knob down.
pub(super) const DEFAULT_FILTER_CUTOFF_HZ: f32 = 8_000.0;

/// Default filter resonance, on the 0..=1 user-facing scale. Zero is
/// the maximally damped end of the range — no peak at all.
pub(super) const DEFAULT_FILTER_RESONANCE: f32 = 0.0;

/// Default filter 2 cutoff frequency, in Hz. Sits at the top of the
/// audible range so a fresh patch (and any pre-M17 preset that never
/// set filter 2) passes signal through the second low-pass unchanged.
pub(super) const DEFAULT_FILTER2_CUTOFF_HZ: f32 = 20_000.0;

/// Default per-oscillator level. All four oscillators arrive at unity;
/// the slot mixer's headroom scale handles the worst-case in-phase
/// sum without clipping.
pub(super) const DEFAULT_OSC_LEVEL: f32 = 1.0;

/// Default per-oscillator detune, in cents. Zero detune = exactly on
/// pitch with the held note.
pub(super) const DEFAULT_OSC_DETUNE_CENTS: f32 = 0.0;

/// Default per-oscillator pan position. Centered.
pub(super) const DEFAULT_OSC_PAN: f32 = 0.0;

/// Default unison voice count per main oscillator. `1` means unison
/// is effectively off — the bank behaves like a single oscillator
/// and unison detune / spread are inert.
pub(super) const DEFAULT_UNISON_VOICES: f32 = 1.0;

/// Default unison detune in cents. Subtle enough not to be obvious if
/// the user enables unison without touching the detune knob, but
/// large enough to actually beat audibly between voices.
pub(super) const DEFAULT_UNISON_DETUNE_CENTS: f32 = 10.0;

/// Default unison stereo spread (0..=1). Half spread is musical when
/// the user first enables unison without dialling in the width
/// explicitly.
pub(super) const DEFAULT_UNISON_SPREAD: f32 = 0.5;

// ── ParameterTree ─────────────────────────────────────────────────────────────

/// The engine's typed parameter tree.
///
/// Owns every sound-affecting parameter the engine exposes. Mutation
/// happens through [`set_continuous`](Self::set_continuous),
/// [`set_waveform`](Self::set_waveform), and similar typed setters; the
/// crate-private `set_active_voice_count` lets the engine reflect runtime
/// voice state into the next snapshot.
///
/// The tree itself does no I/O and holds no audio DSP. It is the place
/// where parameter validation, smoothing, and snapshot building live so
/// that adding a new parameter is one match arm rather than a new
/// engine field.
pub struct ParameterTree {
    pub(super) pitch_offset_semis: SmoothedParam,
    pub(super) filter_cutoff_hz: SmoothedParam,
    pub(super) filter_resonance: SmoothedParam,
    pub(super) filter2_cutoff_hz: SmoothedParam,
    pub(super) filter2_resonance: SmoothedParam,

    pub(super) osc_main_levels: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    pub(super) sub_level: SmoothedParam,

    pub(super) osc_main_detune_cents: [SmoothedParam; MAIN_OSCILLATOR_COUNT],

    pub(super) osc_main_pans: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    pub(super) sub_pan: SmoothedParam,

    pub(super) osc_main_unison_detune_cents: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    pub(super) osc_main_unison_spreads: [SmoothedParam; MAIN_OSCILLATOR_COUNT],

    /// Unison voice counts. Stepped (not smoothed) because the
    /// quantised integer values do not benefit from interpolation —
    /// switching from 3 to 4 voices is meant to be instant, not a
    /// glide through 3.4 voices.
    pub(super) osc_main_unison_voices: [f32; MAIN_OSCILLATOR_COUNT],

    /// Stepped (not smoothed): sampled by the voice at the next
    /// envelope phase transition, not per-sample.
    pub(super) amp_attack_secs: f32,
    pub(super) amp_decay_secs: f32,
    pub(super) amp_sustain_level: f32,
    pub(super) amp_release_secs: f32,

    /// Smoothed so moving the volume knob is click-free.
    pub(super) master_volume: SmoothedParam,

    pub(super) waveform: Waveform,
    pub(super) filter_mode: FilterMode,
    pub(super) filter2_mode: FilterMode,
    pub(super) filter_routing: FilterRouting,
    pub(super) filter_slope: [FilterSlope; 2],

    pub(super) active_voice_count: u8,

    pub(super) pitch_bend_semis: SmoothedParam,
    /// Mod wheel and aftertouch are stepped (plain f32) at M3.3 because
    /// they have no per-sample audio consumer until M6 routes them
    /// through the mod matrix. Smoothing can be added then.
    pub(super) mod_wheel: f32,
    pub(super) channel_aftertouch: f32,
    /// CC values for all 128 controllers, normalised 0..=1. Stepped.
    pub(super) cc_values: [f32; 128],

    // ── LFO 1 ──────────────────────────────────────────────────────────
    pub(super) lfo1_rate_hz: f32,
    pub(super) lfo1_shape_index: usize,
    pub(super) lfo1_reset_on_note_on: bool,
    pub(super) lfo1_sync_enabled: bool,
    pub(super) lfo1_sync_division: SyncDivision,
    pub(super) lfo1_global: bool,

    // ── LFO 2 ──────────────────────────────────────────────────────────
    pub(super) lfo2_rate_hz: f32,
    pub(super) lfo2_shape_index: usize,
    pub(super) lfo2_reset_on_note_on: bool,
    pub(super) lfo2_sync_enabled: bool,
    pub(super) lfo2_sync_division: SyncDivision,
    pub(super) lfo2_global: bool,

    // ── Env2 ───────────────────────────────────────────────────────────
    pub(super) env2_attack_secs: f32,
    pub(super) env2_decay_secs: f32,
    pub(super) env2_sustain_level: f32,
    pub(super) env2_release_secs: f32,
    pub(super) env2_attack_curve: f32,
    pub(super) env2_decay_curve: f32,
    pub(super) env2_release_curve: f32,

    // ── Env3 ───────────────────────────────────────────────────────────
    pub(super) env3_attack_secs: f32,
    pub(super) env3_decay_secs: f32,
    pub(super) env3_sustain_level: f32,
    pub(super) env3_release_secs: f32,
    pub(super) env3_attack_curve: f32,
    pub(super) env3_decay_curve: f32,
    pub(super) env3_release_curve: f32,

    // ── Global ─────────────────────────────────────────────────────────
    pub(super) bpm: f32,

    // ── Live modulator outputs (written by engine each block) ──────────
    pub(super) lfo1_out: f32,
    pub(super) lfo2_out: f32,
    pub(super) env2_out: f32,
    pub(super) env3_out: f32,
    pub(super) vu_peak_left: f32,
    pub(super) vu_peak_right: f32,
    /// Step index currently under the sequencer playhead, or -1 when idle.
    pub(super) seq_current_step: i8,

    // ── Mod matrix mirrors ─────────────────────────────────────────────
    pub(super) mod_slot_enabled: [bool; MOD_MATRIX_SLOTS],
    pub(super) mod_slot_source: [u8; MOD_MATRIX_SLOTS],
    pub(super) mod_slot_dest: [u8; MOD_MATRIX_SLOTS],
    pub(super) mod_slot_amount: [f32; MOD_MATRIX_SLOTS],
    pub(super) mod_slot_via: [u8; MOD_MATRIX_SLOTS],

    // ── FM synthesis ───────────────────────────────────────────────────
    pub(super) slot_level: [f32; 2],
    pub(super) slot_pan: [f32; 2],
    pub(super) fm_algorithm: [u8; 2],
    pub(super) fm_op_ratio_integer: [[u8; OPERATOR_COUNT]; 2],
    pub(super) fm_op_ratio_fine_cents: [[f32; OPERATOR_COUNT]; 2],
    pub(super) fm_op_level: [[f32; OPERATOR_COUNT]; 2],
    pub(super) fm_op_attack_secs: [[f32; OPERATOR_COUNT]; 2],
    pub(super) fm_op_decay_secs: [[f32; OPERATOR_COUNT]; 2],
    pub(super) fm_op_sustain_level: [[f32; OPERATOR_COUNT]; 2],
    pub(super) fm_op_release_secs: [[f32; OPERATOR_COUNT]; 2],
    pub(super) fm_op_feedback: [[f32; OPERATOR_COUNT]; 2],

    // ── FX chain (M8) ─────────────────────────────────────────────────────
    pub(super) fx_eq_enabled: bool,
    pub(super) fx_eq_low_gain_db: f32,
    pub(super) fx_eq_low_freq_hz: f32,
    pub(super) fx_eq_mid_gain_db: f32,
    pub(super) fx_eq_mid_freq_hz: f32,
    pub(super) fx_eq_mid_q: f32,
    pub(super) fx_eq_high_gain_db: f32,
    pub(super) fx_eq_high_freq_hz: f32,
    pub(super) fx_drive_enabled: bool,
    pub(super) fx_drive_drive: f32,
    pub(super) fx_drive_asymmetry: f32,
    pub(super) fx_chorus_enabled: bool,
    pub(super) fx_chorus_rate_hz: f32,
    pub(super) fx_chorus_depth_ms: f32,
    pub(super) fx_chorus_mix: f32,
    pub(super) fx_chorus_spread: f32,
    pub(super) fx_delay_enabled: bool,
    pub(super) fx_delay_time_secs: f32,
    pub(super) fx_delay_feedback: f32,
    pub(super) fx_delay_mix: f32,
    pub(super) fx_delay_lowcut_hz: f32,
    pub(super) fx_delay_ping_pong: bool,
    pub(super) fx_reverb_enabled: bool,
    pub(super) fx_reverb_predelay_ms: f32,
    pub(super) fx_reverb_decay_secs: f32,
    pub(super) fx_reverb_size: f32,
    pub(super) fx_reverb_damping: f32,
    pub(super) fx_reverb_mix: f32,
    // ── Arpeggiator ──────────────────────────────────────────────────────
    pub(super) arp_enabled: bool,
    pub(super) arp_mode: u8,
    pub(super) arp_octaves: u8,
    pub(super) arp_rate: u8,
    pub(super) arp_gate: f32,
    pub(super) arp_swing: f32,
    // ── Step sequencer ───────────────────────────────────────────────────
    pub(super) seq_enabled: bool,
    pub(super) seq_length: u8,
    pub(super) seq_mode: u8,
    pub(super) seq_rate: u8,
    pub(super) seq_swing: f32,
    pub(super) seq_step_note: [i8; SEQ_MAX_STEPS],
    pub(super) seq_step_velocity: [u8; SEQ_MAX_STEPS],
    pub(super) seq_step_gate: [f32; SEQ_MAX_STEPS],
    pub(super) seq_step_rest: [bool; SEQ_MAX_STEPS],
    pub(super) seq_step_tie: [bool; SEQ_MAX_STEPS],
    pub(super) seq_step_mod: [f32; SEQ_MAX_STEPS],
    pub(super) seq_step_mod2: [f32; SEQ_MAX_STEPS],
}

impl ParameterTree {
    /// Creates a tree initialised to [`ParamSnapshot::default`] values
    /// at the given sample rate. The sample rate is captured by the
    /// smoothers and fixed for the tree's lifetime.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        let defaults = ParamSnapshot::default();
        Self {
            pitch_offset_semis: SmoothedParam::new(defaults.pitch_offset_semis, sample_rate_hz),
            filter_cutoff_hz: SmoothedParam::new(defaults.filter_cutoff_hz, sample_rate_hz),
            filter_resonance: SmoothedParam::new(defaults.filter_resonance, sample_rate_hz),
            filter2_cutoff_hz: SmoothedParam::new(defaults.filter2_cutoff_hz, sample_rate_hz),
            filter2_resonance: SmoothedParam::new(defaults.filter2_resonance, sample_rate_hz),
            osc_main_levels: defaults.osc_main_levels.map(|v| SmoothedParam::new(v, sample_rate_hz)),
            sub_level: SmoothedParam::new(defaults.sub_level, sample_rate_hz),
            osc_main_detune_cents: defaults
                .osc_main_detune_cents
                .map(|v| SmoothedParam::new(v, sample_rate_hz)),
            osc_main_pans: defaults.osc_main_pans.map(|v| SmoothedParam::new(v, sample_rate_hz)),
            sub_pan: SmoothedParam::new(defaults.sub_pan, sample_rate_hz),
            osc_main_unison_detune_cents: defaults
                .osc_main_unison_detune_cents
                .map(|v| SmoothedParam::new(v, sample_rate_hz)),
            osc_main_unison_spreads: defaults
                .osc_main_unison_spreads
                .map(|v| SmoothedParam::new(v, sample_rate_hz)),
            osc_main_unison_voices: defaults.osc_main_unison_voices,
            amp_attack_secs: defaults.amp_attack_secs,
            amp_decay_secs: defaults.amp_decay_secs,
            amp_sustain_level: defaults.amp_sustain_level,
            amp_release_secs: defaults.amp_release_secs,
            master_volume: SmoothedParam::new(defaults.master_volume, sample_rate_hz),
            waveform: defaults.waveform,
            filter_mode: defaults.filter_mode,
            filter2_mode: defaults.filter2_mode,
            filter_routing: defaults.filter_routing,
            filter_slope: defaults.filter_slope,
            active_voice_count: defaults.active_voice_count,
            pitch_bend_semis: SmoothedParam::new(defaults.pitch_bend_semis, sample_rate_hz),
            mod_wheel: defaults.mod_wheel,
            channel_aftertouch: defaults.channel_aftertouch,
            cc_values: defaults.cc_values,
            lfo1_rate_hz: defaults.lfo1_rate_hz,
            lfo1_shape_index: defaults.lfo1_shape_index,
            lfo1_reset_on_note_on: defaults.lfo1_reset_on_note_on,
            lfo1_sync_enabled: defaults.lfo1_sync_enabled,
            lfo1_sync_division: SyncDivision::from_index(defaults.lfo1_sync_division_index),
            lfo1_global: defaults.lfo1_global,
            lfo2_rate_hz: defaults.lfo2_rate_hz,
            lfo2_shape_index: defaults.lfo2_shape_index,
            lfo2_reset_on_note_on: defaults.lfo2_reset_on_note_on,
            lfo2_sync_enabled: defaults.lfo2_sync_enabled,
            lfo2_sync_division: SyncDivision::from_index(defaults.lfo2_sync_division_index),
            lfo2_global: defaults.lfo2_global,
            env2_attack_secs: defaults.env2_attack_secs,
            env2_decay_secs: defaults.env2_decay_secs,
            env2_sustain_level: defaults.env2_sustain_level,
            env2_release_secs: defaults.env2_release_secs,
            env2_attack_curve: defaults.env2_attack_curve,
            env2_decay_curve: defaults.env2_decay_curve,
            env2_release_curve: defaults.env2_release_curve,
            env3_attack_secs: defaults.env3_attack_secs,
            env3_decay_secs: defaults.env3_decay_secs,
            env3_sustain_level: defaults.env3_sustain_level,
            env3_release_secs: defaults.env3_release_secs,
            env3_attack_curve: defaults.env3_attack_curve,
            env3_decay_curve: defaults.env3_decay_curve,
            env3_release_curve: defaults.env3_release_curve,
            bpm: defaults.bpm,
            lfo1_out: 0.0,
            lfo2_out: 0.0,
            env2_out: 0.0,
            env3_out: 0.0,
            vu_peak_left: 0.0,
            vu_peak_right: 0.0,
            seq_current_step: -1,
            mod_slot_enabled: [false; MOD_MATRIX_SLOTS],
            mod_slot_source: [0; MOD_MATRIX_SLOTS],
            mod_slot_dest: [0; MOD_MATRIX_SLOTS],
            mod_slot_amount: [0.0; MOD_MATRIX_SLOTS],
            mod_slot_via: [0; MOD_MATRIX_SLOTS],
            slot_level: defaults.slot_level,
            slot_pan: defaults.slot_pan,
            fm_algorithm: defaults.fm_algorithm,
            fm_op_ratio_integer: defaults.fm_op_ratio_integer,
            fm_op_ratio_fine_cents: defaults.fm_op_ratio_fine_cents,
            fm_op_level: defaults.fm_op_level,
            fm_op_attack_secs: defaults.fm_op_attack_secs,
            fm_op_decay_secs: defaults.fm_op_decay_secs,
            fm_op_sustain_level: defaults.fm_op_sustain_level,
            fm_op_release_secs: defaults.fm_op_release_secs,
            fm_op_feedback: defaults.fm_op_feedback,
            fx_eq_enabled: defaults.fx_eq_enabled,
            fx_eq_low_gain_db: defaults.fx_eq_low_gain_db,
            fx_eq_low_freq_hz: defaults.fx_eq_low_freq_hz,
            fx_eq_mid_gain_db: defaults.fx_eq_mid_gain_db,
            fx_eq_mid_freq_hz: defaults.fx_eq_mid_freq_hz,
            fx_eq_mid_q: defaults.fx_eq_mid_q,
            fx_eq_high_gain_db: defaults.fx_eq_high_gain_db,
            fx_eq_high_freq_hz: defaults.fx_eq_high_freq_hz,
            fx_drive_enabled: defaults.fx_drive_enabled,
            fx_drive_drive: defaults.fx_drive_drive,
            fx_drive_asymmetry: defaults.fx_drive_asymmetry,
            fx_chorus_enabled: defaults.fx_chorus_enabled,
            fx_chorus_rate_hz: defaults.fx_chorus_rate_hz,
            fx_chorus_depth_ms: defaults.fx_chorus_depth_ms,
            fx_chorus_mix: defaults.fx_chorus_mix,
            fx_chorus_spread: defaults.fx_chorus_spread,
            fx_delay_enabled: defaults.fx_delay_enabled,
            fx_delay_time_secs: defaults.fx_delay_time_secs,
            fx_delay_feedback: defaults.fx_delay_feedback,
            fx_delay_mix: defaults.fx_delay_mix,
            fx_delay_lowcut_hz: defaults.fx_delay_lowcut_hz,
            fx_delay_ping_pong: defaults.fx_delay_ping_pong,
            fx_reverb_enabled: defaults.fx_reverb_enabled,
            fx_reverb_predelay_ms: defaults.fx_reverb_predelay_ms,
            fx_reverb_decay_secs: defaults.fx_reverb_decay_secs,
            fx_reverb_size: defaults.fx_reverb_size,
            fx_reverb_damping: defaults.fx_reverb_damping,
            fx_reverb_mix: defaults.fx_reverb_mix,
            arp_enabled: defaults.arp_enabled,
            arp_mode: defaults.arp_mode,
            arp_octaves: defaults.arp_octaves,
            arp_rate: defaults.arp_rate,
            arp_gate: defaults.arp_gate,
            arp_swing: defaults.arp_swing,
            seq_enabled: defaults.seq_enabled,
            seq_length: defaults.seq_length,
            seq_mode: defaults.seq_mode,
            seq_rate: defaults.seq_rate,
            seq_swing: defaults.seq_swing,
            seq_step_note: defaults.seq_step_note,
            seq_step_velocity: defaults.seq_step_velocity,
            seq_step_gate: defaults.seq_step_gate,
            seq_step_rest: defaults.seq_step_rest,
            seq_step_tie: defaults.seq_step_tie,
            seq_step_mod: defaults.seq_step_mod,
            seq_step_mod2: defaults.seq_step_mod2,
        }
    }

    /// Applies a continuous parameter change. Smoothed params receive a
    /// new target (per-sample interpolation runs in the audio path);
    /// stepped params latch immediately so the next consumer-side read
    /// sees the new value.
    pub fn set_continuous(&mut self, id: ParamId, value: f32) {
        match id {
            ParamId::PitchOffsetSemis => self.pitch_offset_semis.set_target(value),
            ParamId::AmpAttackSecs => self.amp_attack_secs = value,
            ParamId::AmpDecaySecs => self.amp_decay_secs = value,
            ParamId::AmpSustainLevel => self.amp_sustain_level = value,
            ParamId::AmpReleaseSecs => self.amp_release_secs = value,
            ParamId::MasterVolume => self.master_volume.set_target(value),
            ParamId::FilterCutoffHz => self.filter_cutoff_hz.set_target(value),
            ParamId::FilterResonance => self.filter_resonance.set_target(value),
            ParamId::Filter2CutoffHz => self.filter2_cutoff_hz.set_target(value),
            ParamId::Filter2Resonance => self.filter2_resonance.set_target(value),
            ParamId::Osc1Level => self.osc_main_levels[0].set_target(value),
            ParamId::Osc2Level => self.osc_main_levels[1].set_target(value),
            ParamId::Osc3Level => self.osc_main_levels[2].set_target(value),
            ParamId::SubLevel => self.sub_level.set_target(value),
            ParamId::Osc1DetuneCents => self.osc_main_detune_cents[0].set_target(value),
            ParamId::Osc2DetuneCents => self.osc_main_detune_cents[1].set_target(value),
            ParamId::Osc3DetuneCents => self.osc_main_detune_cents[2].set_target(value),
            ParamId::Osc1Pan => self.osc_main_pans[0].set_target(value),
            ParamId::Osc2Pan => self.osc_main_pans[1].set_target(value),
            ParamId::Osc3Pan => self.osc_main_pans[2].set_target(value),
            ParamId::SubPan => self.sub_pan.set_target(value),
            ParamId::Osc1UnisonVoices => self.osc_main_unison_voices[0] = value,
            ParamId::Osc2UnisonVoices => self.osc_main_unison_voices[1] = value,
            ParamId::Osc3UnisonVoices => self.osc_main_unison_voices[2] = value,
            ParamId::Osc1UnisonDetuneCents => self.osc_main_unison_detune_cents[0].set_target(value),
            ParamId::Osc2UnisonDetuneCents => self.osc_main_unison_detune_cents[1].set_target(value),
            ParamId::Osc3UnisonDetuneCents => self.osc_main_unison_detune_cents[2].set_target(value),
            ParamId::Osc1UnisonSpread => self.osc_main_unison_spreads[0].set_target(value),
            ParamId::Osc2UnisonSpread => self.osc_main_unison_spreads[1].set_target(value),
            ParamId::Osc3UnisonSpread => self.osc_main_unison_spreads[2].set_target(value),
            ParamId::PitchBendSemis => self.pitch_bend_semis.set_target(value),
            ParamId::ModWheel => self.mod_wheel = value,
            ParamId::ChannelAftertouch => self.channel_aftertouch = value,
            ParamId::Lfo1RateHz => self.lfo1_rate_hz = value,
            ParamId::Lfo1Shape => self.lfo1_shape_index = value as usize,
            ParamId::Lfo1ResetOnNoteOn => self.lfo1_reset_on_note_on = value >= 0.5,
            ParamId::Lfo1SyncEnabled => self.lfo1_sync_enabled = value >= 0.5,
            ParamId::Lfo1SyncDivision => {
                self.lfo1_sync_division = SyncDivision::from_index(value as usize);
            }
            ParamId::Lfo1Global => self.lfo1_global = value >= 0.5,
            ParamId::Lfo2RateHz => self.lfo2_rate_hz = value,
            ParamId::Lfo2Shape => self.lfo2_shape_index = value as usize,
            ParamId::Lfo2ResetOnNoteOn => self.lfo2_reset_on_note_on = value >= 0.5,
            ParamId::Lfo2SyncEnabled => self.lfo2_sync_enabled = value >= 0.5,
            ParamId::Lfo2SyncDivision => {
                self.lfo2_sync_division = SyncDivision::from_index(value as usize);
            }
            ParamId::Lfo2Global => self.lfo2_global = value >= 0.5,
            ParamId::Env2AttackSecs => self.env2_attack_secs = value,
            ParamId::Env2DecaySecs => self.env2_decay_secs = value,
            ParamId::Env2SustainLevel => self.env2_sustain_level = value,
            ParamId::Env2ReleaseSecs => self.env2_release_secs = value,
            ParamId::Env2AttackCurve => self.env2_attack_curve = value,
            ParamId::Env2DecayCurve => self.env2_decay_curve = value,
            ParamId::Env2ReleaseCurve => self.env2_release_curve = value,
            ParamId::Env3AttackSecs => self.env3_attack_secs = value,
            ParamId::Env3DecaySecs => self.env3_decay_secs = value,
            ParamId::Env3SustainLevel => self.env3_sustain_level = value,
            ParamId::Env3ReleaseSecs => self.env3_release_secs = value,
            ParamId::Env3AttackCurve => self.env3_attack_curve = value,
            ParamId::Env3DecayCurve => self.env3_decay_curve = value,
            ParamId::Env3ReleaseCurve => self.env3_release_curve = value,
            ParamId::Bpm => self.bpm = value,
            ParamId::ModSlotEnabled(i) => {
                if (i as usize) < MOD_MATRIX_SLOTS {
                    self.mod_slot_enabled[i as usize] = value >= 0.5;
                }
            }
            ParamId::ModSlotSource(i) => {
                if (i as usize) < MOD_MATRIX_SLOTS {
                    self.mod_slot_source[i as usize] =
                        ModSource::from_index(value as u8).unwrap_or_default().to_index();
                }
            }
            ParamId::ModSlotDest(i) => {
                if (i as usize) < MOD_MATRIX_SLOTS {
                    self.mod_slot_dest[i as usize] = ModDest::from_index(value as u8).unwrap_or_default().to_index();
                }
            }
            ParamId::ModSlotAmount(i) => {
                if (i as usize) < MOD_MATRIX_SLOTS {
                    self.mod_slot_amount[i as usize] = value;
                }
            }
            ParamId::ModSlotVia(i) => {
                if (i as usize) < MOD_MATRIX_SLOTS {
                    self.mod_slot_via[i as usize] = ModSource::from_index(value as u8).unwrap_or_default().to_index();
                }
            }
            ParamId::SlotLevel(i) => {
                if (i as usize) < 2 {
                    self.slot_level[i as usize] = value.clamp(0.0, 1.0);
                }
            }
            ParamId::SlotPan(i) => {
                if (i as usize) < 2 {
                    self.slot_pan[i as usize] = value.clamp(-1.0, 1.0);
                }
            }
            ParamId::FmAlgorithm(i) => {
                if (i as usize) < 2 {
                    self.fm_algorithm[i as usize] = (value as u8).min(7);
                }
            }
            ParamId::FmOpRatioInteger(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_ratio_integer[slot][op] = (value as u8).min(15);
                }
            }
            ParamId::FmOpRatioFine(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_ratio_fine_cents[slot][op] = value.clamp(-100.0, 100.0);
                }
            }
            ParamId::FmOpLevel(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_level[slot][op] = value.clamp(0.0, 1.0);
                }
            }
            ParamId::FmOpAttackSecs(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_attack_secs[slot][op] = value;
                }
            }
            ParamId::FmOpDecaySecs(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_decay_secs[slot][op] = value;
                }
            }
            ParamId::FmOpSustainLevel(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_sustain_level[slot][op] = value.clamp(0.0, 1.0);
                }
            }
            ParamId::FmOpReleaseSecs(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_release_secs[slot][op] = value;
                }
            }
            ParamId::FmOpFeedback(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                if slot < 2 && op < OPERATOR_COUNT {
                    self.fm_op_feedback[slot][op] = value.clamp(-1.0, 1.0);
                }
            }
            ParamId::FxEqEnabled => self.fx_eq_enabled = value >= 0.5,
            ParamId::FxEqLowGainDb => self.fx_eq_low_gain_db = value.clamp(-15.0, 15.0),
            ParamId::FxEqLowFreqHz => self.fx_eq_low_freq_hz = value.clamp(20.0, 2_000.0),
            ParamId::FxEqMidGainDb => self.fx_eq_mid_gain_db = value.clamp(-15.0, 15.0),
            ParamId::FxEqMidFreqHz => self.fx_eq_mid_freq_hz = value.clamp(200.0, 8_000.0),
            ParamId::FxEqMidQ => self.fx_eq_mid_q = value.clamp(0.1, 10.0),
            ParamId::FxEqHighGainDb => self.fx_eq_high_gain_db = value.clamp(-15.0, 15.0),
            ParamId::FxEqHighFreqHz => self.fx_eq_high_freq_hz = value.clamp(2_000.0, 20_000.0),
            ParamId::FxDriveEnabled => self.fx_drive_enabled = value >= 0.5,
            ParamId::FxDriveDrive => self.fx_drive_drive = value.clamp(1.0, 20.0),
            ParamId::FxDriveAsymmetry => self.fx_drive_asymmetry = value.clamp(-1.0, 1.0),
            ParamId::FxChorusEnabled => self.fx_chorus_enabled = value >= 0.5,
            ParamId::FxChorusRateHz => self.fx_chorus_rate_hz = value.clamp(0.1, 8.0),
            ParamId::FxChorusDepthMs => self.fx_chorus_depth_ms = value.clamp(0.0, 15.0),
            ParamId::FxChorusMix => self.fx_chorus_mix = value.clamp(0.0, 1.0),
            ParamId::FxChorusSpread => self.fx_chorus_spread = value.clamp(0.0, 1.0),
            ParamId::FxDelayEnabled => self.fx_delay_enabled = value >= 0.5,
            ParamId::FxDelayTimeSecs => self.fx_delay_time_secs = value.clamp(0.001, 2.0),
            ParamId::FxDelayFeedback => self.fx_delay_feedback = value.clamp(0.0, 0.95),
            ParamId::FxDelayMix => self.fx_delay_mix = value.clamp(0.0, 1.0),
            ParamId::FxDelayLowcutHz => self.fx_delay_lowcut_hz = value.clamp(20.0, 2_000.0),
            ParamId::FxDelayPingPong => self.fx_delay_ping_pong = value >= 0.5,
            ParamId::FxReverbEnabled => self.fx_reverb_enabled = value >= 0.5,
            ParamId::FxReverbPredelayMs => self.fx_reverb_predelay_ms = value.clamp(0.0, 50.0),
            ParamId::FxReverbDecaySecs => self.fx_reverb_decay_secs = value.clamp(0.1, 30.0),
            ParamId::FxReverbSize => self.fx_reverb_size = value.clamp(0.1, 1.0),
            ParamId::FxReverbDamping => self.fx_reverb_damping = value.clamp(0.0, 1.0),
            ParamId::FxReverbMix => self.fx_reverb_mix = value.clamp(0.0, 1.0),
            ParamId::ArpEnabled => self.arp_enabled = value >= 0.5,
            ParamId::ArpMode => self.arp_mode = (value as u8).min(4),
            ParamId::ArpOctaves => self.arp_octaves = (value as u8).clamp(1, 4),
            ParamId::ArpRate => self.arp_rate = (value as u8).min(4),
            ParamId::ArpGate => self.arp_gate = value.clamp(0.01, 1.0),
            ParamId::ArpSwing => self.arp_swing = value.clamp(0.5, 0.75),
            ParamId::SeqEnabled => self.seq_enabled = value >= 0.5,
            ParamId::SeqLength => {
                self.seq_length = (value as u8).clamp(1, SEQ_MAX_STEPS as u8);
            }
            ParamId::SeqMode => self.seq_mode = (value as u8).min(3),
            ParamId::SeqRate => self.seq_rate = (value as u8).min(4),
            ParamId::SeqSwing => self.seq_swing = value.clamp(0.5, 0.75),
            ParamId::SeqStepNote(i) => {
                if (i as usize) < SEQ_MAX_STEPS {
                    self.seq_step_note[i as usize] = (value.round() as i32).clamp(-24, 24) as i8;
                }
            }
            ParamId::SeqStepVelocity(i) => {
                if (i as usize) < SEQ_MAX_STEPS {
                    self.seq_step_velocity[i as usize] = (value.round() as i32).clamp(0, 127) as u8;
                }
            }
            ParamId::SeqStepGate(i) => {
                if (i as usize) < SEQ_MAX_STEPS {
                    self.seq_step_gate[i as usize] = value.clamp(0.0, 1.0);
                }
            }
            ParamId::SeqStepRest(i) => {
                if (i as usize) < SEQ_MAX_STEPS {
                    self.seq_step_rest[i as usize] = value >= 0.5;
                }
            }
            ParamId::SeqStepTie(i) => {
                if (i as usize) < SEQ_MAX_STEPS {
                    self.seq_step_tie[i as usize] = value >= 0.5;
                }
            }
            ParamId::SeqStepMod(i) => {
                if (i as usize) < SEQ_MAX_STEPS {
                    self.seq_step_mod[i as usize] = value.clamp(-1.0, 1.0);
                }
            }
            ParamId::SeqStepMod2(i) => {
                if (i as usize) < SEQ_MAX_STEPS {
                    self.seq_step_mod2[i as usize] = value.clamp(-1.0, 1.0);
                }
            }
        }
    }

    /// Stores the normalised (0..=1) value for a raw MIDI CC number.
    /// Called by the engine when a [`EngineEvent::ControlChange`] arrives
    /// for a CC that does not have its own typed `ParamId`. Values are
    /// available to the mod matrix (M6) via the parameter snapshot.
    pub fn set_cc(&mut self, cc: u8, value_normalised: f32) {
        if (cc as usize) < self.cc_values.len() {
            self.cc_values[cc as usize] = value_normalised;
        }
    }

    /// Sets the oscillator waveform. Discrete: takes effect at the next
    /// block boundary per design-patterns.md §2.7.
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    /// Sets the filter output mode. Discrete.
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_mode = mode;
    }

    /// Sets the filter 2 output mode. Discrete.
    pub fn set_filter2_mode(&mut self, mode: FilterMode) {
        self.filter2_mode = mode;
    }

    /// Sets the routing between filter 1 and filter 2. Discrete.
    pub fn set_filter_routing(&mut self, routing: FilterRouting) {
        self.filter_routing = routing;
    }

    /// Sets the slope of one filter (0 = filter 1, 1 = filter 2).
    /// Out-of-range indices are ignored. Discrete.
    pub fn set_filter_slope(&mut self, filter_idx: u8, slope: FilterSlope) {
        if let Some(s) = self.filter_slope.get_mut(filter_idx as usize) {
            *s = slope;
        }
    }

    /// Mirrors the engine's active voice count into the next snapshot.
    /// Not driven by an `EngineEvent` — the engine writes this each
    /// block before publishing. At M2 the value is 0 or 1; the voice
    /// manager at M3 will pass values up to 32.
    pub fn set_active_voice_count(&mut self, count: u8) {
        self.active_voice_count = count;
    }

    /// Snaps smoothed params that should jump to their target on
    /// note-on so the first sample of a new note plays exactly at the
    /// current target value rather than mid-glide. Filter params
    /// deliberately keep smoothing so a note hit does not click the
    /// cutoff. Per-osc level / pan / detune also stay smoothed so a
    /// mid-glide adjustment continues smoothly into the new note.
    pub fn snap_for_note_on(&mut self) {
        self.pitch_offset_semis.snap_to_target();
    }

    /// Returns the current waveform.
    #[must_use]
    pub fn waveform(&self) -> Waveform {
        self.waveform
    }

    /// Returns the current filter mode.
    #[must_use]
    pub fn filter_mode(&self) -> FilterMode {
        self.filter_mode
    }

    /// Returns the current filter 2 mode.
    #[must_use]
    pub fn filter2_mode(&self) -> FilterMode {
        self.filter2_mode
    }

    /// Returns the current filter routing.
    #[must_use]
    pub fn filter_routing(&self) -> FilterRouting {
        self.filter_routing
    }

    /// Returns the slope of each filter; index 0 = filter 1, 1 = filter 2.
    #[must_use]
    pub fn filter_slope(&self) -> [FilterSlope; 2] {
        self.filter_slope
    }

    /// Returns the current amp attack time, in seconds.
    #[must_use]
    pub fn amp_attack_secs(&self) -> f32 {
        self.amp_attack_secs
    }

    /// Returns the current amp decay time, in seconds.
    #[must_use]
    pub fn amp_decay_secs(&self) -> f32 {
        self.amp_decay_secs
    }

    /// Returns the current amp sustain level, 0..=1.
    #[must_use]
    pub fn amp_sustain_level(&self) -> f32 {
        self.amp_sustain_level
    }

    /// Returns the current amp release time, in seconds.
    #[must_use]
    pub fn amp_release_secs(&self) -> f32 {
        self.amp_release_secs
    }

    /// Returns LFO1 effective rate in Hz, accounting for BPM sync.
    /// When sync is enabled, the rate is derived from the BPM and
    /// division; otherwise the free-running rate is returned.
    #[must_use]
    pub fn lfo1_effective_rate_hz(&self) -> f32 {
        if self.lfo1_sync_enabled {
            sync_rate_hz(self.bpm, self.lfo1_sync_division)
        } else {
            self.lfo1_rate_hz
        }
    }

    /// Returns LFO2 effective rate in Hz, accounting for BPM sync.
    #[must_use]
    pub fn lfo2_effective_rate_hz(&self) -> f32 {
        if self.lfo2_sync_enabled {
            sync_rate_hz(self.bpm, self.lfo2_sync_division)
        } else {
            self.lfo2_rate_hz
        }
    }

    /// Returns all LFO1 and LFO2 stepped params needed by the engine
    /// fan-out. Grouped to avoid many individual getters.
    #[must_use]
    pub fn lfo1_shape_index(&self) -> usize {
        self.lfo1_shape_index
    }

    /// Returns true if LFO1 phase resets on note-on.
    #[must_use]
    pub fn lfo1_reset_on_note_on(&self) -> bool {
        self.lfo1_reset_on_note_on
    }

    /// Returns true if LFO1 runs in global (mono) mode.
    #[must_use]
    pub fn lfo1_global(&self) -> bool {
        self.lfo1_global
    }

    /// Returns LFO2 shape index.
    #[must_use]
    pub fn lfo2_shape_index(&self) -> usize {
        self.lfo2_shape_index
    }

    /// Returns true if LFO2 phase resets on note-on.
    #[must_use]
    pub fn lfo2_reset_on_note_on(&self) -> bool {
        self.lfo2_reset_on_note_on
    }

    /// Returns true if LFO2 runs in global (mono) mode.
    #[must_use]
    pub fn lfo2_global(&self) -> bool {
        self.lfo2_global
    }

    /// Returns all Env2 stepped params as a flat tuple for engine fan-out.
    #[must_use]
    pub fn env2_attack_secs(&self) -> f32 {
        self.env2_attack_secs
    }

    /// Returns Env2 decay time.
    #[must_use]
    pub fn env2_decay_secs(&self) -> f32 {
        self.env2_decay_secs
    }

    /// Returns Env2 sustain level.
    #[must_use]
    pub fn env2_sustain_level(&self) -> f32 {
        self.env2_sustain_level
    }

    /// Returns Env2 release time.
    #[must_use]
    pub fn env2_release_secs(&self) -> f32 {
        self.env2_release_secs
    }

    /// Returns Env2 Attack curve.
    #[must_use]
    pub fn env2_attack_curve(&self) -> f32 {
        self.env2_attack_curve
    }

    /// Returns Env2 Decay curve.
    #[must_use]
    pub fn env2_decay_curve(&self) -> f32 {
        self.env2_decay_curve
    }

    /// Returns Env2 Release curve.
    #[must_use]
    pub fn env2_release_curve(&self) -> f32 {
        self.env2_release_curve
    }

    /// Returns Env3 attack time (seconds).
    #[must_use]
    pub fn env3_attack_secs(&self) -> f32 {
        self.env3_attack_secs
    }

    /// Returns Env3 decay time (seconds).
    #[must_use]
    pub fn env3_decay_secs(&self) -> f32 {
        self.env3_decay_secs
    }

    /// Returns Env3 sustain level.
    #[must_use]
    pub fn env3_sustain_level(&self) -> f32 {
        self.env3_sustain_level
    }

    /// Returns Env3 release time (seconds).
    #[must_use]
    pub fn env3_release_secs(&self) -> f32 {
        self.env3_release_secs
    }

    /// Returns Env3 Attack curve.
    #[must_use]
    pub fn env3_attack_curve(&self) -> f32 {
        self.env3_attack_curve
    }

    /// Returns Env3 Decay curve.
    #[must_use]
    pub fn env3_decay_curve(&self) -> f32 {
        self.env3_decay_curve
    }

    /// Returns Env3 Release curve.
    #[must_use]
    pub fn env3_release_curve(&self) -> f32 {
        self.env3_release_curve
    }

    /// Stores the live modulator outputs from the first active voice.
    /// Called by the engine each block, before the snapshot is published.
    pub fn set_modulator_outputs(&mut self, lfo1: f32, lfo2: f32, env2: f32, env3: f32) {
        self.lfo1_out = lfo1;
        self.lfo2_out = lfo2;
        self.env2_out = env2;
        self.env3_out = env3;
    }

    /// Stores the live sequencer playhead position (-1 when idle).
    /// Called by the engine each block, before the snapshot is published.
    pub fn set_seq_current_step(&mut self, step: i8) {
        self.seq_current_step = step;
    }

    /// Stores the per-block peak output level for the VU meter.
    /// Called by the engine immediately after the per-sample loop.
    pub fn set_vu_peak(&mut self, left: f32, right: f32) {
        self.vu_peak_left = left;
        self.vu_peak_right = right;
    }

    /// Advances all smoothed parameters by one sample and returns the
    /// values the audio path consumes per sample. Called once per
    /// frame inside `Engine::process_stereo`.
    pub fn next_sample(&mut self) -> SampleParams {
        SampleParams {
            pitch_offset_semis: self.pitch_offset_semis.next_sample(),
            filter_cutoff_hz: self.filter_cutoff_hz.next_sample(),
            filter_resonance: self.filter_resonance.next_sample(),
            filter2_cutoff_hz: self.filter2_cutoff_hz.next_sample(),
            filter2_resonance: self.filter2_resonance.next_sample(),
            osc_main_levels: [
                self.osc_main_levels[0].next_sample(),
                self.osc_main_levels[1].next_sample(),
                self.osc_main_levels[2].next_sample(),
            ],
            sub_level: self.sub_level.next_sample(),
            osc_main_detune_cents: [
                self.osc_main_detune_cents[0].next_sample(),
                self.osc_main_detune_cents[1].next_sample(),
                self.osc_main_detune_cents[2].next_sample(),
            ],
            osc_main_pans: [
                self.osc_main_pans[0].next_sample(),
                self.osc_main_pans[1].next_sample(),
                self.osc_main_pans[2].next_sample(),
            ],
            sub_pan: self.sub_pan.next_sample(),
            osc_main_unison_voices: self.osc_main_unison_voices,
            osc_main_unison_detune_cents: [
                self.osc_main_unison_detune_cents[0].next_sample(),
                self.osc_main_unison_detune_cents[1].next_sample(),
                self.osc_main_unison_detune_cents[2].next_sample(),
            ],
            osc_main_unison_spreads: [
                self.osc_main_unison_spreads[0].next_sample(),
                self.osc_main_unison_spreads[1].next_sample(),
                self.osc_main_unison_spreads[2].next_sample(),
            ],
            pitch_bend_semis: self.pitch_bend_semis.next_sample(),
            master_volume: self.master_volume.next_sample(),
        }
    }

    /// Builds an outward-facing snapshot without allocating. Each
    /// field reads the current (smoothed) value, so the UI sees what
    /// the audio path is hearing right now.
    #[must_use]
    pub fn snapshot(&self) -> ParamSnapshot {
        ParamSnapshot {
            pitch_offset_semis: self.pitch_offset_semis.current(),
            amp_attack_secs: self.amp_attack_secs,
            amp_decay_secs: self.amp_decay_secs,
            amp_sustain_level: self.amp_sustain_level,
            amp_release_secs: self.amp_release_secs,
            master_volume: self.master_volume.current(),
            waveform: self.waveform,
            filter_cutoff_hz: self.filter_cutoff_hz.current(),
            filter_resonance: self.filter_resonance.current(),
            filter_mode: self.filter_mode,
            filter2_cutoff_hz: self.filter2_cutoff_hz.current(),
            filter2_resonance: self.filter2_resonance.current(),
            filter2_mode: self.filter2_mode,
            filter_routing: self.filter_routing,
            filter_slope: self.filter_slope,
            osc_main_levels: [
                self.osc_main_levels[0].current(),
                self.osc_main_levels[1].current(),
                self.osc_main_levels[2].current(),
            ],
            sub_level: self.sub_level.current(),
            osc_main_detune_cents: [
                self.osc_main_detune_cents[0].current(),
                self.osc_main_detune_cents[1].current(),
                self.osc_main_detune_cents[2].current(),
            ],
            osc_main_pans: [
                self.osc_main_pans[0].current(),
                self.osc_main_pans[1].current(),
                self.osc_main_pans[2].current(),
            ],
            sub_pan: self.sub_pan.current(),
            osc_main_unison_voices: self.osc_main_unison_voices,
            osc_main_unison_detune_cents: [
                self.osc_main_unison_detune_cents[0].current(),
                self.osc_main_unison_detune_cents[1].current(),
                self.osc_main_unison_detune_cents[2].current(),
            ],
            osc_main_unison_spreads: [
                self.osc_main_unison_spreads[0].current(),
                self.osc_main_unison_spreads[1].current(),
                self.osc_main_unison_spreads[2].current(),
            ],
            active_voice_count: self.active_voice_count,
            pitch_bend_semis: self.pitch_bend_semis.current(),
            mod_wheel: self.mod_wheel,
            channel_aftertouch: self.channel_aftertouch,
            cc_values: self.cc_values,
            lfo1_rate_hz: self.lfo1_rate_hz,
            lfo1_shape_index: self.lfo1_shape_index,
            lfo1_reset_on_note_on: self.lfo1_reset_on_note_on,
            lfo1_sync_enabled: self.lfo1_sync_enabled,
            lfo1_sync_division_index: self.lfo1_sync_division.index(),
            lfo1_global: self.lfo1_global,
            lfo2_rate_hz: self.lfo2_rate_hz,
            lfo2_shape_index: self.lfo2_shape_index,
            lfo2_reset_on_note_on: self.lfo2_reset_on_note_on,
            lfo2_sync_enabled: self.lfo2_sync_enabled,
            lfo2_sync_division_index: self.lfo2_sync_division.index(),
            lfo2_global: self.lfo2_global,
            env2_attack_secs: self.env2_attack_secs,
            env2_decay_secs: self.env2_decay_secs,
            env2_sustain_level: self.env2_sustain_level,
            env2_release_secs: self.env2_release_secs,
            env2_attack_curve: self.env2_attack_curve,
            env2_decay_curve: self.env2_decay_curve,
            env2_release_curve: self.env2_release_curve,
            env3_attack_secs: self.env3_attack_secs,
            env3_decay_secs: self.env3_decay_secs,
            env3_sustain_level: self.env3_sustain_level,
            env3_release_secs: self.env3_release_secs,
            env3_attack_curve: self.env3_attack_curve,
            env3_decay_curve: self.env3_decay_curve,
            env3_release_curve: self.env3_release_curve,
            bpm: self.bpm,
            lfo1_out: self.lfo1_out,
            lfo2_out: self.lfo2_out,
            env2_out: self.env2_out,
            env3_out: self.env3_out,
            vu_peak_left: self.vu_peak_left,
            vu_peak_right: self.vu_peak_right,
            mod_slot_enabled: self.mod_slot_enabled,
            mod_slot_source: self.mod_slot_source,
            mod_slot_dest: self.mod_slot_dest,
            mod_slot_amount: self.mod_slot_amount,
            mod_slot_via: self.mod_slot_via,
            slot_level: self.slot_level,
            slot_pan: self.slot_pan,
            fm_algorithm: self.fm_algorithm,
            fm_op_ratio_integer: self.fm_op_ratio_integer,
            fm_op_ratio_fine_cents: self.fm_op_ratio_fine_cents,
            fm_op_level: self.fm_op_level,
            fm_op_attack_secs: self.fm_op_attack_secs,
            fm_op_decay_secs: self.fm_op_decay_secs,
            fm_op_sustain_level: self.fm_op_sustain_level,
            fm_op_release_secs: self.fm_op_release_secs,
            fm_op_feedback: self.fm_op_feedback,
            fx_eq_enabled: self.fx_eq_enabled,
            fx_eq_low_gain_db: self.fx_eq_low_gain_db,
            fx_eq_low_freq_hz: self.fx_eq_low_freq_hz,
            fx_eq_mid_gain_db: self.fx_eq_mid_gain_db,
            fx_eq_mid_freq_hz: self.fx_eq_mid_freq_hz,
            fx_eq_mid_q: self.fx_eq_mid_q,
            fx_eq_high_gain_db: self.fx_eq_high_gain_db,
            fx_eq_high_freq_hz: self.fx_eq_high_freq_hz,
            fx_drive_enabled: self.fx_drive_enabled,
            fx_drive_drive: self.fx_drive_drive,
            fx_drive_asymmetry: self.fx_drive_asymmetry,
            fx_chorus_enabled: self.fx_chorus_enabled,
            fx_chorus_rate_hz: self.fx_chorus_rate_hz,
            fx_chorus_depth_ms: self.fx_chorus_depth_ms,
            fx_chorus_mix: self.fx_chorus_mix,
            fx_chorus_spread: self.fx_chorus_spread,
            fx_delay_enabled: self.fx_delay_enabled,
            fx_delay_time_secs: self.fx_delay_time_secs,
            fx_delay_feedback: self.fx_delay_feedback,
            fx_delay_mix: self.fx_delay_mix,
            fx_delay_lowcut_hz: self.fx_delay_lowcut_hz,
            fx_delay_ping_pong: self.fx_delay_ping_pong,
            fx_reverb_enabled: self.fx_reverb_enabled,
            fx_reverb_predelay_ms: self.fx_reverb_predelay_ms,
            fx_reverb_decay_secs: self.fx_reverb_decay_secs,
            fx_reverb_size: self.fx_reverb_size,
            fx_reverb_damping: self.fx_reverb_damping,
            fx_reverb_mix: self.fx_reverb_mix,
            arp_enabled: self.arp_enabled,
            arp_mode: self.arp_mode,
            arp_octaves: self.arp_octaves,
            arp_rate: self.arp_rate,
            arp_gate: self.arp_gate,
            arp_swing: self.arp_swing,
            seq_enabled: self.seq_enabled,
            seq_length: self.seq_length,
            seq_mode: self.seq_mode,
            seq_rate: self.seq_rate,
            seq_swing: self.seq_swing,
            seq_step_note: self.seq_step_note,
            seq_step_velocity: self.seq_step_velocity,
            seq_step_gate: self.seq_step_gate,
            seq_step_rest: self.seq_step_rest,
            seq_step_tie: self.seq_step_tie,
            seq_step_mod: self.seq_step_mod,
            seq_step_mod2: self.seq_step_mod2,
            seq_current_step: self.seq_current_step,
        }
    }
}

/// Computes the LFO rate in Hz for BPM-sync mode.
///
/// `rate_hz = bpm / 60 / (4 × division.multiplier_bars())`
///
/// The result is not clamped here; [`Lfo::set_rate_hz`] clamps to
/// `[0.01, 20.0]`.
///
/// [`Lfo::set_rate_hz`]: crate::lfo::Lfo::set_rate_hz
pub(super) fn sync_rate_hz(bpm: f32, division: SyncDivision) -> f32 {
    bpm / 60.0 / (4.0 * division.multiplier_bars())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_matches_default() {
        let tree = ParameterTree::new(48_000.0);
        let snap = tree.snapshot();
        assert_eq!(snap, ParamSnapshot::default());
    }

    #[test]
    fn set_continuous_pitch_offset_smooths_toward_target() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::PitchOffsetSemis, 12.0);
        let first = tree.next_sample().pitch_offset_semis;
        assert!(first.abs() < 0.5, "expected gradual rise, got {first}");
    }

    #[test]
    fn snap_for_note_on_skips_smoothing() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::PitchOffsetSemis, 7.0);
        tree.snap_for_note_on();
        let sample = tree.next_sample().pitch_offset_semis;
        assert!((sample - 7.0).abs() < 1e-3, "expected snap to 7.0, got {sample}");
    }

    #[test]
    fn set_continuous_amp_release_latches_immediately() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::AmpReleaseSecs, 1.5);
        assert_eq!(tree.amp_release_secs(), 1.5);
        assert_eq!(tree.snapshot().amp_release_secs, 1.5);
    }

    #[test]
    fn set_continuous_filter_params_smooth_toward_target() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::FilterCutoffHz, 2_000.0);
        tree.set_continuous(ParamId::FilterResonance, 0.8);
        let first = tree.next_sample();
        assert!(
            first.filter_cutoff_hz > 4_000.0,
            "cutoff jumped: {}",
            first.filter_cutoff_hz
        );
        assert!(
            first.filter_resonance < 0.5,
            "resonance jumped: {}",
            first.filter_resonance
        );
    }

    #[test]
    fn per_osc_params_route_to_the_right_slot() {
        // Set each per-osc param to a distinct value and confirm the
        // snapshot reflects the right field. Smoothing has not been
        // ticked, so we just verify the targets are dispatched
        // correctly — current() still reads the default until samples
        // advance, so we tick a long way before asserting.
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::Osc1Level, 0.10);
        tree.set_continuous(ParamId::Osc2Level, 0.20);
        tree.set_continuous(ParamId::Osc3Level, 0.30);
        tree.set_continuous(ParamId::SubLevel, 0.40);
        tree.set_continuous(ParamId::Osc1DetuneCents, 11.0);
        tree.set_continuous(ParamId::Osc2DetuneCents, 22.0);
        tree.set_continuous(ParamId::Osc3DetuneCents, 33.0);
        tree.set_continuous(ParamId::Osc1Pan, -0.5);
        tree.set_continuous(ParamId::Osc2Pan, 0.0);
        tree.set_continuous(ParamId::Osc3Pan, 0.5);
        tree.set_continuous(ParamId::SubPan, 0.25);

        // Tick long enough that smoothers reach their targets.
        for _ in 0..8_192 {
            let _ = tree.next_sample();
        }
        let snap = tree.snapshot();
        assert!((snap.osc_main_levels[0] - 0.10).abs() < 1e-3);
        assert!((snap.osc_main_levels[1] - 0.20).abs() < 1e-3);
        assert!((snap.osc_main_levels[2] - 0.30).abs() < 1e-3);
        assert!((snap.sub_level - 0.40).abs() < 1e-3);
        assert!((snap.osc_main_detune_cents[0] - 11.0).abs() < 1e-2);
        assert!((snap.osc_main_detune_cents[1] - 22.0).abs() < 1e-2);
        assert!((snap.osc_main_detune_cents[2] - 33.0).abs() < 1e-2);
        assert!((snap.osc_main_pans[0] - -0.5).abs() < 1e-3);
        assert!((snap.osc_main_pans[1] - 0.0).abs() < 1e-3);
        assert!((snap.osc_main_pans[2] - 0.5).abs() < 1e-3);
        assert!((snap.sub_pan - 0.25).abs() < 1e-3);
    }

    #[test]
    fn filter_mode_is_visible_in_snapshot() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_filter_mode(FilterMode::BandPass);
        assert_eq!(tree.filter_mode(), FilterMode::BandPass);
        assert_eq!(tree.snapshot().filter_mode, FilterMode::BandPass);
    }

    #[test]
    fn waveform_is_visible_in_snapshot() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_waveform(Waveform::Saw);
        assert_eq!(tree.waveform(), Waveform::Saw);
        assert_eq!(tree.snapshot().waveform, Waveform::Saw);
    }

    #[test]
    fn active_voice_count_is_published() {
        let mut tree = ParameterTree::new(48_000.0);
        assert_eq!(tree.snapshot().active_voice_count, 0);
        tree.set_active_voice_count(3);
        assert_eq!(tree.snapshot().active_voice_count, 3);
    }

    #[test]
    fn bpm_sync_one_bar_at_120_bpm_is_2_hz() {
        // At 120 BPM, one bar lasts 2 s → rate = 0.5 Hz.
        // sync_rate_hz = bpm / 60 / (4 × 1.0) = 120 / 60 / 4 = 0.5 Hz.
        let rate = sync_rate_hz(120.0, SyncDivision::One);
        assert!((rate - 0.5).abs() < 1e-6, "expected 0.5 Hz, got {rate}");
    }

    #[test]
    fn lfo1_effective_rate_switches_on_sync_enable() {
        let mut tree = ParameterTree::new(48_000.0);
        // Free-running at 3 Hz.
        tree.set_continuous(ParamId::Lfo1RateHz, 3.0);
        assert!((tree.lfo1_effective_rate_hz() - 3.0).abs() < 1e-6);
        // Enable sync at 120 BPM / One bar → 0.5 Hz.
        tree.set_continuous(ParamId::Lfo1SyncEnabled, 1.0);
        tree.set_continuous(ParamId::Bpm, 120.0);
        tree.set_continuous(ParamId::Lfo1SyncDivision, 5.0); // index 5 = One bar
        assert!(
            (tree.lfo1_effective_rate_hz() - 0.5).abs() < 1e-6,
            "expected 0.5 Hz, got {}",
            tree.lfo1_effective_rate_hz()
        );
    }
}
