use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::ToneSmithyApp;
use crate::knob::Knob;
use crate::theme;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn eq_section(&mut self, ui: &mut egui::Ui) {
        if ui
            .add(Toggle::new(&mut self.fx_eq_enabled, "EQ").param_key("fx_eq_enabled"))
            .changed()
        {
            self.events.send(EngineEvent::ParameterChange {
                id: ParamId::FxEqEnabled,
                value: if self.fx_eq_enabled { 1.0 } else { 0.0 },
            });
        }
        ui.add_enabled_ui(self.fx_eq_enabled, |ui| {
            ui.label(egui::RichText::new("Low").color(theme::FG1).font(theme::font_small()));
            if ui
                .add(
                    Knob::new(&mut self.fx_eq_low_gain_db, -15.0..=15.0, "Gain")
                        .default_value(0.0)
                        .param_key("fx_eq_low_gain_db")
                        .format(|v| format!("{:+.1} dB", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxEqLowGainDb,
                    value: self.fx_eq_low_gain_db,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_eq_low_freq_hz, 20.0..=2_000.0, "Freq")
                        .default_value(200.0)
                        .param_key("fx_eq_low_freq_hz")
                        .format(|v| format!("{:.0} Hz", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxEqLowFreqHz,
                    value: self.fx_eq_low_freq_hz,
                });
            }
            ui.label(egui::RichText::new("Mid").color(theme::FG1).font(theme::font_small()));
            if ui
                .add(
                    Knob::new(&mut self.fx_eq_mid_gain_db, -15.0..=15.0, "Gain")
                        .default_value(0.0)
                        .param_key("fx_eq_mid_gain_db")
                        .format(|v| format!("{:+.1} dB", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxEqMidGainDb,
                    value: self.fx_eq_mid_gain_db,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_eq_mid_freq_hz, 200.0..=8_000.0, "Freq")
                        .default_value(1_000.0)
                        .param_key("fx_eq_mid_freq_hz")
                        .format(|v| format!("{:.0} Hz", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxEqMidFreqHz,
                    value: self.fx_eq_mid_freq_hz,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_eq_mid_q, 0.1..=10.0, "Q")
                        .default_value(0.7)
                        .param_key("fx_eq_mid_q")
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxEqMidQ,
                    value: self.fx_eq_mid_q,
                });
            }
            ui.label(egui::RichText::new("High").color(theme::FG1).font(theme::font_small()));
            if ui
                .add(
                    Knob::new(&mut self.fx_eq_high_gain_db, -15.0..=15.0, "Gain")
                        .default_value(0.0)
                        .param_key("fx_eq_high_gain_db")
                        .format(|v| format!("{:+.1} dB", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxEqHighGainDb,
                    value: self.fx_eq_high_gain_db,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_eq_high_freq_hz, 2_000.0..=20_000.0, "Freq")
                        .default_value(6_000.0)
                        .param_key("fx_eq_high_freq_hz")
                        .format(|v| format!("{:.0} Hz", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxEqHighFreqHz,
                    value: self.fx_eq_high_freq_hz,
                });
            }
        });
    }
}
