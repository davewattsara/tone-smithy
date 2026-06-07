use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::{FM_OP_ENV_MAX_SECS, FM_OP_ENV_MIN_SECS, FM_RATIO_FINE_MAX, ToneSmithyApp, secs_format};
use crate::knob::Knob;
use crate::theme;

impl ToneSmithyApp {
    /// Compact per-slot controls shown in the 4th column: mode toggle, level,
    /// pan, and (when FM) algorithm. The operator grid is separate — see
    /// [`fm_op_grid`].
    pub(crate) fn fm_slots_section(&mut self, ui: &mut egui::Ui) {
        for slot_idx in 0..2usize {
            let mode_tag = if self.slot_mode[slot_idx] == 0 { "Sub" } else { "FM" };
            let slot_label = format!("Slot {} ({})", slot_idx + 1, mode_tag);
            egui::CollapsingHeader::new(slot_label)
                .id_salt(format!("fm_slot_{slot_idx}"))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let is_sub = self.slot_mode[slot_idx] == 0;
                        let is_fm = self.slot_mode[slot_idx] == 1;
                        if ui.selectable_label(is_sub, "Sub").clicked() && !is_sub {
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

                    if self.slot_mode[slot_idx] == 1 {
                        ui.add_space(4.0);
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
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Alg").color(theme::FG1).font(theme::font_small()));
                            egui::ComboBox::from_id_salt(format!("fm_alg_{slot_idx}"))
                                .selected_text(ALG_LABELS[self.fm_algorithm[slot_idx] as usize])
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
                        // Operator grid — full width is available here since this section
                        // is outside the narrow oscillator columns.
                        egui::ScrollArea::horizontal()
                            .id_salt(format!("fm_ops_scroll_{slot_idx}"))
                            .show(ui, |ui| {
                                egui::Grid::new(format!("fm_ops_{slot_idx}"))
                                    .striped(true)
                                    .spacing([4.0, 4.0])
                                    .show(ui, |ui| {
                                        ui.label(egui::RichText::new("Op").color(theme::FG2).font(theme::font_small()));
                                        ui.label(
                                            egui::RichText::new("Ratio").color(theme::FG2).font(theme::font_small()),
                                        );
                                        ui.label(
                                            egui::RichText::new("Fine").color(theme::FG2).font(theme::font_small()),
                                        );
                                        ui.label(
                                            egui::RichText::new("Level").color(theme::FG2).font(theme::font_small()),
                                        );
                                        ui.label(egui::RichText::new("A").color(theme::FG2).font(theme::font_small()));
                                        ui.label(egui::RichText::new("D").color(theme::FG2).font(theme::font_small()));
                                        ui.label(egui::RichText::new("S").color(theme::FG2).font(theme::font_small()));
                                        ui.label(egui::RichText::new("R").color(theme::FG2).font(theme::font_small()));
                                        ui.label(egui::RichText::new("FB").color(theme::FG2).font(theme::font_small()));
                                        ui.end_row();

                                        for op in 0..4usize {
                                            let packed = ((slot_idx as u8) << 4) | (op as u8);
                                            ui.label(format!("Op {}", op + 1));

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

                                            if ui
                                                .add(
                                                    Knob::new(
                                                        &mut self.fm_op_attack_secs[slot_idx][op],
                                                        FM_OP_ENV_MIN_SECS..=FM_OP_ENV_MAX_SECS,
                                                        "",
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
                                            if ui
                                                .add(
                                                    Knob::new(
                                                        &mut self.fm_op_decay_secs[slot_idx][op],
                                                        FM_OP_ENV_MIN_SECS..=FM_OP_ENV_MAX_SECS,
                                                        "",
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
                                            if ui
                                                .add(
                                                    Knob::new(
                                                        &mut self.fm_op_sustain_level[slot_idx][op],
                                                        0.0..=1.0,
                                                        "",
                                                    )
                                                    .default_value(0.800)
                                                    .format(|v| format!("{:.2}", v)),
                                                )
                                                .changed()
                                            {
                                                self.events.send(EngineEvent::ParameterChange {
                                                    id: ParamId::FmOpSustainLevel(packed),
                                                    value: self.fm_op_sustain_level[slot_idx][op],
                                                });
                                            }
                                            if ui
                                                .add(
                                                    Knob::new(
                                                        &mut self.fm_op_release_secs[slot_idx][op],
                                                        FM_OP_ENV_MIN_SECS..=FM_OP_ENV_MAX_SECS,
                                                        "",
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

                                            if op == 3 {
                                                if ui
                                                    .add(
                                                        Knob::new(
                                                            &mut self.fm_op_feedback[slot_idx][op],
                                                            -1.0..=1.0,
                                                            "FB",
                                                        )
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
                                    }); // Grid
                            }); // ScrollArea
                    } // if FM mode
                }); // CollapsingHeader
        } // for slot_idx
    }
}
