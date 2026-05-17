//! Top-level [`eframe::App`] implementation.
//!
//! M1 surface: a status line, the two parameter-bus sliders (pitch
//! offset in semitones, amp envelope release in seconds), and a
//! waveform toggle. The real panel layout (oscillators / filter /
//! envelopes / mod matrix / FX / master) lands in M4 onwards.

use eframe::egui;
use synth_engine::param_bus::{EngineEventSender, SnapshotSlot, load_snapshot};
use synth_engine::{EngineEvent, ParamId, Waveform};

use crate::keyboard::VirtualKeyboard;

/// Lower bound for the pitch offset slider, in semitones.
const PITCH_OFFSET_MIN_SEMIS: f32 = -24.0;
/// Upper bound for the pitch offset slider, in semitones.
const PITCH_OFFSET_MAX_SEMIS: f32 = 24.0;

/// Lower bound for the release slider, in seconds.
const RELEASE_MIN_SECS: f32 = 0.005;
/// Upper bound for the release slider, in seconds.
const RELEASE_MAX_SECS: f32 = 3.0;

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

    /// UI-side mirror of amp release seconds. Same pattern as
    /// `pitch_offset_semis`.
    amp_release_secs: f32,

    /// UI-side mirror of the discrete waveform parameter.
    waveform: Waveform,

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
            keyboard: VirtualKeyboard::default(),
        }
    }
}

impl eframe::App for ToneSmithyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Pull the latest snapshot once per frame so the displayed
        // "voice active" flag stays current. The slider mirrors are
        // owned by the UI (egui needs a `&mut f32`), so we only resync
        // them from the snapshot if the user hasn't touched them — for
        // M1, "touch" is tracked implicitly: if the UI value differs
        // from the snapshot value we keep the UI value, then on next
        // change we publish.
        let snapshot = load_snapshot(&self.snapshot_slot);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                ui.heading("Tone Smithy");
                ui.add_space(8.0);
                ui.label("M1 — first sound");
                ui.add_space(8.0);
                ui.label(&self.audio_status);
                ui.add_space(24.0);
            });

            ui.separator();
            ui.add_space(12.0);

            egui::Grid::new("m1_params")
                .num_columns(2)
                .spacing([24.0, 12.0])
                .show(ui, |ui| {
                    ui.label("Pitch offset (semitones)");
                    let pitch_response = ui.add(
                        egui::Slider::new(
                            &mut self.pitch_offset_semis,
                            PITCH_OFFSET_MIN_SEMIS..=PITCH_OFFSET_MAX_SEMIS,
                        )
                        .step_by(0.01)
                        .clamping(egui::SliderClamping::Always),
                    );
                    if pitch_response.changed() {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::PitchOffsetSemis,
                            value: self.pitch_offset_semis,
                        });
                    }
                    ui.end_row();

                    ui.label("Amp release (seconds)");
                    let release_response = ui.add(
                        egui::Slider::new(
                            &mut self.amp_release_secs,
                            RELEASE_MIN_SECS..=RELEASE_MAX_SECS,
                        )
                        .logarithmic(true)
                        .step_by(0.001)
                        .clamping(egui::SliderClamping::Always),
                    );
                    if release_response.changed() {
                        self.events.send(EngineEvent::ParameterChange {
                            id: ParamId::AmpReleaseSecs,
                            value: self.amp_release_secs,
                        });
                    }
                    ui.end_row();

                    ui.label("Waveform");
                    ui.horizontal(|ui| {
                        let mut changed = false;
                        if ui
                            .selectable_value(&mut self.waveform, Waveform::Sine, "Sine")
                            .clicked()
                        {
                            changed = true;
                        }
                        if ui
                            .selectable_value(&mut self.waveform, Waveform::Saw, "Saw")
                            .clicked()
                        {
                            changed = true;
                        }
                        if changed {
                            self.events.send(EngineEvent::SetOscillatorWaveform {
                                waveform: self.waveform,
                            });
                        }
                    });
                    ui.end_row();
                });

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label("Click and drag across keys to play.");
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                self.keyboard.show(ui, &self.events);
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(format!(
                "engine snapshot — voice active: {}, waveform: {:?}, pitch: {:+.2} semis, release: {:.3} s",
                snapshot.voice_active,
                snapshot.waveform,
                snapshot.pitch_offset_semis,
                snapshot.amp_release_secs,
            ));
        });

        // egui doesn't repaint by default unless an input event arrives.
        // Request a steady repaint so the "voice active" flag updates as
        // the envelope progresses.
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}
