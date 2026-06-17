use eframe::egui;
use synth_engine::{EngineEvent, MOD_MATRIX_SLOTS, ParamId};

use crate::app::{
    MOD_AMOUNT_RANGES, MOD_DEST_LABELS, MOD_DEST_TOOLTIPS, MOD_SOURCE_LABELS, MOD_SOURCE_ORDER, MOD_SOURCE_TOOLTIPS,
    ToneSmithyApp,
};
use crate::knob::Knob;
use crate::theme;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn modulation_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "MOD MATRIX");

        egui::Grid::new("mod_matrix")
            .min_col_width(70.0)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                let hdr = |text: &str| egui::RichText::new(text).color(theme::FG2).font(theme::font_small());
                ui.label("");
                ui.label(hdr("Source"));
                ui.label(hdr("Dest"));
                ui.label(hdr("Amount"));
                ui.label(hdr("Via"));
                ui.end_row();

                for i in 0..MOD_MATRIX_SLOTS {
                    // Keys and labels are built per row; with 16 slots the old
                    // static arrays would just duplicate `format!` output.
                    let slot_label = (i + 1).to_string();
                    let enabled_key = format!("mod_slot_enabled_{i}");
                    if ui
                        .add(Toggle::new(&mut self.mod_slot_enabled[i], &slot_label).param_key(&enabled_key))
                        .changed()
                    {
                        self.emit_change(EngineEvent::ParameterChange {
                            id: ParamId::ModSlotEnabled(i as u8),
                            value: if self.mod_slot_enabled[i] { 1.0 } else { 0.0 },
                        });
                    }

                    let src_label = MOD_SOURCE_LABELS.get(self.mod_slot_source[i]).copied().unwrap_or("?");
                    egui::ComboBox::from_id_salt(format!("mod_src_{i}"))
                        .selected_text(src_label)
                        .show_ui(ui, |ui| {
                            for &idx in MOD_SOURCE_ORDER {
                                let label = MOD_SOURCE_LABELS.get(idx).copied().unwrap_or("?");
                                let tip = MOD_SOURCE_TOOLTIPS.get(idx).copied().unwrap_or("");
                                if ui
                                    .selectable_value(&mut self.mod_slot_source[i], idx, label)
                                    .on_hover_text(tip)
                                    .changed()
                                {
                                    self.emit_change(EngineEvent::ParameterChange {
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
                                let tip = MOD_DEST_TOOLTIPS.get(idx).copied().unwrap_or("");
                                if ui
                                    .selectable_value(&mut self.mod_slot_dest[i], idx, label)
                                    .on_hover_text(tip)
                                    .changed()
                                {
                                    self.emit_change(EngineEvent::ParameterChange {
                                        id: ParamId::ModSlotDest(i as u8),
                                        value: idx as f32,
                                    });
                                    self.mod_slot_amount[i] = 0.0;
                                    self.emit_change(EngineEvent::ParameterChange {
                                        id: ParamId::ModSlotAmount(i as u8),
                                        value: 0.0,
                                    });
                                }
                            }
                        });

                    let range = MOD_AMOUNT_RANGES.get(self.mod_slot_dest[i]).copied().unwrap_or(1.0);
                    let mod_amount_key = format!("mod_slot_amount_{i}");
                    if ui
                        .add(
                            Knob::new(&mut self.mod_slot_amount[i], -range..=range, "")
                                .default_value(0.0)
                                .param_key(&mod_amount_key)
                                .format(move |v| {
                                    if range >= 100.0 {
                                        format!("{:+.0}", v)
                                    } else {
                                        format!("{:+.2}", v)
                                    }
                                }),
                        )
                        .changed()
                    {
                        self.emit_change(EngineEvent::ParameterChange {
                            id: ParamId::ModSlotAmount(i as u8),
                            value: self.mod_slot_amount[i],
                        });
                    }

                    let via_label = MOD_SOURCE_LABELS.get(self.mod_slot_via[i]).copied().unwrap_or("?");
                    egui::ComboBox::from_id_salt(format!("mod_via_{i}"))
                        .selected_text(via_label)
                        .show_ui(ui, |ui| {
                            for &idx in MOD_SOURCE_ORDER {
                                let label = MOD_SOURCE_LABELS.get(idx).copied().unwrap_or("?");
                                if ui.selectable_value(&mut self.mod_slot_via[i], idx, label).changed() {
                                    self.emit_change(EngineEvent::ParameterChange {
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
