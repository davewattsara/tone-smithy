use eframe::egui;
use synth_engine::{EngineEvent, ParamId, ParamSnapshot};

use crate::app::{BPM_MAX, BPM_MIN, PITCH_OFFSET_RANGE, ToneSmithyApp};
use crate::knob::Knob;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn master_tab(&mut self, ui: &mut egui::Ui, snapshot: &ParamSnapshot) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "MASTER");

        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.master_volume, 0.0..=1.0, "Volume")
                        .default_value(0.8)
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::MasterVolume,
                    value: self.master_volume,
                });
            }

            if ui
                .add(
                    Knob::new(
                        &mut self.pitch_offset_semis,
                        -PITCH_OFFSET_RANGE..=PITCH_OFFSET_RANGE,
                        "Pitch",
                    )
                    .default_value(0.0)
                    .format(|v| format!("{:+.0} st", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::PitchOffsetSemis,
                    value: self.pitch_offset_semis,
                });
            }

            if ui
                .add(
                    Knob::new(&mut self.bpm, BPM_MIN..=BPM_MAX, "BPM")
                        .default_value(120.0)
                        .format(|v| format!("{:.0}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Bpm,
                    value: self.bpm,
                });
            }
        });

        ui.add_space(theme::GROUP_GAP);

        // Voice / modulator status
        ui.label(
            egui::RichText::new(format!(
                "Voices: {}   LFO1: {:.3}   LFO2: {:.3}   Env2: {:.3}",
                snapshot.active_voice_count, snapshot.lfo1_out, snapshot.lfo2_out, snapshot.env2_out,
            ))
            .color(theme::FG2)
            .font(theme::font_small()),
        );

        ui.add_space(theme::GROUP_GAP);

        // Pitch-bend / mod-wheel / sustain controls
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Pitch Bend").color(theme::FG1));
                let old = self.pitch_bend;
                if ui
                    .add(egui::Slider::new(&mut self.pitch_bend, -1.0..=1.0).show_value(false))
                    .changed()
                {
                    self.events.send(EngineEvent::PitchBend {
                        value_normalised: self.pitch_bend,
                    });
                }
                // Spring-back when released
                if ui.input(|i| i.pointer.any_released()) && old != 0.0 {
                    self.pitch_bend = 0.0;
                    self.events.send(EngineEvent::PitchBend { value_normalised: 0.0 });
                }
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Mod Wheel").color(theme::FG1));
                if ui
                    .add(egui::Slider::new(&mut self.mod_wheel, 0.0..=1.0).show_value(false).vertical())
                    .changed()
                {
                    self.events.send(EngineEvent::ControlChange {
                        cc: 1,
                        value_normalised: self.mod_wheel,
                    });
                }
            });
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Sustain").color(theme::FG1));
                let label = if self.sustain_held { "ON" } else { "OFF" };
                if ui.button(label).clicked() {
                    self.sustain_held = !self.sustain_held;
                    self.events.send(EngineEvent::Sustain { held: self.sustain_held });
                }
            });
        });
    }
}
