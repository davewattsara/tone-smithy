//! Oscillator generators for the subtractive (and later FM) engine.
//!
//! - [`subtractive`] contains the multi-shape phase-accumulating
//!   oscillator used by the subtractive voice (sine, saw, square,
//!   triangle).
//! - [`polyblep`] is the band-limiting residual the saw and square
//!   shapes use to suppress aliasing at high pitches per
//!   `docs/planning/05-design/dsp-and-sound.md`.
//!
//! FM operator generators will land in `oscillator/fm.rs` at M7 per
//! [`project-structure.md`](../../../../docs/planning/06-implementation/project-structure.md).

pub use pitch::midi_note_to_hz;
pub use subtractive::{Oscillator, UnisonOscillator, Waveform};

mod pitch;
mod polyblep;
mod subtractive;
