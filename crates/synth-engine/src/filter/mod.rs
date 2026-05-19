//! Filter section for the subtractive voice.
//!
//! M2 ships the single TPT state-variable filter (12 dB/oct, four
//! modes — LP / HP / BP / Notch). The 24 dB/oct ladder option, the
//! second filter slot, and the serial/parallel routing between them
//! are all v1.1 per `docs/planning/05-design/dsp-and-sound.md` and the
//! `ladder.rs` slot predicted in
//! `docs/planning/06-implementation/project-structure.md` stays empty
//! until then.

pub use svf::{FilterMode, StateVariableFilter};

mod svf;
