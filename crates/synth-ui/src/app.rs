//! Top-level [`eframe::App`] implementation.
//!
//! M2 surface: existing M1 controls (pitch offset, amp release,
//! waveform — extended to all four shapes), plus a filter section
//! (mode / cutoff / resonance) and a unison section for oscillator 1
//! (voices / detune / spread). These use egui's default widgets and
//! are intentionally rough — M4 replaces the layout with the proper
//! panel library and custom knob widget, and rounds out per-osc
//! level/pan/detune controls. The goal here is the M2 done-when:
//! audible filter sweeps and unison detune from the running binary.

use eframe::egui;
use synth_engine::param_bus::{EngineEventSender, SnapshotSlot, load_snapshot};
use synth_engine::{EngineEvent, FilterMode, ParamId, Waveform};

use crate::keyboard::VirtualKeyboard;

/// Lower bound for the pitch offset slider, in semitones.
const PITCH_OFFSET_MIN_SEMIS: f32 = -24.0;
/// Upper bound for the pitch offset slider, in semitones.
const PITCH_OFFSET_MAX_SEMIS: f32 = 24.0;

/// Lower bound for the release slider, in seconds.
const RELEASE_MIN_SECS: f32 = 0.005;
/// Upper bound for the release slider, in seconds.
const RELEASE_MAX_SECS: f32 = 3.0;

/// Lower bound for the filter cutoff slider, in Hz. Matches the
/// SVF's internal floor.
const CUTOFF_MIN_HZ: f32 = 20.0;
/// Upper bound for the filter cutoff slider, in Hz. Sits below
/// Nyquist for 44.1/48 kHz sample rates with comfortable headroom.
const CUTOFF_MAX_HZ: f32 = 20_000.0;

/// Top end of the unison detune slider, in cents. Half a semitone is
/// well past anything musical; the smaller increments are where the
/// interesting effect lives.
const UNISON_DETUNE_MAX_CENTS: f32 = 50.0;

/// Maximum unison voice count surfaced in the slider. Matches the
/// engine's hard cap.
const UNISON_VOICES_MAX: f32 = 7.0;

/// The Tone Smithy application UI.
pub struct ToneSmithyApp {
    /// One-line description of the open audio device, set at construction.
    audio_status: String,

    /// Outbound queue to the engine.
    events: EngineEventSender,

    /// Snapshot slot the engine publishes into each block.
    snapshot_slot: SnapshotSlot,

    /// UI-side mirror of pitch offset. Initialised from the snapshot
    /// at construction; rebroadcast as a `ParameterChange` whenever the
    /// user drags the slider. The engine snapshot is the source of
    /// truth — see design-patterns.md §1.3 — so we resync whenever the
    /// snapshot disagrees and the user isn't currently dragging.
    pitch_offset_semis: f32,

    /// UI-side mirror of amp release seconds.
    amp_release_secs: f32,

    /// UI-side mirror of the discrete oscillator waveform.
    waveform: Waveform,

    /// UI-side mirror of the discrete filter output mode.
    filter_mode: FilterMode,
    /// UI-side mirror of the filter cutoff in Hz.
    filter_cutoff_hz: f32,
    /// UI-side mirror of the filter resonance on the 0..=1 scale.
    filter_resonance: f32,

    /// UI-side mirror of oscillator 1's unison voice count.
    /// Carried as f32 because the parameter bus carries f32; the
    /// engine rounds and clamps on the consuming side.
    osc1_unison_voices: f32,
    /// UI-side mirror of oscillator 1's unison detune width in cents.
    osc1_unison_detune_cents: f32,
    /// UI-side mirror of oscillator 1's unison stereo spread (0..=1).
    osc1_unison_spread: f32,

    /// The on-screen keyboard. Owns its own held-note state.
    keyboard: VirtualKeyboard,
}

impl ToneSmithyApp {
    /// Creates a new app with the given audio status string and the
    /// parameter-bus handles from the composition root.
    #[must_use]
    pub fn new(audio_status: String, events: EngineEventSender, snapshot_slot: SnapshotSlot) -> Self {
        // Seed the slider mirrors from whatever the engine published
        // before the window opens (the default `ParamSnapshot` until
        // the first audio block runs).
        let snap = load_snapshot(&snapshot_slot);
        Self {
            audio_status,
            events,
            snapshot_slot,
            pitch_offset_semis: snap.pitch_offset_semis,
            amp_release_secs: snap.amp_release_secs,
            waveform: snap.waveform,
            filter_mode: snap.filter_mode,
            filter_cutoff_hz: snap.filter_cutoff_hz,
            filter_resonance: snap.filter_resonance,
            osc1_unison_voices: snap.osc_main_unison_voices[0],
            osc1_unison_detune_cents: snap.osc_main_unison_detune_cents[0],
            osc1_unison_spread: snap.osc_main_unison_spreads[0],
            keyboard: VirtualKeyboard::default(),
        }
    }
}

