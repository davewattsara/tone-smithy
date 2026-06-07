use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use synth_engine::ParamSnapshot;
use synth_engine::param_bus::{EngineEventSender, SnapshotSlot, load_snapshot};
use synth_engine::{FilterMode, Waveform};

use crate::computer_keyboard::ComputerKeyboard;
use crate::keyboard::VirtualKeyboard;

// ── Constants used by section files ──────────────────────────────────────────

pub(crate) const OSC_LEVEL_MAX: f32 = 1.0;
pub(crate) const OSC_DETUNE_MAX_CENTS: f32 = 100.0;
pub(crate) const UNISON_DETUNE_MAX_CENTS: f32 = 50.0;
pub(crate) const UNISON_VOICES_MAX: f32 = 7.0;
pub(crate) const CUTOFF_MIN_HZ: f32 = 20.0;
pub(crate) const CUTOFF_MAX_HZ: f32 = 20_000.0;
pub(crate) const ENV_MIN_SECS: f32 = 0.001;
pub(crate) const ENV_ATTACK_MAX_SECS: f32 = 10.0;
pub(crate) const ENV_DECAY_MAX_SECS: f32 = 10.0;
pub(crate) const ENV_RELEASE_MAX_SECS: f32 = 10.0;
pub(crate) const PITCH_OFFSET_RANGE: f32 = 24.0;
pub(crate) const LFO_RATE_MIN_HZ: f32 = 0.01;
pub(crate) const LFO_RATE_MAX_HZ: f32 = 20.0;
pub(crate) const ENV2_CURVE_RANGE: f32 = 1.0;
pub(crate) const BPM_MIN: f32 = 20.0;
pub(crate) const BPM_MAX: f32 = 300.0;
pub(crate) const FM_RATIO_FINE_MAX: f32 = 100.0;
pub(crate) const FM_OP_ENV_MIN_SECS: f32 = 0.001;
pub(crate) const FM_OP_ENV_MAX_SECS: f32 = 10.0;

pub(crate) const MOD_SOURCE_LABELS: &[&str] = &[
    "Off", "LFO1", "LFO2", "Env2", "AmpEnv", "Vel", "Key", "ModWhl", "AfterT", "Bend",
];
pub(crate) const MOD_DEST_LABELS: &[&str] = &["Cutoff", "Reso", "Pitch", "Vol", "Osc1Det", "Osc1Pan"];
pub(crate) const MOD_AMOUNT_RANGES: &[f32] = &[
    10_000.0, // FilterCutoffHz
    1.0,      // FilterResonance
    24.0,     // PitchSemis
    1.0,      // Volume
    2400.0,   // Osc1DetuneCents
    1.0,      // Osc1Pan
];

// ── Tab enum ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Osc,
    Filter,
    Envelopes,
    Modulation,
    Arp,
    Fx,
    Master,
}

impl Tab {
    pub(crate) const ALL: &'static [(Tab, &'static str)] = &[
        (Tab::Osc, "Osc"),
        (Tab::Filter, "Filter"),
        (Tab::Envelopes, "Envelopes"),
        (Tab::Modulation, "Modulation"),
        (Tab::Arp, "Arp"),
        (Tab::Fx, "FX"),
        (Tab::Master, "Master"),
    ];
}

// ── Application struct ────────────────────────────────────────────────────────

/// The Tone Smithy application UI.
pub struct ToneSmithyApp {
    pub(crate) audio_status: String,
    pub(crate) events: EngineEventSender,
    pub(crate) snapshot_slot: SnapshotSlot,

    // ── Active tab ───────────────────────────────────────────────────────────
    pub(crate) active_tab: Tab,

    // ── Oscillators (3 main + sub) ───────────────────────────────────────────
    pub(crate) waveform: Waveform,
    /// Level for each main oscillator (index 0-2).
    pub(crate) osc_level: [f32; 3],
    /// Detune in cents for each main oscillator.
    pub(crate) osc_detune_cents: [f32; 3],
    /// Pan for each main oscillator (-1..=1).
    pub(crate) osc_pan: [f32; 3],
    /// Unison voice count for each main oscillator.
    pub(crate) osc_unison_voices: [f32; 3],
    /// Unison detune width in cents for each main oscillator.
    pub(crate) osc_unison_detune_cents: [f32; 3],
    /// Unison stereo spread for each main oscillator.
    pub(crate) osc_unison_spread: [f32; 3],
    /// Sub oscillator level.
    pub(crate) sub_level: f32,
    /// Sub oscillator pan.
    pub(crate) sub_pan: f32,

