use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::ToneSmithyApp;
use crate::knob::Knob;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn arp_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "ARPEGGIATOR");

        ui.horizontal(|ui| {
            let prev_enabled = self.arp_enabled;
            ui.checkbox(&mut self.arp_enabled, "On");
            if self.arp_enabled != prev_enabled {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::ArpEnabled,
                    value: if self.arp_enabled { 1.0 } else { 0.0 },
                });
            }
        });

        ui.add_space(theme::GROUP_GAP);

        ui.add_enabled_ui(self.arp_enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Mode");
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
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Octaves");
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
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Rate");
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
            });

            ui.add_space(theme::GROUP_GAP);

            ui.horizontal(|ui| {
                if ui
                    .add(
                        Knob::new(&mut self.arp_bpm, 20.0..=300.0, "BPM")
                            .default_value(120.0)
                            .format(|v| format!("{:.0}", v)),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ArpBpm,
                        value: self.arp_bpm,
                    });
                }
                if ui
                    .add(
                        Knob::new(&mut self.arp_gate, 0.01..=1.0, "Gate")
                            .default_value(0.5)
                            .format(|v| format!("{:.0}%", v * 100.0)),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ArpGate,
                        value: self.arp_gate,
                    });
                }
                if ui
                    .add(
                        Knob::new(&mut self.arp_swing, 0.5..=0.75, "Swing")
                            .default_value(0.5)
                            .format(|v| format!("{:.0}%", (v - 0.5) / 0.25 * 100.0)),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ArpSwing,
                        value: self.arp_swing,
                    });
                }
            });
        });
    }
}