impl eframe::App for ToneSmithyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Pull the latest snapshot once per frame so the displayed
        // "voice active" flag stays current. The slider mirrors are
        // owned by the UI (egui needs a `&mut f32`); the snapshot
        // drives the read-only status footer only.
        let snapshot = load_snapshot(&self.snapshot_slot);

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(16.0);
                    ui.heading("Tone Smithy");
                    ui.add_space(4.0);
                    ui.label("M2 — subtractive voice");
                    ui.add_space(4.0);
                    ui.label(&self.audio_status);
                    ui.add_space(16.0);
                });

                ui.separator();
                ui.add_space(8.0);

                self.pitch_release_grid(ui);

                ui.add_space(8.0);
                ui.label("Waveform");
                self.waveform_buttons(ui);

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.heading("Filter");
                self.filter_section(ui);

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.heading("Unison (Osc 1)");
                self.unison_section(ui);

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label("Click and drag across keys to play.");
                ui.add_space(6.0);
                ui.vertical_centered(|ui| {
                    self.keyboard.show(ui, &self.events);
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(format!(
                    "voices: {} | wave: {:?} | filter: {:?} {:.0} Hz / res {:.2} | pitch {:+.2} st | release {:.3} s",
                    snapshot.active_voice_count,
                    snapshot.waveform,
                    snapshot.filter_mode,
                    snapshot.filter_cutoff_hz,
                    snapshot.filter_resonance,
                    snapshot.pitch_offset_semis,
                    snapshot.amp_release_secs,
                ));
            });
        });

        // egui doesn't repaint by default unless an input event arrives.
        // Request a steady repaint so the "voice active" flag updates as
        // the envelope progresses.
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

impl ToneSmithyApp {
    fn pitch_release_grid(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("m2_pitch_release")
            .num_columns(2)
            .spacing([24.0, 8.0])
            .show(ui, |ui| {
                ui.label("Pitch offset (semitones)");
                let r = ui.add(
                    egui::Slider::new(
                        &mut self.pitch_offset_semis,
                        PITCH_OFFSET_MIN_SEMIS..=PITCH_OFFSET_MAX_SEMIS,
                    )
                    .step_by(0.01)
                    .clamping(egui::SliderClamping::Always),
                );
                if r.changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::PitchOffsetSemis,
                        value: self.pitch_offset_semis,
                    });
                }
                ui.end_row();

                ui.label("Amp release (seconds)");
                let r = ui.add(
                    egui::Slider::new(&mut self.amp_release_secs, RELEASE_MIN_SECS..=RELEASE_MAX_SECS)
                        .logarithmic(true)
                        .step_by(0.001)
                        .clamping(egui::SliderClamping::Always),
                );
                if r.changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::AmpReleaseSecs,
                        value: self.amp_release_secs,
                    });
                }
                ui.end_row();
            });
    }

    fn waveform_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut changed = false;
            for w in [Waveform::Sine, Waveform::Saw, Waveform::Square, Waveform::Triangle] {
                let label = match w {
                    Waveform::Sine => "Sine",
                    Waveform::Saw => "Saw",
                    Waveform::Square => "Square",
                    Waveform::Triangle => "Triangle",
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
    }

    fn filter_section(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Mode");
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

        egui::Grid::new("m2_filter")
            .num_columns(2)
            .spacing([24.0, 8.0])
            .show(ui, |ui| {
                ui.label("Cutoff (Hz)");
                let r = ui.add(
                    egui::Slider::new(&mut self.filter_cutoff_hz, CUTOFF_MIN_HZ..=CUTOFF_MAX_HZ)
                        .logarithmic(true)
                        .clamping(egui::SliderClamping::Always),
                );
                if r.changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::FilterCutoffHz,
                        value: self.filter_cutoff_hz,
                    });
                }
                ui.end_row();

                ui.label("Resonance");
                let r = ui.add(
                    egui::Slider::new(&mut self.filter_resonance, 0.0..=1.0)
                        .step_by(0.01)
                        .clamping(egui::SliderClamping::Always),
                );
                if r.changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::FilterResonance,
                        value: self.filter_resonance,
                    });
                }
                ui.end_row();
            });
    }

    fn unison_section(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("m2_unison_osc1")
            .num_columns(2)
            .spacing([24.0, 8.0])
            .show(ui, |ui| {
                ui.label("Voices");
                let r = ui.add(
                    egui::Slider::new(&mut self.osc1_unison_voices, 1.0..=UNISON_VOICES_MAX)
                        .step_by(1.0)
                        .clamping(egui::SliderClamping::Always),
                );
                if r.changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::Osc1UnisonVoices,
                        value: self.osc1_unison_voices,
                    });
                }
                ui.end_row();

                ui.label("Detune (cents)");
                let r = ui.add(
                    egui::Slider::new(&mut self.osc1_unison_detune_cents, 0.0..=UNISON_DETUNE_MAX_CENTS)
                        .step_by(0.1)
                        .clamping(egui::SliderClamping::Always),
                );
                if r.changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::Osc1UnisonDetuneCents,
                        value: self.osc1_unison_detune_cents,
                    });
                }
                ui.end_row();

                ui.label("Spread");
                let r = ui.add(
                    egui::Slider::new(&mut self.osc1_unison_spread, 0.0..=1.0)
                        .step_by(0.01)
                        .clamping(egui::SliderClamping::Always),
                );
                if r.changed() {
                    self.events.send(EngineEvent::ParameterChange {
                        id: ParamId::Osc1UnisonSpread,
                        value: self.osc1_unison_spread,
                    });
                }
                ui.end_row();
            });
    }
}
