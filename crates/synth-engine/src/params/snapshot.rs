use crate::MAIN_OSCILLATOR_COUNT;
use crate::filter::{FilterMode, FilterRouting, FilterSlope};
use crate::fm::OPERATOR_COUNT;
use crate::mod_matrix::MOD_MATRIX_SLOTS;
use crate::oscillator::Waveform;
use crate::seq::SEQ_MAX_STEPS;

use super::tree::{
    DEFAULT_AMP_ATTACK_SECS, DEFAULT_AMP_DECAY_SECS, DEFAULT_AMP_RELEASE_SECS, DEFAULT_AMP_SUSTAIN_LEVEL,
    DEFAULT_FILTER_CUTOFF_HZ, DEFAULT_FILTER_RESONANCE, DEFAULT_FILTER2_CUTOFF_HZ, DEFAULT_MASTER_VOLUME,
    DEFAULT_OSC_DETUNE_CENTS, DEFAULT_OSC_LEVEL, DEFAULT_OSC_PAN, DEFAULT_UNISON_DETUNE_CENTS, DEFAULT_UNISON_SPREAD,
    DEFAULT_UNISON_VOICES,
};

/// An immutable snapshot of the engine's outward-facing parameter
/// state, published once per audio block.
///
/// Built by [`ParameterTree::snapshot`] without allocating. The audio
/// callback is responsible for wrapping it in an `Arc` and storing in
/// the snapshot slot — the recycled-pool optimisation from
/// `docs/planning/03-architecture/design-patterns.md` §2.5 is a later
/// milestone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSnapshot {
    /// Current pitch offset, in semitones.
    pub pitch_offset_semis: f32,

    /// Current amp attack time, in seconds.
    pub amp_attack_secs: f32,

    /// Current amp decay time, in seconds.
    pub amp_decay_secs: f32,

    /// Current amp sustain level, 0..=1.
    pub amp_sustain_level: f32,

    /// Current amp release time, in seconds.
    pub amp_release_secs: f32,

    /// Current master output volume, 0..=1.
    pub master_volume: f32,

    /// Current oscillator waveform (applied to all three main
    /// oscillators; the sub is always sine).
    pub waveform: Waveform,

    /// Current filter cutoff frequency, in Hz.
    pub filter_cutoff_hz: f32,

    /// Current filter resonance on the 0..=1 user scale.
    pub filter_resonance: f32,

    /// Current filter output mode.
    pub filter_mode: FilterMode,

    /// Current filter 2 cutoff frequency, in Hz.
    pub filter2_cutoff_hz: f32,

    /// Current filter 2 resonance on the 0..=1 user scale.
    pub filter2_resonance: f32,

    /// Current filter 2 output mode.
    pub filter2_mode: FilterMode,

    /// How filters 1 and 2 are connected (serial vs. parallel).
    pub filter_routing: FilterRouting,

    /// Roll-off slope of each filter; index 0 = filter 1, 1 = filter 2.
    pub filter_slope: [FilterSlope; 2],

    /// Per-main-oscillator levels (0..=1), indexed as the voice
    /// indexes its main oscillators.
    pub osc_main_levels: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator level (0..=1).
    pub sub_level: f32,

    /// Per-main-oscillator detune in cents.
    pub osc_main_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator pan position (-1..=1).
    pub osc_main_pans: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator pan position (-1..=1).
    pub sub_pan: f32,

    /// Per-main-oscillator unison voice count (1..=MAX_UNISON_VOICES,
    /// stored as f32 because the parameter bus carries f32 values;
    /// rounded and clamped at the consumer).
    pub osc_main_unison_voices: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison detune width, in cents.
    pub osc_main_unison_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison stereo spread (0..=1).
    pub osc_main_unison_spreads: [f32; MAIN_OSCILLATOR_COUNT],

    /// Number of voices currently producing audio (not idle). Zero means
    /// the engine is silent. At M2 this is 0 or 1; the voice manager
    /// at M3 raises the ceiling to 32.
    pub active_voice_count: u8,

    /// Pitch-bend wheel position in semitones (±[`crate::engine::PITCH_BEND_RANGE_SEMIS`]).
    pub pitch_bend_semis: f32,

    /// Mod wheel (CC #1) position, 0..=1.
    pub mod_wheel: f32,

    /// Channel aftertouch pressure, 0..=1.
    pub channel_aftertouch: f32,

    /// Raw CC values for all 128 controllers, normalised to 0..=1.
    /// Indexed by CC number. Available for the mod matrix (M6) to read
    /// as modulation sources without a further API change.
    pub cc_values: [f32; 128],

    // ── LFO 1 parameter mirrors ────────────────────────────────────────
    /// LFO1 rate in Hz (free-running).
    pub lfo1_rate_hz: f32,
    /// LFO1 waveform shape as a zero-based `LfoShape` index.
    pub lfo1_shape_index: usize,
    /// LFO1 phase-reset-on-note-on flag.
    pub lfo1_reset_on_note_on: bool,
    /// LFO1 BPM-sync enabled flag.
    pub lfo1_sync_enabled: bool,
    /// LFO1 BPM-sync division as a zero-based `SyncDivision` index.
    pub lfo1_sync_division_index: usize,
    /// LFO1 global (mono) mode flag: one shared instance across all voices.
    pub lfo1_global: bool,

    // ── LFO 2 parameter mirrors ────────────────────────────────────────
    /// LFO2 rate in Hz (free-running).
    pub lfo2_rate_hz: f32,
    /// LFO2 waveform shape index.
    pub lfo2_shape_index: usize,
    /// LFO2 phase-reset-on-note-on flag.
    pub lfo2_reset_on_note_on: bool,
    /// LFO2 BPM-sync enabled flag.
    pub lfo2_sync_enabled: bool,
    /// LFO2 BPM-sync division index.
    pub lfo2_sync_division_index: usize,
    /// LFO2 global (mono) mode flag.
    pub lfo2_global: bool,

    // ── Env2 parameter mirrors ─────────────────────────────────────────
    /// Env2 attack time, seconds.
    pub env2_attack_secs: f32,
    /// Env2 decay time, seconds.
    pub env2_decay_secs: f32,
    /// Env2 sustain level, 0..=1.
    pub env2_sustain_level: f32,
    /// Env2 release time, seconds.
    pub env2_release_secs: f32,
    /// Env2 Attack stage curve, -1..=1.
    pub env2_attack_curve: f32,
    /// Env2 Decay stage curve, -1..=1.
    pub env2_decay_curve: f32,
    /// Env2 Release stage curve, -1..=1.
    pub env2_release_curve: f32,

    // ── Env3 parameter mirrors ─────────────────────────────────────────
    /// Env3 attack time, seconds.
    pub env3_attack_secs: f32,
    /// Env3 decay time, seconds.
    pub env3_decay_secs: f32,
    /// Env3 sustain level, 0..=1.
    pub env3_sustain_level: f32,
    /// Env3 release time, seconds.
    pub env3_release_secs: f32,
    /// Env3 Attack stage curve, -1..=1.
    pub env3_attack_curve: f32,
    /// Env3 Decay stage curve, -1..=1.
    pub env3_decay_curve: f32,
    /// Env3 Release stage curve, -1..=1.
    pub env3_release_curve: f32,

    // ── Global ─────────────────────────────────────────────────────────
    /// Global tempo in BPM.
    pub bpm: f32,

    // ── Live modulator outputs ─────────────────────────────────────────
    /// Most recent LFO1 output from the first active voice, or 0.0.
    pub lfo1_out: f32,
    /// Most recent LFO2 output from the first active voice, or 0.0.
    pub lfo2_out: f32,
    /// Most recent Env2 output from the first active voice, or 0.0.
    pub env2_out: f32,
    /// Most recent Env3 output from the first active voice, or 0.0.
    pub env3_out: f32,

    // ── VU meter (peak per block, written by the engine) ──────────────
    /// Peak output level for the left channel over the last audio block,
    /// linear (0..=∞; values above 1.0 indicate clipping).
    pub vu_peak_left: f32,
    /// Peak output level for the right channel over the last audio block.
    pub vu_peak_right: f32,

    // ── Mod matrix mirrors ─────────────────────────────────────────────
    /// Enable flag for each of the [`MOD_MATRIX_SLOTS`] mod slots.
    pub mod_slot_enabled: [bool; MOD_MATRIX_SLOTS],
    /// Source index for each slot (matches `ModSource::to_index`).
    pub mod_slot_source: [u8; MOD_MATRIX_SLOTS],
    /// Destination index for each slot (matches `ModDest::to_index`).
    pub mod_slot_dest: [u8; MOD_MATRIX_SLOTS],
    /// Amount for each slot, in destination-natural units.
    pub mod_slot_amount: [f32; MOD_MATRIX_SLOTS],
    /// Via-source index for each slot (0 = Off = no scaling).
    pub mod_slot_via: [u8; MOD_MATRIX_SLOTS],

    // ── FM synthesis mirrors ───────────────────────────────────────────
    /// Per-slot mix level, 0..=1.
    pub slot_level: [f32; 2],
    /// Per-slot mix pan, -1..=1.
    pub slot_pan: [f32; 2],
    /// FM algorithm index per slot, 0..=7.
    pub fm_algorithm: [u8; 2],
    /// FM operator integer ratio per `[slot][op]`, 0..=15.
    pub fm_op_ratio_integer: [[u8; OPERATOR_COUNT]; 2],
    /// FM operator fine ratio in cents per `[slot][op]`, -100..=100.
    pub fm_op_ratio_fine_cents: [[f32; OPERATOR_COUNT]; 2],
    /// FM operator output level per `[slot][op]`, 0..=1.
    pub fm_op_level: [[f32; OPERATOR_COUNT]; 2],
    /// FM operator envelope attack in seconds per `[slot][op]`.
    pub fm_op_attack_secs: [[f32; OPERATOR_COUNT]; 2],
    /// FM operator envelope decay in seconds per `[slot][op]`.
    pub fm_op_decay_secs: [[f32; OPERATOR_COUNT]; 2],
    /// FM operator envelope sustain level per `[slot][op]`, 0..=1.
    pub fm_op_sustain_level: [[f32; OPERATOR_COUNT]; 2],
    /// FM operator envelope release in seconds per `[slot][op]`.
    pub fm_op_release_secs: [[f32; OPERATOR_COUNT]; 2],
    /// FM operator self-feedback amount per `[slot][op]`, -1..=1.
    pub fm_op_feedback: [[f32; OPERATOR_COUNT]; 2],
    /// FM custom-routing connection toggles per `[slot][conn_idx]` (0..=5 into
    /// `FM_CUSTOM_CONN_TABLE`). Only used when the slot's algorithm is Custom.
    pub fm_custom_conn: [[bool; 6]; 2],
    /// FM custom-routing carrier toggles per `[slot][op]`. Only used when the
    /// slot's algorithm is Custom.
    pub fm_custom_carrier: [[bool; OPERATOR_COUNT]; 2],

    // ── FX chain mirrors (M8) ──────────────────────────────────────────────
    pub fx_eq_enabled: bool,
    pub fx_eq_low_gain_db: f32,
    pub fx_eq_low_freq_hz: f32,
    pub fx_eq_mid_gain_db: f32,
    pub fx_eq_mid_freq_hz: f32,
    pub fx_eq_mid_q: f32,
    pub fx_eq_high_gain_db: f32,
    pub fx_eq_high_freq_hz: f32,
    pub fx_drive_enabled: bool,
    pub fx_drive_drive: f32,
    pub fx_drive_asymmetry: f32,
    pub fx_chorus_enabled: bool,
    pub fx_chorus_rate_hz: f32,
    pub fx_chorus_depth_ms: f32,
    pub fx_chorus_mix: f32,
    pub fx_chorus_spread: f32,
    pub fx_delay_enabled: bool,
    pub fx_delay_time_secs: f32,
    pub fx_delay_feedback: f32,
    pub fx_delay_mix: f32,
    pub fx_delay_lowcut_hz: f32,
    pub fx_delay_ping_pong: bool,
    pub fx_reverb_enabled: bool,
    pub fx_reverb_predelay_ms: f32,
    pub fx_reverb_decay_secs: f32,
    pub fx_reverb_size: f32,
    pub fx_reverb_damping: f32,
    pub fx_reverb_mix: f32,

    // ── Arpeggiator ──────────────────────────────────────────────────────
    pub arp_enabled: bool,
    pub arp_mode: u8,
    pub arp_octaves: u8,
    pub arp_rate: u8,
    pub arp_gate: f32,
    pub arp_swing: f32,

    // ── Step sequencer ─────────────────────────────────────────────────────
    pub seq_enabled: bool,
    /// Active step count, 1–16.
    pub seq_length: u8,
    /// Playback mode: 0=Forward 1=Reverse 2=PingPong 3=Random.
    pub seq_mode: u8,
    /// Step rate: 0=1/32 1=1/16 2=1/8 3=1/4 4=1/2.
    pub seq_rate: u8,
    /// Swing fraction, 0.5–0.75.
    pub seq_swing: f32,
    /// Per-step note offset from the held root, -24..=24.
    pub seq_step_note: [i8; SEQ_MAX_STEPS],
    /// Per-step velocity, 0–127.
    pub seq_step_velocity: [u8; SEQ_MAX_STEPS],
    /// Per-step gate fraction, 0.0–1.0.
    pub seq_step_gate: [f32; SEQ_MAX_STEPS],
    /// Per-step rest toggle.
    pub seq_step_rest: [bool; SEQ_MAX_STEPS],
    /// Per-step tie toggle (hold the previous note).
    pub seq_step_tie: [bool; SEQ_MAX_STEPS],
    /// Per-step mod-lane CV, -1.0..=1.0.
    pub seq_step_mod: [f32; SEQ_MAX_STEPS],
    /// Per-step second mod-lane CV, -1.0..=1.0 (the `Seq2` source).
    pub seq_step_mod2: [f32; SEQ_MAX_STEPS],
    /// Live: step index currently under the playhead, or -1 when idle.
    pub seq_current_step: i8,
}

