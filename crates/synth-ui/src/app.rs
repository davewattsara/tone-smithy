//! Top-level [`eframe::App`] implementation.
//!
//! M4: Three panels (Osc 1, Filter, Amp Envelope) plus master volume and a
//! status footer. All continuous parameters use the custom [`Knob`] widget.
//! Discrete parameters (waveform, filter mode) keep their button rows.
//! The virtual keyboard and computer-keyboard layer are unchanged from M3.
//!
//! [`Knob`]: crate::knob::Knob

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use eframe::egui;
use synth_engine::param_bus::{EngineEventSender, SnapshotSlot, load_snapshot};
use synth_engine::{EngineEvent, FilterMode, ParamId, Waveform};

use crate::computer_keyboard::ComputerKeyboard;
use crate::keyboard::VirtualKeyboard;
use crate::knob::Knob;

// ── Oscillator 1 range constants ────────────────────────────────────────────

const OSC_LEVEL_MAX: f32 = 1.0;
const OSC_DETUNE_MAX_CENTS: f32 = 100.0;
const UNISON_DETUNE_MAX_CENTS: f32 = 50.0;
const UNISON_VOICES_MAX: f32 = 7.0;

// ── Filter range constants ───────────────────────────────────────────────────

const CUTOFF_MIN_HZ: f32 = 20.0;
const CUTOFF_MAX_HZ: f32 = 20_000.0;

// ── Amp envelope range constants ─────────────────────────────────────────────

const ENV_MIN_SECS: f32 = 0.001;
const ENV_ATTACK_MAX_SECS: f32 = 10.0;
const ENV_DECAY_MAX_SECS: f32 = 10.0;
const ENV_RELEASE_MAX_SECS: f32 = 10.0;

// ── Pitch offset ─────────────────────────────────────────────────────────────

const PITCH_OFFSET_RANGE: f32 = 24.0;

/// The Tone Smithy application UI.
pub struct ToneSmithyApp {
    audio_status: String,
    events: EngineEventSender,
    snapshot_slot: SnapshotSlot,

    // ── Osc 1 mirrors ────────────────────────────────────────────────────────
    osc1_level: f32,
    osc1_detune_cents: f32,
    osc1_pan: f32,
    waveform: Waveform,
    osc1_unison_voices: f32,
    osc1_unison_detune_cents: f32,
    osc1_unison_spread: f32,

    // ── Filter mirrors ───────────────────────────────────────────────────────
    filter_mode: FilterMode,
    filter_cutoff_hz: f32,
    filter_resonance: f32,

    // ── Amp envelope mirrors ─────────────────────────────────────────────────
    amp_attack_secs: f32,
    amp_decay_secs: f32,
    amp_sustain_level: f32,
    amp_release_secs: f32,

    // ── Global ───────────────────────────────────────────────────────────────
    pitch_offset_semis: f32,
    master_volume: f32,

    // ── Input ────────────────────────────────────────────────────────────────
    keyboard: VirtualKeyboard,
    computer_keyboard: ComputerKeyboard,

    /// Pitch-bend wheel position, -1.0..=1.0. Snaps back to 0.0 when
    /// the user releases the slider.
    pitch_bend: f32,
    /// True while the on-screen sustain pedal button is toggled on.
    sustain_held: bool,

    /// CPU load arc from the audio thread (f32 bits stored as u32).
    cpu_load: Arc<AtomicU32>,
}

impl ToneSmithyApp {
    /// Creates a new app. `cpu_load` is the arc written by the audio callback.
    #[must_use]
    pub fn new(
        audio_status: String,
        events: EngineEventSender,
        snapshot_slot: SnapshotSlot,
        cpu_load: Arc<AtomicU32>,
    ) -> Self {
        let snap = load_snapshot(&snapshot_slot);
        Self {
            audio_status,
            events,
            snapshot_slot,
            osc1_level: snap.osc_main_levels[0],
            osc1_detune_cents: snap.osc_main_detune_cents[0],
            osc1_pan: snap.osc_main_pans[0],
            waveform: snap.waveform,
            osc1_unison_voices: snap.osc_main_unison_voices[0],
            osc1_unison_detune_cents: snap.osc_main_unison_detune_cents[0],
            osc1_unison_spread: snap.osc_main_unison_spreads[0],
            filter_mode: snap.filter_mode,
            filter_cutoff_hz: snap.filter_cutoff_hz,
            filter_resonance: snap.filter_resonance,
            amp_attack_secs: snap.amp_attack_secs,
            amp_decay_secs: snap.amp_decay_secs,
            amp_sustain_level: snap.amp_sustain_level,
            amp_release_secs: snap.amp_release_secs,
            pitch_offset_semis: snap.pitch_offset_semis,
            master_volume: snap.master_volume,
            keyboard: VirtualKeyboard::default(),
            computer_keyboard: ComputerKeyboard::default(),
            pitch_bend: 0.0,
            sustain_held: false,
            cpu_load,
        }
    }
}

