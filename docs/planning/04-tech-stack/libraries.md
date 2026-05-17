# Libraries

Concrete crate choices for v1. Versions are not pinned here; they will be in `Cargo.toml` and updated as part of normal dependency maintenance.

Crates are grouped by where they are used. All listed crates have permissive licences (MIT, Apache-2.0, MIT-or-Apache-2.0, or similar) compatible with whatever licence we ultimately choose — see [`../07-distribution/licensing.md`](../07-distribution/licensing.md). License compliance is checked in CI via `cargo deny`.

> **Note:** the lists below are the *target* set for v1 — crates are added to `Cargo.toml` only at the milestone that actually needs them, per the dependency policy at the foot of this page. The M0 scaffold pulls in the audio I/O, MIDI I/O, UI shell, concurrency, serialisation, logging and error crates; the rest arrive milestone-by-milestone.

## Core (used in `synth-engine`)

- **No third-party DSP crates.** The engine is hand-rolled.
- `num-traits` — generic numeric trait bounds for DSP helpers.
- `wide` — portable SIMD (`f32x4`, `f32x8`) until `std::simd` stabilises on our MSRV.
- `bitflags` — for engine flag enums (filter routing modes, etc.) where it improves readability.
- `smallvec` — for small heap-free vectors used during setup (not on audio path; audio path uses fixed arrays).

## Audio I/O (`synth-host`)

- **`cpal`** — audio I/O. WASAPI on Windows for v1.
  - ASIO support is gated behind a `asio` feature; SDK vendoring is done out-of-tree and is a v1.x effort.

## MIDI I/O (`synth-host`)

- **`midir`** — MIDI device enumeration and input.

## UI (`synth-ui`)

- **`eframe`** — application shell (window, event loop, renderer setup).
- **`egui`** — UI toolkit.
- **`egui_extras`** — table/grid helpers, mostly used by the preset browser and mod matrix.

## Concurrency / parameter bus

- **`crossbeam-channel`** — bounded SPSC/MPSC queues between threads.
- **`arc-swap`** — atomic `Arc` swap for parameter snapshots.
- **`parking_lot`** — better mutexes/condvars for non-audio threads where mutexes are appropriate.
- **`atomic_float`** — atomic float types for telemetry (CPU usage, level meters).

## Serialisation (`synth-presets`, `synth-app`)

- **`serde`** + **`serde_derive`** — derive Serialize / Deserialize for parameter and metadata types.
- **`ron`** — preset, settings, UI-state format.
- **`serde_json`** — developer-mode JSON dump of state (debugging only).

## File system / paths

- **`directories`** — resolve `%APPDATA%` / `~/.config` / `~/Library/Application Support` paths.
- **`rfd`** — native file/folder dialogs for preset import/export.
- **`notify`** — watch the user preset directory for changes (so the browser refreshes when the user drops in a preset pack).

## Logging / observability

- **`tracing`** — structured logging and spans.
- **`tracing-subscriber`** — formatting and sink for `tracing` events.
- **`tracing-appender`** — non-blocking rotating file appender for the log sink.

## Errors

- **`thiserror`** — typed error enums in library crates.
- **`anyhow`** — at the `synth-app` top level.

## Testing & benchmarking

- **`criterion`** — benchmarks for DSP hot paths.
- **`insta`** — snapshot tests for preset (de)serialisation and engine output fingerprints.
- **`assert_no_alloc`** — used in tests to verify no allocations on the audio path.
- **`proptest`** — property-based tests for parameter serialisation round-trips and migration steps.

## Time / clocks

- **`time`** or **`chrono`** — wall-clock timestamps for preset metadata and log entries. Choice deferred to first use; both are acceptable.

## Build / packaging support

- **`cargo-dist`** *(dev-only)* — optional; we may use it to produce release artefacts. Alternative is a small `xtask` crate.
- **`cargo-deny`** *(dev-only)* — licence and advisory checks in CI.

## Notably absent

- **No GUI/HTML rendering crate** (`webview`, `tao`, etc.) — egui owns the entire UI.
- **No async runtime** — `tokio`/`async-std` are not pulled in. The audio thread is synchronous; background work uses regular `std::thread`. If a specific feature later needs futures (e.g. a network update check), `futures-lite` plus `pollster` is the lightweight choice.
- **No ECS / scene-graph framework** — overkill for a single-window synth UI.

## Dependency policy

- Prefer crates with at least one major version published, recent activity, and a permissive licence.
- Add a dependency only when it removes real complexity or risk; avoid trivial wrappers.
- Audit transitive dependency count on each addition (`cargo tree`).
- Every dependency is reviewed by `cargo audit` and `cargo deny` in CI.
