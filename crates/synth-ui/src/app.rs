//! Top-level [`eframe::App`] implementation.
//!
//! M4: Three panels (Osc 1, Filter, Amp Envelope) plus master volume and a
//! status footer. All continuous parameters use the custom [`Knob`] widget.
//! Discrete parameters (waveform, filter mode) keep their button rows.
//!
//! M5: LFO1, LFO2, and Env2 panels added below the original three, plus a
//! BPM knob in the master volume row. Live readouts show current modulator
//! output from the first active voice.
//!
//! M6: Mod matrix table with 8 rows (source / dest / amount / via).
//!
//! [`Knob`]: crate::knob::Knob

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use eframe::egui;
use synth_engine::param_bus::{EngineEventSender, SnapshotSlot, load_snapshot};
use synth_engine::{EngineEvent, FilterMode, ParamId, ParamSnapshot, Waveform};
use synth_presets::{Preset, map_to_events, map_to_snapshot, snapshot_to_map};

use crate::computer_keyboard::ComputerKeyboard;
use crate::keyboard::VirtualKeyboard;
use crate::knob::Knob;

// ── Oscillator 1 range constants ────────────────────────────────────────────

const OSC_LEVEL_MAX: f32 = 1.0;
const OSC_DETUNE_MAX_CENTS: f32 = 100.0;
const UNISON_DETUNE_MAX_CENTS: f32 = 50.0;
const UNISON_VOICES_MAX: f32 = 7.0;

// ── Filter range constants ───────────────────────────────────────────────────

const CUTOFF_MIN_HZ: f32 = 20.0;
const CUTOFF_MAX_HZ: f32 = 20_000.0;

// ── Amp envelope range constants ─────────────────────────────────────────────

const ENV_MIN_SECS: f32 = 0.001;
const ENV_ATTACK_MAX_SECS: f32 = 10.0;
const ENV_DECAY_MAX_SECS: f32 = 10.0;
const ENV_RELEASE_MAX_SECS: f32 = 10.0;

// ── Pitch offset ─────────────────────────────────────────────────────────────

const PITCH_OFFSET_RANGE: f32 = 24.0;

// ── LFO range constants ───────────────────────────────────────────────────────

const LFO_RATE_MIN_HZ: f32 = 0.01;
const LFO_RATE_MAX_HZ: f32 = 20.0;

// ── Env2 range constants ──────────────────────────────────────────────────────

const ENV2_CURVE_RANGE: f32 = 1.0;

// ── BPM range constants ───────────────────────────────────────────────────────

const BPM_MIN: f32 = 20.0;
const BPM_MAX: f32 = 300.0;

// ── FM synthesis constants ────────────────────────────────────────────────────

const FM_RATIO_FINE_MAX: f32 = 100.0;
const FM_OP_ENV_MIN_SECS: f32 = 0.001;
const FM_OP_ENV_MAX_SECS: f32 = 10.0;

// ── Mod matrix constants ──────────────────────────────────────────────────────

const MOD_SOURCE_LABELS: &[&str] = &[
    "Off", "LFO1", "LFO2", "Env2", "AmpEnv", "Vel", "Key", "ModWhl", "AfterT", "Bend",
];
const MOD_DEST_LABELS: &[&str] = &["Cutoff", "Reso", "Pitch", "Vol", "Osc1Det", "Osc1Pan"];
/// Amount knob range per destination index (max absolute value).
const MOD_AMOUNT_RANGES: &[f32] = &[
    10_000.0, // FilterCutoffHz
    1.0,      // FilterResonance
    24.0,     // PitchSemis
    1.0,      // Volume
    2400.0,   // Osc1DetuneCents
    1.0,      // Osc1Pan
];

/// The Tone Smithy application UI.
pub struct ToneSmithyApp {
    audio_status: String,
    events: EngineEventSender,
    snapshot_slot: SnapshotSlot,

    // ── Osc 1 mirrors ────────────────────────────────────────────────────────
    osc1_level: f32,
    osc1_detune_cents: f32,
    osc1_pan: f32,
    waveform: Waveform,
    osc1_unison_voices: f32,
    osc1_unison_detune_cents: f32,
    osc1_unison_spread: f32,

    // ── Filter mirrors ───────────────────────────────────────────────────────
    filter_mode: FilterMode,
    filter_cutoff_hz: f32,
    filter_resonance: f32,

    // ── Amp envelope mirrors ─────────────────────────────────────────────────
    amp_attack_secs: f32,
    amp_decay_secs: f32,
    amp_sustain_level: f32,
    amp_release_secs: f32,

    // ── LFO 1 mirrors ────────────────────────────────────────────────────────
    lfo1_rate_hz: f32,
    lfo1_shape_index: usize,
    lfo1_reset_on_note_on: bool,
    lfo1_sync_enabled: bool,
    lfo1_sync_division_index: usize,

    // ── LFO 2 mirrors ────────────────────────────────────────────────────────
    lfo2_rate_hz: f32,
    lfo2_shape_index: usize,
    lfo2_reset_on_note_on: bool,
    lfo2_sync_enabled: bool,
    lfo2_sync_division_index: usize,

    // ── Env2 mirrors ─────────────────────────────────────────────────────────
    env2_attack_secs: f32,
    env2_decay_secs: f32,
    env2_sustain_level: f32,
    env2_release_secs: f32,
    env2_attack_curve: f32,
    env2_decay_curve: f32,
    env2_release_curve: f32,

    // ── Mod matrix mirrors ───────────────────────────────────────────────────
    mod_slot_enabled: [bool; 8],
    mod_slot_source: [usize; 8],
    mod_slot_dest: [usize; 8],
    mod_slot_amount: [f32; 8],
    mod_slot_via: [usize; 8],

    // ── FM synthesis mirrors ─────────────────────────────────────────────────
    slot_mode: [u8; 2],
    slot_level: [f32; 2],
    slot_pan: [f32; 2],
    fm_algorithm: [u8; 2],
    fm_op_ratio_integer: [[u8; 4]; 2],
    fm_op_ratio_fine: [[f32; 4]; 2],
    fm_op_level: [[f32; 4]; 2],
    fm_op_attack_secs: [[f32; 4]; 2],
    fm_op_decay_secs: [[f32; 4]; 2],
    fm_op_sustain_level: [[f32; 4]; 2],
    fm_op_release_secs: [[f32; 4]; 2],
    fm_op_feedback: [[f32; 4]; 2],

    // ── FX chain mirrors ─────────────────────────────────────────────────────
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

