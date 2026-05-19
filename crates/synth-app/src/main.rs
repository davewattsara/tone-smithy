//! Tone Smithy standalone application entry point.
//!
//! Wires the audio output (`synth-host`) and the egui window (`synth-ui`)
//! together. This is the composition root — the only file that knows about
//! every other crate (see
//! `docs/planning/03-architecture/design-patterns.md`, §1.6).
//!
//! M1: the UI's parameter sliders + on-screen keyboard drive the
//! engine through `synth_engine::param_bus`; nothing is hardcoded here
//! beyond the choice of default waveform.

use anyhow::{Context, Result};
use synth_engine::Engine;
use synth_engine::param_bus;
use synth_host::audio::{self, AudioStream};
use synth_host::midi::MidiInputStream;
use synth_ui::app::ToneSmithyApp;

/// Owns the audio + MIDI streams and delegates UI work to [`ToneSmithyApp`].
///
/// The streams live here (rather than inside the UI app) because
/// `cpal::Stream` is `!Send` and binding it to the UI struct keeps the
/// lifetime obvious — when the window closes and this struct drops, audio
/// stops. The MIDI input rides alongside for the same reason. The UI app
/// owns its bus handles (sender + snapshot slot) so it can read snapshots
/// and send parameter changes directly.
struct AppShell {
    _audio: AudioStream,
    _midi: MidiInputStream,
    ui: ToneSmithyApp,
}

impl eframe::App for AppShell {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.ui.update(ctx, frame);
    }
}

fn main() -> Result<()> {
    init_logging();

    // Query the device first so we know the sample rate before building
    // the engine. The stream itself is opened by `start_with_engine`.
    let device_format = audio::default_output_format().context("could not query default audio output device")?;

    let engine = Engine::new(device_format.sample_rate as f32);
    let (events_tx, events_rx, snapshot_slot) = param_bus::new_param_bus();

    let audio =
        audio::start_with_engine(engine, events_rx, snapshot_slot.clone()).context("could not start audio output")?;
    // Open the first available MIDI input port (if any). The events
    // sender is cloned here — the bus is MPMC, so the UI thread, the
    // computer keyboard, and the MIDI thread all push into it.
    let midi = MidiInputStream::start(events_tx.clone()).context("could not start MIDI input")?;
    let midi_status = match midi.port_name() {
        Some(name) => format!("MIDI in: {name}"),
        None => "MIDI in: no device".to_string(),
    };
    let status = format!(
        "audio out: {} Hz, {} channel(s), {} | {midi_status}",
        audio.sample_rate, audio.channels, audio.buffer_latency_hint,
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
        _midi: midi,
        ui: ToneSmithyApp::new(status, events_tx, snapshot_slot),
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
