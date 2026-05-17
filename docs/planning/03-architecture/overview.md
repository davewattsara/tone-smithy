# Architecture overview

A high-level map of the system. The detail of each subsystem lives in its own document.

## Guiding principles

1. **Real-time first.** The audio thread is treated as hard real-time. No allocation, no locks, no syscalls, no blocking I/O on it. Every other concern bends around this rule.
2. **Engine independent of host.** The DSP engine should not know about cpal, egui, MIDI hardware, or the file system. It exposes a parameter interface and a `process(buffer)` method. This keeps the door open for adding plugin formats in v2.
3. **One source of truth for parameters.** All state lives in a parameter tree owned by the engine. The UI reads snapshots and writes change events. Presets are serialised parameter trees.
4. **Composition over inheritance.** Rust pushes us toward small types and explicit composition; we lean into that rather than fighting it with trait objects.
5. **Boring is good.** Prefer well-understood, audited DSP techniques (PolyBLEP, TPT state-variable filters, FDN reverbs) before clever ones. Distinctiveness comes from tuning and combination, not from novel algorithms.

## Runtime threads

| Thread | Owner | Responsibilities |
| --- | --- | --- |
| **Audio** | `cpal` callback | Pull MIDI events from a lock-free queue; pull parameter changes from a lock-free queue; run voice manager; write samples to the output buffer. |
| **MIDI** | `midir` callback | Receive MIDI bytes; convert to engine events; push to the audio thread queue. |
| **GUI** | `eframe` event loop | Render egui; respond to user input; write parameter change events to the audio thread queue; read engine telemetry (CPU, levels, voice count). |
| **Background** | std thread pool | Preset scanning, file I/O, settings load/save, factory bank indexing. |

Cross-thread communication is via `crossbeam` SPSC/MPSC channels and atomics. The audio thread is the consumer of MIDI and UI events; it never blocks waiting on them.

## Module split

The codebase is a Cargo workspace with the following crates (see [`../06-implementation/project-structure.md`](../06-implementation/project-structure.md) for the on-disk layout):

- **`synth-engine`** — pure DSP. Oscillators, filters, envelopes, LFOs, voice manager, modulation matrix, effects. No I/O dependencies. Highly testable.
- **`synth-host`** — bridges the engine to the world. Audio I/O via `cpal`, MIDI I/O via `midir`, parameter event ring buffers, lock-free queues.
- **`synth-ui`** — egui front end. Custom widgets, panel layout, theming, parameter binding.
- **`synth-presets`** — preset format (RON), validation, browsing, factory vs user separation, file I/O.
- **`synth-app`** — the binary. Wires the four other crates together and owns the application lifecycle.

The dependency graph is acyclic:

```
                synth-app
              /     |     \
       synth-ui  synth-host  synth-presets
              \     |     /
                 synth-engine
```

## Parameter model

A single, statically-typed parameter tree owns all sound-affecting state. Each parameter has:

- A stable **id** (used in presets and for MIDI Learn mappings).
- A **type** (float in a range, integer, enum, boolean).
- A **default**.
- Optional **modulation slot** if the parameter is a destination.

The tree is owned by the engine. The UI keeps a mirror snapshot for rendering. Changes flow UI → engine through a single SPSC ring of `(id, value)` events. Modulation is added on top by the engine; the UI sees a base value plus a current modulated value for display.

This approach maps cleanly onto plugin parameter systems later — `nih-plug` and CLAP both prefer flat, stable id-based parameter sets.

## Preset = parameter snapshot

A preset is the serialised parameter tree plus metadata (name, author, category, tags, comment). The format is **RON** (Rusty Object Notation), chosen because:

- It is human-readable (presets can be inspected and hand-edited).
- It plays well with `serde`.
- It tolerates evolving schemas if we include a top-level version field and a small migration step on load.

See [`persistence.md`](persistence.md).

## What this overview does *not* cover

- Specific DSP algorithms — [`audio-engine.md`](audio-engine.md).
- UI architecture and widget design — [`ui-layer.md`](ui-layer.md).
- MIDI device handling — [`midi-and-input.md`](midi-and-input.md).
- File formats and locations — [`persistence.md`](persistence.md).
