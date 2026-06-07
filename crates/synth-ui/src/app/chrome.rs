use std::sync::atomic::Ordering;

use eframe::egui;
use synth_engine::{EngineEvent, ParamSnapshot};

use crate::theme;

use super::state::Tab;
use super::state::ToneSmithyApp;
use super::utils::midi_note_label;

impl ToneSmithyApp {
    pub(crate) fn header_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            ui.label(
                egui::RichText::new("Tone Smithy")
                    .color(theme::FG0)
                    .font(theme::font_display()),
            );
            ui.separator();
            ui.label(egui::RichText::new("Patch:").color(theme::FG1).font(theme::font_body()));
            ui.add(
                egui::TextEdit::singleline(&mut self.patch_name)
                    .desired_width(180.0)
                    .hint_text("Untitled")
                    .font(theme::font_body()),
            );
            if ui.button("Save").clicked() {
                self.save_preset();
            }
            if ui.button("Load").clicked() {
                self.load_preset();
            }
            ui.separator();
            ui.label(
                egui::RichText::new(&self.audio_status)
                    .color(theme::FG2)
                    .font(theme::font_micro()),
            );
        });
    }

    pub(crate) fn tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            for &(tab, label) in Tab::ALL {
                let selected = self.active_tab == tab;
                let text = egui::RichText::new(label).font(theme::font_body());
                let text = if selected {
                    text.color(theme::ACCENT)
                } else {
                    text.color(theme::FG1)
                };
                if ui.selectable_label(selected, text).clicked() {
                    self.active_tab = tab;
                }
                ui.add_space(4.0);
            }
        });
    }

    pub(crate) fn keyboard_strip(&mut self, ui: &mut egui::Ui) {
        // Keep virtual keyboard range in sync with computer keyboard octave.
        if let Some(stuck) = self.keyboard.set_start_note(self.computer_keyboard.octave_base()) {
            self.events.send(EngineEvent::NoteOff { note_midi: stuck });
        }
        let kb_notes = self.computer_keyboard.held_notes();

        ui.horizontal(|ui| {
            // Pitch-bend slider: springs to 0 on release
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("PB").color(theme::FG2).font(theme::font_micro()));
                let pb_r = ui.add(
                    egui::Slider::new(&mut self.pitch_bend, -1.0..=1.0)
                        .vertical()
                        .show_value(false),
                );
                if pb_r.changed() {
                    self.events.send(EngineEvent::PitchBend {
                        value_normalised: self.pitch_bend,
                    });
                }
                if !pb_r.is_pointer_button_down_on() && self.pitch_bend != 0.0 {
                    self.pitch_bend = 0.0;
                    self.events.send(EngineEvent::PitchBend { value_normalised: 0.0 });
                }
            });

            // Mod wheel slider: stays where left
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("MW").color(theme::FG2).font(theme::font_micro()));
                if ui
                    .add(
                        egui::Slider::new(&mut self.mod_wheel, 0.0..=1.0)
                            .vertical()
                            .show_value(false),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ControlChange {
                        cc: 1,
                        value_normalised: self.mod_wheel,
                    });
                }
            });

            // Virtual keyboard
            self.keyboard.show(ui, &self.events, kb_notes);

            // Sustain pedal
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("Sustain")
                        .color(theme::FG2)
                        .font(theme::font_micro()),
                );
                if ui
                    .selectable_label(self.sustain_held, if self.sustain_held { "ON " } else { "OFF" })
                    .clicked()
                {
                    self.sustain_held = !self.sustain_held;
                    self.events.send(EngineEvent::Sustain {
                        held: self.sustain_held,
                    });
                }
            });

            // Octave hint
            ui.label(
                egui::RichText::new(format!(
                    "Oct: {} ({})",
                    self.computer_keyboard.octave_base(),
                    midi_note_label(self.computer_keyboard.octave_base())
                ))
                .color(theme::FG2)
                .font(theme::font_micro()),
            );
        });
    }

    pub(crate) fn footer_bar(&self, ui: &mut egui::Ui, snapshot: &ParamSnapshot) {
        ui.horizontal_centered(|ui| {
            ui.add_space(theme::PANEL_PADDING);
            let cpu = f32::from_bits(self.cpu_load.load(Ordering::Relaxed));
            let text = format!(
                "CPU: {cpu:.1}%   Voices: {}   {}",
                snapshot.active_voice_count, self.audio_status
            );
            ui.label(egui::RichText::new(text).color(theme::FG2).font(theme::font_micro()));
        });
    }
}
