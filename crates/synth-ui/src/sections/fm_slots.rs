use eframe::egui;
use synth_engine::{ALGORITHMS, CUSTOM_ALGORITHM_INDEX, EngineEvent, FM_CUSTOM_CONN_TABLE, ParamId};

use crate::app::{FM_OP_ENV_MAX_SECS, FM_OP_ENV_MIN_SECS, FM_RATIO_FINE_MAX, ModDisplay, ToneSmithyApp, secs_format};
use crate::knob::Knob;
use crate::midi_learn_ext::attach_learn_menu;
use crate::theme;

impl ToneSmithyApp {
    /// Per-slot controls: level, pan, and slot-specific content.
    /// Slot 0 (Sub) expands to show oscillator controls; slot 1 (FM) shows
    /// the algorithm selector and operator grid.
    pub(crate) fn fm_slots_section(&mut self, ui: &mut egui::Ui, md: ModDisplay) {
        for slot_idx in 0..2usize {
            let mode_tag = if slot_idx == 0 { "Sub" } else { "FM" };
            let slot_label = format!("Slot {} ({})", slot_idx + 1, mode_tag);
            // On the frame immediately after a preset load, force the open
            // state based on slot level so zero-level slots start collapsed.
            let forced_open = if self.just_loaded_preset {
                Some(self.slot_foldout_open[slot_idx])
            } else {
                None
            };
            egui::CollapsingHeader::new(slot_label)
                .id_salt(format!("fm_slot_{slot_idx}"))
                .open(forced_open)
                .show(ui, |ui| {
                    let slot_level_key = ["slot_level_0", "slot_level_1"][slot_idx];
                    let slot_pan_key = ["slot_pan_0", "slot_pan_1"][slot_idx];
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                Knob::new(&mut self.slot_level[slot_idx], 0.0..=1.0, "Level")
                                    .default_value(if slot_idx == 0 { 1.0 } else { 0.0 })
                                    .param_key(slot_level_key)
                                    .format(|v| format!("{:.2}", v)),
                            )
                            .changed()
                        {
                            self.emit_change(EngineEvent::ParameterChange {
                                id: ParamId::SlotLevel(slot_idx as u8),
                                value: self.slot_level[slot_idx],
                            });
                        }
                        if ui
                            .add(
                                Knob::new(&mut self.slot_pan[slot_idx], -1.0..=1.0, "Pan")
                                    .default_value(0.0)
                                    .param_key(slot_pan_key)
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
                            self.emit_change(EngineEvent::ParameterChange {
                                id: ParamId::SlotPan(slot_idx as u8),
                                value: self.slot_pan[slot_idx],
                            });
                        }
                    });

                    if slot_idx == 0 {
                        ui.add_space(4.0);
                        self.osc_sub_controls_inline(ui, md);
                    } else {
                        ui.add_space(4.0);
                        const ALG_LABELS: [&str; 9] = [
                            "1 Stack",
                            "2 Stack+FB",
                            "3 Two stacks",
                            "4 Para mod",
                            "5 Branch",
                            "6 Mixed",
                            "7 Additive",
                            "8 Paired",
                            "9 Custom",
                        ];
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Alg").color(theme::FG1).font(theme::font_small()));
                            let prev_alg = self.fm_algorithm[slot_idx];
                            egui::ComboBox::from_id_salt(format!("fm_alg_{slot_idx}"))
                                .selected_text(ALG_LABELS[self.fm_algorithm[slot_idx] as usize])
                                .show_ui(ui, |ui| {
                                    for (idx, &label) in ALG_LABELS.iter().enumerate() {
                                        if ui
                                            .selectable_value(&mut self.fm_algorithm[slot_idx], idx as u8, label)
                                            .changed()
                                        {
                                            // Switching into Custom from a factory algorithm seeds the
                                            // editable routing from that algorithm so the sound is
                                            // continuous; the user then edits from there.
                                            if idx as u8 == CUSTOM_ALGORITHM_INDEX && prev_alg < CUSTOM_ALGORITHM_INDEX
                                            {
                                                self.seed_fm_custom_from_factory(slot_idx, prev_alg);
                                            }
                                            self.emit_change(EngineEvent::ParameterChange {
                                                id: ParamId::FmAlgorithm(slot_idx as u8),
                                                value: idx as f32,
                                            });
                                        }
                                    }
                                });
                        });

