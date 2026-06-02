//! The engine's parameter tree and outward-facing snapshot.
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
use crate::filter::FilterMode;
use crate::fm::OPERATOR_COUNT;
use crate::lfo::SyncDivision;
use crate::mod_matrix::{ModDest, ModSource};
use crate::oscillator::Waveform;
use crate::slot::SlotMode;
use crate::smoothing::SmoothedParam;

/// Default amp envelope attack time, in seconds.
const DEFAULT_AMP_ATTACK_SECS: f32 = 0.010;

/// Default amp envelope decay time, in seconds.
const DEFAULT_AMP_DECAY_SECS: f32 = 0.200;

/// Default amp envelope sustain level, on the 0..=1 scale.
const DEFAULT_AMP_SUSTAIN_LEVEL: f32 = 0.8;

/// Default amp envelope release time, in seconds.
const DEFAULT_AMP_RELEASE_SECS: f32 = 0.200;

/// Default master output volume, on the 0..=1 scale. Leaves headroom
/// for the polyphony summing that accumulates before M8's limiter.
const DEFAULT_MASTER_VOLUME: f32 = 0.8;

/// Default filter cutoff frequency, in Hz. Sits well above the
/// fundamental of every playable MIDI note, so a fresh patch is
/// effectively wide open until the user turns the knob down.
const DEFAULT_FILTER_CUTOFF_HZ: f32 = 8_000.0;

/// Default filter resonance, on the 0..=1 user-facing scale. Zero is
/// the maximally damped end of the range — no peak at all.
const DEFAULT_FILTER_RESONANCE: f32 = 0.0;

/// Default per-oscillator level. All four oscillators arrive at unity;
/// the slot mixer's headroom scale handles the worst-case in-phase
/// sum without clipping.
const DEFAULT_OSC_LEVEL: f32 = 1.0;

/// Default per-oscillator detune, in cents. Zero detune = exactly on
/// pitch with the held note.
const DEFAULT_OSC_DETUNE_CENTS: f32 = 0.0;

/// Default per-oscillator pan position. Centered.
const DEFAULT_OSC_PAN: f32 = 0.0;

/// Default unison voice count per main oscillator. `1` means unison
/// is effectively off — the bank behaves like a single oscillator
/// and unison detune / spread are inert.
const DEFAULT_UNISON_VOICES: f32 = 1.0;

/// Default unison detune in cents. Subtle enough not to be obvious if
/// the user enables unison without touching the detune knob, but
/// large enough to actually beat audibly between voices.
const DEFAULT_UNISON_DETUNE_CENTS: f32 = 10.0;

/// Default unison stereo spread (0..=1). Half spread is musical when
/// the user first enables unison without dialling in the width
/// explicitly.
const DEFAULT_UNISON_SPREAD: f32 = 0.5;

