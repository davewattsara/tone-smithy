//! Top-level [`eframe::App`] implementation.
//!
//! `ToneSmithyApp` owns the UI state mirrors, the event sender, and the
//! snapshot slot. Each synth section lives in its own file under
//! `sections/`; this file covers the chrome: header bar, tab bar, keyboard
//! strip, footer, and the `eframe::App::update` loop.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use eframe::egui;
use synth_engine::ParamSnapshot;
use synth_engine::param_bus::{EngineEventSender, SnapshotSlot, load_snapshot};
use synth_engine::{EngineEvent, FilterMode, Waveform};
use synth_presets::{Preset, map_to_events, map_to_snapshot, snapshot_to_map};

use crate::computer_keyboard::ComputerKeyboard;
use crate::keyboard::VirtualKeyboard;
use crate::theme;

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
    const ALL: &'static [(Tab, &'static str)] = &[
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
    audio_status: String,
    pub(crate) events: EngineEventSender,
    snapshot_slot: SnapshotSlot,

    // ── Active tab ───────────────────────────────────────────────────────────
    active_tab: Tab,

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
    keyboard: VirtualKeyboard,
    computer_keyboard: ComputerKeyboard,
    pub(crate) pitch_bend: f32,
    pub(crate) mod_wheel: f32,
    pub(crate) sustain_held: bool,

    /// CPU load arc from the audio thread (f32 bits stored as u32).
    cpu_load: Arc<AtomicU32>,

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
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for ToneSmithyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(theme::make_visuals());
        self.computer_keyboard.handle_input(ctx, &self.events);
        let snapshot = load_snapshot(&self.snapshot_slot);

        // Footer — must be added before the central panel
        egui::TopBottomPanel::bottom("footer")
            .exact_height(theme::FOOTER_HEIGHT)
            .show(ctx, |ui| {
                self.footer_bar(ui, &snapshot);
            });

        // Virtual keyboard strip — sits above the footer
        egui::TopBottomPanel::bottom("keyboard_strip")
            .exact_height(theme::KEYBOARD_HEIGHT)
            .show(ctx, |ui| {
                self.keyboard_strip(ui);
            });

        // Header — title + preset controls
        egui::TopBottomPanel::top("header")
            .exact_height(theme::HEADER_HEIGHT)
            .show(ctx, |ui| {
                self.header_bar(ui);
            });

        // Error bar — only present when a preset error is active
        if self.preset_error.is_some() {
            egui::TopBottomPanel::top("error_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(theme::PANEL_PADDING);
                    if let Some(ref err) = self.preset_error.clone() {
                        ui.colored_label(theme::WARN, err);
                    }
                    if ui.small_button("x").clicked() {
                        self.preset_error = None;
                    }
                });
            });
        }

        // Tab bar — sits below the header (and error bar if visible)
        egui::TopBottomPanel::top("tab_bar")
            .exact_height(theme::TAB_BAR_HEIGHT)
            .show(ctx, |ui| {
                self.tab_bar(ui);
            });

        // Central panel — active tab content, scrollable so tall sections
        // (e.g. expanded FM operator grid) are not clipped by the keyboard strip.
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.active_tab {
                Tab::Osc => self.osc_tab(ui),
                Tab::Filter => self.filter_tab(ui),
                Tab::Envelopes => self.envelopes_tab(ui, &snapshot),
                Tab::Modulation => self.modulation_tab(ui),
                Tab::Arp => self.arp_tab(ui),
                Tab::Fx => self.fx_tab(ui),
                Tab::Master => self.master_tab(ui, &snapshot),
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

// ── Chrome panels ─────────────────────────────────────────────────────────────

impl ToneSmithyApp {
    fn header_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            ui.label(
                egui::RichText::new("Tone Smithy")
                    .color(theme::FG0)
                    .font(theme::font_display()),
            );
            ui.separator();
            ui.label(egui::RichText::new("Patch:").color(theme::FG1).font(theme::font_body()));
            ui.add(
                egui::TextEdit::singleline(&mut self.patch_name)
                    .desired_width(180.0)
                    .hint_text("Untitled")
                    .font(theme::font_body()),
            );
            if ui.button("Save").clicked() {
                self.save_preset();
            }
            if ui.button("Load").clicked() {
                self.load_preset();
            }
            ui.separator();
            ui.label(
                egui::RichText::new(&self.audio_status)
                    .color(theme::FG2)
                    .font(theme::font_micro()),
            );
        });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            for &(tab, label) in Tab::ALL {
                let selected = self.active_tab == tab;
                let text = egui::RichText::new(label).font(theme::font_body());
                let text = if selected {
                    text.color(theme::ACCENT)
                } else {
                    text.color(theme::FG1)
                };
                if ui.selectable_label(selected, text).clicked() {
                    self.active_tab = tab;
                }
                ui.add_space(4.0);
            }
        });
    }

    fn keyboard_strip(&mut self, ui: &mut egui::Ui) {
        // Keep virtual keyboard range in sync with computer keyboard octave.
        if let Some(stuck) = self.keyboard.set_start_note(self.computer_keyboard.octave_base()) {
            self.events.send(EngineEvent::NoteOff { note_midi: stuck });
        }
        let kb_notes = self.computer_keyboard.held_notes();

        ui.horizontal(|ui| {
            // Pitch-bend slider: springs to 0 on release
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("PB").color(theme::FG2).font(theme::font_micro()));
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
                if !pb_r.is_pointer_button_down_on() && self.pitch_bend != 0.0 {
                    self.pitch_bend = 0.0;
                    self.events.send(EngineEvent::PitchBend { value_normalised: 0.0 });
                }
            });

            // Mod wheel slider: stays where left
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("MW").color(theme::FG2).font(theme::font_micro()));
                if ui
                    .add(
                        egui::Slider::new(&mut self.mod_wheel, 0.0..=1.0)
                            .vertical()
                            .show_value(false),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ControlChange {
                        cc: 1,
                        value_normalised: self.mod_wheel,
                    });
                }
            });

            // Virtual keyboard
            self.keyboard.show(ui, &self.events, kb_notes);

            // Sustain pedal
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Sustain")
                        .color(theme::FG2)
                        .font(theme::font_micro()),
                );
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

            // Octave hint
            ui.label(
                egui::RichText::new(format!(
                    "Oct: {} ({})",
                    self.computer_keyboard.octave_base(),
                    midi_note_label(self.computer_keyboard.octave_base())
                ))
                .color(theme::FG2)
                .font(theme::font_micro()),
            );
        });
    }

    fn footer_bar(&self, ui: &mut egui::Ui, snapshot: &ParamSnapshot) {
        ui.horizontal_centered(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            let cpu = f32::from_bits(self.cpu_load.load(Ordering::Relaxed));
            let text = format!(
                "CPU: {cpu:.1}%   Voices: {}   {}",
                snapshot.active_voice_count, self.audio_status
            );
            ui.label(egui::RichText::new(text).color(theme::FG2).font(theme::font_micro()));
        });
    }
}

