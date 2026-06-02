use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::{MOD_AMOUNT_RANGES, MOD_DEST_LABELS, MOD_SOURCE_LABELS, ToneSmithyApp};
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn modulation_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "MOD MATRIX");

        egui::Grid::new("mod_matrix").min_col_width(70.0).show(ui, |ui| {
            ui.label("");
            ui.label("Source");
            ui.label("Dest");
            ui.label("Amount");
            ui.label("Via");
            ui.end_row();

            for i in 0..8usize {
                let mut enabled = self.mod_slot_enabled[i];
                if ui.checkbox(&mut enabled, format!("Slot {}", i + 1)).changed() {
                    self.mod_slot_enabled[i] = enabled;
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::ModSlotEnabled(i as u8),
                        value: if enabled { 1.0 } else { 0.0 },
                    });
                }

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
}