impl Default for ParamSnapshot {
    fn default() -> Self {
        Self {
            pitch_offset_semis: 0.0,
            amp_attack_secs: DEFAULT_AMP_ATTACK_SECS,
            amp_decay_secs: DEFAULT_AMP_DECAY_SECS,
            amp_sustain_level: DEFAULT_AMP_SUSTAIN_LEVEL,
            amp_release_secs: DEFAULT_AMP_RELEASE_SECS,
            master_volume: DEFAULT_MASTER_VOLUME,
            waveform: Waveform::Saw,
            filter_cutoff_hz: DEFAULT_FILTER_CUTOFF_HZ,
            filter_resonance: DEFAULT_FILTER_RESONANCE,
            filter_mode: FilterMode::LowPass,
            filter2_cutoff_hz: DEFAULT_FILTER2_CUTOFF_HZ,
            filter2_resonance: DEFAULT_FILTER_RESONANCE,
            filter2_mode: FilterMode::LowPass,
            filter_routing: FilterRouting::Off,
            filter_slope: [FilterSlope::TwelveDbOct; 2],
            osc_main_levels: [DEFAULT_OSC_LEVEL; MAIN_OSCILLATOR_COUNT],
            sub_level: DEFAULT_OSC_LEVEL,
            osc_main_detune_cents: [DEFAULT_OSC_DETUNE_CENTS; MAIN_OSCILLATOR_COUNT],
            osc_main_pans: [DEFAULT_OSC_PAN; MAIN_OSCILLATOR_COUNT],
            sub_pan: DEFAULT_OSC_PAN,
            osc_main_unison_voices: [DEFAULT_UNISON_VOICES; MAIN_OSCILLATOR_COUNT],
            osc_main_unison_detune_cents: [DEFAULT_UNISON_DETUNE_CENTS; MAIN_OSCILLATOR_COUNT],
            osc_main_unison_spreads: [DEFAULT_UNISON_SPREAD; MAIN_OSCILLATOR_COUNT],
            active_voice_count: 0,
            pitch_bend_semis: 0.0,
            mod_wheel: 0.0,
            channel_aftertouch: 0.0,
            cc_values: [0.0; 128],
            lfo1_rate_hz: 1.0,
            lfo1_shape_index: 0,
            lfo1_reset_on_note_on: false,
            lfo1_sync_enabled: false,
            lfo1_sync_division_index: 5, // 1 bar
            lfo1_global: false,
            lfo2_rate_hz: 1.0,
            lfo2_shape_index: 0,
            lfo2_reset_on_note_on: false,
            lfo2_sync_enabled: false,
            lfo2_sync_division_index: 5,
            lfo2_global: false,
            env2_attack_secs: 0.010,
            env2_decay_secs: 0.200,
            env2_sustain_level: 0.8,
            env2_release_secs: 0.200,
            env2_attack_curve: 0.0,
            env2_decay_curve: 0.0,
            env2_release_curve: 0.0,
            env3_attack_secs: 0.010,
            env3_decay_secs: 0.200,
            env3_sustain_level: 0.8,
            env3_release_secs: 0.200,
            env3_attack_curve: 0.0,
            env3_decay_curve: 0.0,
            env3_release_curve: 0.0,
            bpm: 120.0,
            lfo1_out: 0.0,
            lfo2_out: 0.0,
            vu_peak_left: 0.0,
            vu_peak_right: 0.0,
            env2_out: 0.0,
            env3_out: 0.0,
            mod_slot_enabled: [false; MOD_MATRIX_SLOTS],
            mod_slot_source: [0; MOD_MATRIX_SLOTS],
            mod_slot_dest: [0; MOD_MATRIX_SLOTS],
            mod_slot_amount: [0.0; MOD_MATRIX_SLOTS],
            mod_slot_via: [0; MOD_MATRIX_SLOTS],
            slot_level: [1.0, 0.0],
            slot_pan: [0.0; 2],
            fm_algorithm: [0; 2],
            fm_op_ratio_integer: [[1; OPERATOR_COUNT]; 2],
            fm_op_ratio_fine_cents: [[0.0; OPERATOR_COUNT]; 2],
            fm_op_level: [[1.0; OPERATOR_COUNT]; 2],
            fm_op_attack_secs: [[DEFAULT_AMP_ATTACK_SECS; OPERATOR_COUNT]; 2],
            fm_op_decay_secs: [[DEFAULT_AMP_DECAY_SECS; OPERATOR_COUNT]; 2],
            fm_op_sustain_level: [[DEFAULT_AMP_SUSTAIN_LEVEL; OPERATOR_COUNT]; 2],
            fm_op_release_secs: [[DEFAULT_AMP_RELEASE_SECS; OPERATOR_COUNT]; 2],
            fm_op_feedback: [[0.0; OPERATOR_COUNT]; 2],
            fm_custom_conn: [[false; 6]; 2],
            fm_custom_carrier: [[false; OPERATOR_COUNT]; 2],
            fx_eq_enabled: false,
            fx_eq_low_gain_db: 0.0,
            fx_eq_low_freq_hz: 200.0,
            fx_eq_mid_gain_db: 0.0,
            fx_eq_mid_freq_hz: 1_000.0,
            fx_eq_mid_q: 0.7,
            fx_eq_high_gain_db: 0.0,
            fx_eq_high_freq_hz: 6_000.0,
            fx_drive_enabled: false,
            fx_drive_drive: 1.0,
            fx_drive_asymmetry: 0.0,
            fx_chorus_enabled: false,
            fx_chorus_rate_hz: 0.5,
            fx_chorus_depth_ms: 3.0,
            fx_chorus_mix: 0.5,
            fx_chorus_spread: 0.5,
            fx_delay_enabled: false,
            fx_delay_time_secs: 0.375,
            fx_delay_feedback: 0.35,
            fx_delay_mix: 0.30,
            fx_delay_lowcut_hz: 200.0,
            fx_delay_ping_pong: false,
            fx_reverb_enabled: false,
            fx_reverb_predelay_ms: 10.0,
            fx_reverb_decay_secs: 2.0,
            fx_reverb_size: 0.7,
            fx_reverb_damping: 0.5,
            fx_reverb_mix: 0.25,
            arp_enabled: false,
            arp_mode: 0,
            arp_octaves: 1,
            arp_rate: 2, // 1/8
            arp_gate: 0.5,
            arp_swing: 0.5,
            seq_enabled: false,
            seq_length: SEQ_MAX_STEPS as u8,
            seq_mode: 0,    // Forward
            seq_rate: 1,    // 1/16
            seq_swing: 0.5, // straight
            seq_step_note: [0; SEQ_MAX_STEPS],
            seq_step_velocity: [100; SEQ_MAX_STEPS],
            seq_step_gate: [0.5; SEQ_MAX_STEPS],
            seq_step_rest: [false; SEQ_MAX_STEPS],
            seq_step_tie: [false; SEQ_MAX_STEPS],
            seq_step_mod: [0.0; SEQ_MAX_STEPS],
            seq_step_mod2: [0.0; SEQ_MAX_STEPS],
            seq_current_step: -1,
        }
    }
}

