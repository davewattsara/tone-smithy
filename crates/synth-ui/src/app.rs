//! Top-level [`eframe::App`] implementation.
//!
//! M0 is an empty window with a status label showing what the audio device
//! opened with. The real panel layout (oscillators / filter / envelopes /
//! mod matrix / FX / master) lands in M4 onwards.

use eframe::egui;

/// The Tone Smithy application UI.
///
/// In M0 this is intentionally bare: a centred heading and a status line
/// reporting the audio device. Holding state here (rather than constants in
/// `update`) sets up the pattern we'll grow as panels are added.
pub struct ToneSmithyApp {
    /// One-line description of the open audio device, set at construction.
    audio_status: String,
}

impl ToneSmithyApp {
    /// Creates a new app with the given audio status string.
    ///
    /// The caller (typically `synth-app`) is responsible for the audio
    /// device and just passes a human-readable summary to display.
    #[must_use]
    pub fn new(audio_status: String) -> Self {
        Self { audio_status }
    }
}

impl eframe::App for ToneSmithyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(48.0);
                ui.heading("Tone Smithy");
                ui.add_space(8.0);
                ui.label("M0 scaffold — empty window, silent audio");
                ui.add_space(24.0);
                ui.label(&self.audio_status);
            });
        });
    }
}
