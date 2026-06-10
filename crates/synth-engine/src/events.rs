//! Engine input events.
//!
//! Every input to the engine is an immutable [`EngineEvent`]. Adapters
//! (`synth-host` for MIDI, `synth-ui` for parameter changes) push events into
//! a queue that the engine drains at the top of each audio block. The engine
//! never calls back into adapters synchronously — see
//! `docs/planning/03-architecture/design-patterns.md` §1.2.
//!
//! M3.3 adds the MIDI controller surface: pitch bend, sustain pedal, mod
//! wheel, channel aftertouch, and arbitrary CC. The mod-matrix wiring that
//! routes these as modulation sources to destinations lives in M6.

use crate::filter::{FilterMode, FilterRouting};
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

    /// Panic / all-notes-off. Releases every sounding voice, clears any
    /// sustain-deferred releases, lifts the sustain latch, and empties
    /// the arpeggiator's held-note set. The recovery path for a stuck
    /// note — emitted by the UI panic control and by MIDI CC 120 (All
    /// Sound Off) and CC 123 (All Notes Off).
    AllNotesOff,

    /// Change the oscillator waveform. Discrete; takes effect on the
    /// next block (the engine drains events before processing).
    SetOscillatorWaveform {
        /// New waveform.
        waveform: Waveform,
    },

    /// Change the filter output mode. Discrete; takes effect at the
    /// next block. The integrator state is preserved across the
    /// transition so mode flips are click-free.
    SetFilterMode {
        /// New filter mode.
        mode: FilterMode,
    },

    /// Change the output mode of the second filter. Discrete; same
    /// click-free semantics as [`SetFilterMode`](Self::SetFilterMode).
    SetFilter2Mode {
        /// New mode for filter 2.
        mode: FilterMode,
    },

    /// Change how the two per-voice filters are connected (serial vs.
    /// parallel). Discrete; takes effect at the next block.
    SetFilterRouting {
        /// New routing between filter 1 and filter 2.
        routing: FilterRouting,
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

    /// MIDI pitch-bend wheel position. `value_normalised` is -1.0 (full
    /// down) to +1.0 (full up); 0.0 is centre / no bend. The engine maps
    /// this to semitones via [`crate::engine::PITCH_BEND_RANGE_SEMIS`].
    PitchBend {
        /// Normalised wheel position, -1.0..=1.0.
        value_normalised: f32,
    },

    /// MIDI sustain pedal (CC #64). While `held` is true incoming
    /// note-offs are deferred; they fire when the pedal is released.
    Sustain {
        /// `true` when the pedal crosses the threshold going down;
        /// `false` when it rises above the threshold.
        held: bool,
    },

    /// MIDI channel aftertouch (0xD0). `value_normalised` is 0..=1.
    /// Stored in the parameter snapshot so the mod matrix (M6) can
    /// route it as a modulation source.
    ChannelAftertouch {
        /// Normalised aftertouch pressure, 0.0..=1.0.
        value_normalised: f32,
    },

    /// MIDI control change (CC). `cc` is the controller number 0..=127;
    /// `value_normalised` is 0..=1. Mod wheel (CC #1) is routed to
    /// [`ParamId::ModWheel`] by the engine; sustain (CC #64) uses the
    /// typed [`Sustain`](Self::Sustain) variant instead. All other CCs
    /// are stored in the snapshot for future mod-matrix wiring (M6).
    ControlChange {
        /// MIDI controller number, 0..=127.
        cc: u8,
        /// Normalised value, 0.0..=1.0.
        value_normalised: f32,
    },
}