                        if self.fm_algorithm[slot_idx] == CUSTOM_ALGORITHM_INDEX {
                            ui.add_space(4.0);
                            self.fm_custom_routing_grid(ui, slot_idx);
                        }

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

                                        // Static key tables avoid lifetime issues with format!()
                                        const FM_OP_RATIO_INT_K: [[&str; 4]; 2] = [
                                            [
                                                "fm_op_ratio_integer_0_0",
                                                "fm_op_ratio_integer_0_1",
                                                "fm_op_ratio_integer_0_2",
                                                "fm_op_ratio_integer_0_3",
                                            ],
                                            [
                                                "fm_op_ratio_integer_1_0",
                                                "fm_op_ratio_integer_1_1",
                                                "fm_op_ratio_integer_1_2",
                                                "fm_op_ratio_integer_1_3",
                                            ],
                                        ];
                                        const FM_OP_RATIO_FINE_K: [[&str; 4]; 2] = [
                                            [
                                                "fm_op_ratio_fine_0_0",
                                                "fm_op_ratio_fine_0_1",
                                                "fm_op_ratio_fine_0_2",
                                                "fm_op_ratio_fine_0_3",
                                            ],
                                            [
                                                "fm_op_ratio_fine_1_0",
                                                "fm_op_ratio_fine_1_1",
                                                "fm_op_ratio_fine_1_2",
                                                "fm_op_ratio_fine_1_3",
                                            ],
                                        ];
                                        const FM_OP_LEVEL_K: [[&str; 4]; 2] = [
                                            [
                                                "fm_op_level_0_0",
                                                "fm_op_level_0_1",
                                                "fm_op_level_0_2",
                                                "fm_op_level_0_3",
                                            ],
                                            [
                                                "fm_op_level_1_0",
                                                "fm_op_level_1_1",
                                                "fm_op_level_1_2",
                                                "fm_op_level_1_3",
                                            ],
                                        ];
                                        const FM_OP_ATK_K: [[&str; 4]; 2] = [
                                            [
                                                "fm_op_attack_secs_0_0",
                                                "fm_op_attack_secs_0_1",
                                                "fm_op_attack_secs_0_2",
                                                "fm_op_attack_secs_0_3",
                                            ],
                                            [
                                                "fm_op_attack_secs_1_0",
                                                "fm_op_attack_secs_1_1",
                                                "fm_op_attack_secs_1_2",
                                                "fm_op_attack_secs_1_3",
                                            ],
                                        ];
                                        const FM_OP_DCY_K: [[&str; 4]; 2] = [
                                            [
                                                "fm_op_decay_secs_0_0",
                                                "fm_op_decay_secs_0_1",
                                                "fm_op_decay_secs_0_2",
                                                "fm_op_decay_secs_0_3",
                                            ],
                                            [
                                                "fm_op_decay_secs_1_0",
                                                "fm_op_decay_secs_1_1",
                                                "fm_op_decay_secs_1_2",
                                                "fm_op_decay_secs_1_3",
                                            ],
                                        ];
                                        const FM_OP_SUS_K: [[&str; 4]; 2] = [
                                            [
                                                "fm_op_sustain_level_0_0",
                                                "fm_op_sustain_level_0_1",
                                                "fm_op_sustain_level_0_2",
                                                "fm_op_sustain_level_0_3",
                                            ],
                                            [
                                                "fm_op_sustain_level_1_0",
                                                "fm_op_sustain_level_1_1",
                                                "fm_op_sustain_level_1_2",
                                                "fm_op_sustain_level_1_3",
                                            ],
                                        ];
                                        const FM_OP_REL_K: [[&str; 4]; 2] = [
                                            [
                                                "fm_op_release_secs_0_0",
                                                "fm_op_release_secs_0_1",
                                                "fm_op_release_secs_0_2",
                                                "fm_op_release_secs_0_3",
                                            ],
                                            [
                                                "fm_op_release_secs_1_0",
                                                "fm_op_release_secs_1_1",
                                                "fm_op_release_secs_1_2",
                                                "fm_op_release_secs_1_3",
                                            ],
                                        ];
                                        const FM_OP_FB_K: [&str; 2] = ["fm_op_feedback_0_3", "fm_op_feedback_1_3"];

