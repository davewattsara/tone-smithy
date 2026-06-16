use eframe::egui;
use synth_engine::{EngineEvent, ParamId, ParamSnapshot, SEQ_MAX_STEPS};

use crate::app::ToneSmithyApp;
use crate::knob::Knob;
use crate::theme;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn seq_tab(&mut self, ui: &mut egui::Ui, snapshot: &ParamSnapshot) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "STEP SEQUENCER");

        if ui
            .add(Toggle::new(&mut self.seq_enabled, "Enabled").param_key("seq_enabled"))
            .changed()
        {
            // Mutually exclusive with the arp: the engine forces the arp off
            // when the sequencer turns on, so mirror that locally too (the
            // snapshot only re-syncs the toggles on preset load).
            if self.seq_enabled {
                self.arp_enabled = false;
            }
            self.events.send(EngineEvent::ParameterChange {
                id: ParamId::SeqEnabled,
                value: if self.seq_enabled { 1.0 } else { 0.0 },
            });
        }

        ui.label(
            egui::RichText::new("Sequencer and arpeggiator are mutually exclusive — enabling one disables the other. Tempo follows the Master-tab BPM.")
                .color(theme::FG2)
                .font(theme::font_micro()),
        );

        ui.add_space(theme::GROUP_GAP);

        ui.add_enabled_ui(self.seq_enabled, |ui| {
            self.seq_transport_row(ui);
            ui.add_space(theme::GROUP_GAP);
            theme::subtle_separator(ui);
            ui.add_space(theme::GROUP_GAP);
            self.seq_step_grid(ui, snapshot);
        });
    }

    /// Length / rate / mode / swing controls. BPM lives in the Master tab.
    fn seq_transport_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Length")
                    .color(theme::FG1)
                    .font(theme::font_small()),
            );
            let mut len = self.seq_length;
            if ui
                .add(egui::DragValue::new(&mut len).range(1..=SEQ_MAX_STEPS as u8).speed(0.1))
                .changed()
            {
                self.seq_length = len;
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SeqLength,
                    value: f32::from(self.seq_length),
                });
            }

            ui.add_space(theme::GROUP_GAP);

            ui.label(egui::RichText::new("Rate").color(theme::FG1).font(theme::font_small()));
            let rate_labels = ["1/32", "1/16", "1/8", "1/4", "1/2"];
            egui::ComboBox::from_id_salt("seq_rate")
                .selected_text(rate_labels[(self.seq_rate as usize).min(4)])
                .show_ui(ui, |ui| {
                    for (i, label) in rate_labels.iter().enumerate() {
                        if ui.selectable_value(&mut self.seq_rate, i as u8, *label).changed() {
                            self.events.send(EngineEvent::ParameterChange {
                                id: ParamId::SeqRate,
                                value: self.seq_rate as f32,
                            });
                        }
                    }
                });

            ui.add_space(theme::GROUP_GAP);

            ui.label(egui::RichText::new("Mode").color(theme::FG1).font(theme::font_small()));
            let mode_labels = ["Fwd", "Rev", "Ping", "Rand"];
            egui::ComboBox::from_id_salt("seq_mode")
                .selected_text(mode_labels[(self.seq_mode as usize).min(3)])
                .show_ui(ui, |ui| {
                    for (i, label) in mode_labels.iter().enumerate() {
                        if ui.selectable_value(&mut self.seq_mode, i as u8, *label).changed() {
                            self.events.send(EngineEvent::ParameterChange {
                                id: ParamId::SeqMode,
                                value: self.seq_mode as f32,
                            });
                        }
                    }
                });

            ui.add_space(theme::GROUP_GAP);

            if ui
                .add(
                    Knob::new(&mut self.seq_swing, 0.5..=0.75, "Swing")
                        .default_value(0.5)
                        .param_key("seq_swing")
                        .format(|v| format!("{:.0}%", (v - 0.5) / 0.25 * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SeqSwing,
                    value: self.seq_swing,
                });
            }
        });
    }

    /// The 16-column step grid: per step a note offset, velocity, gate, mod
    /// value, and rest toggle. The active step (playhead) is highlighted.
    fn seq_step_grid(&mut self, ui: &mut egui::Ui, snapshot: &ParamSnapshot) {
        let length = (self.seq_length as usize).clamp(1, SEQ_MAX_STEPS);
        let playhead = snapshot.seq_current_step; // -1 when idle

        ui.horizontal_top(|ui| {
            for i in 0..SEQ_MAX_STEPS {
                let in_range = i < length;
                let is_playhead = i32::from(playhead) == i as i32;
                // A step is "consumed" when the previous step (index order,
                // wrapping within the active range) ties its note forward into
                // it: this step does not articulate, so its note/velocity/gate
                // and rest do nothing. Its mod lane and tie toggle still matter.
                let consumed = in_range && length > 1 && self.seq_step_tie[(i + length - 1) % length];

                ui.vertical(|ui| {
                    ui.set_width(38.0);

                    // Step number / playhead indicator.
                    let num_color = if is_playhead {
                        theme::ACCENT
                    } else if in_range {
                        theme::FG1
                    } else {
                        theme::FG2
                    };
                    ui.label(
                        egui::RichText::new(format!("{:>2}", i + 1))
                            .color(num_color)
                            .font(theme::font_small()),
                    );

                    ui.add_enabled_ui(in_range, |ui| {
                        self.seq_step_column(ui, i, consumed);
                    });
                });

                if i + 1 < SEQ_MAX_STEPS {
                    ui.add_space(2.0);
                }
            }
        });

        ui.add_space(theme::GROUP_GAP);
        ui.label(
            egui::RichText::new(
                "Per step: note offset (semitones from lowest held note), velocity, gate, mod lane, rest (R), and tie (T = extend this note into the next step).",
            )
            .color(theme::FG2)
            .font(theme::font_micro()),
        );
    }

    /// One step's stacked controls. When `consumed` is true the step's note is
    /// supplied by a tie from the previous step, so its note/velocity/gate and
    /// rest are greyed out (they have no effect); the mod lane and tie toggle
    /// stay active.
    fn seq_step_column(&mut self, ui: &mut egui::Ui, i: usize, consumed: bool) {
        // Note offset, velocity, and gate are dead on a consumed (tied-into)
        // step — disable them so it is clear they do nothing.
        ui.add_enabled_ui(!consumed, |ui| {
            // Note offset.
            let mut note = self.seq_step_note[i];
            if ui
                .add(egui::DragValue::new(&mut note).range(-24..=24).speed(0.15).prefix("n "))
                .on_disabled_hover_text("Consumed by a tie from the previous step")
                .changed()
            {
                self.seq_step_note[i] = note;
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SeqStepNote(i as u8),
                    value: f32::from(note),
                });
            }

            // Velocity.
            let mut vel = self.seq_step_velocity[i];
            if ui
                .add_sized(
                    [28.0, 56.0],
                    egui::Slider::new(&mut vel, 0..=127).vertical().show_value(false),
                )
                .on_hover_text("Velocity")
                .on_disabled_hover_text("Consumed by a tie from the previous step")
                .changed()
            {
                self.seq_step_velocity[i] = vel;
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SeqStepVelocity(i as u8),
                    value: f32::from(vel),
                });
            }

            // Gate.
            let mut gate = self.seq_step_gate[i];
            if ui
                .add_sized(
                    [28.0, 56.0],
                    egui::Slider::new(&mut gate, 0.0..=1.0).vertical().show_value(false),
                )
                .on_hover_text("Gate (scaled across the tie span for the originating step)")
                .on_disabled_hover_text("Consumed by a tie from the previous step")
                .changed()
            {
                self.seq_step_gate[i] = gate;
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SeqStepGate(i as u8),
                    value: gate,
                });
            }
        });

        // Mod lane (still advances on a consumed step, so it stays enabled).
        let mut modv = self.seq_step_mod[i];
        if ui
            .add_sized(
                [28.0, 56.0],
                egui::Slider::new(&mut modv, -1.0..=1.0).vertical().show_value(false),
            )
            .on_hover_text("Mod lane (Seq source)")
            .changed()
        {
            self.seq_step_mod[i] = modv;
            self.events.send(EngineEvent::ParameterChange {
                id: ParamId::SeqStepMod(i as u8),
                value: modv,
            });
        }

        // Rest (R) and tie (T) toggles, side by side. A tie extends *this*
        // step's note forward into the next step.
        ui.horizontal(|ui| {
            let rest = self.seq_step_rest[i];
            let rest_label = egui::RichText::new("R")
                .color(if rest { theme::WARN } else { theme::FG2 })
                .font(theme::font_small());
            // Rest is dead on a consumed step (it never articulates anyway).
            if ui
                .add_enabled(!consumed, egui::SelectableLabel::new(rest, rest_label))
                .on_hover_text("Rest (silent step)")
                .on_disabled_hover_text("Consumed by a tie from the previous step")
                .clicked()
            {
                self.seq_step_rest[i] = !rest;
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SeqStepRest(i as u8),
                    value: if self.seq_step_rest[i] { 1.0 } else { 0.0 },
                });
            }

            // Tie stays enabled even on a consumed step: toggling it extends the
            // run by one more step.
            let tie = self.seq_step_tie[i];
            let tie_label = egui::RichText::new("T")
                .color(if tie { theme::ACCENT } else { theme::FG2 })
                .font(theme::font_small());
            if ui
                .selectable_label(tie, tie_label)
                .on_hover_text("Tie (extend this step's note into the next step)")
                .clicked()
            {
                self.seq_step_tie[i] = !tie;
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SeqStepTie(i as u8),
                    value: if self.seq_step_tie[i] { 1.0 } else { 0.0 },
                });
            }
        });
    }
}
