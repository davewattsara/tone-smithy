use eframe::egui;
use synth_engine::{EngineEvent, ParamId};

use crate::app::{ToneSmithyApp, secs_format};
use crate::knob::Knob;
use crate::theme;
use crate::toggle::Toggle;

impl ToneSmithyApp {
    pub(crate) fn fx_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "FX CHAIN");
        ui.add_space(4.0);

        ui.columns(5, |cols| {
            cols[0].vertical(|ui| self.eq_section(ui));
            cols[1].vertical(|ui| self.drive_section(ui));
            cols[2].vertical(|ui| self.chorus_section(ui));
            cols[3].vertical(|ui| self.delay_section(ui));
            cols[4].vertical(|ui| self.reverb_section(ui));
        });
    }

    fn eq_section(&mut self, ui: &mut egui::Ui) {
        if ui.add(Toggle::new(&mut self.fx_eq_enabled, "EQ")).changed() {
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

    fn drive_section(&mut self, ui: &mut egui::Ui) {
        if ui.add(Toggle::new(&mut self.fx_drive_enabled, "Drive")).changed() {
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

    fn chorus_section(&mut self, ui: &mut egui::Ui) {
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

    fn delay_section(&mut self, ui: &mut egui::Ui) {
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

    fn reverb_section(&mut self, ui: &mut egui::Ui) {
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
