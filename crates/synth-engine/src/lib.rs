//! Tone Smithy DSP engine.
//!
//! Houses voice management, oscillators, filters, envelopes, LFOs, the
//! modulation matrix, and effects. Pure DSP — no audio I/O, no MIDI, no UI.
//!
//! See `docs/planning/03-architecture/audio-engine.md` for the architecture
//! and `docs/planning/03-architecture/design-patterns.md` for the real-time
//! safety rules that govern code in this crate.
//!
//! M2 exposes the public surface needed to drive the engine from the
//! host: [`Engine`] and [`EngineEvent`] for the audio thread,
//! [`ParamId`] / [`ParamSnapshot`] for the parameter bus, and the
//! underlying [`Oscillator`] / [`Adsr`] / [`Voice`] / [`ParameterTree`]
//! types for unit testing.

#![doc(html_no_source)]

/// Compile-time version of the engine, matched to the workspace `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use crate::engine::{Engine, MAX_BLOCK_SIZE};
pub use crate::envelope::Adsr;
pub use crate::events::EngineEvent;
pub use crate::oscillator::{Oscillator, Waveform, midi_note_to_hz};
pub use crate::params::{ParamId, ParamSnapshot, ParameterTree, SampleParams};
pub use crate::smoothing::{DEFAULT_TIME_CONSTANT_MS, SmoothedParam};
pub use crate::voice::Voice;

pub mod param_bus;

mod engine;
mod envelope;
mod events;
mod oscillator;
mod params;
mod smoothing;
mod voice;