/// Identifies a continuous parameter for [`EngineEvent::ParameterChange`].
///
/// Discrete parameters (e.g. waveform, filter mode) have their own
/// typed `EngineEvent` variants so the value type is checked at
/// compile time rather than reinterpreted from `f32`.
///
/// Ids are stable: once shipped in a preset, a variant's discriminant
/// and meaning do not change. New parameters get new variants
/// appended.
///
/// [`EngineEvent::ParameterChange`]: crate::EngineEvent::ParameterChange
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamId {
    /// Pitch offset applied on top of held MIDI note, in semitones.
    /// Range -24..=24 by convention; the engine does not clamp.
    PitchOffsetSemis,

    /// Amp envelope release time, in seconds. Range 0.001..=10.0 by
    /// convention; the envelope clamps below one sample period.
    AmpReleaseSecs,

    /// Filter cutoff frequency, in Hz. Range 20..=~Nyquist; the SVF
    /// clamps internally.
    FilterCutoffHz,

    /// Filter resonance on a 0..=1 scale, mapped internally to a
    /// musically useful Q range. Values outside 0..=1 are clamped.
    FilterResonance,

    /// Main oscillator 1 level (0..=1).
    Osc1Level,
    /// Main oscillator 2 level (0..=1).
    Osc2Level,
    /// Main oscillator 3 level (0..=1).
    Osc3Level,
    /// Sub oscillator level (0..=1).
    SubLevel,

    /// Main oscillator 1 detune, in cents (±100 = one semitone).
    Osc1DetuneCents,
    /// Main oscillator 2 detune, in cents.
    Osc2DetuneCents,
    /// Main oscillator 3 detune, in cents.
    Osc3DetuneCents,

    /// Main oscillator 1 pan position (-1 = full left, +1 = full
    /// right). Equal-power.
    Osc1Pan,
    /// Main oscillator 2 pan position.
    Osc2Pan,
    /// Main oscillator 3 pan position.
    Osc3Pan,
    /// Sub oscillator pan position.
    SubPan,

    /// Main oscillator 1 unison voice count, treated as an integer
    /// 1..=MAX_UNISON_VOICES (rounded and clamped when consumed).
    Osc1UnisonVoices,
    /// Main oscillator 2 unison voice count.
    Osc2UnisonVoices,
    /// Main oscillator 3 unison voice count.
    Osc3UnisonVoices,

    /// Main oscillator 1 unison detune width, in cents. Voices spread
    /// across `[-detune, +detune]`.
    Osc1UnisonDetuneCents,
    /// Main oscillator 2 unison detune width, in cents.
    Osc2UnisonDetuneCents,
    /// Main oscillator 3 unison detune width, in cents.
    Osc3UnisonDetuneCents,

    /// Main oscillator 1 unison stereo spread (0..=1). Voices spread
    /// across the stereo field around the per-osc pan.
    Osc1UnisonSpread,
    /// Main oscillator 2 unison stereo spread (0..=1).
    Osc2UnisonSpread,
    /// Main oscillator 3 unison stereo spread (0..=1).
    Osc3UnisonSpread,

    /// Amp envelope attack time, in seconds. Range 0.001..=10.0 by
    /// convention; the envelope clamps below one sample period.
    AmpAttackSecs,

    /// Amp envelope decay time, in seconds. Same range as attack.
    AmpDecaySecs,

    /// Amp envelope sustain level, 0..=1.
    AmpSustainLevel,

    /// Master output volume, 0..=1. Smoothed to prevent clicks when
    /// the user moves the knob. Applied after polyphony summing.
    MasterVolume,

    /// Pitch-bend wheel position converted to semitones. The engine
    /// scales the normalised -1..1 wheel value by
    /// [`crate::engine::PITCH_BEND_RANGE_SEMIS`] before writing here.
    PitchBendSemis,

    /// Mod wheel (MIDI CC #1), normalised 0..=1. Not yet wired to a
    /// destination; stored so M6 can route it without an API change.
    ModWheel,

    /// Channel aftertouch, normalised 0..=1. Same M6 rationale as
    /// `ModWheel`.
    ChannelAftertouch,

    // ── LFO 1 ──────────────────────────────────────────────────────────
    /// LFO1 rate in Hz when sync is off. Range 0.01..=20.0. Stepped.
    Lfo1RateHz,
    /// LFO1 waveform shape; value is the zero-based `LfoShape` index.
    Lfo1Shape,
    /// LFO1 phase-reset on note-on; 0.0 = off, 1.0 = on. Stepped.
    Lfo1ResetOnNoteOn,
    /// LFO1 BPM-sync enable; 0.0 = free, 1.0 = synced. Stepped.
    Lfo1SyncEnabled,
    /// LFO1 BPM-sync division; value is the zero-based `SyncDivision`
    /// index. Only used when sync is enabled.
    Lfo1SyncDivision,

    // ── LFO 2 ──────────────────────────────────────────────────────────
    /// LFO2 rate in Hz when sync is off.
    Lfo2RateHz,
    /// LFO2 waveform shape index.
    Lfo2Shape,
    /// LFO2 phase-reset on note-on.
    Lfo2ResetOnNoteOn,
    /// LFO2 BPM-sync enable.
    Lfo2SyncEnabled,
    /// LFO2 BPM-sync division index.
    Lfo2SyncDivision,

    // ── Env2 (modulation envelope) ─────────────────────────────────────
    /// Env2 attack time, in seconds.
    Env2AttackSecs,
    /// Env2 decay time, in seconds.
    Env2DecaySecs,
    /// Env2 sustain level, 0..=1.
    Env2SustainLevel,
    /// Env2 release time, in seconds.
    Env2ReleaseSecs,
    /// Env2 Attack stage curve, -1..=1.
    Env2AttackCurve,
    /// Env2 Decay stage curve, -1..=1.
    Env2DecayCurve,
    /// Env2 Release stage curve, -1..=1.
    Env2ReleaseCurve,

    // ── Global ─────────────────────────────────────────────────────────
    /// Global tempo in BPM. Used for BPM-sync LFO rate computation.
    /// Range 20..=300. Stepped.
    Bpm,

    // ── Mod matrix (8 slots, indexed 0..=7) ────────────────────────────
    /// Enable flag for slot `i`. 0.0 = off, 1.0 = on.
    ModSlotEnabled(u8),
    /// Source index for slot `i`. Cast to [`ModSource`] via
    /// [`ModSource::from_index`].
    ModSlotSource(u8),
    /// Destination index for slot `i`. Cast to [`ModDest`] via
    /// [`ModDest::from_index`].
    ModSlotDest(u8),
    /// Signed amount for slot `i`, in destination-natural units.
    ModSlotAmount(u8),
    /// Via-source index for slot `i`. `ModSource::Off` (index 0) means
    /// no via scaling.
    ModSlotVia(u8),

    // ── FM synthesis (M7.3) ────────────────────────────────────────────────
    /// Slot synthesis mode. Slot index 0..=1; value 0.0 = Subtractive,
    /// 1.0 = FM.
    SlotMode(u8),
    /// Per-slot mix level, 0..=1. Slot index 0..=1.
    SlotLevel(u8),
    /// Per-slot mix pan, -1..=1. Slot index 0..=1.
    SlotPan(u8),
    /// FM algorithm for a slot. Slot index 0..=1; value 0.0..=7.0.
    FmAlgorithm(u8),
    /// FM operator integer ratio. Packed `(slot << 4) | op`. Value 0.0..=15.0.
    FmOpRatioInteger(u8),
    /// FM operator fine ratio in cents. Packed `(slot << 4) | op`. Value -100.0..=100.0.
    FmOpRatioFine(u8),
    /// FM operator output level, 0..=1. Packed `(slot << 4) | op`.
    FmOpLevel(u8),
    /// FM operator envelope attack, seconds. Packed `(slot << 4) | op`.
    FmOpAttackSecs(u8),
    /// FM operator envelope decay, seconds. Packed `(slot << 4) | op`.
    FmOpDecaySecs(u8),
    /// FM operator envelope sustain level, 0..=1. Packed `(slot << 4) | op`.
    FmOpSustainLevel(u8),
    /// FM operator envelope release, seconds. Packed `(slot << 4) | op`.
    FmOpReleaseSecs(u8),
    /// FM operator self-feedback, -1..=1. Packed `(slot << 4) | op`.
    /// Only meaningful for op 3 in the 8 starter algorithms.
    FmOpFeedback(u8),

    // ── FX chain (M8) ─────────────────────────────────────────────────────
    /// EQ stage enabled; 0.0 = off, 1.0 = on.
    FxEqEnabled,
    /// EQ low-shelf gain, -15..=15 dB.
    FxEqLowGainDb,
    /// EQ low-shelf frequency, 20..=2000 Hz.
    FxEqLowFreqHz,
    /// EQ mid-peak gain, -15..=15 dB.
    FxEqMidGainDb,
    /// EQ mid-peak frequency, 200..=8000 Hz.
    FxEqMidFreqHz,
    /// EQ mid-peak Q, 0.1..=10.
    FxEqMidQ,
    /// EQ high-shelf gain, -15..=15 dB.
    FxEqHighGainDb,
    /// EQ high-shelf frequency, 2000..=20000 Hz.
    FxEqHighFreqHz,
    /// Drive stage enabled; 0.0 = off, 1.0 = on.
    FxDriveEnabled,
    /// Drive pre-clip gain, 1..=20.
    FxDriveDrive,
    /// Drive asymmetry, -1..=1.
    FxDriveAsymmetry,
    /// Chorus stage enabled; 0.0 = off, 1.0 = on.
    FxChorusEnabled,
    /// Chorus LFO rate, 0.1..=8 Hz.
    FxChorusRateHz,
    /// Chorus modulation depth, 0..=15 ms.
    FxChorusDepthMs,
    /// Chorus dry/wet mix, 0..=1.
    FxChorusMix,
    /// Chorus stereo spread, 0..=1.
    FxChorusSpread,
    /// Delay stage enabled; 0.0 = off, 1.0 = on.
    FxDelayEnabled,
    /// Delay time in seconds, 0.001..=2.0.
    FxDelayTimeSecs,
    /// Delay feedback, 0..=0.95.
    FxDelayFeedback,
    /// Delay dry/wet mix, 0..=1.
    FxDelayMix,
    /// Delay feedback low-cut frequency, 20..=2000 Hz.
    FxDelayLowcutHz,
    /// Delay ping-pong mode; 0.0 = off, 1.0 = on.
    FxDelayPingPong,
    /// Reverb stage enabled; 0.0 = off, 1.0 = on.
    FxReverbEnabled,
    /// Reverb pre-delay, 0..=50 ms.
    FxReverbPredelayMs,
    /// Reverb decay time, 0.1..=30 s.
    FxReverbDecaySecs,
    /// Reverb room size, 0.1..=1.0.
    FxReverbSize,
    /// Reverb HF damping, 0..=1.
    FxReverbDamping,
    /// Reverb dry/wet mix, 0..=1.
    FxReverbMix,

    // ── Arpeggiator ────────────────────────────────────────────────────────
    /// Arp on/off, 0.0 = off, 1.0 = on.
    ArpEnabled,
    /// Arp mode: 0=Up 1=Down 2=UpDown 3=Random 4=Played.
    ArpMode,
    /// Octave range, 1–4.
    ArpOctaves,
    /// Step rate: 0=1/32 1=1/16 2=1/8 3=1/4 4=1/2.
    ArpRate,
    /// Internal BPM, 20–300.
    ArpBpm,
    /// Gate fraction of step duration, 0.01–1.0.
    ArpGate,
    /// Swing fraction, 0.5–0.75.
    ArpSwing,
}

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

    // ── Mod matrix mirrors ─────────────────────────────────────────────
    /// Enable flag for each of the 8 mod slots.
    pub mod_slot_enabled: [bool; 8],
    /// Source index for each slot (matches `ModSource::to_index`).
    pub mod_slot_source: [u8; 8],
    /// Destination index for each slot (matches `ModDest::to_index`).
    pub mod_slot_dest: [u8; 8],
    /// Amount for each slot, in destination-natural units.
    pub mod_slot_amount: [f32; 8],
    /// Via-source index for each slot (0 = Off = no scaling).
    pub mod_slot_via: [u8; 8],

    // ── FM synthesis mirrors ───────────────────────────────────────────
    /// Slot mode per slot: 0 = Subtractive, 1 = FM.
    pub slot_mode: [u8; 2],
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
    pub arp_bpm: f32,
    pub arp_gate: f32,
    pub arp_swing: f32,
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
            lfo2_rate_hz: 1.0,
            lfo2_shape_index: 0,
            lfo2_reset_on_note_on: false,
            lfo2_sync_enabled: false,
            lfo2_sync_division_index: 5,
            env2_attack_secs: 0.010,
            env2_decay_secs: 0.200,
            env2_sustain_level: 0.8,
            env2_release_secs: 0.200,
            env2_attack_curve: 0.0,
            env2_decay_curve: 0.0,
            env2_release_curve: 0.0,
            bpm: 120.0,
            lfo1_out: 0.0,
            lfo2_out: 0.0,
            env2_out: 0.0,
            mod_slot_enabled: [false; 8],
            mod_slot_source: [0; 8],
            mod_slot_dest: [0; 8],
            mod_slot_amount: [0.0; 8],
            mod_slot_via: [0; 8],
            slot_mode: [0; 2],
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
            arp_bpm: 120.0,
            arp_gate: 0.5,
            arp_swing: 0.5,
        }
    }
}

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
    pitch_offset_semis: SmoothedParam,
    filter_cutoff_hz: SmoothedParam,
    filter_resonance: SmoothedParam,

    osc_main_levels: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    sub_level: SmoothedParam,

    osc_main_detune_cents: [SmoothedParam; MAIN_OSCILLATOR_COUNT],

    osc_main_pans: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    sub_pan: SmoothedParam,

    osc_main_unison_detune_cents: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    osc_main_unison_spreads: [SmoothedParam; MAIN_OSCILLATOR_COUNT],

    /// Unison voice counts. Stepped (not smoothed) because the
    /// quantised integer values do not benefit from interpolation —
    /// switching from 3 to 4 voices is meant to be instant, not a
    /// glide through 3.4 voices.
    osc_main_unison_voices: [f32; MAIN_OSCILLATOR_COUNT],

    /// Stepped (not smoothed): sampled by the voice at the next
    /// envelope phase transition, not per-sample.
    amp_attack_secs: f32,
    amp_decay_secs: f32,
    amp_sustain_level: f32,
    amp_release_secs: f32,

    /// Smoothed so moving the volume knob is click-free.
    master_volume: SmoothedParam,

    waveform: Waveform,
    filter_mode: FilterMode,

    active_voice_count: u8,

    pitch_bend_semis: SmoothedParam,
    /// Mod wheel and aftertouch are stepped (plain f32) at M3.3 because
    /// they have no per-sample audio consumer until M6 routes them
    /// through the mod matrix. Smoothing can be added then.
    mod_wheel: f32,
    channel_aftertouch: f32,
    /// CC values for all 128 controllers, normalised 0..=1. Stepped.
    cc_values: [f32; 128],

    // ── LFO 1 ──────────────────────────────────────────────────────────
    lfo1_rate_hz: f32,
    lfo1_shape_index: usize,
    lfo1_reset_on_note_on: bool,
    lfo1_sync_enabled: bool,
    lfo1_sync_division: SyncDivision,

    // ── LFO 2 ──────────────────────────────────────────────────────────
    lfo2_rate_hz: f32,
    lfo2_shape_index: usize,
    lfo2_reset_on_note_on: bool,
    lfo2_sync_enabled: bool,
    lfo2_sync_division: SyncDivision,

    // ── Env2 ───────────────────────────────────────────────────────────
    env2_attack_secs: f32,
    env2_decay_secs: f32,
    env2_sustain_level: f32,
    env2_release_secs: f32,
    env2_attack_curve: f32,
    env2_decay_curve: f32,
    env2_release_curve: f32,

    // ── Global ─────────────────────────────────────────────────────────
    bpm: f32,

    // ── Live modulator outputs (written by engine each block) ──────────
    lfo1_out: f32,
    lfo2_out: f32,
    env2_out: f32,

    // ── Mod matrix mirrors ─────────────────────────────────────────────
    mod_slot_enabled: [bool; 8],
    mod_slot_source: [u8; 8],
    mod_slot_dest: [u8; 8],
    mod_slot_amount: [f32; 8],
    mod_slot_via: [u8; 8],

    // ── FM synthesis ───────────────────────────────────────────────────
    slot_mode: [SlotMode; 2],
    slot_level: [f32; 2],
    slot_pan: [f32; 2],
    fm_algorithm: [u8; 2],
    fm_op_ratio_integer: [[u8; OPERATOR_COUNT]; 2],
    fm_op_ratio_fine_cents: [[f32; OPERATOR_COUNT]; 2],
    fm_op_level: [[f32; OPERATOR_COUNT]; 2],
    fm_op_attack_secs: [[f32; OPERATOR_COUNT]; 2],
    fm_op_decay_secs: [[f32; OPERATOR_COUNT]; 2],
    fm_op_sustain_level: [[f32; OPERATOR_COUNT]; 2],
    fm_op_release_secs: [[f32; OPERATOR_COUNT]; 2],
    fm_op_feedback: [[f32; OPERATOR_COUNT]; 2],

    // ── FX chain (M8) ─────────────────────────────────────────────────────
    fx_eq_enabled: bool,
    fx_eq_low_gain_db: f32,
    fx_eq_low_freq_hz: f32,
    fx_eq_mid_gain_db: f32,
    fx_eq_mid_freq_hz: f32,
    fx_eq_mid_q: f32,
    fx_eq_high_gain_db: f32,
    fx_eq_high_freq_hz: f32,
    fx_drive_enabled: bool,
    fx_drive_drive: f32,
    fx_drive_asymmetry: f32,
    fx_chorus_enabled: bool,
    fx_chorus_rate_hz: f32,
    fx_chorus_depth_ms: f32,
    fx_chorus_mix: f32,
    fx_chorus_spread: f32,
    fx_delay_enabled: bool,
    fx_delay_time_secs: f32,
    fx_delay_feedback: f32,
    fx_delay_mix: f32,
    fx_delay_lowcut_hz: f32,
    fx_delay_ping_pong: bool,
    fx_reverb_enabled: bool,
    fx_reverb_predelay_ms: f32,
    fx_reverb_decay_secs: f32,
    fx_reverb_size: f32,
    fx_reverb_damping: f32,
    fx_reverb_mix: f32,
    // ── Arpeggiator ──────────────────────────────────────────────────────
    arp_enabled: bool,
    arp_mode: u8,
    arp_octaves: u8,
    arp_rate: u8,
    arp_bpm: f32,
    arp_gate: f32,
    arp_swing: f32,
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
            lfo2_rate_hz: defaults.lfo2_rate_hz,
            lfo2_shape_index: defaults.lfo2_shape_index,
            lfo2_reset_on_note_on: defaults.lfo2_reset_on_note_on,
            lfo2_sync_enabled: defaults.lfo2_sync_enabled,
            lfo2_sync_division: SyncDivision::from_index(defaults.lfo2_sync_division_index),
            env2_attack_secs: defaults.env2_attack_secs,
            env2_decay_secs: defaults.env2_decay_secs,
            env2_sustain_level: defaults.env2_sustain_level,
            env2_release_secs: defaults.env2_release_secs,
            env2_attack_curve: defaults.env2_attack_curve,
            env2_decay_curve: defaults.env2_decay_curve,
            env2_release_curve: defaults.env2_release_curve,
            bpm: defaults.bpm,
            lfo1_out: 0.0,
            lfo2_out: 0.0,
            env2_out: 0.0,
            mod_slot_enabled: [false; 8],
            mod_slot_source: [0; 8],
            mod_slot_dest: [0; 8],
            mod_slot_amount: [0.0; 8],
            mod_slot_via: [0; 8],
            slot_mode: [SlotMode::Subtractive; 2],
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
            arp_bpm: defaults.arp_bpm,
            arp_gate: defaults.arp_gate,
            arp_swing: defaults.arp_swing,
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
            ParamId::Lfo2RateHz => self.lfo2_rate_hz = value,
            ParamId::Lfo2Shape => self.lfo2_shape_index = value as usize,
            ParamId::Lfo2ResetOnNoteOn => self.lfo2_reset_on_note_on = value >= 0.5,
            ParamId::Lfo2SyncEnabled => self.lfo2_sync_enabled = value >= 0.5,
            ParamId::Lfo2SyncDivision => {
                self.lfo2_sync_division = SyncDivision::from_index(value as usize);
            }
            ParamId::Env2AttackSecs => self.env2_attack_secs = value,
            ParamId::Env2DecaySecs => self.env2_decay_secs = value,
            ParamId::Env2SustainLevel => self.env2_sustain_level = value,
            ParamId::Env2ReleaseSecs => self.env2_release_secs = value,
            ParamId::Env2AttackCurve => self.env2_attack_curve = value,
            ParamId::Env2DecayCurve => self.env2_decay_curve = value,
            ParamId::Env2ReleaseCurve => self.env2_release_curve = value,
            ParamId::Bpm => self.bpm = value,
            ParamId::ModSlotEnabled(i) => {
                if (i as usize) < 8 {
                    self.mod_slot_enabled[i as usize] = value >= 0.5;
                }
            }
            ParamId::ModSlotSource(i) => {
                if (i as usize) < 8 {
                    self.mod_slot_source[i as usize] =
                        ModSource::from_index(value as u8).unwrap_or_default().to_index();
                }
            }
            ParamId::ModSlotDest(i) => {
                if (i as usize) < 8 {
                    self.mod_slot_dest[i as usize] = ModDest::from_index(value as u8).unwrap_or_default().to_index();
                }
            }
            ParamId::ModSlotAmount(i) => {
                if (i as usize) < 8 {
                    self.mod_slot_amount[i as usize] = value;
                }
            }
            ParamId::ModSlotVia(i) => {
                if (i as usize) < 8 {
                    self.mod_slot_via[i as usize] = ModSource::from_index(value as u8).unwrap_or_default().to_index();
                }
            }
            ParamId::SlotMode(i) => {
                if (i as usize) < 2 {
                    self.slot_mode[i as usize] = if value >= 0.5 {
                        SlotMode::Fm
                    } else {
                        SlotMode::Subtractive
                    };
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
            ParamId::ArpBpm => self.arp_bpm = value.clamp(20.0, 300.0),
            ParamId::ArpGate => self.arp_gate = value.clamp(0.01, 1.0),
            ParamId::ArpSwing => self.arp_swing = value.clamp(0.5, 0.75),
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

    /// Stores the live modulator outputs from the first active voice.
    /// Called by the engine each block, before the snapshot is published.
    pub fn set_modulator_outputs(&mut self, lfo1: f32, lfo2: f32, env2: f32) {
        self.lfo1_out = lfo1;
        self.lfo2_out = lfo2;
        self.env2_out = env2;
    }

    /// Advances all smoothed parameters by one sample and returns the
    /// values the audio path consumes per sample. Called once per
    /// frame inside `Engine::process_stereo`.
    pub fn next_sample(&mut self) -> SampleParams {
        SampleParams {
            pitch_offset_semis: self.pitch_offset_semis.next_sample(),
            filter_cutoff_hz: self.filter_cutoff_hz.next_sample(),
            filter_resonance: self.filter_resonance.next_sample(),
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
            lfo2_rate_hz: self.lfo2_rate_hz,
            lfo2_shape_index: self.lfo2_shape_index,
            lfo2_reset_on_note_on: self.lfo2_reset_on_note_on,
            lfo2_sync_enabled: self.lfo2_sync_enabled,
            lfo2_sync_division_index: self.lfo2_sync_division.index(),
            env2_attack_secs: self.env2_attack_secs,
            env2_decay_secs: self.env2_decay_secs,
            env2_sustain_level: self.env2_sustain_level,
            env2_release_secs: self.env2_release_secs,
            env2_attack_curve: self.env2_attack_curve,
            env2_decay_curve: self.env2_decay_curve,
            env2_release_curve: self.env2_release_curve,
            bpm: self.bpm,
            lfo1_out: self.lfo1_out,
            lfo2_out: self.lfo2_out,
            env2_out: self.env2_out,
            mod_slot_enabled: self.mod_slot_enabled,
            mod_slot_source: self.mod_slot_source,
            mod_slot_dest: self.mod_slot_dest,
            mod_slot_amount: self.mod_slot_amount,
            mod_slot_via: self.mod_slot_via,
            slot_mode: self.slot_mode.map(|m| if m == SlotMode::Fm { 1 } else { 0 }),
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
            arp_bpm: self.arp_bpm,
            arp_gate: self.arp_gate,
            arp_swing: self.arp_swing,
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
fn sync_rate_hz(bpm: f32, division: SyncDivision) -> f32 {
    bpm / 60.0 / (4.0 * division.multiplier_bars())
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