/// Per-sample smoothed parameter values consumed by the audio path.
///
/// Returned by [`ParameterTree::next_sample`] once per frame. Grown
/// field-by-field as new smoothed params land in later milestones —
/// keeping it a flat struct (vs. a map) means each consumer reads the
/// exact field it needs with no lookup cost.
#[derive(Debug, Clone, Copy)]
pub struct SampleParams {
    /// Pitch offset to apply on top of any held MIDI note, in
    /// semitones.
    pub pitch_offset_semis: f32,

    /// Filter cutoff frequency for this sample, in Hz.
    pub filter_cutoff_hz: f32,

    /// Filter resonance for this sample, on the 0..=1 user scale.
    pub filter_resonance: f32,

    /// Filter 2 cutoff frequency for this sample, in Hz.
    pub filter2_cutoff_hz: f32,

    /// Filter 2 resonance for this sample, on the 0..=1 user scale.
    pub filter2_resonance: f32,

    /// Per-main-oscillator levels for this sample.
    pub osc_main_levels: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator level for this sample.
    pub sub_level: f32,

    /// Per-main-oscillator detune in cents for this sample.
    pub osc_main_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator pan positions for this sample.
    pub osc_main_pans: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator pan position for this sample.
    pub sub_pan: f32,

    /// Per-main-oscillator unison voice counts. Carried as f32 to
    /// match the parameter bus; the voice rounds and clamps when
    /// consuming. Stepped, so this field's value is constant across
    /// the samples between two `ParameterChange` events.
    pub osc_main_unison_voices: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison detune width in cents for this
    /// sample.
    pub osc_main_unison_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison stereo spread (0..=1) for this
    /// sample.
    pub osc_main_unison_spreads: [f32; MAIN_OSCILLATOR_COUNT],

    /// Pitch-bend offset in semitones for this sample. Added to the
    /// held MIDI note and any per-osc detune in the voice's frequency
    /// calculation.
    pub pitch_bend_semis: f32,

    /// Master output volume for this sample, 0..=1. Applied after
    /// polyphony summing in the engine — the voice does not see it.
    pub master_volume: f32,
}
