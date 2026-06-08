//! Settings tab — audio device picker, MIDI port picker, device list refresh.

use eframe::egui;

use crate::app::{DeviceChange, ToneSmithyApp};
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "SETTINGS");

        ui.columns(2, |cols| {
            // ── Left: Audio output ──────────────────────────────────────────
            cols[0].add_space(theme::PANEL_PADDING);
            theme::section_label(&mut cols[0], "AUDIO OUTPUT");

            let current_audio = self
                .settings
                .audio_output_device
                .clone()
                .unwrap_or_else(|| "(OS default)".to_string());

            cols[0].horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Device:")
                        .color(theme::FG1)
                        .font(theme::font_body()),
                );
                egui::ComboBox::from_id_salt("audio_device_picker")
                    .selected_text(&current_audio)
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.settings.audio_output_device.is_none(), "(OS default)")
                            .clicked()
                        {
                            self.request_device_change(DeviceChange::Audio(None));
                        }
                        let devices = self.audio_devices.clone();
                        for device in &devices {
                            let selected = self.settings.audio_output_device.as_deref() == Some(device);
                            if ui.selectable_label(selected, device).clicked() {
                                self.request_device_change(DeviceChange::Audio(Some(device.clone())));
                            }
                        }
                    });
            });

            cols[0].add_space(4.0);
            cols[0].label(
                egui::RichText::new(format!("Sample rate: {} Hz", self.current_sample_rate()))
                    .color(theme::FG2)
                    .font(theme::font_small()),
            );

            cols[0].add_space(theme::GROUP_GAP);
            theme::subtle_separator(&mut cols[0]);
            cols[0].add_space(theme::GROUP_GAP);

            // ── Left: MIDI input ────────────────────────────────────────────
            theme::section_label(&mut cols[0], "MIDI INPUT");

            let current_midi = self
                .settings
                .midi_input_port
                .clone()
                .unwrap_or_else(|| "(first available)".to_string());

            cols[0].horizontal(|ui| {
                ui.label(egui::RichText::new("Port:").color(theme::FG1).font(theme::font_body()));
                egui::ComboBox::from_id_salt("midi_port_picker")
                    .selected_text(&current_midi)
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.settings.midi_input_port.is_none(), "(first available)")
                            .clicked()
                        {
                            self.request_device_change(DeviceChange::Midi(None));
                        }
                        let ports = self.midi_ports.clone();
                        for port in &ports {
                            let selected = self.settings.midi_input_port.as_deref() == Some(port);
                            if ui.selectable_label(selected, port).clicked() {
                                self.request_device_change(DeviceChange::Midi(Some(port.clone())));
                            }
                        }
                    });
            });

            if self.midi_ports.is_empty() {
                cols[0].label(
                    egui::RichText::new("No MIDI input ports detected.")
                        .color(theme::FG2)
                        .font(theme::font_small()),
                );
            }

            cols[0].add_space(theme::GROUP_GAP);
            if cols[0].button("Refresh device lists").clicked() {
                self.refresh_device_lists();
            }

            // ── Right: status and MIDI Learn summary ────────────────────────
            cols[1].add_space(theme::PANEL_PADDING);
            theme::section_label(&mut cols[1], "STATUS");
            cols[1].label(
                egui::RichText::new(&self.audio_status)
                    .color(theme::FG1)
                    .font(theme::font_small()),
            );

            cols[1].add_space(theme::GROUP_GAP);
            theme::subtle_separator(&mut cols[1]);
            cols[1].add_space(theme::GROUP_GAP);

            theme::section_label(&mut cols[1], "MIDI LEARN BINDINGS");
            if self.midi_learn_mappings.is_empty() {
                cols[1].label(
                    egui::RichText::new("No bindings yet. Right-click any knob to learn a CC.")
                        .color(theme::FG2)
                        .font(theme::font_small()),
                );
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("midi_learn_list")
                    .max_height(200.0)
                    .show(&mut cols[1], |ui| {
                        let mut to_remove: Option<usize> = None;
                        for (i, entry) in self.midi_learn_mappings.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("CC {:3}  →  {}", entry.cc, entry.parameter))
                                        .color(theme::FG1)
                                        .font(theme::font_small()),
                                );
                                if ui.small_button("x").clicked() {
                                    to_remove = Some(i);
                                }
                            });
                        }
                        if let Some(idx) = to_remove {
                            self.midi_learn_mappings.remove(idx);
                        }
                    });
            }
            if !self.midi_learn_mappings.is_empty() {
                cols[1].add_space(4.0);
                if cols[1].small_button("Clear all bindings").clicked() {
                    self.midi_learn_mappings.clear();
                }
            }
        });
    }

    fn current_sample_rate(&self) -> u32 {
        // Parse sample rate from the status string set by AppShell.
        self.audio_status
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }
}
