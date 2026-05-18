# Project structure

The on-disk layout of the repository. Aims for clarity and a clean dependency graph; one workspace, several small focused crates, all binary output coming from a single `synth-app` crate.

## Repository layout

```
/
├── Cargo.toml                    # workspace manifest
├── Cargo.lock
├── rust-toolchain.toml           # pinned stable toolchain
├── rustfmt.toml
├── clippy.toml
├── .editorconfig
├── .gitignore
├── .gitattributes
├── deny.toml                     # cargo-deny config (licences, advisories)
│
├── README.md                     # user-facing project overview
├── LICENSE-MIT                   # dual-licensed: MIT
├── LICENSE-APACHE                # dual-licensed: Apache-2.0
│
├── crates/
│   ├── synth-engine/             # pure DSP + parameter-bus port types, no I/O
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── voice.rs
│   │   │   ├── voice_manager.rs
│   │   │   ├── oscillator/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── subtractive.rs
│   │   │   │   ├── polyblep.rs
│   │   │   │   └── fm.rs
│   │   │   ├── filter/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── svf.rs
│   │   │   │   └── ladder.rs
│   │   │   ├── envelope.rs
│   │   │   ├── smoothing.rs      # one-pole parameter smoother (design-patterns §2.6)
│   │   │   ├── lfo.rs
│   │   │   ├── modulation.rs
│   │   │   ├── effects/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── eq.rs
│   │   │   │   ├── drive.rs
│   │   │   │   ├── chorus.rs
│   │   │   │   ├── delay.rs
│   │   │   │   └── reverb.rs
│   │   │   ├── params/
│   │   │   │   ├── mod.rs        # parameter tree, ids, defaults
│   │   │   │   └── snapshot.rs
│   │   │   ├── param_bus.rs      # lock-free SPSC + ArcSwap snapshot slot
│   │   │   ├── events.rs         # EngineEvent enum
│   │   │   └── engine.rs         # top-level process() + lifecycle
│   │   ├── benches/
│   │   │   ├── oscillator.rs
│   │   │   ├── filter.rs
│   │   │   ├── fm.rs
│   │   │   └── reverb.rs
│   │   └── tests/
│   │       ├── engine_snapshot.rs
│   │       └── no_alloc.rs
│   │
│   ├── synth-host/               # audio + MIDI I/O
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── audio.rs          # cpal integration
│   │       ├── midi.rs           # midir integration
│   │       └── settings.rs       # audio/MIDI device selection
│   │
│   ├── synth-presets/            # preset format, browser, I/O
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── format.rs         # RON schema + (de)serialisation
│   │       ├── migration.rs      # version migrations
│   │       ├── browser.rs        # in-memory index, search, filter
│   │       └── paths.rs          # factory vs user, OS-specific paths
│   │
│   ├── synth-ui/                 # egui front end
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs            # top-level egui App
│   │       ├── theme.rs          # palette, type scale, tokens
│   │       ├── widgets/
│   │       │   ├── mod.rs
│   │       │   ├── knob.rs
│   │       │   ├── slider.rs
│   │       │   ├── toggle.rs
│   │       │   ├── dropdown.rs
│   │       │   ├── step_grid.rs
│   │       │   ├── mod_row.rs
│   │       │   ├── name_editor.rs
│   │       │   └── meter.rs
│   │       ├── panels/
│   │       │   ├── mod.rs
│   │       │   ├── header.rs
│   │       │   ├── oscillators.rs
│   │       │   ├── filter.rs
│   │       │   ├── envelopes_lfos.rs
│   │       │   ├── mod_matrix.rs
│   │       │   ├── arp_seq.rs
│   │       │   ├── effects.rs
│   │       │   ├── master.rs
│   │       │   ├── browser.rs
│   │       │   ├── virtual_keyboard.rs
│   │       │   └── footer.rs
│   │       └── ui_state.rs       # window layout state persistence
│   │
│   └── synth-app/                # the binary
│       ├── Cargo.toml
│       └── src/
│           └── main.rs           # wires engine + host + ui + presets
│                                 # builds to tonesmithy.exe
│
├── xtask/                        # build tasks (dist, installer, etc.)
│   ├── Cargo.toml
│   └── src/main.rs
│
├── assets/
│   ├── fonts/
│   ├── icons/
│   └── presets/
│       └── factory/
│           ├── Bass/
│           ├── Lead/
│           ├── Pad/
│           ├── Pluck/
│           ├── Keys/
│           └── FX/
│
├── installer/
│   ├── installer.iss             # Inno Setup script
│   └── README.md
│
├── docs/
│   ├── planning/                 # this folder
│   └── user/                     # getting-started, manual (added later)
│
├── .github/
│   └── workflows/
│       └── ci.yml
│
└── .githooks/
    └── pre-commit
```

## Workspace dependency graph

```
synth-app  ──▶  synth-ui  ──▶  synth-engine
       └──▶  synth-host    ─▶  synth-engine
       └──▶  synth-presets ─▶  synth-engine
                                  ▲
                                  │
                            (engine has no internal deps)
```

No cycles. The engine is a leaf and can be reasoned about and tested in isolation.

## Why these boundaries

- **`synth-engine` separate** so it has no I/O dependencies. Adding plugin formats in v2 is then a matter of building a new "host" alongside `synth-host`. The parameter-bus port types (`param_bus.rs`) live here rather than in `synth-host` because both `synth-host` and `synth-ui` pass raw bus types in their public APIs — putting them in the engine keeps both adapters layered above one shared definition, and the only deps it pulls in are `crossbeam-channel` and `arc-swap` (pure-Rust concurrency primitives, not I/O).
- **`synth-presets` separate** because the preset format is data-only and shouldn't be locked behind a UI or I/O dependency. It depends on `synth-engine` only for parameter types.
- **`synth-ui` separate** so the UI can be developed, tested, and styled without dragging audio I/O into every build.
- **`synth-app`** is the only crate that knows about all four — it's the assembly point.

## File naming conventions

- `snake_case.rs` for files and modules.
- `mod.rs` for module roots when a module is a directory.
- One primary type per file where reasonable; helpers in the same file are fine.
- Test files in `tests/` for integration tests; unit tests inline via `#[cfg(test)] mod tests`.

## Lockfile and dependency hygiene

- `Cargo.lock` is committed.
- `cargo update` is run deliberately (not as part of normal work), with the diff reviewed.
- New dependencies require a short justification in the PR description.