impl eframe::App for ToneSmithyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.computer_keyboard.handle_input(ctx, &self.events);
        let snapshot = load_snapshot(&self.snapshot_slot);

        // Footer (bottom panel, must be added before central panel).
        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            self.footer_bar(ui, &snapshot);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Title row
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("Tone Smithy");
                ui.separator();
                ui.label(&self.audio_status);
            });
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(8.0);

            // Three panels side by side
            ui.columns(3, |cols| {
                self.osc1_panel(&mut cols[0]);
                self.filter_panel(&mut cols[1]);
                self.amp_env_panel(&mut cols[2]);
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // Master volume + pitch offset row
            ui.horizontal(|ui| {
                ui.label("Master");
                if ui
                    .add(
                        Knob::new(&mut self.master_volume, 0.0..=1.0, "Volume")
                            .default_value(0.8)
                            .format(|v| format!("{:.0}%", v * 100.0)),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::MasterVolume,
                        value: self.master_volume,
                    });
                }

                ui.add_space(16.0);
                ui.label("Pitch");
                if ui
                    .add(
                        Knob::new(
                            &mut self.pitch_offset_semis,
                            -PITCH_OFFSET_RANGE..=PITCH_OFFSET_RANGE,
                            "Offset",
                        )
                        .default_value(0.0)
                        .format(|v| format!("{:+.2} st", v)),
                    )
                    .changed()
                {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::PitchOffsetSemis,
                        value: self.pitch_offset_semis,
                    });
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // Computer keyboard hint
            ui.label(format!(
                "Keyboard: A S D F G H J (white) / W E T Y U (black). Z/X shift octave. Octave base: MIDI {} ({}).",
                self.computer_keyboard.octave_base(),
                midi_note_label(self.computer_keyboard.octave_base()),
            ));
            ui.add_space(6.0);

            // Keep the virtual keyboard's visible range in sync with the
            // computer keyboard's current octave so highlighted keys are
            // always visible. If a mouse-held note was active when the
            // range shifted, send NoteOff so the engine releases it.
            if let Some(stuck) = self.keyboard.set_start_note(self.computer_keyboard.octave_base()) {
                self.events.send(EngineEvent::NoteOff { note_midi: stuck });
            }
            let kb_notes = self.computer_keyboard.held_notes();

            ui.horizontal(|ui| {
                // Pitch-bend strip: vertical slider that springs to 0 on release.
                ui.vertical(|ui| {
                    ui.label("PB");
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
                    if pb_r.drag_stopped() {
                        self.pitch_bend = 0.0;
                        self.events.send(EngineEvent::PitchBend { value_normalised: 0.0 });
                    }
                });

                // Virtual keyboard.
                self.keyboard.show(ui, &self.events, kb_notes);

                // Sustain pedal toggle.
                ui.vertical(|ui| {
                    ui.label("Sustain");
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
            });
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

impl ToneSmithyApp {
    fn osc1_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Osc 1");
        ui.add_space(6.0);

        // Waveform selector
        ui.label("Waveform");
        ui.horizontal_wrapped(|ui| {
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

        ui.add_space(8.0);

        // Level / detune / pan knobs
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.osc1_level, 0.0..=OSC_LEVEL_MAX, "Level")
                        .default_value(1.0)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1Level,
                    value: self.osc1_level,
                });
            }
            if ui
                .add(
                    Knob::new(
                        &mut self.osc1_detune_cents,
                        -OSC_DETUNE_MAX_CENTS..=OSC_DETUNE_MAX_CENTS,
                        "Detune",
                    )
                    .default_value(0.0)
                    .format(|v| format!("{:+.1} ct", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1DetuneCents,
                    value: self.osc1_detune_cents,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.osc1_pan, -1.0..=1.0, "Pan")
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
                    id: ParamId::Osc1Pan,
                    value: self.osc1_pan,
                });
            }
        });

        ui.add_space(8.0);
        ui.label("Unison");
        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.osc1_unison_voices, 1.0..=UNISON_VOICES_MAX, "Voices")
                        .default_value(1.0)
                        .format(|v| format!("{}", v.round() as u8)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1UnisonVoices,
                    value: self.osc1_unison_voices,
                });
            }
            if ui
                .add(
                    Knob::new(
                        &mut self.osc1_unison_detune_cents,
                        0.0..=UNISON_DETUNE_MAX_CENTS,
                        "Detune",
                    )
                    .default_value(10.0)
                    .format(|v| format!("{:.1} ct", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1UnisonDetuneCents,
                    value: self.osc1_unison_detune_cents,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.osc1_unison_spread, 0.0..=1.0, "Spread")
                        .default_value(0.5)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::Osc1UnisonSpread,
                    value: self.osc1_unison_spread,
                });
            }
        });
    }

    fn filter_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Filter");
        ui.add_space(6.0);

        // Mode selector
        ui.label("Mode");
        ui.horizontal_wrapped(|ui| {
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

        ui.add_space(8.0);

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

    fn amp_env_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Amp Env");
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui
                .add(
                    Knob::new(&mut self.amp_attack_secs, ENV_MIN_SECS..=ENV_ATTACK_MAX_SECS, "A")
                        .default_value(0.010)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpAttackSecs,
                    value: self.amp_attack_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_decay_secs, ENV_MIN_SECS..=ENV_DECAY_MAX_SECS, "D")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpDecaySecs,
                    value: self.amp_decay_secs,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_sustain_level, 0.0..=1.0, "S")
                        .default_value(0.8)
                        .format(|v| format!("{:.2}", v)),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpSustainLevel,
                    value: self.amp_sustain_level,
                });
            }
            if ui
                .add(
                    Knob::new(&mut self.amp_release_secs, ENV_MIN_SECS..=ENV_RELEASE_MAX_SECS, "R")
                        .default_value(0.200)
                        .format(secs_format),
                )
                .changed()
            {
                self.events.send(EngineEvent::ParameterChange {
                    id: ParamId::AmpReleaseSecs,
                    value: self.amp_release_secs,
                });
            }
        });
    }

    fn footer_bar(&self, ui: &mut egui::Ui, snapshot: &synth_engine::ParamSnapshot) {
        ui.horizontal(|ui| {
            let cpu = f32::from_bits(self.cpu_load.load(Ordering::Relaxed));
            ui.label(format!("CPU: {cpu:.1}%"));
            ui.separator();
            ui.label(format!("voices: {}", snapshot.active_voice_count));
            ui.separator();
            ui.label(&self.audio_status);
        });
    }
}

/// Formats seconds as ms (< 1 s) or seconds (>= 1 s) for envelope tooltips.
fn secs_format(v: f32) -> String {
    if v < 1.0 {
        format!("{:.0} ms", v * 1000.0)
    } else {
        format!("{:.2} s", v)
    }
}

/// Formats a MIDI note number as scientific pitch notation (C4 = 60).
fn midi_note_label(note_midi: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = i32::from(note_midi / 12) - 1;
    let name = NAMES[usize::from(note_midi % 12)];
    format!("{name}{octave}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_60_is_c4() {
        assert_eq!(midi_note_label(60), "C4");
    }

    #[test]
    fn midi_48_is_c3() {
        assert_eq!(midi_note_label(48), "C3");
    }

    #[test]
    fn midi_69_is_a4() {
        assert_eq!(midi_note_label(69), "A4");
    }

    #[test]
    fn secs_format_below_one_second_shows_ms() {
        assert_eq!(secs_format(0.010), "10 ms");
        assert_eq!(secs_format(0.200), "200 ms");
    }

    #[test]
    fn secs_format_at_or_above_one_second_shows_s() {
        assert!(secs_format(1.0).contains('s'));
        assert!(secs_format(3.5).contains('s'));
    }
}
