# Tech stack

The chosen tools, with rationale. Specific crates are in [`libraries.md`](libraries.md); dev tooling and CI are in [`tooling.md`](tooling.md).

## Language

**Rust (stable channel)**.

- Memory-safe, which materially reduces a class of bugs that plague C++ audio code (use-after-free, data races in the audio callback).
- Predictable performance — no GC, manual control over allocation, zero-cost abstractions.
- Excellent FFI when needed.
- Modern tooling out of the box: `cargo`, `rustfmt`, `clippy`.

The trade-off is a smaller ecosystem of audio-specific crates than C++/JUCE. That's accepted because (a) the core DSP is hand-rolled per [DSP & sound design](../05-design/dsp-and-sound.md), and (b) the surrounding glue (audio I/O, MIDI, UI) is well-covered by mature Rust crates.

We pin to the **stable** channel via `rust-toolchain.toml` (current MSRV TBD at the start of M0). Nightly-only features (`std::simd`, allocator hooks for real-time checks) are avoided in the main build; CI may run an extra nightly job for additional checks.

## Audio I/O

**`cpal`** — cross-platform audio I/O library. Used by many production Rust audio projects.

- Targets WASAPI on Windows out of the box (good latency, no extra installation).
- Same API will get us CoreAudio (macOS) and ALSA/JACK/PulseAudio (Linux) for v2.
- ASIO is supported as a build-time feature but requires manually vendoring the Steinberg ASIO SDK. v1 will ship with WASAPI only; ASIO is a v1.x consideration.

## MIDI I/O

**`midir`** — cross-platform MIDI I/O. Same author and design philosophy as `cpal`.

- Robust device enumeration, hot-plug-friendly.
- WinMM backend on Windows works fine for the use cases we care about.

## UI

**`egui`** via **`eframe`**.

- Immediate-mode — fast to iterate, easy to wire to a parameter store, easy to add custom widgets.
- Renders via `wgpu` (good GPU compatibility) or `glow` (OpenGL fallback). `wgpu` is the default.
- Native window via `winit`, included by `eframe`.
- Hot for synth-style dense parameter UIs; many small custom widgets are straightforward to build.

Trade-off: less mature accessibility story than retained-mode frameworks. v1 accepts this; screen-reader support is a v2 item.

## DSP

**Hand-rolled** in pure Rust within the `synth-engine` crate.

- Per the project decision, the synth's character is built in-house rather than inherited from a library.
- We do **not** depend on `fundsp` or similar all-in-one DSP crates for engine internals. We may consult their source for reference but ship our own implementations.
- Utility crates (FFT for analysis, BPM math, simple buffer types) are fair game.

SIMD strategy:
- Hot loops use `std::simd` once it's stable enough on our MSRV.
- Until then, we use the `wide` crate as a portable f32x4/f32x8 layer.
- Scalar fallback path exists for every SIMD path, and a test verifies parity.

## Serialisation

**`serde`** + **`ron`** for human-readable formats (presets, settings, UI state).

- No binary format in v1.
- `serde_json` available as a developer-mode escape hatch.

## Concurrency primitives

**`crossbeam`** for SPSC/MPSC channels and useful concurrency utilities.

- Audio thread uses `crossbeam-channel` bounded queues to receive events from MIDI and GUI threads.
- For the parameter snapshot (single writer = engine; single reader = UI), we use `arc-swap` or a hand-rolled atomic pointer swap with `Arc`.

## Logging

**`tracing`** + **`tracing-subscriber`**.

- Audio thread: no logging in release; debug builds may use a non-allocating ring buffer collected from outside.
- Other threads: standard `tracing` spans and events; subscriber writes to a rolling file in the user data directory.

## Error handling

**`thiserror`** for crate-internal error types (typed, structured).
**`anyhow`** at the application boundary in `synth-app` for ergonomic top-level error handling.

## Why not these alternatives

- **C++/JUCE** — the industry default. Rejected because the project explicitly chose Rust for safety and ergonomics, and because plugin-format support (the main JUCE advantage) is out of scope until v2 (and Rust has `nih-plug` for that).
- **`fundsp`** for the engine — would shortcut v1 but compromise the synth's audible identity.
- **`iced` instead of `egui`** — retained-mode, better accessibility, but more ceremony for dense knob-heavy panels. Picked `egui` for synth UX fit.
- **A web stack (Tauri + Web Audio)** — UI would be easier, audio engine would not match native quality for a flagship.
