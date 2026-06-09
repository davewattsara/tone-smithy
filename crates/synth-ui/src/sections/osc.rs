use eframe::egui;
use synth_engine::{EngineEvent, ParamId, Waveform};

use crate::app::{
    ModDisplay, OSC_DETUNE_MAX_CENTS, OSC_LEVEL_MAX, ToneSmithyApp, UNISON_DETUNE_MAX_CENTS, UNISON_VOICES_MAX,
};
use crate::knob::Knob;
use crate::theme;

impl ToneSmithyApp {
    pub(crate) fn osc_tab(&mut self, ui: &mut egui::Ui, md: ModDisplay) {
        ui.add_space(theme::PANEL_PADDING);

        // Waveform selector applies to all three main oscillators
        ui.horizontal(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            ui.label(egui::RichText::new("Waveform").color(theme::FG1));
            ui.add_space(4.0);
            let mut changed = false;
            for w in [Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle] {
                let label = match w {
                    Waveform::Sine => "Sine",
                    Waveform::Saw => "Saw",
                    Waveform::Square => "Sq",
                    Waveform::Triangle => "Tri",
                };
                if ui.selectable_value(&mut self.waveform, w, label).clicked() {
                    changed = true;
                }
            }
            if changed {
                self.events.send(EngineEvent::SetOscillatorWaveform {
                    waveform: self.waveform,
                });
            }
        });

        ui.add_space(theme::GROUP_GAP);

        // Three oscillator columns
        ui.columns(3, |cols| {
            cols[0].vertical(|ui| {
                ui.add_space(theme::PANEL_PADDING);
                theme::section_label(ui, "OSC 1");
                self.osc_controls(ui, 0, Some(md));
            });
            cols[1].vertical(|ui| {
                ui.add_space(theme::PANEL_PADDING);
                theme::section_label(ui, "OSC 2");
                self.osc_controls(ui, 1, None);
            });
            cols[2].vertical(|ui| {
                ui.add_space(theme::PANEL_PADDING);
                theme::section_label(ui, "OSC 3 + Sub");
                self.osc_controls(ui, 2, None);
                ui.add_space(theme::GROUP_GAP);
                theme::subtle_separator(ui);
                ui.add_space(4.0);
                self.sub_controls(ui);
            });
        });

        // Slots / FM section — full width below the oscillator columns
        ui.add_space(theme::GROUP_GAP);
        theme::subtle_separator(ui);
        ui.add_space(theme::GROUP_GAP);
        theme::section_label(ui, "SLOTS / FM");
        self.fm_slots_section(ui);
    }

    /// Renders level/detune/pan + unison knobs for main oscillator `idx` (0, 1, or 2).
    /// `md` is `Some` only for osc 0 (osc1), which is the only oscillator currently
    /// addressable as a mod matrix destination.
    fn osc_controls(&mut self, ui: &mut egui::Ui, idx: usize, md: Option<ModDisplay>) {
        let (level_id, detune_id, pan_id, uv_id, ud_id, us_id) = match idx {
            0 => (
                ParamId::Osc1Level,
                ParamId::Osc1DetuneCents,
                ParamId::Osc1Pan,
                ParamId::Osc1UnisonVoices,
                ParamId::Osc1UnisonDetuneCents,
                ParamId::Osc1UnisonSpread,
            ),
            1 => (
                ParamId::Osc2Level,
                ParamId::Osc2DetuneCents,
                ParamId::Osc2Pan,
                ParamId::Osc2UnisonVoices,
                ParamId::Osc2UnisonDetuneCents,
                ParamId::Osc2UnisonSpread,
            ),
            _ => (
                ParamId::Osc3Level,
                ParamId::Osc3DetuneCents,
                ParamId::Osc3Pan,
                ParamId::Osc3UnisonVoices,
                ParamId::Osc3UnisonDetuneCents,
                ParamId::Osc3UnisonSpread,
            ),
        };

        let osc_level_key = ["osc1_level", "osc2_level", "osc3_level"][idx];
        let osc_detune_key = ["osc1_detune_cents", "osc2_detune_cents", "osc3_detune_cents"][idx];
        let osc_pan_key = ["osc1_pan", "osc2_pan", "osc3_pan"][idx];
        let osc_uvoices_key = ["osc1_unison_voices", "osc2_unison_voices", "osc3_unison_voices"][idx];
        let osc_udetune_key = [
            "osc1_unison_detune_cents",
            "osc2_unison_detune_cents",
            "osc3_unison_detune_cents",
        ][idx];
        let osc_uspread_key = ["osc1_unison_spread", "osc2_unison_spread", "osc3_unison_spread"][idx];
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.osc_level[idx], 0.0..=OSC_LEVEL_MAX, "Level")
                        .default_value(1.0)
                        .param_key(osc_level_key)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: level_id,
                    value: self.osc_level[idx],
                });
            }
            if ui
                .add(
                    Knob::new(
                        &mut self.osc_detune_cents[idx],
                        -OSC_DETUNE_MAX_CENTS..=OSC_DETUNE_MAX_CENTS,
                        "Detune",
                    )
                    .default_value(0.0)
                    .mod_offset(md.map_or(0.0, |m| m.osc1_detune))
                    .param_key(osc_detune_key)
                    .format(|v| format!("{:+.1} ct", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: detune_id,
                    value: self.osc_detune_cents[idx],
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.osc_pan[idx], -1.0..=1.0, "Pan")
                        .default_value(0.0)
                        .mod_offset(md.map_or(0.0, |m| m.osc1_pan))
                        .param_key(osc_pan_key)
                        .format(|v| {
                            if v < -0.01 {
                                format!("L{:.0}", v.abs() * 100.0)
                            } else if v > 0.01 {
                                format!("R{:.0}", v * 100.0)
                            } else {
                                "C".to_string()
                            }
                        }),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: pan_id,
                    value: self.osc_pan[idx],
                });
            }
        });

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Unison").color(theme::FG1));
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.osc_unison_voices[idx], 1.0..=UNISON_VOICES_MAX, "Voices")
                        .default_value(1.0)
                        .param_key(osc_uvoices_key)
                        .format(|v| format!("{}", v.round() as u8)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: uv_id,
                    value: self.osc_unison_voices[idx],
                });
            }
            if ui
                .add(
                    Knob::new(
                        &mut self.osc_unison_detune_cents[idx],
                        0.0..=UNISON_DETUNE_MAX_CENTS,
                        "Detune",
                    )
                    .default_value(10.0)
                    .param_key(osc_udetune_key)
                    .format(|v| format!("{:.1} ct", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ud_id,
                    value: self.osc_unison_detune_cents[idx],
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.osc_unison_spread[idx], 0.0..=1.0, "Spread")
                        .default_value(0.5)
                        .param_key(osc_uspread_key)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: us_id,
                    value: self.osc_unison_spread[idx],
                });
            }
        });
    }

    fn sub_controls(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Sub Osc").color(theme::FG1));
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.sub_level, 0.0..=OSC_LEVEL_MAX, "Level")
                        .default_value(0.0)
                        .param_key("sub_level")
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SubLevel,
                    value: self.sub_level,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.sub_pan, -1.0..=1.0, "Pan")
                        .default_value(0.0)
                        .format(|v| {
                            if v < -0.01 {
                                format!("L{:.0}", v.abs() * 100.0)
                            } else if v > 0.01 {
                                format!("R{:.0}", v * 100.0)
                            } else {
                                "C".to_string()
                            }
                        }),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::SubPan,
                    value: self.sub_pan,
                });
            }
        });
    }
}
