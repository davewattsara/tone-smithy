//! Tone Smithy standalone application entry point.
//!
//! Wires the silent audio output (`synth-host`) and the egui window
//! (`synth-ui`) together. This is the composition root — the only file that
//! knows about every other crate (see
//! `docs/planning/03-architecture/design-patterns.md`, §1.6).

use anyhow::{Context, Result};
use synth_host::audio::{self, AudioStream};
use synth_ui::app::ToneSmithyApp;

/// Owns the audio stream and delegates UI work to [`ToneSmithyApp`].
///
/// The audio stream lives here (rather than inside the UI app) because
/// `cpal::Stream` is `!Send` and binding it to the UI struct keeps the
/// lifetime obvious — when the window closes and this struct drops, audio
/// stops.
struct AppShell {
    _audio: AudioStream,
    ui: ToneSmithyApp,
}

impl eframe::App for AppShell {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.ui.update(ctx, frame);
    }
}

fn main() -> Result<()> {
    init_logging();

    let audio = audio::start_silent().context("could not start audio output")?;
    let status = format!(
        "audio out: {} Hz, {} channel(s) — writing silence",
        audio.sample_rate, audio.channels
    );
    tracing::info!("{status}");

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([1280.0, 720.0])
            .with_title("Tone Smithy"),
        ..Default::default()
    };

    let shell = AppShell {
        _audio: audio,
        ui: ToneSmithyApp::new(status),
    };

    eframe::run_native("Tone Smithy", native_options, Box::new(move |_cc| Ok(Box::new(shell))))
        .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

/// Installs a `tracing` subscriber that reads `RUST_LOG` if set and defaults
/// to `info` otherwise.
fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