    // ── Arpeggiator ──────────────────────────────────────────────────────────
    arp_enabled: bool,
    arp_mode: u8,
    arp_octaves: u8,
    arp_rate: u8,
    arp_bpm: f32,
    arp_gate: f32,
    arp_swing: f32,

    // ── Global ───────────────────────────────────────────────────────────────
    pitch_offset_semis: f32,
    master_volume: f32,
    bpm: f32,

    // ── Input ────────────────────────────────────────────────────────────────
    keyboard: VirtualKeyboard,
    computer_keyboard: ComputerKeyboard,

    /// Pitch-bend wheel position, -1.0..=1.0. Snaps back to 0.0 when
    /// the user releases the slider.
    pitch_bend: f32,
    /// Mod wheel position, 0.0..=1.0. Stays where left (no spring-back).
    mod_wheel: f32,
    /// True while the on-screen sustain pedal button is toggled on.
    sustain_held: bool,

    /// CPU load arc from the audio thread (f32 bits stored as u32).
    cpu_load: Arc<AtomicU32>,

    // ── Preset ───────────────────────────────────────────────────────────────
    /// Current patch name shown in the header bar.
    patch_name: String,
    /// Last preset error string, shown to the user until dismissed.
    preset_error: Option<String>,
}

impl ToneSmithyApp {
    /// Creates a new app. `cpu_load` is the arc written by the audio callback.
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
            osc1_level: snap.osc_main_levels[0],
            osc1_detune_cents: snap.osc_main_detune_cents[0],
            osc1_pan: snap.osc_main_pans[0],
            waveform: snap.waveform,
            osc1_unison_voices: snap.osc_main_unison_voices[0],
            osc1_unison_detune_cents: snap.osc_main_unison_detune_cents[0],
            osc1_unison_spread: snap.osc_main_unison_spreads[0],
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
    /// Call this immediately after pushing a batch of preset load events so the
    /// controls show the new values before the engine snapshot catches up.
    fn sync_from_snapshot(&mut self, snap: &ParamSnapshot) {
        self.waveform = snap.waveform;
        self.filter_mode = snap.filter_mode;
        self.filter_cutoff_hz = snap.filter_cutoff_hz;
        self.filter_resonance = snap.filter_resonance;
        self.osc1_level = snap.osc_main_levels[0];
        self.osc1_detune_cents = snap.osc_main_detune_cents[0];
        self.osc1_pan = snap.osc_main_pans[0];
        self.osc1_unison_voices = snap.osc_main_unison_voices[0];
        self.osc1_unison_detune_cents = snap.osc_main_unison_detune_cents[0];
        self.osc1_unison_spread = snap.osc_main_unison_spreads[0];
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

    /// Saves the current patch state to a file chosen via a native dialog.
    fn save_preset(&mut self) {
        let snapshot = load_snapshot(&self.snapshot_slot);
        let mut preset = Preset::new(self.patch_name.clone());
        preset.parameters = snapshot_to_map(&snapshot);

        let default_filename = format!("{}.tsmith", self.patch_name);
        let start_dir = synth_presets::user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

        let result = rfd::FileDialog::new()
            .set_title("Save Preset")
            .set_file_name(&default_filename)
            .add_filter("Tone Smithy Preset", &["tsmith"])
            .set_directory(&start_dir)
            .save_file();

        if let Some(path) = result {
            if let Err(e) = synth_presets::save(&path, &preset) {
                self.preset_error = Some(format!("Save failed: {e}"));
            } else {
                self.preset_error = None;
            }
        }
    }

    /// Loads a preset from a file chosen via a native dialog.
    fn load_preset(&mut self) {
        let start_dir = synth_presets::user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

        let result = rfd::FileDialog::new()
            .set_title("Load Preset")
            .add_filter("Tone Smithy Preset", &["tsmith"])
            .set_directory(&start_dir)
            .pick_file();

        if let Some(path) = result {
            match synth_presets::load(&path) {
                Ok(preset) => {
                    // Push all param events to the engine
                    for event in map_to_events(&preset.parameters) {
                        self.events.send(event);
                    }
                    // Immediately sync UI local fields from preset data
                    let snap = map_to_snapshot(&preset.parameters);
                    self.sync_from_snapshot(&snap);
                    self.patch_name = preset.metadata.name.clone();
                    self.preset_error = None;
                }
                Err(e) => {
                    self.preset_error = Some(format!("Load failed: {e}"));
                }
            }
        }
    }
}

impl eframe::App for ToneSmithyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.computer_keyboard.handle_input(ctx, &self.events);
        let snapshot = load_snapshot(&self.snapshot_slot);

