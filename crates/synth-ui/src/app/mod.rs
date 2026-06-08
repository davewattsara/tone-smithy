//! Top-level [`eframe::App`] implementation.
//!
//! `ToneSmithyApp` owns the UI state mirrors, the event sender, and the
//! snapshot slot. Each synth section lives in its own file under
//! `sections/`; this module covers the chrome: header bar, tab bar, keyboard
//! strip, footer, and the `eframe::App::update` loop.

mod chrome;
mod midi_learn;
mod mod_display;
mod preset;
pub(crate) mod state;
mod utils;
mod wizard;

pub(crate) use mod_display::ModDisplay;
pub use state::{DeviceChange, Tab, ToneSmithyApp};
pub(crate) use utils::secs_format;

// Re-export constants used by section files so they can still import from `crate::app`.
pub(crate) use state::{
    BPM_MAX, BPM_MIN, CUTOFF_MAX_HZ, CUTOFF_MIN_HZ, ENV_ATTACK_MAX_SECS, ENV_DECAY_MAX_SECS, ENV_MIN_SECS,
    ENV_RELEASE_MAX_SECS, ENV2_CURVE_RANGE, FM_OP_ENV_MAX_SECS, FM_OP_ENV_MIN_SECS, FM_RATIO_FINE_MAX, LFO_RATE_MAX_HZ,
    LFO_RATE_MIN_HZ, MOD_AMOUNT_RANGES, MOD_DEST_LABELS, MOD_SOURCE_LABELS, OSC_DETUNE_MAX_CENTS, OSC_LEVEL_MAX,
    PITCH_OFFSET_RANGE, UNISON_DETUNE_MAX_CENTS, UNISON_VOICES_MAX,
};

use eframe::egui;
use synth_engine::param_bus::load_snapshot;

use crate::theme;

impl eframe::App for ToneSmithyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(theme::make_visuals());
        self.computer_keyboard.handle_input(ctx, &self.events);
        // Drain file-watcher notifications; refresh preset list if any arrived.
        if self.file_watch_rx.try_recv().is_ok() {
            self.refresh_preset_list();
            // Drain any queued-up duplicates from the same event burst.
            while self.file_watch_rx.try_recv().is_ok() {}
        }
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

        // MIDI Learn: detect CC movement and bind it, then apply all mappings.
        self.tick_midi_learn(ctx, &snapshot);

        // Compute per-frame modulation display offsets from snapshot.
        let mod_display = ModDisplay::from_snapshot(&snapshot);

        // First-run wizard — modal overlay until dismissed.
        self.first_run_wizard(ctx);

        // Central panel — active tab content, scrollable so tall sections
        // (e.g. expanded FM operator grid) are not clipped by the keyboard strip.
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.active_tab {
                Tab::Osc => self.osc_tab(ui, mod_display),
                Tab::Filter => self.filter_tab(ui, mod_display),
                Tab::Envelopes => self.envelopes_tab(ui, &snapshot),
                Tab::Modulation => self.modulation_tab(ui),
                Tab::Arp => self.arp_tab(ui),
                Tab::Fx => self.fx_tab(ui),
                Tab::Master => self.master_tab(ui, &snapshot, mod_display),
                Tab::Presets => self.presets_tab(ui),
                Tab::Settings => self.settings_tab(ui),
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}
