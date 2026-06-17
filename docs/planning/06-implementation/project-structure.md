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
├── deny.toml                     # cargo-deny config (licences, advisories)
├── about.toml                    # cargo-about config (third-party licence generation)
├── about.hbs                     # cargo-about HTML template
├── .gitignore
├── .gitattributes
│
├── README.md                     # user-facing project overview
├── CHANGELOG.md                  # release notes
├── instructions.md               # end-user quick-start (shipped with installer)
├── LICENSE-MIT                   # dual-licensed: MIT
├── LICENSE-APACHE                # dual-licensed: Apache-2.0
│
├── crates/
│   ├── synth-engine/             # pure DSP + parameter-bus port types, no I/O
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs         # top-level process() + lifecycle
│   │       ├── voice.rs
│   │       ├── voice_manager.rs
│   │       ├── oscillator/
│   │       │   ├── mod.rs
│   │       │   ├── subtractive.rs
│   │       │   ├── polyblep.rs
│   │       │   └── pitch.rs      # pitch helpers (bend, transpose, detune)
│   │       ├── fm.rs             # FM operator + algorithm
│   │       ├── slot.rs           # FM slot routing
│   │       ├── filter/
│   │       │   ├── mod.rs
│   │       │   └── svf.rs        # state-variable filter (2-pole + 4-pole cascade)
│   │       ├── envelope.rs       # ADSR amp/filter envelopes
│   │       ├── mod_env.rs        # Env3 modulation envelope
│   │       ├── mod_matrix.rs     # 16-slot modulation matrix
│   │       ├── lfo.rs
│   │       ├── arp.rs            # arpeggiator
│   │       ├── seq.rs            # step sequencer
│   │       ├── panning.rs        # stereo pan law
│   │       ├── halfband.rs       # half-band FIR for oversampling
│   │       ├── smoothing.rs      # one-pole parameter smoother (design-patterns §2.6)
│   │       ├── fx/
│   │       │   ├── mod.rs
│   │       │   ├── biquad.rs     # generic biquad building block
│   │       │   ├── eq.rs
│   │       │   ├── drive.rs
│   │       │   ├── chorus.rs
│   │       │   ├── delay.rs
│   │       │   └── reverb.rs
│   │       ├── params/
│   │       │   ├── mod.rs        # parameter tree, ids, defaults
│   │       │   ├── ids.rs        # ParamId enum
│   │       │   ├── tree.rs       # parameter metadata + range definitions
│   │       │   └── snapshot.rs
│   │       ├── param_bus.rs      # lock-free SPSC + ArcSwap snapshot slot
│   │       ├── events.rs         # EngineEvent enum
│   │       └── tests/
│   │           └── no_alloc.rs
│   │
│   ├── synth-host/               # audio + MIDI I/O
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── audio.rs          # cpal integration
│   │       └── midi.rs           # midir integration
│   │
│   ├── synth-presets/            # preset format, factory bank, I/O
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── format.rs         # RON schema + (de)serialisation
│   │       ├── migrate.rs        # version migrations
│   │       ├── factory.rs        # compiled-in factory bank (120 presets)
│   │       ├── preset_params.rs  # preset↔ParamSnapshot conversion
│   │       ├── io.rs             # user preset load/save paths
│   │       └── settings.rs       # audio/MIDI device selection persistence
│   │
│   ├── synth-ui/                 # egui front end
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app/              # top-level egui App
│   │       │   ├── mod.rs
│   │       │   ├── state.rs      # UiState — window layout + transient UI state
│   │       │   ├── chrome.rs     # menu bar, title bar, window frame
│   │       │   ├── preset.rs     # preset load/save/rename actions
│   │       │   ├── mod_display.rs# mod-matrix display helpers
│   │       │   ├── midi_learn.rs # MIDI-learn overlay logic
│   │       │   ├── wizard.rs     # first-run device-setup wizard
│   │       │   └── utils.rs
│   │       ├── theme.rs          # palette, type scale, tokens
│   │       ├── knob.rs           # custom knob widget
│   │       ├── toggle.rs         # toggle / LED widget
│   │       ├── meter.rs          # level meter widget
│   │       ├── keyboard.rs       # on-screen piano keyboard widget
│   │       ├── computer_keyboard.rs # computer-key→note mapping
│   │       ├── midi_learn_ext.rs # egui extension trait for MIDI-learn right-click
│   │       └── sections/         # one file per UI panel
│   │           ├── mod.rs
│   │           ├── osc.rs
│   │           ├── filter.rs
│   │           ├── envelopes.rs
│   │           ├── modulation.rs # mod matrix panel
│   │           ├── fm_slots.rs   # FM operator slots panel
│   │           ├── arp.rs        # arpeggiator panel
│   │           ├── seq.rs        # step sequencer panel
│   │           ├── fx/
│   │           │   ├── mod.rs
│   │           │   ├── eq.rs
│   │           │   ├── drive.rs
│   │           │   ├── chorus.rs
│   │           │   ├── delay.rs
│   │           │   └── reverb.rs
│   │           ├── master.rs
│   │           ├── browser.rs    # preset browser panel
│   │           └── settings.rs   # audio/MIDI settings panel
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
├── installer/
│   └── installer.iss             # Inno Setup script (Windows)
│
├── docs/
│   ├── planning/                 # this folder
│   ├── conversations/            # session logs
│   └── getting-started.md
│
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml           # builds + publishes GitHub Release on v* tag
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
- **`synth-presets` separate** because the preset format is data-only and shouldn't be locked behind a UI or I/O dependency. It depends on `synth-engine` only for parameter types. Factory presets are compiled in via `factory.rs` rather than shipped as loose files.
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
