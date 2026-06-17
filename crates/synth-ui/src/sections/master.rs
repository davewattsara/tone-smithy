use eframe::egui;
use synth_engine::{EngineEvent, ParamId, ParamSnapshot};

use crate::app::{BPM_MAX, BPM_MIN, ModDisplay, PITCH_OFFSET_RANGE, ToneSmithyApp};
use crate::knob::Knob;
use crate::meter::VuMeter;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn master_tab(&mut self, ui: &mut egui::Ui, snapshot: &ParamSnapshot, md: ModDisplay) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "MASTER");

        // VU meter + main controls side by side
        ui.horizontal(|ui| {
            ui.add(VuMeter::new(snapshot.vu_peak_left, snapshot.vu_peak_right));
            ui.add_space(theme::GROUP_GAP);
            ui.vertical(|ui| {
                self.master_knobs(ui, snapshot, md);
            });
        });
    }

    fn master_knobs(&mut self, ui: &mut egui::Ui, snapshot: &ParamSnapshot, md: ModDisplay) {
        // Main controls row
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.master_volume, 0.0..=1.0, "Volume")
                        .default_value(0.8)
                        .mod_offset(md.volume)
                        .param_key("master_volume")
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.emit_change(EngineEvent::ParameterChange {
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
                    .mod_offset(md.pitch)
                    .param_key("pitch_offset_semis")
                    .format(|v| format!("{:+.0} st", v)),
                )
                .changed()
            {
                self.emit_change(EngineEvent::ParameterChange {
                    id: ParamId::PitchOffsetSemis,
                    value: self.pitch_offset_semis,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.bpm, BPM_MIN..=BPM_MAX, "BPM")
                        .default_value(120.0)
                        .param_key("bpm")
                        .format(|v| format!("{:.0}", v)),
                )
                .changed()
            {
                self.emit_change(EngineEvent::ParameterChange {
                    id: ParamId::Bpm,
                    value: self.bpm,
                });
            }
        });

        ui.add_space(theme::GROUP_GAP);
        theme::subtle_separator(ui);
        ui.add_space(theme::GROUP_GAP);

        // Live status readout
        theme::section_label(ui, "STATUS");
        ui.horizontal(|ui| {
            status_chip(ui, "Voices", &snapshot.active_voice_count.to_string());
            ui.add_space(8.0);
            status_chip(ui, "LFO 1", &format!("{:+.3}", snapshot.lfo1_out));
            ui.add_space(8.0);
            status_chip(ui, "LFO 2", &format!("{:+.3}", snapshot.lfo2_out));
            ui.add_space(8.0);
            status_chip(ui, "Env 2", &format!("{:.3}", snapshot.env2_out));
        });
    }
}

/// Renders a small `label: value` status pair.
fn status_chip(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(
        egui::RichText::new(format!("{label}:"))
            .color(theme::FG2)
            .font(theme::font_small()),
    );
    ui.label(egui::RichText::new(value).color(theme::FG1).font(theme::font_small()));
}
