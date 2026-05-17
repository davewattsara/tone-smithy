# Persistence

What we save to disk, where, and in what format. Owned by the `synth-presets` crate, plus a small amount of application-level state owned by `synth-app`.

## What is persisted

| Kind | Owner | Format | Location |
| --- | --- | --- | --- |
| Synth presets | `synth-presets` | RON | Factory: install dir; User: `%APPDATA%\Tone Smithy\presets\user\` |
| App settings | `synth-app` | RON | `%APPDATA%\Tone Smithy\settings.ron` |
| UI state | `synth-ui` | RON | `%APPDATA%\Tone Smithy\ui_state.ron` |
| Crash logs | `synth-app` | Plain text | `%APPDATA%\Tone Smithy\logs\` (rotated, max 5 files) |

The exact directory is resolved via the `directories` crate so the same code path works on macOS/Linux when added later.

## Preset format

A preset is the **serialised parameter tree** plus **metadata**.

```ron
(
    version: 1,
    metadata: (
        name: "Warm Saw Pad",
        author: "Factory",
        category: "Pad",
        tags: ["analog", "warm", "slow attack"],
        description: "Slow-attack pad with two detuned saws and a long release.",
        created_utc: "2026-05-17T10:00:00Z",
    ),
    parameters: {
        "osc1.slot_type": "Subtractive",
        "osc1.sub.osc1.waveform": "Saw",
        "osc1.sub.osc1.detune_cents": 7.0,
        "filter.f1.cutoff_hz": 1800.0,
        "filter.f1.resonance": 0.25,
        // ... etc, one entry per parameter id
    },
    modulation_matrix: [
        ( source: "Env2", destination: "filter.f1.cutoff_hz", amount: 0.6, via: None ),
        ( source: "LFO1", destination: "osc1.sub.osc1.pitch_cents", amount: 0.05, via: Some("ModWheel") ),
    ],
    midi_learn: [
        ( cc: 1, parameter: "filter.f1.cutoff_hz" ),
    ],
)
```

### Choice of format
**RON** is chosen over JSON / TOML / a custom binary because:

- It's human-readable, which makes preset bugs diagnosable.
- It supports Rust-friendly types (enums with data, tuples, nested structs) without contortions.
- `serde` integration is mature.
- Diffs are reviewable in git, which helps factory bank curation.

A binary format may be added later for distribution-size reasons, but parsing speed is not a concern at v1's scale.

### Schema evolution
- Top-level `version` field present from day one.
- On load, an explicit migration step runs through every version less than the current. Each step is a small Rust function that mutates the deserialised representation.
- Unknown fields are tolerated and ignored (forward compatibility for minor additions).
- Missing fields fall back to the parameter's declared default.

### Validation
- After deserialising and migrating, every parameter is clamped to its declared range.
- Out-of-range or unknown parameter ids are logged and skipped, not fatal.
- A preset that fails to parse at all produces a clear error in the browser and a log entry — it does not crash anything.

## Factory bank

- Bundled inside the installer at `<install-dir>/presets/factory/`.
- Read-only — the UI prevents saving over factory presets and offers "Save As" if the user edits a factory patch.
- Organised in subfolders by category for tidy browsing.
- Each preset includes `author: "Factory"` and a `description`.

## User presets

- Stored at `%APPDATA%\Tone Smithy\presets\user\`.
- Free to organise in subfolders — the browser surfaces folders as a tree.
- Import / export single presets via the OS file dialog. File extension: **`.tsmith`** (provisional — chosen with the product name; revisit if a clearer alternative emerges).

## App settings

`settings.ron`:

```ron
(
    version: 1,
    audio: (
        driver: "WASAPI",
        device_name: "Speakers (Realtek Audio)",
        sample_rate: 48000,
        buffer_size: 256,
    ),
    midi: (
        inputs: [
            ( device_name: "Launchkey Mini MK3 MIDI", channel: Omni ),
        ],
    ),
    qwerty_input_enabled: true,
    theme: "dark",
)
```

## UI state

`ui_state.ron` holds window size/position, last-used preset path, preset browser filter/search, panel collapsed states.

## Crash logs

- A `panic` hook installed at startup writes the stack trace and a context block (recent parameter changes, recent MIDI events, current device, sample rate, buffer size) to `logs/crash-<timestamp>.log`.
- Logs are local. They are **never** uploaded anywhere automatically.
- Rotation keeps the five most recent crash logs.

## File locations summary

```
%APPDATA%\Tone Smithy\
    settings.ron
    ui_state.ron
    presets\
        user\
            ...
    logs\
        crash-2026-05-17_141532.log

%ProgramFiles%\Tone Smithy\
    tonesmithy.exe
    presets\
        factory\
            Bass\
            Lead\
            Pad\
            ...
    assets\
        fonts\
        icons\
```
