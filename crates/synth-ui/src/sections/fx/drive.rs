use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::ToneSmithyApp;
use crate::knob::Knob;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn drive_section(&mut self, ui: &mut egui::Ui) {
        if ui
            .add(Toggle::new(&mut self.fx_drive_enabled, "Drive").param_key("fx_drive_enabled"))
            .changed()
        {
            self.events.send(EngineEvent::ParameterChange {
                id: ParamId::FxDriveEnabled,
                value: if self.fx_drive_enabled { 1.0 } else { 0.0 },
            });
        }
        ui.add_enabled_ui(self.fx_drive_enabled, |ui| {
            if ui
                .add(
                    Knob::new(&mut self.fx_drive_drive, 1.0..=20.0, "Drive")
                        .default_value(1.0)
                        .param_key("fx_drive_drive")
                        .format(|v| format!("{:.1}x", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxDriveDrive,
                    value: self.fx_drive_drive,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.fx_drive_asymmetry, -1.0..=1.0, "Asym")
                        .default_value(0.0)
                        .param_key("fx_drive_asymmetry")
                        .format(|v| format!("{:+.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::FxDriveAsymmetry,
                    value: self.fx_drive_asymmetry,
                });
            }
        });
    }
}
