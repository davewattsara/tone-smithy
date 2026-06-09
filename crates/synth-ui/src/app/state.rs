use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::Receiver;

use notify::RecommendedWatcher;
use synth_engine::ParamSnapshot;
use synth_engine::param_bus::{EngineEventSender, SnapshotSlot, load_snapshot};
use synth_engine::{FilterMode, Waveform};
use synth_presets::{
    AppSettings, MidiLearnEntry, PresetEntry, factory_entries, scan_dir, start_watcher, user_presets_dir,
};

use crate::computer_keyboard::ComputerKeyboard;
use crate::keyboard::VirtualKeyboard;
use crate::sections::browser::LoadAction;

// ── Device change request (read by AppShell each frame) ──────────────────────

/// Sent from the Settings tab to the composition root (`AppShell`) to request
/// a live audio or MIDI device switch.
#[derive(Debug, Clone)]
pub enum DeviceChange {
    /// Switch to a named audio output device (`None` = OS default).
    Audio(Option<String>),
    /// Switch to a named MIDI input port (`None` = first available).
    Midi(Option<String>),
}

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
    Presets,
    Settings,
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
        (Tab::Presets, "Presets"),
        (Tab::Settings, "Settings"),
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

    // ── Preset browser (M12) ─────────────────────────────────────────────────
    /// All visible presets (factory + user), refreshed on file-system changes.
    pub(crate) preset_entries: Vec<PresetEntry>,
    /// Current search query (empty = no filter).
    pub(crate) preset_search: String,
    /// Active category filter (empty string = all categories).
    pub(crate) preset_category_filter: String,
    /// Receives a `()` whenever the user preset directory changes on disk.
    pub(crate) file_watch_rx: Receiver<()>,
    /// Keeps the file watcher alive for the app's lifetime.
    _file_watcher: Option<RecommendedWatcher>,
    /// Deferred actions from browser row rendering (load/delete/save-as).
    pub(crate) load_actions: Vec<LoadAction>,

    // ── Settings + MIDI Learn (M13) ──────────────────────────────────────────
    /// Persisted app settings (audio device, MIDI port, etc.).
    pub(crate) settings: AppSettings,
    /// Available audio output device names (cached at startup / on refresh).
    pub(crate) audio_devices: Vec<String>,
    /// Available MIDI input port names (cached at startup / on refresh).
    pub(crate) midi_ports: Vec<String>,
    /// Pending device change to be consumed by `AppShell` after `update()`.
    pending_device_change: Option<DeviceChange>,
    /// Parameter key + range currently waiting for a CC to be bound.
    /// Set when the user clicks "MIDI Learn" in a knob context menu.
    /// Tuple: `(param_key, range_start, range_end)`.
    pub(crate) midi_learn_target: Option<(String, f32, f32)>,
    /// CC values from the previous frame — used to detect incoming CC movement
    /// during MIDI Learn mode.
    pub(crate) prev_cc_values: [f32; 128],
    /// Active MIDI Learn bindings: CC number → parameter key.
    pub(crate) midi_learn_mappings: Vec<MidiLearnEntry>,
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
        settings: AppSettings,
    ) -> Self {
        let snap = load_snapshot(&snapshot_slot);
        let mut app = Self {
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
            preset_entries: Vec::new(),
            preset_search: String::new(),
            preset_category_filter: String::new(),
            file_watch_rx: {
                // Throw-away receiver; the real one is set below.
                std::sync::mpsc::channel::<()>().1
            },
            _file_watcher: None,
            load_actions: Vec::new(),
            settings,
            audio_devices: Vec::new(),
            midi_ports: Vec::new(),
            pending_device_change: None,
            midi_learn_target: None,
            prev_cc_values: [0.0; 128],
            midi_learn_mappings: Vec::new(),
        };
        app.init_browser();
        app.refresh_device_lists();
        app
    }

    /// Refreshes the cached audio device and MIDI port lists.
    pub(crate) fn refresh_device_lists(&mut self) {
        self.audio_devices = synth_host::audio::list_output_devices();
        self.midi_ports = synth_host::midi::list_ports().unwrap_or_default();
    }

    // ── AppShell interface ────────────────────────────────────────────────────

    /// Returns the snapshot slot so `AppShell` can read the current engine state.
    #[must_use]
    pub fn snapshot_slot(&self) -> &SnapshotSlot {
        &self.snapshot_slot
    }

    /// Returns the event sender so `AppShell` can hand it to a reconnected
    /// MIDI stream.
    #[must_use]
    pub fn events_sender(&self) -> &EngineEventSender {
        &self.events
    }

    /// Takes the pending device change (if any) — called by `AppShell` each frame.
    pub fn take_pending_device_change(&mut self) -> Option<DeviceChange> {
        self.pending_device_change.take()
    }

    /// Queues a device change to be applied by `AppShell` after `update()`.
    pub(crate) fn request_device_change(&mut self, change: DeviceChange) {
        self.pending_device_change = Some(change);
    }

    /// Reconnects the UI to a new engine bus after a device switch.
    pub fn reconnect_bus(&mut self, events: EngineEventSender, snapshot_slot: SnapshotSlot) {
        self.events = events;
        self.snapshot_slot = snapshot_slot;
    }

    /// Updates the audio status string (shown in the header bar).
    pub fn set_audio_status(&mut self, status: String) {
        self.audio_status = status;
    }

    /// Reflects a successfully-applied audio device change into the UI's
    /// settings copy. Without this the Settings dropdown keeps showing the
    /// startup device, so a new selection appears to "do nothing".
    pub fn set_audio_device(&mut self, device: Option<String>) {
        self.settings.audio_output_device = device;
    }

    /// Reflects a successfully-applied MIDI port change into the UI's
    /// settings copy, keeping the Settings dropdown in sync.
    pub fn set_midi_port(&mut self, port: Option<String>) {
        self.settings.midi_input_port = port;
    }

    /// Sets a preset error message (shown in the error bar).
    pub fn set_preset_error(&mut self, msg: String) {
        self.preset_error = Some(msg);
    }

    /// Starts the file watcher and populates the initial preset list.
    fn init_browser(&mut self) {
        let user_dir = user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let (watcher, rx) = start_watcher(&user_dir);
        self._file_watcher = watcher;
        self.file_watch_rx = rx;
        self.refresh_preset_list();
    }

    /// Rebuilds `preset_entries` from the embedded factory list + the user
    /// presets directory. Called once at startup and whenever the watcher fires.
    pub(crate) fn refresh_preset_list(&mut self) {
        let user_dir = user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let mut entries = factory_entries();
        entries.extend(scan_dir(&user_dir, false));
        self.preset_entries = entries;
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
