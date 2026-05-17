//! Tone Smithy DSP engine.
//!
//! Houses voice management, oscillators, filters, envelopes, LFOs, the
//! modulation matrix, and effects. Pure DSP — no audio I/O, no MIDI, no UI.
//!
//! See `docs/planning/03-architecture/audio-engine.md` for the architecture
//! and `docs/planning/03-architecture/design-patterns.md` for the real-time
//! safety rules that govern code in this crate.

#![doc(html_no_source)]

/// Compile-time version of the engine, matched to the workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
