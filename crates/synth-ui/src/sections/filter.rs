use eframe::egui;
use synth_engine::{EngineEvent, FilterMode, ParamId};

use crate::app::{CUTOFF_MAX_HZ, CUTOFF_MIN_HZ, ToneSmithyApp};
use crate::knob::Knob;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn filter_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "FILTER");

        // Mode selector
        ui.label(egui::RichText::new("Mode").color(theme::FG1).font(theme::font_body()));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut changed = false;
            for m in [
                FilterMode::LowPass,
                FilterMode::HighPass,
                FilterMode::BandPass,
                FilterMode::Notch,
            ] {
                let label = match m {
                    FilterMode::LowPass => "LP",
                    FilterMode::HighPass => "HP",
                    FilterMode::BandPass => "BP",
                    FilterMode::Notch => "Notch",
                };
                if ui.selectable_value(&mut self.filter_mode, m, label).clicked() {
                    changed = true;
                }
            }
            if changed {
                self.events.send(EngineEvent::SetFilterMode { mode: self.filter_mode });
            }
        });

        ui.add_space(theme::GROUP_GAP);

        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.filter_cutoff_hz, CUTOFF_MIN_HZ..=CUTOFF_MAX_HZ, "Cutoff")
                        .default_value(8_000.0)
                        .format(|v| {
                            if v >= 1_000.0 {
                                format!("{:.1} kHz", v / 1000.0)
                            } else {
                                format!("{:.0} Hz", v)
                            }
                        }),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FilterCutoffHz,
                    value: self.filter_cutoff_hz,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.filter_resonance, 0.0..=1.0, "Res")
                        .default_value(0.0)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FilterResonance,
                    value: self.filter_resonance,
                });
            }
        });
    }
}
