use eframe::egui;
use synth_engine::{EngineEvent, ParamId, ParamSnapshot};

use crate::app::{
    ENV_ATTACK_MAX_SECS, ENV_DECAY_MAX_SECS, ENV_MIN_SECS, ENV_RELEASE_MAX_SECS, ENV2_CURVE_RANGE, LFO_RATE_MAX_HZ,
    LFO_RATE_MIN_HZ, ToneSmithyApp, secs_format,
};
use crate::knob::Knob;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn envelopes_tab(&mut self, ui: &mut egui::Ui, snapshot: &ParamSnapshot) {
        ui.add_space(theme::PANEL_PADDING);
        ui.columns(4, |cols| {
            cols[0].vertical(|ui| {
                ui.add_space(theme::PANEL_PADDING);
                theme::section_label(ui, "AMP ENV");
                self.amp_env_section(ui);
            });
            cols[1].vertical(|ui| {
                ui.add_space(theme::PANEL_PADDING);
                theme::section_label(ui, "ENV 2");
                self.env2_section(ui, snapshot);
            });
            cols[2].vertical(|ui| {
                ui.add_space(theme::PANEL_PADDING);
                theme::section_label(ui, "LFO 1");
                self.lfo_section(ui, 1, snapshot);
            });
            cols[3].vertical(|ui| {
                ui.add_space(theme::PANEL_PADDING);
                theme::section_label(ui, "LFO 2");
                self.lfo_section(ui, 2, snapshot);
            });
        });
    }

    fn amp_env_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.amp_attack_secs, ENV_MIN_SECS..=ENV_ATTACK_MAX_SECS, "A")
                        .default_value(0.010)
                        .param_key("amp_attack_secs")
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpAttackSecs,
                    value: self.amp_attack_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_decay_secs, ENV_MIN_SECS..=ENV_DECAY_MAX_SECS, "D")
                        .default_value(0.200)
                        .param_key("amp_decay_secs")
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpDecaySecs,
                    value: self.amp_decay_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_sustain_level, 0.0..=1.0, "S")
                        .default_value(0.8)
                        .param_key("amp_sustain_level")
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpSustainLevel,
                    value: self.amp_sustain_level,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_release_secs, ENV_MIN_SECS..=ENV_RELEASE_MAX_SECS, "R")
                        .default_value(0.200)
                        .param_key("amp_release_secs")
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpReleaseSecs,
                    value: self.amp_release_secs,
                });
            }
        });
    }

    fn env2_section(&mut self, ui: &mut egui::Ui, snapshot: &ParamSnapshot) {
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.env2_attack_secs, ENV_MIN_SECS..=ENV_ATTACK_MAX_SECS, "A")
                        .default_value(0.010)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2AttackSecs,
                    value: self.env2_attack_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_decay_secs, ENV_MIN_SECS..=ENV_DECAY_MAX_SECS, "D")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2DecaySecs,
                    value: self.env2_decay_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_sustain_level, 0.0..=1.0, "S")
                        .default_value(0.8)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2SustainLevel,
                    value: self.env2_sustain_level,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_release_secs, ENV_MIN_SECS..=ENV_RELEASE_MAX_SECS, "R")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2ReleaseSecs,
                    value: self.env2_release_secs,
                });
            }
        });

        ui.add_space(4.0);
        ui.label(egui::RichText::new("Curve").color(theme::FG1).font(theme::font_small()));
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.env2_attack_curve, -ENV2_CURVE_RANGE..=ENV2_CURVE_RANGE, "A")
                        .default_value(0.0)
                        .format(|v| format!("{:+.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2AttackCurve,
                    value: self.env2_attack_curve,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_decay_curve, -ENV2_CURVE_RANGE..=ENV2_CURVE_RANGE, "D")
                        .default_value(0.0)
                        .format(|v| format!("{:+.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2DecayCurve,
                    value: self.env2_decay_curve,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.env2_release_curve, -ENV2_CURVE_RANGE..=ENV2_CURVE_RANGE, "R")
                        .default_value(0.0)
                        .format(|v| format!("{:+.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Env2ReleaseCurve,
                    value: self.env2_release_curve,
                });
            }
        });

        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Out: {:.3}", snapshot.env2_out)).color(theme::FG2));
    }

    fn lfo_section(&mut self, ui: &mut egui::Ui, lfo_num: u8, snapshot: &ParamSnapshot) {
        let (rate_id, shape_id, reset_id, sync_id, div_id) = if lfo_num == 1 {
            (
                ParamId::Lfo1RateHz,
                ParamId::Lfo1Shape,
                ParamId::Lfo1ResetOnNoteOn,
                ParamId::Lfo1SyncEnabled,
                ParamId::Lfo1SyncDivision,
            )
        } else {
            (
                ParamId::Lfo2RateHz,
                ParamId::Lfo2Shape,
                ParamId::Lfo2ResetOnNoteOn,
                ParamId::Lfo2SyncEnabled,
                ParamId::Lfo2SyncDivision,
            )
        };
        let mut rate_hz = if lfo_num == 1 {
            self.lfo1_rate_hz
        } else {
            self.lfo2_rate_hz
        };
        let mut shape_index = if lfo_num == 1 {
            self.lfo1_shape_index
        } else {
            self.lfo2_shape_index
        };
        let mut reset_on_note_on = if lfo_num == 1 {
            self.lfo1_reset_on_note_on
        } else {
            self.lfo2_reset_on_note_on
        };
        let mut sync_enabled = if lfo_num == 1 {
            self.lfo1_sync_enabled
        } else {
            self.lfo2_sync_enabled
        };
        let mut div_index = if lfo_num == 1 {
            self.lfo1_sync_division_index
        } else {
            self.lfo2_sync_division_index
        };
        let live_out = if lfo_num == 1 {
            snapshot.lfo1_out
        } else {
            snapshot.lfo2_out
        };
        let events = self.events.clone();

        const SHAPE_LABELS: [&str; 7] = ["Sin", "Tri", "Saw+", "Saw-", "Sq", "S&H", "Rnd"];
        ui.label(egui::RichText::new("Shape").color(theme::FG1).font(theme::font_small()));
        ui.horizontal_wrapped(|ui| {
            for (i, label) in SHAPE_LABELS.iter().enumerate() {
                if ui.selectable_label(shape_index == i, *label).clicked() {
                    shape_index = i;
                    events.send(EngineEvent::ParameterChange {
                        id: shape_id,
                        value: i as f32,
                    });
                }
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let lfo_rate_key = if lfo_num == 1 { "lfo1_rate_hz" } else { "lfo2_rate_hz" };
            if !sync_enabled
                && ui
                    .add(
                        Knob::new(&mut rate_hz, LFO_RATE_MIN_HZ..=LFO_RATE_MAX_HZ, "Rate")
                            .default_value(1.0)
                            .param_key(lfo_rate_key)
                            .format(|v| format!("{:.2} Hz", v)),
                    )
                    .changed()
            {
                events.send(EngineEvent::ParameterChange {
                    id: rate_id,
                    value: rate_hz,
                });
            }
            if ui.selectable_label(reset_on_note_on, "Reset").clicked() {
                reset_on_note_on = !reset_on_note_on;
                events.send(EngineEvent::ParameterChange {
                    id: reset_id,
                    value: if reset_on_note_on { 1.0 } else { 0.0 },
                });
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.selectable_label(sync_enabled, "Sync").clicked() {
                sync_enabled = !sync_enabled;
                events.send(EngineEvent::ParameterChange {
                    id: sync_id,
                    value: if sync_enabled { 1.0 } else { 0.0 },
                });
            }
            if sync_enabled {
                const DIV_LABELS: [&str; 8] = ["1/32", "1/16", "1/8", "1/4", "1/2", "1", "2", "4"];
                for (i, label) in DIV_LABELS.iter().enumerate() {
                    if ui.selectable_label(div_index == i, *label).clicked() {
                        div_index = i;
                        events.send(EngineEvent::ParameterChange {
                            id: div_id,
                            value: i as f32,
                        });
                    }
                }
            }
        });

        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Out: {:.3}", live_out)).color(theme::FG2));

        if lfo_num == 1 {
            self.lfo1_rate_hz = rate_hz;
            self.lfo1_shape_index = shape_index;
            self.lfo1_reset_on_note_on = reset_on_note_on;
            self.lfo1_sync_enabled = sync_enabled;
            self.lfo1_sync_division_index = div_index;
        } else {
            self.lfo2_rate_hz = rate_hz;
            self.lfo2_shape_index = shape_index;
            self.lfo2_reset_on_note_on = reset_on_note_on;
            self.lfo2_sync_enabled = sync_enabled;
            self.lfo2_sync_division_index = div_index;
        }
    }
}
