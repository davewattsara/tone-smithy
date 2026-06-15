//! Tone Smithy standalone application entry point.
//!
//! Wires the audio output (`synth-host`) and the egui window (`synth-ui`)
//! together. This is the composition root — the only file that knows about
//! every other crate (see `docs/planning/03-architecture/design-patterns.md`,
//! §1.6).

use anyhow::{Context, Result};
use synth_engine::Engine;
use synth_engine::param_bus;
use synth_engine::param_bus::EngineEventReceiver;
use synth_host::audio::{self, AudioStream};
use synth_host::midi::MidiInputStream;
use synth_presets::preset_params::map_to_events;
use synth_presets::preset_params::snapshot_to_map;
use synth_presets::{AppSettings, load_settings, save_settings};
use synth_ui::app::{DeviceChange, ToneSmithyApp};

/// Owns the audio + MIDI streams and delegates UI work to [`ToneSmithyApp`].
///
/// Streams live here (not in the UI) because `cpal::Stream` is `!Send`.
/// Device switches are handled by dropping the old stream and opening a new
/// one while keeping the same event sender / snapshot slot so the UI stays
/// connected.
struct AppShell {
    audio: AudioStream,
    midi: MidiInputStream,
    ui: ToneSmithyApp,
    /// Receives the raw event stream from MIDI/computer keyboard.
    /// Kept here so it can be handed to a replacement audio stream.
    events_rx: EngineEventReceiver,
    settings: AppSettings,
}

impl AppShell {
    fn audio_status(audio: &AudioStream, midi: &MidiInputStream) -> String {
        let midi_str = match midi.port_name() {
            Some(name) => format!("MIDI: {name}"),
            None => "MIDI: none".to_string(),
        };
        format!(
            "{} Hz, {} ch, {} | {midi_str}",
            audio.sample_rate, audio.channels, audio.buffer_latency_hint
        )
    }

    /// Applies a pending device change requested by the UI.
    fn apply_device_change(&mut self, change: DeviceChange) {
        match change {
            DeviceChange::Audio(device_name) => {
                // Snapshot current engine state so the new engine starts
                // with identical parameters.
                let snapshot = synth_engine::param_bus::load_snapshot(self.ui.snapshot_slot());
                let events_for_new = map_to_events(&snapshot_to_map(&snapshot));

                // Size the new engine at the *target* device's sample rate.
                // Querying the OS default device here was a latent bug: it
                // built the engine at the wrong rate whenever the user picked
                // a non-default device. Fall back to the rate the current
                // stream is running at if the target can't be queried.
                let new_sample_rate = audio::output_format(device_name.as_deref())
                    .map(|f| f.sample_rate)
                    .unwrap_or(self.audio.sample_rate);

                let (new_tx, new_rx, new_slot) = param_bus::new_param_bus();
                let mut new_engine = Engine::new(new_sample_rate as f32);
                for event in events_for_new {
                    new_engine.handle(event);
                }

                match audio::start_on_device(device_name.as_deref(), new_engine, new_rx, new_slot.clone()) {
                    Ok(new_audio) => {
                        self.events_rx = {
                            // The old events_rx is no longer attached to any
                            // stream — just let it drain silently. The UI gets
                            // a fresh sender so computer keyboard and MIDI
                            // still deliver into the new stream.
                            let (_dummy_tx, dummy_rx, _) = param_bus::new_param_bus();
                            // Reconnect MIDI to the new bus.
                            let midi_port = self.midi.port_name().map(str::to_owned);
                            if let Ok(new_midi) = MidiInputStream::start_on_port(midi_port.as_deref(), new_tx.clone()) {
                                self.midi = new_midi;
                            }
                            self.ui.reconnect_bus(new_tx, new_slot.clone());
                            let status = Self::audio_status(&new_audio, &self.midi);
                            self.ui.set_audio_status(status);
                            self.ui.set_audio_device(device_name.clone());
                            self.settings.audio_output_device = device_name;
                            save_settings(&self.settings);
                            self.audio = new_audio;
                            dummy_rx
                        };
                    }
                    Err(e) => {
                        tracing::error!("audio device switch failed: {e}");
                        self.ui.set_preset_error(format!("Audio switch failed: {e}"));
                    }
                }
            }
            DeviceChange::Midi(port_name) => {
                let sender = self.ui.events_sender().clone();
                match MidiInputStream::start_on_port(port_name.as_deref(), sender) {
                    Ok(new_midi) => {
                        self.midi = new_midi;
                        let status = Self::audio_status(&self.audio, &self.midi);
                        self.ui.set_audio_status(status);
                        self.ui.set_midi_port(port_name.clone());
                        self.settings.midi_input_port = port_name;
                        save_settings(&self.settings);
                    }
                    Err(e) => {
                        tracing::error!("MIDI port switch failed: {e}");
                        self.ui.set_preset_error(format!("MIDI switch failed: {e}"));
                    }
                }
            }
        }
    }
}

impl eframe::App for AppShell {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.ui.update(ctx, frame);
        if let Some(change) = self.ui.take_pending_device_change() {
            self.apply_device_change(change);
        }
    }
}

fn main() -> Result<()> {
    init_logging();

    let settings = load_settings();

    let device_format = audio::default_output_format().context("could not query audio output device")?;
    let engine = Engine::new(device_format.sample_rate as f32);
    let (events_tx, events_rx, snapshot_slot) = param_bus::new_param_bus();

    let audio = audio::start_on_device(
        settings.audio_output_device.as_deref(),
        engine,
        events_rx,
        snapshot_slot.clone(),
    )
    .context("could not start audio output")?;

    let midi = MidiInputStream::start_on_port(settings.midi_input_port.as_deref(), events_tx.clone())
        .context("could not start MIDI input")?;

    let status = AppShell::audio_status(&audio, &midi);
    tracing::info!("{status}");

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([1280.0, 720.0])
            .with_title("Tone Smithy"),
        ..Default::default()
    };

    let cpu_load = audio.cpu_load.clone();
    // Throw-away rx; the shell owns the live one.
    let (_, dummy_rx, _) = param_bus::new_param_bus();
    let mut ui = ToneSmithyApp::new(status, events_tx, snapshot_slot, cpu_load, settings.clone());

    // `.tsmith` file association / command line: the first positional argument,
    // if any, is a preset path to open on startup. The audio stream is already
    // live, so the resulting events are consumed as soon as they are queued.
    if let Some(path) = preset_path_arg() {
        tracing::info!("opening preset from argument: {}", path.display());
        ui.open_preset_file(&path);
    }

    let shell = AppShell {
        audio,
        midi,
        ui,
        events_rx: dummy_rx,
        settings,
    };

    eframe::run_native("Tone Smithy", native_options, Box::new(move |_cc| Ok(Box::new(shell))))
        .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

/// The preset path to open on startup, taken from the first command-line
/// argument. Returns `None` when no argument is given. The path is not required
/// to end in `.tsmith` — Windows passes the associated file's full path, and a
/// non-existent path is reported by the loader as a preset error rather than
/// failing the launch.
fn preset_path_arg() -> Option<std::path::PathBuf> {
    std::env::args_os().nth(1).map(std::path::PathBuf::from)
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
