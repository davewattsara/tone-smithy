use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::{ToneSmithyApp, secs_format};
use crate::knob::Knob;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn reverb_section(&mut self, ui: &mut egui::Ui) {
        if ui.add(Toggle::new(&mut self.fx_reverb_enabled, "Reverb")).changed() {
            self.events.send(EngineEvent::ParameterChange {
                id: ParamId::FxReverbEnabled,
                value: if self.fx_reverb_enabled { 1.0 } else { 0.0 },
            });
        }
        ui.add_enabled_ui(self.fx_reverb_enabled, |ui| {
            if ui
                .add(
                    Knob::new(&mut self.fx_reverb_predelay_ms, 0.0..=50.0, "Pre")
                        .default_value(10.0)
                        .format(|v| format!("{:.0} ms", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxReverbPredelayMs,
                    value: self.fx_reverb_predelay_ms,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_reverb_decay_secs, 0.1..=30.0, "Decay")
                        .default_value(2.0)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxReverbDecaySecs,
                    value: self.fx_reverb_decay_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_reverb_size, 0.1..=1.0, "Size")
                        .default_value(0.7)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxReverbSize,
                    value: self.fx_reverb_size,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_reverb_damping, 0.0..=1.0, "Damp")
                        .default_value(0.5)
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxReverbDamping,
                    value: self.fx_reverb_damping,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_reverb_mix, 0.0..=1.0, "Mix")
                        .default_value(0.25)
                        .format(|v| format!("{:.0}%", v * 100.0)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxReverbMix,
                    value: self.fx_reverb_mix,
                });
            }
        });
    }
}
