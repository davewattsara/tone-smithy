//! Audio and MIDI I/O for Tone Smithy.
//!
//! Wraps `cpal` (audio output) and `midir` (MIDI input). M1 wires the
//! audio side end-to-end and drives the engine via the parameter bus
//! defined in `synth_engine::param_bus`; MIDI lands at M3.
//!
//! See `docs/planning/03-architecture/overview.md` for the threading model
//! and `docs/planning/03-architecture/design-patterns.md` for the real-time
//! safety rules that govern audio-callback code.

pub mod audio;
