use eframe::egui;
use synth_engine::{EngineEvent, FilterMode, FilterRouting, FilterSlope, ParamId};

use crate::app::{CUTOFF_MAX_HZ, CUTOFF_MIN_HZ, ModDisplay, ToneSmithyApp};
use crate::knob::Knob;
use crate::theme;

/// The four filter modes: short label and tooltip, shared by both mode selectors.
const FILTER_MODES: [(FilterMode, &str, &str); 4] = [
    (
        FilterMode::LowPass,
        "LP",
        "Low-pass: attenuates frequencies above the cutoff.",
    ),
    (
        FilterMode::HighPass,
        "HP",
        "High-pass: attenuates frequencies below the cutoff.",
    ),
    (
        FilterMode::BandPass,
        "BP",
        "Band-pass: passes a band around the cutoff; attenuates above and below.",
    ),
    (
        FilterMode::Notch,
        "Notch",
        "Notch: cuts a narrow band at the cutoff, passes the rest.",
    ),
];

impl ToneSmithyApp {
    pub(crate) fn filter_tab(&mut self, ui: &mut egui::Ui, md: ModDisplay) {
        ui.add_space(theme::PANEL_PADDING);
        ui.add_space(theme::PANEL_PADDING);
        theme::section_label(ui, "FILTER 1");

        // Filter 1 mode selector
        ui.label(egui::RichText::new("Mode").color(theme::FG1).font(theme::font_body()));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut changed = false;
            for (m, label, tip) in FILTER_MODES {
                if ui
                    .selectable_value(&mut self.filter_mode, m, label)
                    .on_hover_text(tip)
                    .clicked()
                {
                    changed = true;
                }
            }
            if changed {
                self.emit_change(EngineEvent::SetFilterMode { mode: self.filter_mode });
            }
        });

        ui.add_space(theme::GROUP_GAP);

        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.filter_cutoff_hz, CUTOFF_MIN_HZ..=CUTOFF_MAX_HZ, "Cutoff")
                        .default_value(8_000.0)
                        .mod_offset(md.cutoff)
                        .param_key("filter_cutoff_hz")
                        .format(cutoff_format),
                )
                .changed()
            {
                self.emit_change(EngineEvent::ParameterChange {
                    id: ParamId::FilterCutoffHz,
                    value: self.filter_cutoff_hz,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.filter_resonance, 0.0..=1.0, "Res")
                        .default_value(0.0)
                        .mod_offset(md.resonance)
                        .param_key("filter_resonance")
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.emit_change(EngineEvent::ParameterChange {
                    id: ParamId::FilterResonance,
                    value: self.filter_resonance,
                });
            }
        });

        ui.add_space(4.0);
        self.slope_selector(ui, 0);

        ui.add_space(theme::GROUP_GAP);

        // Routing between the two filters
        ui.label(
            egui::RichText::new("Routing")
                .color(theme::FG1)
                .font(theme::font_body()),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let mut changed = false;
            // Off first: it is the default, and selecting it bypasses filter 2.
            for (r, label, tip) in [
                (
                    FilterRouting::Off,
                    "Off",
                    "Filter 2 is bypassed — only Filter 1 is active.",
                ),
                (
                    FilterRouting::Serial,
                    "Series",
                    "Signal passes through Filter 1 then Filter 2.",
                ),
                (
                    FilterRouting::Parallel,
                    "Parallel",
                    "Both filters run independently; their outputs are summed.",
                ),
            ] {
                if ui
                    .selectable_value(&mut self.filter_routing, r, label)
                    .on_hover_text(tip)
                    .clicked()
                {
                    changed = true;
                }
            }
            if changed {
                self.emit_change(EngineEvent::SetFilterRouting {
                    routing: self.filter_routing,
                });
            }
        });

        ui.add_space(theme::GROUP_GAP);
        theme::section_label(ui, "FILTER 2");

        // Filter 2 is bypassed when routing is Off, so grey out its controls.
        let f2_enabled = self.filter_routing != FilterRouting::Off;
        ui.add_enabled_ui(f2_enabled, |ui| {
            // Filter 2 mode selector
            ui.label(egui::RichText::new("Mode").color(theme::FG1).font(theme::font_body()));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut changed = false;
                for (m, label, tip) in FILTER_MODES {
                    if ui
                        .selectable_value(&mut self.filter2_mode, m, label)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        changed = true;
                    }
                }
                if changed {
                    self.emit_change(EngineEvent::SetFilter2Mode {
                        mode: self.filter2_mode,
                    });
                }
            });

            ui.add_space(theme::GROUP_GAP);

            ui.horizontal(|ui| {
                if ui
                    .add(
                        Knob::new(&mut self.filter2_cutoff_hz, CUTOFF_MIN_HZ..=CUTOFF_MAX_HZ, "Cutoff")
                            .default_value(20_000.0)
                            .mod_offset(md.filter2_cutoff)
                            .param_key("filter2_cutoff_hz")
                            .format(cutoff_format),
                    )
                    .changed()
                {
                    self.emit_change(EngineEvent::ParameterChange {
                        id: ParamId::Filter2CutoffHz,
                        value: self.filter2_cutoff_hz,
                    });
                }
                if ui
                    .add(
                        Knob::new(&mut self.filter2_resonance, 0.0..=1.0, "Res")
                            .default_value(0.0)
                            .mod_offset(md.filter2_resonance)
                            .param_key("filter2_resonance")
                            .format(|v| format!("{:.2}", v)),
                    )
                    .changed()
                {
                    self.emit_change(EngineEvent::ParameterChange {
                        id: ParamId::Filter2Resonance,
                        value: self.filter2_resonance,
                    });
                }
            });

            ui.add_space(4.0);
            self.slope_selector(ui, 1);
        });
    }

    /// Renders the 12 / 24 dB-per-octave slope toggle for one filter and
    /// emits a `SetFilterSlope` event on change. `filter_idx` is 0 or 1.
    fn slope_selector(&mut self, ui: &mut egui::Ui, filter_idx: usize) {
        let current = &mut self.filter_slope[filter_idx];
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Slope").color(theme::FG1).font(theme::font_small()));
            for (slope, label, tip) in [
                (
                    FilterSlope::TwelveDbOct,
                    "12 dB",
                    "12 dB/oct rolloff — gentler, more musical.",
                ),
                (
                    FilterSlope::TwentyFourDbOct,
                    "24 dB",
                    "24 dB/oct rolloff — steeper, classic Moog-style character.",
                ),
            ] {
                if ui.selectable_value(current, slope, label).on_hover_text(tip).clicked() {
                    changed = true;
                }
            }
        });
        if changed {
            self.emit_change(EngineEvent::SetFilterSlope {
                filter_idx: filter_idx as u8,
                slope: self.filter_slope[filter_idx],
            });
        }
    }
}

/// Shared cutoff-knob value formatter: kHz above 1 kHz, Hz below.
fn cutoff_format(v: f32) -> String {
    if v >= 1_000.0 {
        format!("{:.1} kHz", v / 1000.0)
    } else {
        format!("{:.0} Hz", v)
    }
}