    // ── Filter ───────────────────────────────────────────────────────────────
    pub(crate) filter_mode: FilterMode,
    pub(crate) filter_cutoff_hz: f32,
    pub(crate) filter_resonance: f32,

    // ── Amp envelope ─────────────────────────────────────────────────────────
    pub(crate) amp_attack_secs: f32,
    pub(crate) amp_decay_secs: f32,
    pub(crate) amp_sustain_level: f32,
    pub(crate) amp_release_secs: f32,

    // ── LFO 1 ────────────────────────────────────────────────────────────────
    pub(crate) lfo1_rate_hz: f32,
    pub(crate) lfo1_shape_index: usize,
    pub(crate) lfo1_reset_on_note_on: bool,
    pub(crate) lfo1_sync_enabled: bool,
    pub(crate) lfo1_sync_division_index: usize,

    // ── LFO 2 ────────────────────────────────────────────────────────────────
    pub(crate) lfo2_rate_hz: f32,
    pub(crate) lfo2_shape_index: usize,
    pub(crate) lfo2_reset_on_note_on: bool,
    pub(crate) lfo2_sync_enabled: bool,
    pub(crate) lfo2_sync_division_index: usize,

    // ── Env2 ─────────────────────────────────────────────────────────────────
    pub(crate) env2_attack_secs: f32,
    pub(crate) env2_decay_secs: f32,
    pub(crate) env2_sustain_level: f32,
    pub(crate) env2_release_secs: f32,
    pub(crate) env2_attack_curve: f32,
    pub(crate) env2_decay_curve: f32,
    pub(crate) env2_release_curve: f32,

    // ── Mod matrix ───────────────────────────────────────────────────────────
    pub(crate) mod_slot_enabled: [bool; 8],
    pub(crate) mod_slot_source: [usize; 8],
    pub(crate) mod_slot_dest: [usize; 8],
    pub(crate) mod_slot_amount: [f32; 8],
    pub(crate) mod_slot_via: [usize; 8],

    // ── FM synthesis ─────────────────────────────────────────────────────────
    pub(crate) slot_mode: [u8; 2],
    pub(crate) slot_level: [f32; 2],
    pub(crate) slot_pan: [f32; 2],
    pub(crate) fm_algorithm: [u8; 2],
    pub(crate) fm_op_ratio_integer: [[u8; 4]; 2],
    pub(crate) fm_op_ratio_fine: [[f32; 4]; 2],
    pub(crate) fm_op_level: [[f32; 4]; 2],
    pub(crate) fm_op_attack_secs: [[f32; 4]; 2],
    pub(crate) fm_op_decay_secs: [[f32; 4]; 2],
    pub(crate) fm_op_sustain_level: [[f32; 4]; 2],
    pub(crate) fm_op_release_secs: [[f32; 4]; 2],
    pub(crate) fm_op_feedback: [[f32; 4]; 2],

    // ── FX chain ─────────────────────────────────────────────────────────────
    pub(crate) fx_eq_enabled: bool,
    pub(crate) fx_eq_low_gain_db: f32,
    pub(crate) fx_eq_low_freq_hz: f32,
    pub(crate) fx_eq_mid_gain_db: f32,
    pub(crate) fx_eq_mid_freq_hz: f32,
    pub(crate) fx_eq_mid_q: f32,
    pub(crate) fx_eq_high_gain_db: f32,
    pub(crate) fx_eq_high_freq_hz: f32,
    pub(crate) fx_drive_enabled: bool,
    pub(crate) fx_drive_drive: f32,
    pub(crate) fx_drive_asymmetry: f32,
    pub(crate) fx_chorus_enabled: bool,
    pub(crate) fx_chorus_rate_hz: f32,
    pub(crate) fx_chorus_depth_ms: f32,
    pub(crate) fx_chorus_mix: f32,
    pub(crate) fx_chorus_spread: f32,
    pub(crate) fx_delay_enabled: bool,
    pub(crate) fx_delay_time_secs: f32,
    pub(crate) fx_delay_feedback: f32,
    pub(crate) fx_delay_mix: f32,
    pub(crate) fx_delay_lowcut_hz: f32,
    pub(crate) fx_delay_ping_pong: bool,
    pub(crate) fx_reverb_enabled: bool,
    pub(crate) fx_reverb_predelay_ms: f32,
    pub(crate) fx_reverb_decay_secs: f32,
    pub(crate) fx_reverb_size: f32,
    pub(crate) fx_reverb_damping: f32,
    pub(crate) fx_reverb_mix: f32,