                                        for op in 0..4usize {
                                            let packed = ((slot_idx as u8) << 4) | (op as u8);
                                            ui.label(format!("Op {}", op + 1));

                                            let mut ratio_int = self.fm_op_ratio_integer[slot_idx][op] as i32;
                                            let mut resp_int =
                                                ui.add(egui::DragValue::new(&mut ratio_int).range(1..=15).speed(0.1));
                                            if resp_int.changed() {
                                                self.fm_op_ratio_integer[slot_idx][op] = ratio_int as u8;
                                                self.emit_change(EngineEvent::ParameterChange {
                                                    id: ParamId::FmOpRatioInteger(packed),
                                                    value: ratio_int as f32,
                                                });
                                            }
                                            attach_learn_menu(
                                                &mut resp_int,
                                                ui,
                                                FM_OP_RATIO_INT_K[slot_idx][op],
                                                1.0,
                                                15.0,
                                            );

                                            let mut resp_fine = ui.add(
                                                egui::DragValue::new(&mut self.fm_op_ratio_fine[slot_idx][op])
                                                    .range(-FM_RATIO_FINE_MAX..=FM_RATIO_FINE_MAX)
                                                    .speed(0.5)
                                                    .suffix(" ct"),
                                            );
                                            if resp_fine.changed() {
                                                self.emit_change(EngineEvent::ParameterChange {
                                                    id: ParamId::FmOpRatioFine(packed),
                                                    value: self.fm_op_ratio_fine[slot_idx][op],
                                                });
                                            }
                                            attach_learn_menu(
                                                &mut resp_fine,
                                                ui,
                                                FM_OP_RATIO_FINE_K[slot_idx][op],
                                                -FM_RATIO_FINE_MAX,
                                                FM_RATIO_FINE_MAX,
                                            );

                                            if ui
                                                .add(
                                                    Knob::new(&mut self.fm_op_level[slot_idx][op], 0.0..=1.0, "Lv")
                                                        .default_value(1.0)
                                                        .param_key(FM_OP_LEVEL_K[slot_idx][op])
                                                        .format(|v| format!("{:.2}", v)),
                                                )
                                                .changed()
                                            {
                                                self.emit_change(EngineEvent::ParameterChange {
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
                                                    .param_key(FM_OP_ATK_K[slot_idx][op])
                                                    .format(secs_format),
                                                )
                                                .changed()
                                            {
                                                self.emit_change(EngineEvent::ParameterChange {
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
                                                    .param_key(FM_OP_DCY_K[slot_idx][op])
                                                    .format(secs_format),
                                                )
                                                .changed()
                                            {
                                                self.emit_change(EngineEvent::ParameterChange {
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
                                                    .param_key(FM_OP_SUS_K[slot_idx][op])
                                                    .format(|v| format!("{:.2}", v)),
                                                )
                                                .changed()
                                            {
                                                self.emit_change(EngineEvent::ParameterChange {
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
                                                    .param_key(FM_OP_REL_K[slot_idx][op])
                                                    .format(secs_format),
                                                )
                                                .changed()
                                            {
                                                self.emit_change(EngineEvent::ParameterChange {
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
                                                        .param_key(FM_OP_FB_K[slot_idx])
                                                        .format(|v| format!("{:.2}", v)),
                                                    )
                                                    .changed()
                                                {
                                                    self.emit_change(EngineEvent::ParameterChange {
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
                    } // slot-specific controls
                }); // CollapsingHeader
        } // for slot_idx

        // Flag has served its purpose now that the section has rendered.
        self.just_loaded_preset = false;
    }

    /// Seeds slot `slot`'s editable Custom routing from factory algorithm
    /// `factory_idx` (0..8), updating the UI mirror and pushing one event per
    /// connection / carrier so the engine and a saved preset match what is
    /// shown. Called when the user switches the algorithm selector to Custom.
    fn seed_fm_custom_from_factory(&mut self, slot: usize, factory_idx: u8) {
        let alg = ALGORITHMS[factory_idx as usize];
        for (conn_idx, &(src, dest)) in FM_CUSTOM_CONN_TABLE.iter().enumerate() {
            let on = alg.mod_sources[dest] & (1 << src) != 0;
            self.fm_custom_conn[slot][conn_idx] = on;
            self.emit_change(EngineEvent::ParameterChange {
                id: ParamId::FmCustomConn(slot as u8 * 6 + conn_idx as u8),
                value: if on { 1.0 } else { 0.0 },
            });
        }
        for (op, &carrier) in alg.is_carrier.iter().enumerate() {
            self.fm_custom_carrier[slot][op] = carrier;
            self.emit_change(EngineEvent::ParameterChange {
                id: ParamId::FmCustomCarrier(slot as u8 * 4 + op as u8),
                value: if carrier { 1.0 } else { 0.0 },
            });
        }
    }

    /// Editable routing grid shown when a slot's algorithm is Custom: one row
    /// per operator with a carrier toggle and a checkbox for each legal
    /// higher-index modulator (op 3's self-modulation is the Feedback knob).
    fn fm_custom_routing_grid(&mut self, ui: &mut egui::Ui, slot: usize) {
        ui.label(
            egui::RichText::new("Custom routing — carrier + modulators per operator")
                .color(theme::FG2)
                .font(theme::font_small()),
        );
        egui::Grid::new(format!("fm_custom_grid_{slot}"))
            .spacing([6.0, 4.0])
            .show(ui, |ui| {
                for dest in 0..4usize {
                    // Carrier toggle.
                    let mut carrier = self.fm_custom_carrier[slot][dest];
                    if ui
                        .checkbox(&mut carrier, "Carrier")
                        .on_hover_text("Operator output is summed into the slot's audio")
                        .changed()
                    {
                        self.fm_custom_carrier[slot][dest] = carrier;
                        self.emit_change(EngineEvent::ParameterChange {
                            id: ParamId::FmCustomCarrier(slot as u8 * 4 + dest as u8),
                            value: if carrier { 1.0 } else { 0.0 },
                        });
                    }

                    ui.label(
                        egui::RichText::new(format!("Op {}", dest + 1))
                            .color(theme::FG1)
                            .font(theme::font_small()),
                    );

                    if dest == 3 {
                        ui.label(
                            egui::RichText::new("(feedback via FB knob)")
                                .color(theme::FG2)
                                .font(theme::font_small()),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("mod by")
                                .color(theme::FG2)
                                .font(theme::font_small()),
                        );
                        ui.horizontal(|ui| {
                            for (conn_idx, &(src, conn_dest)) in FM_CUSTOM_CONN_TABLE.iter().enumerate() {
                                if conn_dest != dest {
                                    continue;
                                }
                                let mut on = self.fm_custom_conn[slot][conn_idx];
                                if ui.checkbox(&mut on, format!("Op{}", src + 1)).changed() {
                                    self.fm_custom_conn[slot][conn_idx] = on;
                                    self.emit_change(EngineEvent::ParameterChange {
                                        id: ParamId::FmCustomConn(slot as u8 * 6 + conn_idx as u8),
                                        value: if on { 1.0 } else { 0.0 },
                                    });
                                }
                            }
                        });
                    }
                    ui.end_row();
                }
            });
    }
}
