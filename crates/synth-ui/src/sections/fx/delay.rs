use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::{ToneSmithyApp, secs_format};
use crate::knob::Knob;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn delay_section(&mut self, ui: &mut egui::Ui) {
        if ui.add(Toggle::new(&mut self.fx_delay_enabled, "Delay")).changed() {
            self.events.send(EngineEvent::ParameterChange {
                id: ParamId::FxDelayEnabled,
                value: if self.fx_delay_enabled { 1.0 } else { 0.0 },
            });
        }
        ui.add_enabled_ui(self.fx_delay_enabled, |ui| {
            if ui
                .add(
                    Knob::new(&mut self.fx_delay_time_secs, 0.001..=2.0, "Time")
                        .default_value(0.375)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxDelayTimeSecs,
                    value: self.fx_delay_time_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_delay_feedback, 0.0..=0.95, "Fdbk")
                        .default_value(0.35)
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxDelayFeedback,
                    value: self.fx_delay_feedback,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_delay_mix, 0.0..=1.0, "Mix")
                        .default_value(0.30)
                        .param_key("fx_delay_mix")
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxDelayMix,
                    value: self.fx_delay_mix,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_delay_lowcut_hz, 20.0..=2_000.0, "LoCut")
                        .default_value(200.0)
                        .format(|v| format!("{:.0} Hz", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxDelayLowcutHz,
                    value: self.fx_delay_lowcut_hz,
                });
            }
            if ui.add(Toggle::new(&mut self.fx_delay_ping_pong, "Ping-pong")).changed() {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxDelayPingPong,
                    value: if self.fx_delay_ping_pong { 1.0 } else { 0.0 },
                });
            }
        });
    }
}