    // ── Arpeggiator ──────────────────────────────────────────────────────────
    pub(crate) arp_enabled: bool,
    pub(crate) arp_mode: u8,
    pub(crate) arp_octaves: u8,
    pub(crate) arp_rate: u8,
    pub(crate) arp_bpm: f32,
    pub(crate) arp_gate: f32,
    pub(crate) arp_swing: f32,

    // ── Global ───────────────────────────────────────────────────────────────
    pub(crate) pitch_offset_semis: f32,
    pub(crate) master_volume: f32,
    pub(crate) bpm: f32,

    // ── Input ────────────────────────────────────────────────────────────────
    pub(crate) keyboard: VirtualKeyboard,
    pub(crate) computer_keyboard: ComputerKeyboard,
    pub(crate) pitch_bend: f32,
    pub(crate) mod_wheel: f32,
    pub(crate) sustain_held: bool,

    /// CPU load arc from the audio thread (f32 bits stored as u32).
    pub(crate) cpu_load: Arc<AtomicU32>,

    // ── Preset ───────────────────────────────────────────────────────────────
    pub(crate) patch_name: String,
    pub(crate) preset_error: Option<String>,
}

// ── Construction ──────────────────────────────────────────────────────────────

impl ToneSmithyApp {
    /// Creates a new app initialised from the current engine snapshot.
    #[must_use]
    pub fn new(
        audio_status: String,
        events: EngineEventSender,
        snapshot_slot: SnapshotSlot,
        cpu_load: Arc<AtomicU32>,
    ) -> Self {
        let snap = load_snapshot(&snapshot_slot);
        Self {
            audio_status,
            events,
            snapshot_slot,
            active_tab: Tab::default(),
            waveform: snap.waveform,
            osc_level: snap.osc_main_levels,
            osc_detune_cents: snap.osc_main_detune_cents,
            osc_pan: snap.osc_main_pans,
            osc_unison_voices: snap.osc_main_unison_voices,
            osc_unison_detune_cents: snap.osc_main_unison_detune_cents,
            osc_unison_spread: snap.osc_main_unison_spreads,
            sub_level: snap.sub_level,
            sub_pan: snap.sub_pan,
            filter_mode: snap.filter_mode,
            filter_cutoff_hz: snap.filter_cutoff_hz,
            filter_resonance: snap.filter_resonance,
            amp_attack_secs: snap.amp_attack_secs,
            amp_decay_secs: snap.amp_decay_secs,
            amp_sustain_level: snap.amp_sustain_level,
            amp_release_secs: snap.amp_release_secs,
            lfo1_rate_hz: snap.lfo1_rate_hz,
            lfo1_shape_index: snap.lfo1_shape_index,
            lfo1_reset_on_note_on: snap.lfo1_reset_on_note_on,
            lfo1_sync_enabled: snap.lfo1_sync_enabled,
            lfo1_sync_division_index: snap.lfo1_sync_division_index,
            lfo2_rate_hz: snap.lfo2_rate_hz,
            lfo2_shape_index: snap.lfo2_shape_index,
            lfo2_reset_on_note_on: snap.lfo2_reset_on_note_on,
            lfo2_sync_enabled: snap.lfo2_sync_enabled,
            lfo2_sync_division_index: snap.lfo2_sync_division_index,
            env2_attack_secs: snap.env2_attack_secs,
            env2_decay_secs: snap.env2_decay_secs,
            env2_sustain_level: snap.env2_sustain_level,
            env2_release_secs: snap.env2_release_secs,
            env2_attack_curve: snap.env2_attack_curve,
            env2_decay_curve: snap.env2_decay_curve,
            env2_release_curve: snap.env2_release_curve,
            mod_slot_enabled: snap.mod_slot_enabled,
            mod_slot_source: snap.mod_slot_source.map(|v| v as usize),
            mod_slot_dest: snap.mod_slot_dest.map(|v| v as usize),
            mod_slot_amount: snap.mod_slot_amount,
            mod_slot_via: snap.mod_slot_via.map(|v| v as usize),
            slot_mode: snap.slot_mode,
            slot_level: snap.slot_level,
            slot_pan: snap.slot_pan,
            fm_algorithm: snap.fm_algorithm,
            fm_op_ratio_integer: snap.fm_op_ratio_integer,
            fm_op_ratio_fine: snap.fm_op_ratio_fine_cents,
            fm_op_level: snap.fm_op_level,
            fm_op_attack_secs: snap.fm_op_attack_secs,
            fm_op_decay_secs: snap.fm_op_decay_secs,
            fm_op_sustain_level: snap.fm_op_sustain_level,
            fm_op_release_secs: snap.fm_op_release_secs,
            fm_op_feedback: snap.fm_op_feedback,
            fx_eq_enabled: snap.fx_eq_enabled,
            fx_eq_low_gain_db: snap.fx_eq_low_gain_db,
            fx_eq_low_freq_hz: snap.fx_eq_low_freq_hz,
            fx_eq_mid_gain_db: snap.fx_eq_mid_gain_db,
            fx_eq_mid_freq_hz: snap.fx_eq_mid_freq_hz,
            fx_eq_mid_q: snap.fx_eq_mid_q,
            fx_eq_high_gain_db: snap.fx_eq_high_gain_db,
            fx_eq_high_freq_hz: snap.fx_eq_high_freq_hz,
            fx_drive_enabled: snap.fx_drive_enabled,
            fx_drive_drive: snap.fx_drive_drive,
            fx_drive_asymmetry: snap.fx_drive_asymmetry,
            fx_chorus_enabled: snap.fx_chorus_enabled,
            fx_chorus_rate_hz: snap.fx_chorus_rate_hz,
            fx_chorus_depth_ms: snap.fx_chorus_depth_ms,
            fx_chorus_mix: snap.fx_chorus_mix,
            fx_chorus_spread: snap.fx_chorus_spread,
            fx_delay_enabled: snap.fx_delay_enabled,
            fx_delay_time_secs: snap.fx_delay_time_secs,
            fx_delay_feedback: snap.fx_delay_feedback,
            fx_delay_mix: snap.fx_delay_mix,
            fx_delay_lowcut_hz: snap.fx_delay_lowcut_hz,
            fx_delay_ping_pong: snap.fx_delay_ping_pong,
            fx_reverb_enabled: snap.fx_reverb_enabled,
            fx_reverb_predelay_ms: snap.fx_reverb_predelay_ms,
            fx_reverb_decay_secs: snap.fx_reverb_decay_secs,
            fx_reverb_size: snap.fx_reverb_size,
            fx_reverb_damping: snap.fx_reverb_damping,
            fx_reverb_mix: snap.fx_reverb_mix,
            arp_enabled: snap.arp_enabled,
            arp_mode: snap.arp_mode,
            arp_octaves: snap.arp_octaves,
            arp_rate: snap.arp_rate,
            arp_bpm: snap.arp_bpm,
            arp_gate: snap.arp_gate,
            arp_swing: snap.arp_swing,
            pitch_offset_semis: snap.pitch_offset_semis,
            master_volume: snap.master_volume,
            bpm: snap.bpm,
            keyboard: VirtualKeyboard::default(),
            computer_keyboard: ComputerKeyboard::default(),
            pitch_bend: 0.0,
            mod_wheel: snap.mod_wheel,
            sustain_held: false,
            cpu_load,
            patch_name: "Untitled".into(),
            preset_error: None,
        }
    }

