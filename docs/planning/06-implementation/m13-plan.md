# M13 — Settings + MIDI Learn

**Status:** Complete (merged to `main`, tag `m13`)

## Goal

Make Tone Smithy self-configuring: the user picks audio and MIDI devices from inside the app, a MIDI CC can be bound to any knob and that binding survives preset changes, and a first-run wizard guides a fresh install.

---

## Done when

- A fresh install shows a first-run wizard with device pickers before the main window.
- Audio output and MIDI input can be changed from the Settings tab without restarting.
- Right-clicking any knob and choosing MIDI Learn, then moving a CC, binds that CC to that knob.
- The binding persists when a preset is saved and reloaded.

---

## Architecture

### Settings persistence (`synth-presets/src/settings.rs`)

```rust
pub struct AppSettings {
    pub audio_output_device: Option<String>,  // None = OS default
    pub midi_input_port:     Option<String>,  // None = first available
    pub last_preset_path:    Option<PathBuf>,
    pub first_run_complete:  bool,
}
```

Stored as RON in `%APPDATA%/Tone Smithy/Tone Smithy/settings.ron`. Loaded once at startup; saved whenever a device changes or the wizard is dismissed.

### Device enumeration (`synth-host`)

New free functions:
- `audio::list_output_devices() -> Vec<String>` — all device names from the default cpal host
- `audio::start_on_device(name: Option<&str>, engine, events_rx, slot) -> Result<AudioStream>` — opens named device, falling back to default if `None` or name not found
- `midi::list_ports() -> Result<Vec<String>>` — all midir input port names
- `midi::start_on_port(name: Option<&str>, sender) -> Result<MidiInputStream>` — connects named port, falling back to first if `None` or name not found

### Live device switching

`AppShell` (in `synth-app/src/main.rs`) wraps `ToneSmithyApp`. It checks `ToneSmithyApp::pending_device_change` after every `ui.update()` call. When a change is pending:
1. Read the current `ParamSnapshot` from the snapshot slot.
2. Drop the old `AudioStream` (stops the cpal thread).
3. Build a new `Engine` at the new sample rate.
4. Replay all current parameters into it via `map_to_events`.
5. Start a new `AudioStream` on the requested device.
6. Update `ToneSmithyApp::audio_status` and the shared `cpu_load` Arc.

MIDI switching is simpler (no Engine rebuild): drop the old `MidiInputStream`, open the new port.

### Settings tab UI (`sections/settings.rs`)

New `Tab::Settings`. Contains:
- **Audio output** — dropdown of device names from `audio::list_output_devices()`, "Apply" button sends `DeviceChange::Audio(name)`.
- **MIDI input** — dropdown of port names from `midi::list_ports()`, "Apply" button sends `DeviceChange::Midi(name)`.
- Status readout (sample rate, buffer hint, open MIDI port name).

Device lists are cached in `ToneSmithyApp` at startup and on "Refresh" click; they do not re-scan every frame.

### MIDI Learn

State in `ToneSmithyApp`:
```rust
pub(crate) midi_learn_target: Option<String>, // param key currently being learned
prev_cc_values: [f32; 128],                   // CC snapshot from last frame
pub(crate) midi_learn_mappings: Vec<MidiLearnEntry>, // active mappings
```

Flow:
1. Knob right-click → "MIDI Learn" stores the param key in egui memory under `"ts_midi_learn_pending"`.
2. `update()` reads that key once per frame; sets `midi_learn_target`.
3. While `midi_learn_target.is_some()`, compare `snapshot.cc_values` against `prev_cc_values` each frame. First CC that changed (delta > 0.02) → create `MidiLearnEntry { cc, parameter: target }`, add to `midi_learn_mappings`, clear target. Show a "learning…" indicator in the status bar.
4. Each frame, for every active mapping: read `snapshot.cc_values[cc]`, compute the parameter value (CC 0.0–1.0 maps to param range), send `ParameterChange`.
5. On preset save: copy `midi_learn_mappings` into `Preset::midi_learn`.
6. On preset load: copy `Preset::midi_learn` into `midi_learn_mappings`.

Knob `.param_key("cutoff")` builder method enables the MIDI Learn menu item and deposits the key in egui memory on click.

### First-run wizard

Rendered as a centered `egui::Window` when `!settings.first_run_complete`. Blocks interaction with the rest of the UI via a modal background. Contains the same device dropdowns as the Settings tab plus a "Get started" button that saves settings and sets `first_run_complete = true`.

---

## Out of scope

| Item | Where |
|---|---|
| Per-channel MIDI filter | v1.1 |
| Global vs per-preset MIDI Learn layers | M13 delivers per-preset only; global layer deferred |
| CC value display on learned knobs | M14 or post |
| MIDI output | post-v1 |
| ASIO driver support | feature flag, post-v1 |
