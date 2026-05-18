//! Parameter ids and the M1 parameter snapshot.
//!
//! M1's parameter system is intentionally minimal — just the two
//! continuous parameters the milestone's parameter-bus prototype routes
//! (pitch offset, amp release) plus the discrete waveform selection.
//! M2 lifts this into the full typed `ParameterTree` with stable
//! `ParamId`s, smoothing, and the snapshot pool described in
//! `docs/planning/03-architecture/design-patterns.md` §1.3–§1.4 and
//! §2.5.

use crate::oscillator::Waveform;

/// Identifies a continuous parameter for [`EngineEvent::ParameterChange`].
///
/// Discrete parameters (e.g. waveform) have their own typed
/// `EngineEvent` variants so the value type is checked at compile time
/// rather than reinterpreted from `f32`.
///
/// [`EngineEvent::ParameterChange`]: crate::EngineEvent::ParameterChange
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamId {
    /// Pitch offset applied on top of held MIDI note, in semitones.
    /// Range -24..=24 by convention; the engine does not clamp.
    PitchOffsetSemis,

    /// Amp envelope release time, in seconds. Range 0.001..=10.0 by
    /// convention; the envelope clamps below one sample period.
    AmpReleaseSecs,
}

/// An immutable snapshot of the engine's outward-facing parameter
/// state, published once per audio block.
///
/// Built by [`Engine::snapshot`] without allocating. The audio callback
/// is responsible for wrapping it in an `Arc` and storing in the
/// snapshot slot — the recycled-pool optimisation from
/// `docs/planning/03-architecture/design-patterns.md` §2.5 is a later
/// milestone.
///
/// [`Engine::snapshot`]: crate::Engine::snapshot
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSnapshot {
    /// Current pitch offset, in semitones.
    pub pitch_offset_semis: f32,

    /// Current amp release time, in seconds.
    pub amp_release_secs: f32,

    /// Current oscillator waveform.
    pub waveform: Waveform,

    /// True if a voice is currently producing audio (not idle).
    pub voice_active: bool,
}

impl Default for ParamSnapshot {
    fn default() -> Self {
        Self {
            pitch_offset_semis: 0.0,
            amp_release_secs: 0.200,
            waveform: Waveform::Sine,
            voice_active: false,
        }
    }
}