    /// Copies all saveable fields from `snap` into the UI's local mirror state.
    pub(crate) fn sync_from_snapshot(&mut self, snap: &ParamSnapshot) {
        self.waveform = snap.waveform;
        self.filter_mode = snap.filter_mode;
        self.osc_level = snap.osc_main_levels;
        self.osc_detune_cents = snap.osc_main_detune_cents;
        self.osc_pan = snap.osc_main_pans;
        self.osc_unison_voices = snap.osc_main_unison_voices;
        self.osc_unison_detune_cents = snap.osc_main_unison_detune_cents;
        self.osc_unison_spread = snap.osc_main_unison_spreads;
        self.sub_level = snap.sub_level;
        self.sub_pan = snap.sub_pan;
        self.filter_cutoff_hz = snap.filter_cutoff_hz;
        self.filter_resonance = snap.filter_resonance;
        self.amp_attack_secs = snap.amp_attack_secs;
        self.amp_decay_secs = snap.amp_decay_secs;
        self.amp_sustain_level = snap.amp_sustain_level;
        self.amp_release_secs = snap.amp_release_secs;
        self.lfo1_rate_hz = snap.lfo1_rate_hz;
        self.lfo1_shape_index = snap.lfo1_shape_index;
        self.lfo1_reset_on_note_on = snap.lfo1_reset_on_note_on;
        self.lfo1_sync_enabled = snap.lfo1_sync_enabled;
        self.lfo1_sync_division_index = snap.lfo1_sync_division_index;
        self.lfo2_rate_hz = snap.lfo2_rate_hz;
        self.lfo2_shape_index = snap.lfo2_shape_index;
        self.lfo2_reset_on_note_on = snap.lfo2_reset_on_note_on;
        self.lfo2_sync_enabled = snap.lfo2_sync_enabled;
        self.lfo2_sync_division_index = snap.lfo2_sync_division_index;
        self.env2_attack_secs = snap.env2_attack_secs;
        self.env2_decay_secs = snap.env2_decay_secs;
        self.env2_sustain_level = snap.env2_sustain_level;
        self.env2_release_secs = snap.env2_release_secs;
        self.env2_attack_curve = snap.env2_attack_curve;
        self.env2_decay_curve = snap.env2_decay_curve;
        self.env2_release_curve = snap.env2_release_curve;
        self.mod_slot_enabled = snap.mod_slot_enabled;
        self.mod_slot_source = snap.mod_slot_source.map(|v| v as usize);
        self.mod_slot_dest = snap.mod_slot_dest.map(|v| v as usize);
        self.mod_slot_amount = snap.mod_slot_amount;
        self.mod_slot_via = snap.mod_slot_via.map(|v| v as usize);
        self.slot_mode = snap.slot_mode;
        self.slot_level = snap.slot_level;
        self.slot_pan = snap.slot_pan;
        self.fm_algorithm = snap.fm_algorithm;
        self.fm_op_ratio_integer = snap.fm_op_ratio_integer;
        self.fm_op_ratio_fine = snap.fm_op_ratio_fine_cents;
        self.fm_op_level = snap.fm_op_level;
        self.fm_op_attack_secs = snap.fm_op_attack_secs;
        self.fm_op_decay_secs = snap.fm_op_decay_secs;
        self.fm_op_sustain_level = snap.fm_op_sustain_level;
        self.fm_op_release_secs = snap.fm_op_release_secs;
        self.fm_op_feedback = snap.fm_op_feedback;
        self.fx_eq_enabled = snap.fx_eq_enabled;
        self.fx_eq_low_gain_db = snap.fx_eq_low_gain_db;
        self.fx_eq_low_freq_hz = snap.fx_eq_low_freq_hz;
        self.fx_eq_mid_gain_db = snap.fx_eq_mid_gain_db;
        self.fx_eq_mid_freq_hz = snap.fx_eq_mid_freq_hz;
        self.fx_eq_mid_q = snap.fx_eq_mid_q;
        self.fx_eq_high_gain_db = snap.fx_eq_high_gain_db;
        self.fx_eq_high_freq_hz = snap.fx_eq_high_freq_hz;
        self.fx_drive_enabled = snap.fx_drive_enabled;
        self.fx_drive_drive = snap.fx_drive_drive;
        self.fx_drive_asymmetry = snap.fx_drive_asymmetry;
        self.fx_chorus_enabled = snap.fx_chorus_enabled;
        self.fx_chorus_rate_hz = snap.fx_chorus_rate_hz;
        self.fx_chorus_depth_ms = snap.fx_chorus_depth_ms;
        self.fx_chorus_mix = snap.fx_chorus_mix;
        self.fx_chorus_spread = snap.fx_chorus_spread;
        self.fx_delay_enabled = snap.fx_delay_enabled;
        self.fx_delay_time_secs = snap.fx_delay_time_secs;
        self.fx_delay_feedback = snap.fx_delay_feedback;
        self.fx_delay_mix = snap.fx_delay_mix;
        self.fx_delay_lowcut_hz = snap.fx_delay_lowcut_hz;
        self.fx_delay_ping_pong = snap.fx_delay_ping_pong;
        self.fx_reverb_enabled = snap.fx_reverb_enabled;
        self.fx_reverb_predelay_ms = snap.fx_reverb_predelay_ms;
        self.fx_reverb_decay_secs = snap.fx_reverb_decay_secs;
        self.fx_reverb_size = snap.fx_reverb_size;
        self.fx_reverb_damping = snap.fx_reverb_damping;
        self.fx_reverb_mix = snap.fx_reverb_mix;
        self.arp_enabled = snap.arp_enabled;
        self.arp_mode = snap.arp_mode;
        self.arp_octaves = snap.arp_octaves;
        self.arp_rate = snap.arp_rate;
        self.arp_bpm = snap.arp_bpm;
        self.arp_gate = snap.arp_gate;
        self.arp_swing = snap.arp_swing;
        self.pitch_offset_semis = snap.pitch_offset_semis;
        self.master_volume = snap.master_volume;
        self.bpm = snap.bpm;
    }
}
