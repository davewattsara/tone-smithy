# M10 — Preset Save / Load

**Status:** Complete (tag `m10`)

## Goal

Any patch can be saved to disk and reloaded with bit-identical parameter state. A round-trip test exercises every saveable parameter. A minimal save/load UI surface lives in the header bar; the polished browser arrives in M12.

## Scope

- **RON format** — `Preset` struct with `version`, `metadata`, `parameters` (`BTreeMap<String, f32>`), `midi_learn`
- **Schema versioning** — `version: 1` from day one; a `migrate()` stub in place so future migrations have a home
- **`synth-presets` crate** — saves/loads `.tsmith` files; `snapshot_to_map` + `map_to_events` conversion pair; user presets directory via `directories` crate
- **UI surface** — patch name text field + Save + Load buttons in the header bar; native file dialogs via `rfd`
- **Round-trip test** — deterministic test that fills every saveable param with non-default values, saves, loads, and compares

## Saveable params

All `ParamSnapshot` fields that represent deliberate patch state (NOT live MIDI or runtime state):

- Waveform and filter mode (encoded as index)
- Pitch offset, master volume
- Amp envelope (A/D/S/R)
- Filter cutoff/resonance
- Osc 1 level/detune/pan, unison (voices/detune/spread)
- LFO 1 & 2 (rate, shape, reset, sync, division)
- Env2 (A/D/S/R/curves)
- Global BPM
- Mod matrix (8 slots: enabled/source/dest/amount/via)
- FM: slot mode/level/pan, algorithm, per-operator (ratio int/fine/level/ADSR/feedback) for both slots
- FX chain (EQ/drive/chorus/delay/reverb — all params)
- Arpeggiator (enabled/mode/octaves/rate/bpm/gate/swing)

NOT saved: `active_voice_count`, `pitch_bend_semis`, `mod_wheel`, `channel_aftertouch`, `cc_values`, `lfo1_out`, `lfo2_out`, `env2_out`.

## Architecture

```
synth-presets
  format.rs    — Preset / PresetMetadata / MidiLearnEntry (serde RON)
  migrate.rs   — CURRENT_VERSION = 1, migrate() stub
  preset_params.rs — snapshot_to_map(), map_to_events(), map_to_snapshot()
  io.rs        — save(), load(), user_presets_dir()
  lib.rs       — module decls, public re-exports
```

`synth-ui` depends on `synth-presets` and `rfd`. On Save: `snapshot_to_map` → `Preset` → `io::save`. On Load: `io::load` → `map_to_snapshot` → `sync_from_snapshot` + `map_to_events` → push to event bus.

## Key files touched

- `Cargo.toml` — add `rfd`, `directories` workspace deps
- `crates/synth-engine/src/oscillator/subtractive.rs` — `Waveform::index()` / `from_index()`
- `crates/synth-engine/src/filter/svf.rs` — `FilterMode::index()` / `from_index()`
- `crates/synth-presets/` — new crate implementation (5 files)
- `crates/synth-ui/Cargo.toml` — add `synth-presets`, `rfd`
- `crates/synth-ui/src/app.rs` — preset bar UI, `sync_from_snapshot()`, load/save handlers

## Done when

- Any patch can be saved and reloaded with bit-identical parameter state
- Round-trip test passes in CI
- UI has Save and Load buttons in the header bar that work