// ── Preset helpers ────────────────────────────────────────────────────────────

impl ToneSmithyApp {
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

    fn save_preset(&mut self) {
        let snapshot = load_snapshot(&self.snapshot_slot);
        let mut preset = Preset::new(self.patch_name.clone());
        preset.parameters = snapshot_to_map(&snapshot);

        let default_filename = format!("{}.tsmith", self.patch_name);
        let start_dir = synth_presets::user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

        if let Some(path) = rfd::FileDialog::new()
            .set_title("Save Preset")
            .set_file_name(&default_filename)
            .add_filter("Tone Smithy Preset", &["tsmith"])
            .set_directory(&start_dir)
            .save_file()
        {
            if let Err(e) = synth_presets::save(&path, &preset) {
                self.preset_error = Some(format!("Save failed: {e}"));
            } else {
                self.preset_error = None;
            }
        }
    }

    fn load_preset(&mut self) {
        let start_dir = synth_presets::user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

        if let Some(path) = rfd::FileDialog::new()
            .set_title("Load Preset")
            .add_filter("Tone Smithy Preset", &["tsmith"])
            .set_directory(&start_dir)
            .pick_file()
        {
            match synth_presets::load(&path) {
                Ok(preset) => {
                    for event in map_to_events(&preset.parameters) {
                        self.events.send(event);
                    }
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

// ── Utility functions (pub(crate) so section files can use them) ──────────────

/// Formats seconds as `"N ms"` below 1 s or `"N.NN s"` at or above 1 s.
pub(crate) fn secs_format(v: f32) -> String {
    if v < 1.0 {
        format!("{:.0} ms", v * 1000.0)
    } else {
        format!("{:.2} s", v)
    }
}

/// Formats a MIDI note number as scientific pitch notation (`C4` = 60).
pub(crate) fn midi_note_label(note_midi: u8) -> String {
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
