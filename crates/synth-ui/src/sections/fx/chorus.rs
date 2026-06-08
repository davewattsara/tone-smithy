use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::ToneSmithyApp;
use crate::knob::Knob;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn chorus_section(&mut self, ui: &mut egui::Ui) {
        if ui.add(Toggle::new(&mut self.fx_chorus_enabled, "Chorus")).changed() {
            self.events.send(EngineEvent::ParameterChange {
                id: ParamId::FxChorusEnabled,
                value: if self.fx_chorus_enabled { 1.0 } else { 0.0 },
            });
        }
        ui.add_enabled_ui(self.fx_chorus_enabled, |ui| {
            if ui
                .add(
                    Knob::new(&mut self.fx_chorus_rate_hz, 0.1..=8.0, "Rate")
                        .default_value(0.5)
                        .format(|v| format!("{:.2} Hz", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxChorusRateHz,
                    value: self.fx_chorus_rate_hz,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_chorus_depth_ms, 0.0..=15.0, "Depth")
                        .default_value(3.0)
                        .format(|v| format!("{:.1} ms", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxChorusDepthMs,
                    value: self.fx_chorus_depth_ms,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_chorus_mix, 0.0..=1.0, "Mix")
                        .default_value(0.5)
                        .param_key("fx_chorus_mix")
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxChorusMix,
                    value: self.fx_chorus_mix,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_chorus_spread, 0.0..=1.0, "Spread")
                        .default_value(0.5)
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxChorusSpread,
                    value: self.fx_chorus_spread,
                });
            }
        });
    }
}