        // Footer (bottom panel, must be added before central panel).
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            self.footer_bar(ui, &snapshot);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Title + preset bar
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.heading("Tone Smithy");
                    ui.separator();
                    ui.label("Patch:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.patch_name)
                            .desired_width(160.0)
                            .hint_text("Untitled"),
                    );
                    if ui.button("Save").clicked() {
                        self.save_preset();
                    }
                    if ui.button("Load").clicked() {
                        self.load_preset();
                    }
                    ui.separator();
                    ui.label(&self.audio_status);
                });
                if let Some(ref err) = self.preset_error.clone() {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::RED, err);
                        if ui.small_button("x").clicked() {
                            self.preset_error = None;
                        }
                    });
                }
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(8.0);

                // Three synthesis panels side by side
                ui.columns(3, |cols| {
                    self.osc1_panel(&mut cols[0]);
                    self.filter_panel(&mut cols[1]);
                    self.amp_env_panel(&mut cols[2]);
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // LFO and Env2 panels
                ui.columns(3, |cols| {
                    self.lfo_panel(&mut cols[0], 1, &snapshot);
                    self.lfo_panel(&mut cols[1], 2, &snapshot);
                    self.env2_panel(&mut cols[2], &snapshot);
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // FM synthesis panel (slots + operators)
                self.fm_panel(ui);

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Mod matrix
                self.mod_matrix_panel(ui);

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Arpeggiator
                self.arp_panel(ui);

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // FX chain
                self.fx_panel(ui);

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Master volume + pitch offset + BPM row
                ui.horizontal(|ui| {
                    ui.label("Master");
                    if ui
                        .add(
                            Knob::new(&mut self.master_volume, 0.0..=1.0, "Volume")
                                .default_value(0.8)
                                .format(|v| format!("{:.0}%", v * 100.0)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::MasterVolume,
                            value: self.master_volume,
                        });
                    }

                    ui.add_space(16.0);
                    ui.label("Pitch");
                    if ui
                        .add(
                            Knob::new(
                                &mut self.pitch_offset_semis,
                                -PITCH_OFFSET_RANGE..=PITCH_OFFSET_RANGE,
                                "Offset",
                            )
                            .default_value(0.0)
                            .format(|v| format!("{:+.2} st", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::PitchOffsetSemis,
                            value: self.pitch_offset_semis,
                        });
                    }

                    ui.add_space(16.0);
                    ui.label("BPM");
                    if ui
                        .add(
                            Knob::new(&mut self.bpm, BPM_MIN..=BPM_MAX, "BPM")
                                .default_value(120.0)
                                .format(|v| format!("{:.1}", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::Bpm,
                            value: self.bpm,
                        });
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Computer keyboard hint
                ui.label(format!(
                    "Keyboard: A S D F G H J (white) / W E T Y U (black). Z/X shift octave. Octave base: MIDI {} ({}).",
                    self.computer_keyboard.octave_base(),
                    midi_note_label(self.computer_keyboard.octave_base()),
                ));
                ui.add_space(6.0);

                // Keep the virtual keyboard's visible range in sync with the
                // computer keyboard's current octave so highlighted keys are
                // always visible. If a mouse-held note was active when the
                // range shifted, send NoteOff so the engine releases it.
                if let Some(stuck) = self.keyboard.set_start_note(self.computer_keyboard.octave_base()) {
                    self.events.send(EngineEvent::NoteOff { note_midi: stuck });
                }
                let kb_notes = self.computer_keyboard.held_notes();

                ui.horizontal(|ui| {
                    // Pitch-bend strip: vertical slider that springs to 0 on release.
                    ui.vertical(|ui| {
                        ui.label("PB");
                        let pb_r = ui.add(
                            egui::Slider::new(&mut self.pitch_bend, -1.0..=1.0)
                                .vertical()
                                .show_value(false),
                        );
                        if pb_r.changed() {
                            self.events.send(EngineEvent::PitchBend {
                                value_normalised: self.pitch_bend,
                            });
                        }
                        // Spring back to centre the moment the mouse button is released,
                        // whether the interaction was a drag or a click.
                        if !pb_r.is_pointer_button_down_on() && self.pitch_bend != 0.0 {
                            self.pitch_bend = 0.0;
                            self.events.send(EngineEvent::PitchBend { value_normalised: 0.0 });
                        }
                    });

                    // Mod wheel strip: vertical slider, stays where left.
                    ui.vertical(|ui| {
                        ui.label("MW");
                        let mw_r = ui.add(
                            egui::Slider::new(&mut self.mod_wheel, 0.0..=1.0)
                                .vertical()
                                .show_value(false),
                        );
                        if mw_r.changed() {
                            self.events.send(EngineEvent::ControlChange {
                                cc: 1,
                                value_normalised: self.mod_wheel,
                            });
                        }
                    });

                    // Virtual keyboard.
                    self.keyboard.show(ui, &self.events, kb_notes);

                    // Sustain pedal toggle.
                    ui.vertical(|ui| {
                        ui.label("Sustain");
                        if ui
                            .selectable_label(self.sustain_held, if self.sustain_held { "ON " } else { "OFF" })
                            .clicked()
                        {
                            self.sustain_held = !self.sustain_held;
                            self.events.send(EngineEvent::Sustain {
                                held: self.sustain_held,
                            });
                        }
                    });
                });
            }); // ScrollArea
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

impl ToneSmithyApp {
    fn osc1_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Osc 1");
        ui.add_space(6.0);

        // Waveform selector
        ui.label("Waveform");
        ui.horizontal_wrapped(|ui| {
            let mut changed = false;
            for w in [Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle] {
                let label = match w {
                    Waveform::Sine => "Sine",
                    Waveform::Saw => "Saw",
                    Waveform::Square => "Sq",
                    Waveform::Triangle => "Tri",
                };
                if ui.selectable_value(&mut self.waveform, w, label).clicked() {
                    changed = true;
                }
            }
            if changed {
                self.events.send(EngineEvent::SetOscillatorWaveform {
                    waveform: self.waveform,
                });
            }
        });

        ui.add_space(8.0);

        // Level / detune / pan knobs
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.osc1_level, 0.0..=OSC_LEVEL_MAX, "Level")
                        .default_value(1.0)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1Level,
                    value: self.osc1_level,
                });
            }
            if ui
                .add(
                    Knob::new(
                        &mut self.osc1_detune_cents,
                        -OSC_DETUNE_MAX_CENTS..=OSC_DETUNE_MAX_CENTS,
                        "Detune",
                    )
                    .default_value(0.0)
                    .format(|v| format!("{:+.1} ct", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1DetuneCents,
                    value: self.osc1_detune_cents,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.osc1_pan, -1.0..=1.0, "Pan")
                        .default_value(0.0)
                        .format(|v| {
                            if v < -0.01 {
                                format!("L{:.0}", v.abs() * 100.0)
                            } else if v > 0.01 {
                                format!("R{:.0}", v * 100.0)
                            } else {
                                "C".to_string()
                            }
                        }),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1Pan,
                    value: self.osc1_pan,
                });
            }
        });

        ui.add_space(8.0);
        ui.label("Unison");
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.osc1_unison_voices, 1.0..=UNISON_VOICES_MAX, "Voices")
                        .default_value(1.0)
                        .format(|v| format!("{}", v.round() as u8)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1UnisonVoices,
                    value: self.osc1_unison_voices,
                });
            }
            if ui
                .add(
                    Knob::new(
                        &mut self.osc1_unison_detune_cents,
                        0.0..=UNISON_DETUNE_MAX_CENTS,
                        "Detune",
                    )
                    .default_value(10.0)
                    .format(|v| format!("{:.1} ct", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1UnisonDetuneCents,
                    value: self.osc1_unison_detune_cents,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.osc1_unison_spread, 0.0..=1.0, "Spread")
                        .default_value(0.5)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1UnisonSpread,
                    value: self.osc1_unison_spread,
                });
            }
        });
    }

    fn filter_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Filter");
        ui.add_space(6.0);

        // Mode selector
        ui.label("Mode");
        ui.horizontal_wrapped(|ui| {
            let mut changed = false;
            for m in [
                FilterMode::LowPass,
                FilterMode::HighPass,
                FilterMode::BandPass,
                FilterMode::Notch,
            ] {
                let label = match m {
                    FilterMode::LowPass => "LP",
                    FilterMode::HighPass => "HP",
                    FilterMode::BandPass => "BP",
                    FilterMode::Notch => "Notch",
                };
                if ui.selectable_value(&mut self.filter_mode, m, label).clicked() {
                    changed = true;
                }
            }
            if changed {
                self.events.send(EngineEvent::SetFilterMode { mode: self.filter_mode });
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.filter_cutoff_hz, CUTOFF_MIN_HZ..=CUTOFF_MAX_HZ, "Cutoff")
                        .default_value(8_000.0)
                        .format(|v| {
                            if v >= 1_000.0 {
                                format!("{:.1} kHz", v / 1000.0)
                            } else {
                                format!("{:.0} Hz", v)
                            }
                        }),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FilterCutoffHz,
                    value: self.filter_cutoff_hz,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.filter_resonance, 0.0..=1.0, "Res")
                        .default_value(0.0)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FilterResonance,
                    value: self.filter_resonance,
                });
            }
        });
    }

    fn amp_env_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Amp Env");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.amp_attack_secs, ENV_MIN_SECS..=ENV_ATTACK_MAX_SECS, "A")
                        .default_value(0.010)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpAttackSecs,
                    value: self.amp_attack_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_decay_secs, ENV_MIN_SECS..=ENV_DECAY_MAX_SECS, "D")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpDecaySecs,
                    value: self.amp_decay_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_sustain_level, 0.0..=1.0, "S")
                        .default_value(0.8)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpSustainLevel,
                    value: self.amp_sustain_level,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_release_secs, ENV_MIN_SECS..=ENV_RELEASE_MAX_SECS, "R")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpReleaseSecs,
                    value: self.amp_release_secs,
                });
            }
        });
    }

    fn lfo_panel(&mut self, ui: &mut egui::Ui, lfo_num: u8, snapshot: &synth_engine::ParamSnapshot) {
        // Extract local copies so closures below don't conditionally borrow self.
        let (rate_id, shape_id, reset_id, sync_id, div_id) = if lfo_num == 1 {
            (
                ParamId::Lfo1RateHz,
                ParamId::Lfo1Shape,
                ParamId::Lfo1ResetOnNoteOn,
                ParamId::Lfo1SyncEnabled,
                ParamId::Lfo1SyncDivision,
            )
        } else {
            (
                ParamId::Lfo2RateHz,
                ParamId::Lfo2Shape,
                ParamId::Lfo2ResetOnNoteOn,
                ParamId::Lfo2SyncEnabled,
                ParamId::Lfo2SyncDivision,
            )
        };
        let mut rate_hz = if lfo_num == 1 {
            self.lfo1_rate_hz
        } else {
            self.lfo2_rate_hz
        };
        let mut shape_index = if lfo_num == 1 {
            self.lfo1_shape_index
        } else {
            self.lfo2_shape_index
        };
        let mut reset_on_note_on = if lfo_num == 1 {
            self.lfo1_reset_on_note_on
        } else {
            self.lfo2_reset_on_note_on
        };
        let mut sync_enabled = if lfo_num == 1 {
            self.lfo1_sync_enabled
        } else {
            self.lfo2_sync_enabled
        };
        let mut div_index = if lfo_num == 1 {
            self.lfo1_sync_division_index
        } else {
            self.lfo2_sync_division_index
        };
        let live_out = if lfo_num == 1 {
            snapshot.lfo1_out
        } else {
            snapshot.lfo2_out
        };
        let events = self.events.clone();

        ui.heading(if lfo_num == 1 { "LFO 1" } else { "LFO 2" });
        ui.add_space(6.0);

        // Shape selector — 7 shapes, index matches LfoShape::index().
        const SHAPE_LABELS: [&str; 7] = ["Sin", "Tri", "Saw+", "Saw-", "Sq", "S&H", "Rnd"];
        ui.label("Shape");
        ui.horizontal_wrapped(|ui| {
            for (i, label) in SHAPE_LABELS.iter().enumerate() {
                if ui.selectable_label(shape_index == i, *label).clicked() {
                    shape_index = i;
                    events.send(EngineEvent::ParameterChange {
                        id: shape_id,
                        value: i as f32,
                    });
                }
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if !sync_enabled
                && ui
                    .add(
                        Knob::new(&mut rate_hz, LFO_RATE_MIN_HZ..=LFO_RATE_MAX_HZ, "Rate")
                            .default_value(1.0)
                            .format(|v| format!("{:.2} Hz", v)),
                    )
                    .changed()
            {
                events.send(EngineEvent::ParameterChange {
                    id: rate_id,
                    value: rate_hz,
                });
            }

            if ui.selectable_label(reset_on_note_on, "Reset").clicked() {
                reset_on_note_on = !reset_on_note_on;
                events.send(EngineEvent::ParameterChange {
                    id: reset_id,
                    value: if reset_on_note_on { 1.0 } else { 0.0 },
                });
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.selectable_label(sync_enabled, "Sync").clicked() {
                sync_enabled = !sync_enabled;
                events.send(EngineEvent::ParameterChange {
                    id: sync_id,
                    value: if sync_enabled { 1.0 } else { 0.0 },
                });
            }

            if sync_enabled {
                const DIV_LABELS: [&str; 8] = ["1/32", "1/16", "1/8", "1/4", "1/2", "1", "2", "4"];
                for (i, label) in DIV_LABELS.iter().enumerate() {
                    if ui.selectable_label(div_index == i, *label).clicked() {
                        div_index = i;
                        events.send(EngineEvent::ParameterChange {
                            id: div_id,
                            value: i as f32,
                        });
                    }
                }
            }
        });

        ui.add_space(4.0);
        ui.label(format!("Out: {:.3}", live_out));

        // Write back modified locals.
        if lfo_num == 1 {
            self.lfo1_rate_hz = rate_hz;
            self.lfo1_shape_index = shape_index;
            self.lfo1_reset_on_note_on = reset_on_note_on;
            self.lfo1_sync_enabled = sync_enabled;
            self.lfo1_sync_division_index = div_index;
        } else {
            self.lfo2_rate_hz = rate_hz;
            self.lfo2_shape_index = shape_index;
            self.lfo2_reset_on_note_on = reset_on_note_on;
            self.lfo2_sync_enabled = sync_enabled;
            self.lfo2_sync_division_index = div_index;
        }
    }

    fn env2_panel(&mut self, ui: &mut egui::Ui, snapshot: &synth_engine::ParamSnapshot) {
        ui.heading("Env2");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.env2_attack_secs, ENV_MIN_SECS..=ENV_ATTACK_MAX_SECS, "A")
                        .default_value(0.010)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2AttackSecs,
                    value: self.env2_attack_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_decay_secs, ENV_MIN_SECS..=ENV_DECAY_MAX_SECS, "D")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2DecaySecs,
                    value: self.env2_decay_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_sustain_level, 0.0..=1.0, "S")
                        .default_value(0.8)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2SustainLevel,
                    value: self.env2_sustain_level,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_release_secs, ENV_MIN_SECS..=ENV_RELEASE_MAX_SECS, "R")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2ReleaseSecs,
                    value: self.env2_release_secs,
                });
            }
        });

        ui.add_space(4.0);
        ui.label("Curve");
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.env2_attack_curve, -ENV2_CURVE_RANGE..=ENV2_CURVE_RANGE, "A")
                        .default_value(0.0)
                        .format(|v| format!("{:+.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2AttackCurve,
                    value: self.env2_attack_curve,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_decay_curve, -ENV2_CURVE_RANGE..=ENV2_CURVE_RANGE, "D")
                        .default_value(0.0)
                        .format(|v| format!("{:+.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2DecayCurve,
                    value: self.env2_decay_curve,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_release_curve, -ENV2_CURVE_RANGE..=ENV2_CURVE_RANGE, "R")
                        .default_value(0.0)
                        .format(|v| format!("{:+.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2ReleaseCurve,
                    value: self.env2_release_curve,
                });
            }
        });

        ui.add_space(4.0);
        ui.label(format!("Out: {:.3}", snapshot.env2_out));
    }

    fn mod_matrix_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mod Matrix");
        ui.add_space(4.0);

        egui::Grid::new("mod_matrix").min_col_width(70.0).show(ui, |ui| {
            // Header row — same grid as data rows so columns align.
            ui.label("");
            ui.label("Source");
            ui.label("Dest");
            ui.label("Amount");
            ui.label("Via");
            ui.end_row();

            for i in 0..8usize {
                // Enable toggle
                let mut enabled = self.mod_slot_enabled[i];
                if ui.checkbox(&mut enabled, format!("Slot {}", i + 1)).changed() {
                    self.mod_slot_enabled[i] = enabled;
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ModSlotEnabled(i as u8),
                        value: if enabled { 1.0 } else { 0.0 },
                    });
                }

                // Source combo
                let src_label = MOD_SOURCE_LABELS.get(self.mod_slot_source[i]).copied().unwrap_or("?");
                egui::ComboBox::from_id_salt(format!("mod_src_{i}"))
                    .selected_text(src_label)
                    .show_ui(ui, |ui| {
                        for (idx, &label) in MOD_SOURCE_LABELS.iter().enumerate() {
                            if ui.selectable_value(&mut self.mod_slot_source[i], idx, label).changed() {
                                self.events.send(EngineEvent::ParameterChange {
                                    id: ParamId::ModSlotSource(i as u8),
                                    value: idx as f32,
                                });
                            }
                        }
                    });

                // Dest combo — changing dest resets amount to 0 so the new
                // destination's range is clean (stale amounts from a different
                // dest would be out-of-range and lock the knob at its maximum).
                let dest_label = MOD_DEST_LABELS.get(self.mod_slot_dest[i]).copied().unwrap_or("?");
                egui::ComboBox::from_id_salt(format!("mod_dst_{i}"))
                    .selected_text(dest_label)
                    .show_ui(ui, |ui| {
                        for (idx, &label) in MOD_DEST_LABELS.iter().enumerate() {
                            if ui.selectable_value(&mut self.mod_slot_dest[i], idx, label).changed() {
                                self.events.send(EngineEvent::ParameterChange {
                                    id: ParamId::ModSlotDest(i as u8),
                                    value: idx as f32,
                                });
                                self.mod_slot_amount[i] = 0.0;
                                self.events.send(EngineEvent::ParameterChange {
                                    id: ParamId::ModSlotAmount(i as u8),
                                    value: 0.0,
                                });
                            }
                        }
                    });

                // Amount — DragValue so the number is always visible.
                // Drag speed is range/100 so a 100 px drag covers the full range.
                let range = MOD_AMOUNT_RANGES.get(self.mod_slot_dest[i]).copied().unwrap_or(1.0);
                if ui
                    .add(
                        egui::DragValue::new(&mut self.mod_slot_amount[i])
                            .range(-range..=range)
                            .speed(range / 100.0),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ModSlotAmount(i as u8),
                        value: self.mod_slot_amount[i],
                    });
                }

                // Via combo
                let via_label = MOD_SOURCE_LABELS.get(self.mod_slot_via[i]).copied().unwrap_or("?");
                egui::ComboBox::from_id_salt(format!("mod_via_{i}"))
                    .selected_text(via_label)
                    .show_ui(ui, |ui| {
                        for (idx, &label) in MOD_SOURCE_LABELS.iter().enumerate() {
                            if ui.selectable_value(&mut self.mod_slot_via[i], idx, label).changed() {
                                self.events.send(EngineEvent::ParameterChange {
                                    id: ParamId::ModSlotVia(i as u8),
                                    value: idx as f32,
                                });
                            }
                        }
                    });

                ui.end_row();
            }
        });
    }

    fn fm_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("FM Synthesis");
        ui.add_space(4.0);

        for slot_idx in 0..2usize {
            let mode_tag = if self.slot_mode[slot_idx] == 0 {
                "Subtractive"
            } else {
                "FM"
            };
            let slot_label = format!("Slot {} ({})", slot_idx, mode_tag);
            egui::CollapsingHeader::new(slot_label)
                .id_salt(slot_idx)
                .show(ui, |ui| {
                    // Mode toggle
                    ui.horizontal(|ui| {
                        ui.label("Mode:");
                        let is_sub = self.slot_mode[slot_idx] == 0;
                        let is_fm = self.slot_mode[slot_idx] == 1;
                        if ui.selectable_label(is_sub, "Subtractive").clicked() && !is_sub {
                            self.slot_mode[slot_idx] = 0;
                            self.events.send(EngineEvent::ParameterChange {
                                id: ParamId::SlotMode(slot_idx as u8),
                                value: 0.0,
                            });
                        }
                        if ui.selectable_label(is_fm, "FM").clicked() && !is_fm {
                            self.slot_mode[slot_idx] = 1;
                            self.events.send(EngineEvent::ParameterChange {
                                id: ParamId::SlotMode(slot_idx as u8),
                                value: 1.0,
                            });
                        }
                    });

                    // Level and Pan
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                Knob::new(&mut self.slot_level[slot_idx], 0.0..=1.0, "Level")
                                    .default_value(if slot_idx == 0 { 1.0 } else { 0.0 })
                                    .format(|v| format!("{:.2}", v)),
                            )
                            .changed()
                        {
                            self.events.send(EngineEvent::ParameterChange {
                                id: ParamId::SlotLevel(slot_idx as u8),
                                value: self.slot_level[slot_idx],
                            });
                        }
                        if ui
                            .add(
                                Knob::new(&mut self.slot_pan[slot_idx], -1.0..=1.0, "Pan")
                                    .default_value(0.0)
                                    .format(|v| {
                                        if v < -0.01 {
                                            format!("L{:.0}", v.abs() * 100.0)
                                        } else if v > 0.01 {
                                            format!("R{:.0}", v * 100.0)
                                        } else {
                                            "C".to_string()
                                        }
                                    }),
                            )
                            .changed()
                        {
                            self.events.send(EngineEvent::ParameterChange {
                                id: ParamId::SlotPan(slot_idx as u8),
                                value: self.slot_pan[slot_idx],
                            });
                        }
                    });

                    // FM-only controls
                    if self.slot_mode[slot_idx] == 1 {
                        ui.add_space(4.0);

                        // Algorithm picker
                        ui.horizontal(|ui| {
                            ui.label("Algorithm:");
                            let alg = self.fm_algorithm[slot_idx];
                            const ALG_LABELS: [&str; 8] = [
                                "1 Stack",
                                "2 Stack+FB",
                                "3 Two stacks",
                                "4 Para mod",
                                "5 Branch",
                                "6 Mixed",
                                "7 Additive",
                                "8 Paired",
                            ];
                            egui::ComboBox::from_id_salt(format!("fm_alg_{slot_idx}"))
                                .selected_text(ALG_LABELS[alg as usize])
                                .show_ui(ui, |ui| {
                                    for (idx, &label) in ALG_LABELS.iter().enumerate() {
                                        if ui
                                            .selectable_value(&mut self.fm_algorithm[slot_idx], idx as u8, label)
                                            .changed()
                                        {
                                            self.events.send(EngineEvent::ParameterChange {
                                                id: ParamId::FmAlgorithm(slot_idx as u8),
                                                value: idx as f32,
                                            });
                                        }
                                    }
                                });
                        });

                        ui.add_space(4.0);

                        // Operator rows in a grid: Op | Ratio Int | Ratio Fine | Level | A | D | S | R | [Feedback]
                        egui::Grid::new(format!("fm_ops_{slot_idx}"))
                            .striped(true)
                            .spacing([4.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("Op");
                                ui.label("Ratio");
                                ui.label("Fine");
                                ui.label("Level");
                                ui.label("A");
                                ui.label("D");
                                ui.label("S");
                                ui.label("R");
                                ui.label("FB");
                                ui.end_row();

                                for op in 0..4usize {
                                    let packed = ((slot_idx as u8) << 4) | (op as u8);
                                    ui.label(format!("Op {}", op + 1));

                                    // Ratio integer
                                    let mut ratio_int = self.fm_op_ratio_integer[slot_idx][op] as i32;
                                    if ui
                                        .add(egui::DragValue::new(&mut ratio_int).range(1..=15).speed(0.1))
                                        .changed()
                                    {
                                        self.fm_op_ratio_integer[slot_idx][op] = ratio_int as u8;
                                        self.events.send(EngineEvent::ParameterChange {
                                            id: ParamId::FmOpRatioInteger(packed),
                                            value: ratio_int as f32,
                                        });
                                    }

                                    // Ratio fine (cents)
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut self.fm_op_ratio_fine[slot_idx][op])
                                                .range(-FM_RATIO_FINE_MAX..=FM_RATIO_FINE_MAX)
                                                .speed(0.5)
                                                .suffix(" ct"),
                                        )
                                        .changed()
                                    {
                                        self.events.send(EngineEvent::ParameterChange {
                                            id: ParamId::FmOpRatioFine(packed),
                                            value: self.fm_op_ratio_fine[slot_idx][op],
                                        });
                                    }

                                    // Level
                                    if ui
                                        .add(
                                            Knob::new(&mut self.fm_op_level[slot_idx][op], 0.0..=1.0, "Lv")
                                                .default_value(1.0)
                                                .format(|v| format!("{:.2}", v)),
                                        )
                                        .changed()
                                    {
                                        self.events.send(EngineEvent::ParameterChange {
                                            id: ParamId::FmOpLevel(packed),
                                            value: self.fm_op_level[slot_idx][op],
                                        });
                                    }

                                    // Attack
                                    if ui
                                        .add(
                                            Knob::new(
                                                &mut self.fm_op_attack_secs[slot_idx][op],
                                                FM_OP_ENV_MIN_SECS..=FM_OP_ENV_MAX_SECS,
                                                "A",
                                            )
                                            .default_value(0.010)
                                            .format(secs_format),
                                        )
                                        .changed()
                                    {
                                        self.events.send(EngineEvent::ParameterChange {
                                            id: ParamId::FmOpAttackSecs(packed),
                                            value: self.fm_op_attack_secs[slot_idx][op],
                                        });
                                    }

                                    // Decay
                                    if ui
                                        .add(
                                            Knob::new(
                                                &mut self.fm_op_decay_secs[slot_idx][op],
                                                FM_OP_ENV_MIN_SECS..=FM_OP_ENV_MAX_SECS,
                                                "D",
                                            )
                                            .default_value(0.200)
                                            .format(secs_format),
                                        )
                                        .changed()
                                    {
                                        self.events.send(EngineEvent::ParameterChange {
                                            id: ParamId::FmOpDecaySecs(packed),
                                            value: self.fm_op_decay_secs[slot_idx][op],
                                        });
                                    }

                                    // Sustain
                                    if ui
                                        .add(
                                            Knob::new(&mut self.fm_op_sustain_level[slot_idx][op], 0.0..=1.0, "S")
                                                .default_value(0.8)
                                                .format(|v| format!("{:.2}", v)),
                                        )
                                        .changed()
                                    {
                                        self.events.send(EngineEvent::ParameterChange {
                                            id: ParamId::FmOpSustainLevel(packed),
                                            value: self.fm_op_sustain_level[slot_idx][op],
                                        });
                                    }

                                    // Release
                                    if ui
                                        .add(
                                            Knob::new(
                                                &mut self.fm_op_release_secs[slot_idx][op],
                                                FM_OP_ENV_MIN_SECS..=FM_OP_ENV_MAX_SECS,
                                                "R",
                                            )
                                            .default_value(0.200)
                                            .format(secs_format),
                                        )
                                        .changed()
                                    {
                                        self.events.send(EngineEvent::ParameterChange {
                                            id: ParamId::FmOpReleaseSecs(packed),
                                            value: self.fm_op_release_secs[slot_idx][op],
                                        });
                                    }

                                    // Feedback (op 3 only in starter algorithms)
                                    if op == 3 {
                                        if ui
                                            .add(
                                                Knob::new(&mut self.fm_op_feedback[slot_idx][op], -1.0..=1.0, "FB")
                                                    .default_value(0.0)
                                                    .format(|v| format!("{:.2}", v)),
                                            )
                                            .changed()
                                        {
                                            self.events.send(EngineEvent::ParameterChange {
                                                id: ParamId::FmOpFeedback(packed),
                                                value: self.fm_op_feedback[slot_idx][op],
                                            });
                                        }
                                    } else {
                                        ui.label("-");
                                    }

                                    ui.end_row();
                                }
                            });
                    }
                });
        }
    }

    fn arp_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Arpeggiator");
        ui.horizontal(|ui| {
            // Enable toggle
            let prev_enabled = self.arp_enabled;
            ui.checkbox(&mut self.arp_enabled, "On");
            if self.arp_enabled != prev_enabled {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::ArpEnabled,
                    value: if self.arp_enabled { 1.0 } else { 0.0 },
                });
            }

            ui.add_enabled_ui(self.arp_enabled, |ui| {
                // Mode
                let mode_labels = ["Up", "Down", "Up/Dn", "Rand", "Played"];
                egui::ComboBox::from_id_salt("arp_mode")
                    .selected_text(mode_labels[self.arp_mode as usize])
                    .show_ui(ui, |ui| {
                        for (i, label) in mode_labels.iter().enumerate() {
                            if ui.selectable_value(&mut self.arp_mode, i as u8, *label).changed() {
                                self.events.send(EngineEvent::ParameterChange {
                                    id: ParamId::ArpMode,
                                    value: self.arp_mode as f32,
                                });
                            }
                        }
                    });

                // Octaves
                let oct_labels = ["1 oct", "2 oct", "3 oct", "4 oct"];
                egui::ComboBox::from_id_salt("arp_oct")
                    .selected_text(oct_labels[(self.arp_octaves as usize).saturating_sub(1).min(3)])
                    .show_ui(ui, |ui| {
                        for (i, label) in oct_labels.iter().enumerate() {
                            let v = (i + 1) as u8;
                            if ui.selectable_value(&mut self.arp_octaves, v, *label).changed() {
                                self.events.send(EngineEvent::ParameterChange {
                                    id: ParamId::ArpOctaves,
                                    value: self.arp_octaves as f32,
                                });
                            }
                        }
                    });

                // Rate
                let rate_labels = ["1/32", "1/16", "1/8", "1/4", "1/2"];
                egui::ComboBox::from_id_salt("arp_rate")
                    .selected_text(rate_labels[self.arp_rate as usize])
                    .show_ui(ui, |ui| {
                        for (i, label) in rate_labels.iter().enumerate() {
                            if ui.selectable_value(&mut self.arp_rate, i as u8, *label).changed() {
                                self.events.send(EngineEvent::ParameterChange {
                                    id: ParamId::ArpRate,
                                    value: self.arp_rate as f32,
                                });
                            }
                        }
                    });

                // BPM knob
                if ui.add(Knob::new(&mut self.arp_bpm, 20.0..=300.0, "BPM")).changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ArpBpm,
                        value: self.arp_bpm,
                    });
                }

                // Gate knob
                if ui.add(Knob::new(&mut self.arp_gate, 0.01..=1.0, "Gate")).changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ArpGate,
                        value: self.arp_gate,
                    });
                }

                // Swing knob
                if ui.add(Knob::new(&mut self.arp_swing, 0.5..=0.75, "Swing")).changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ArpSwing,
                        value: self.arp_swing,
                    });
                }
            });
        });
    }

    fn fx_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("FX Chain");
        ui.add_space(4.0);

        ui.columns(5, |cols| {
            // ── EQ ─────────────────────────────────────────────────────
            cols[0].vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.fx_eq_enabled, "EQ").changed() {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqEnabled,
                            value: if self.fx_eq_enabled { 1.0 } else { 0.0 },
                        });
                    }
                });
                ui.add_enabled_ui(self.fx_eq_enabled, |ui| {
                    ui.label("Low");
                    if ui
                        .add(
                            Knob::new(&mut self.fx_eq_low_gain_db, -15.0..=15.0, "Gain")
                                .default_value(0.0)
                                .format(|v| format!("{:+.1} dB", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqLowGainDb,
                            value: self.fx_eq_low_gain_db,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_eq_low_freq_hz, 20.0..=2_000.0, "Freq")
                                .default_value(200.0)
                                .format(|v| format!("{:.0} Hz", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqLowFreqHz,
                            value: self.fx_eq_low_freq_hz,
                        });
                    }
                    ui.label("Mid");
                    if ui
                        .add(
                            Knob::new(&mut self.fx_eq_mid_gain_db, -15.0..=15.0, "Gain")
                                .default_value(0.0)
                                .format(|v| format!("{:+.1} dB", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqMidGainDb,
                            value: self.fx_eq_mid_gain_db,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_eq_mid_freq_hz, 200.0..=8_000.0, "Freq")
                                .default_value(1_000.0)
                                .format(|v| format!("{:.0} Hz", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqMidFreqHz,
                            value: self.fx_eq_mid_freq_hz,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_eq_mid_q, 0.1..=10.0, "Q")
                                .default_value(0.7)
                                .format(|v| format!("{:.2}", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqMidQ,
                            value: self.fx_eq_mid_q,
                        });
                    }
                    ui.label("High");
                    if ui
                        .add(
                            Knob::new(&mut self.fx_eq_high_gain_db, -15.0..=15.0, "Gain")
                                .default_value(0.0)
                                .format(|v| format!("{:+.1} dB", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqHighGainDb,
                            value: self.fx_eq_high_gain_db,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_eq_high_freq_hz, 2_000.0..=20_000.0, "Freq")
                                .default_value(6_000.0)
                                .format(|v| format!("{:.0} Hz", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxEqHighFreqHz,
                            value: self.fx_eq_high_freq_hz,
                        });
                    }
                });
            });

            // ── Drive ──────────────────────────────────────────────────
            cols[1].vertical(|ui| {
                if ui.checkbox(&mut self.fx_drive_enabled, "Drive").changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::FxDriveEnabled,
                        value: if self.fx_drive_enabled { 1.0 } else { 0.0 },
                    });
                }
                ui.add_enabled_ui(self.fx_drive_enabled, |ui| {
                    if ui
                        .add(
                            Knob::new(&mut self.fx_drive_drive, 1.0..=20.0, "Drive")
                                .default_value(1.0)
                                .format(|v| format!("{:.1}x", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxDriveDrive,
                            value: self.fx_drive_drive,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_drive_asymmetry, -1.0..=1.0, "Asym")
                                .default_value(0.0)
                                .format(|v| format!("{:+.2}", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxDriveAsymmetry,
                            value: self.fx_drive_asymmetry,
                        });
                    }
                });
            });

            // ── Chorus ─────────────────────────────────────────────────
            cols[2].vertical(|ui| {
                if ui.checkbox(&mut self.fx_chorus_enabled, "Chorus").changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::FxChorusEnabled,
                        value: if self.fx_chorus_enabled { 1.0 } else { 0.0 },
                    });
                }
                ui.add_enabled_ui(self.fx_chorus_enabled, |ui| {
                    if ui
                        .add(
                            Knob::new(&mut self.fx_chorus_rate_hz, 0.1..=8.0, "Rate")
                                .default_value(0.5)
                                .format(|v| format!("{:.2} Hz", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxChorusRateHz,
                            value: self.fx_chorus_rate_hz,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_chorus_depth_ms, 0.0..=15.0, "Depth")
                                .default_value(3.0)
                                .format(|v| format!("{:.1} ms", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxChorusDepthMs,
                            value: self.fx_chorus_depth_ms,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_chorus_mix, 0.0..=1.0, "Mix")
                                .default_value(0.5)
                                .format(|v| format!("{:.0}%", v * 100.0)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxChorusMix,
                            value: self.fx_chorus_mix,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_chorus_spread, 0.0..=1.0, "Spread")
                                .default_value(0.5)
                                .format(|v| format!("{:.0}%", v * 100.0)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxChorusSpread,
                            value: self.fx_chorus_spread,
                        });
                    }
                });
            });

            // ── Delay ──────────────────────────────────────────────────
            cols[3].vertical(|ui| {
                if ui.checkbox(&mut self.fx_delay_enabled, "Delay").changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::FxDelayEnabled,
                        value: if self.fx_delay_enabled { 1.0 } else { 0.0 },
                    });
                }
                ui.add_enabled_ui(self.fx_delay_enabled, |ui| {
                    if ui
                        .add(
                            Knob::new(&mut self.fx_delay_time_secs, 0.001..=2.0, "Time")
                                .default_value(0.375)
                                .format(secs_format),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxDelayTimeSecs,
                            value: self.fx_delay_time_secs,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_delay_feedback, 0.0..=0.95, "Fdbk")
                                .default_value(0.35)
                                .format(|v| format!("{:.0}%", v * 100.0)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxDelayFeedback,
                            value: self.fx_delay_feedback,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_delay_mix, 0.0..=1.0, "Mix")
                                .default_value(0.30)
                                .format(|v| format!("{:.0}%", v * 100.0)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxDelayMix,
                            value: self.fx_delay_mix,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_delay_lowcut_hz, 20.0..=2_000.0, "LoCut")
                                .default_value(200.0)
                                .format(|v| format!("{:.0} Hz", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxDelayLowcutHz,
                            value: self.fx_delay_lowcut_hz,
                        });
                    }
                    if ui.checkbox(&mut self.fx_delay_ping_pong, "Ping-pong").changed() {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxDelayPingPong,
                            value: if self.fx_delay_ping_pong { 1.0 } else { 0.0 },
                        });
                    }
                });
            });

            // ── Reverb ─────────────────────────────────────────────────
            cols[4].vertical(|ui| {
                if ui.checkbox(&mut self.fx_reverb_enabled, "Reverb").changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::FxReverbEnabled,
                        value: if self.fx_reverb_enabled { 1.0 } else { 0.0 },
                    });
                }
                ui.add_enabled_ui(self.fx_reverb_enabled, |ui| {
                    if ui
                        .add(
                            Knob::new(&mut self.fx_reverb_predelay_ms, 0.0..=50.0, "Pre")
                                .default_value(10.0)
                                .format(|v| format!("{:.0} ms", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxReverbPredelayMs,
                            value: self.fx_reverb_predelay_ms,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_reverb_decay_secs, 0.1..=30.0, "Decay")
                                .default_value(2.0)
                                .format(secs_format),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxReverbDecaySecs,
                            value: self.fx_reverb_decay_secs,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_reverb_size, 0.1..=1.0, "Size")
                                .default_value(0.7)
                                .format(|v| format!("{:.2}", v)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxReverbSize,
                            value: self.fx_reverb_size,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_reverb_damping, 0.0..=1.0, "Damp")
                                .default_value(0.5)
                                .format(|v| format!("{:.0}%", v * 100.0)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxReverbDamping,
                            value: self.fx_reverb_damping,
                        });
                    }
                    if ui
                        .add(
                            Knob::new(&mut self.fx_reverb_mix, 0.0..=1.0, "Mix")
                                .default_value(0.25)
                                .format(|v| format!("{:.0}%", v * 100.0)),
                        )
                        .changed()
                    {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::FxReverbMix,
                            value: self.fx_reverb_mix,
                        });
                    }
                });
            });
        });
    }

    fn footer_bar(&self, ui: &mut egui::Ui, snapshot: &synth_engine::ParamSnapshot) {
        ui.horizontal(|ui| {
            let cpu = f32::from_bits(self.cpu_load.load(Ordering::Relaxed));
            ui.label(format!("CPU: {cpu:.1}%"));
            ui.separator();
            ui.label(format!("voices: {}", snapshot.active_voice_count));
            ui.separator();
            ui.label(&self.audio_status);
        });
    }
}

/// Formats seconds as ms (< 1 s) or seconds (>= 1 s) for envelope tooltips.
fn secs_format(v: f32) -> String {
    if v < 1.0 {
        format!("{:.0} ms", v * 1000.0)
    } else {
        format!("{:.2} s", v)
    }
}

/// Formats a MIDI note number as scientific pitch notation (C4 = 60).
fn midi_note_label(note_midi: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = i32::from(note_midi / 12) - 1;
    let name = NAMES[usize::from(note_midi % 12)];
    format!("{name}{octave}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_60_is_c4() {
        assert_eq!(midi_note_label(60), "C4");
    }

    #[test]
    fn midi_48_is_c3() {
        assert_eq!(midi_note_label(48), "C3");
    }

    #[test]
    fn midi_69_is_a4() {
        assert_eq!(midi_note_label(69), "A4");
    }

    #[test]
    fn secs_format_below_one_second_shows_ms() {
        assert_eq!(secs_format(0.010), "10 ms");
        assert_eq!(secs_format(0.200), "200 ms");
    }

    #[test]
    fn secs_format_at_or_above_one_second_shows_s() {
        assert!(secs_format(1.0).contains('s'));
        assert!(secs_format(3.5).contains('s'));
    }
}
