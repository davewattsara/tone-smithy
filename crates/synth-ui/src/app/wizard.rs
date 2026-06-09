//! First-run wizard — shown once on a fresh install to let the user pick
//! audio and MIDI devices before entering the main UI.

use eframe::egui;
use synth_presets::save_settings;

use crate::app::state::{DeviceChange, ToneSmithyApp};
use crate::theme;

impl ToneSmithyApp {
    /// Renders the first-run wizard as a modal window. Does nothing once
    /// `settings.first_run_complete` is true.
    pub(crate) fn first_run_wizard(&mut self, ctx: &egui::Context) {
        if self.settings.first_run_complete {
            return;
        }

        // Dim background
        egui::Area::new(egui::Id::new("wizard_backdrop"))
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));
            });

        egui::Window::new("Welcome to Tone Smithy")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .min_width(420.0)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "Choose your audio output device and MIDI input port below.\n\
                         You can change these later in the Settings tab.",
                    )
                    .color(theme::FG1)
                    .font(theme::font_body()),
                );
                ui.add_space(theme::GROUP_GAP);

                // Audio device picker
                ui.label(
                    egui::RichText::new("Audio output:")
                        .color(theme::FG1)
                        .font(theme::font_body()),
                );
                let current_audio = self
                    .settings
                    .audio_output_device
                    .clone()
                    .unwrap_or_else(|| "(OS default)".to_string());
                egui::ComboBox::from_id_salt("wizard_audio")
                    .selected_text(&current_audio)
                    .width(360.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.settings.audio_output_device.is_none(), "(OS default)")
                            .clicked()
                        {
                            self.request_device_change(DeviceChange::Audio(None));
                        }
                        let devices = self.audio_devices.clone();
                        for device in &devices {
                            let selected = self.settings.audio_output_device.as_deref() == Some(device.as_str());
                            if ui.selectable_label(selected, device).clicked() {
                                self.request_device_change(DeviceChange::Audio(Some(device.clone())));
                            }
                        }
                    });

                ui.add_space(8.0);

                // MIDI port picker
                ui.label(
                    egui::RichText::new("MIDI input:")
                        .color(theme::FG1)
                        .font(theme::font_body()),
                );
                let current_midi = self
                    .settings
                    .midi_input_port
                    .clone()
                    .unwrap_or_else(|| "(first available)".to_string());
                egui::ComboBox::from_id_salt("wizard_midi")
                    .selected_text(&current_midi)
                    .width(360.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.settings.midi_input_port.is_none(), "(first available)")
                            .clicked()
                        {
                            self.request_device_change(DeviceChange::Midi(None));
                        }
                        let ports = self.midi_ports.clone();
                        for port in &ports {
                            let selected = self.settings.midi_input_port.as_deref() == Some(port.as_str());
                            if ui.selectable_label(selected, port).clicked() {
                                self.request_device_change(DeviceChange::Midi(Some(port.clone())));
                            }
                        }
                    });

                ui.add_space(theme::GROUP_GAP);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new("Get started").font(theme::font_body()))
                        .clicked()
                    {
                        self.settings.first_run_complete = true;
                        save_settings(&self.settings);
                    }
                    ui.add_space(8.0);
                    if ui.small_button("Refresh device lists").clicked() {
                        self.refresh_device_lists();
                    }
                });
            });
    }
}
