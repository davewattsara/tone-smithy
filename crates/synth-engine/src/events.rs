//! Engine input events.
//!
//! Every input to the engine is an immutable [`EngineEvent`]. Adapters
//! (`synth-host` for MIDI, `synth-ui` for parameter changes) push events into
//! a queue that the engine drains at the top of each audio block. The engine
//! never calls back into adapters synchronously — see
//! `docs/planning/03-architecture/design-patterns.md` §1.2.
//!
//! M1 ships only the variants needed for "play a note triggered from the UI"
//! plus the few engine settings exposed in C2/C4 (waveform, pitch offset,
//! release time). Pitch bend, sustain, CC, presets, and the rest of the
//! MIDI surface arrive in later milestones when their consumers exist.

use crate::oscillator::Waveform;
use crate::params::ParamId;

/// One input to the engine.
///
/// Variants are added milestone-by-milestone. Adapters that produce events
/// they cannot yet justify should keep them out of the enum until there is
/// a consumer in the engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineEvent {
    /// Start a note. `note_midi` is the MIDI note number (0..=127);
    /// `velocity` is 0..=127, with 0 conventionally meaning "release".
    NoteOn {
        /// MIDI note number, 0..=127. Middle C is 60.
        note_midi: u8,

        /// MIDI velocity, 0..=127. A velocity of 0 is treated by some
        /// hosts as a note-off; in the engine we still create a voice
        /// at zero gain so the trigger is auditable.
        velocity: u8,
    },

    /// Release the note. `note_midi` matches the corresponding
    /// [`EngineEvent::NoteOn`].
    NoteOff {
        /// MIDI note number, 0..=127.
        note_midi: u8,
    },

    /// Change the oscillator waveform. Discrete; takes effect on the
    /// next block (the engine drains events before processing).
    SetOscillatorWaveform {
        /// New waveform.
        waveform: Waveform,
    },

    /// Change a continuous parameter identified by [`ParamId`]. The
    /// engine clamps/validates per parameter; out-of-range values
    /// are accepted without panicking.
    ParameterChange {
        /// Which parameter to change.
        id: ParamId,

        /// New target value. Units depend on the parameter — see
        /// [`ParamId`] for the convention per variant.
        value: f32,
    },
}
