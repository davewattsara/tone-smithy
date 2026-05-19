//! MIDI input via `midir`.
//!
//! Opens the first available MIDI input port at startup and routes
//! note-on / note-off messages into the engine's event bus. The
//! `midir` callback runs on its own OS thread; events are pushed
//! through a cloned [`EngineEventSender`] (the bus is MPMC, so the
//! audio thread sees a coherent stream from every input source).
//!
//! M3.2 scope: notes with velocity, omnidirectional channel. CCs
//! (mod wheel, sustain, pitch bend, aftertouch) and the device picker
//! land later in M3 / M13 respectively. Hot-plug is intentionally
//! deferred — if the user plugs in a controller after launch, they
//! restart the app for now. The audio-device hot-plug limitation
//! documented at M4 in `milestones.md` is the same shape of issue.

use midir::{Ignore, MidiInput, MidiInputConnection};
use synth_engine::EngineEvent;
use synth_engine::param_bus::EngineEventSender;
use thiserror::Error;

/// Client name `midir` reports to the host MIDI subsystem. Shows up
/// in port pickers in DAWs and MIDI utilities.
const MIDI_CLIENT_NAME: &str = "Tone Smithy";
/// Connection name reported for our input. Distinct from the client
/// name so a future MIDI output (M8+) can sit alongside it.
const MIDI_CONNECTION_NAME: &str = "tone-smithy-midi-in";

/// Errors that can occur while initialising or opening a MIDI input.
#[derive(Debug, Error)]
pub enum MidiError {
    /// `midir` could not initialise its host MIDI client.
    #[error("midir init error: {0}")]
    Init(#[from] midir::InitError),

    /// `midir` could not open the chosen MIDI input port. The inner
    /// `ConnectError<MidiInput>` is stored as a string so the variant
    /// is `Sync` (the wrapped `MidiInput` itself isn't), letting this
    /// error flow through `anyhow::Context`.
    #[error("could not connect to MIDI port: {0}")]
    Connect(String),

    /// The chosen port returned no readable name.
    #[error("could not read MIDI port name: {0}")]
    PortName(#[from] midir::PortInfoError),
}

impl From<midir::ConnectError<MidiInput>> for MidiError {
    fn from(error: midir::ConnectError<MidiInput>) -> Self {
        Self::Connect(error.to_string())
    }
}

/// A running MIDI input stream.
///
/// The underlying [`MidiInputConnection`] stays alive while this
/// value lives — drop it to close the port. If no MIDI device was
/// available at startup, [`port_name`](Self::port_name) returns
/// `None` and the value is harmlessly inert.
pub struct MidiInputStream {
    /// Owned `midir` connection. `Option` so we can return a no-op
    /// stream when no MIDI device is connected without distinguishing
    /// it at the type level.
    _connection: Option<MidiInputConnection<EngineEventSender>>,

    /// Human-readable name of the opened port, or `None` if none was
    /// available. Used by the composition root for the status line.
    port_name: Option<String>,
}

impl MidiInputStream {
    /// Opens the first available MIDI input port (if any) and routes
    /// note events into `sender`.
    ///
    /// Returns `Ok` with `port_name == None` if no MIDI device is
    /// present — that's a normal startup state, not a failure.
    ///
    /// # Errors
    ///
    /// Returns [`MidiError`] if `midir` cannot initialise, if reading
    /// a port name fails, or if the chosen port refuses to connect.
    pub fn start(sender: EngineEventSender) -> Result<Self, MidiError> {
        let mut midi_in = MidiInput::new(MIDI_CLIENT_NAME)?;
        // We only care about voice messages right now. Ignoring SysEx,
        // time, and active-sensing here trims the callback's input;
        // the parser would ignore them anyway, but skipping the
        // bytes is cheaper.
        midi_in.ignore(Ignore::All);

        let ports = midi_in.ports();
        let Some(port) = ports.first() else {
            return Ok(Self {
                _connection: None,
                port_name: None,
            });
        };
        let port_name = midi_in.port_name(port)?;
        tracing::info!("opening MIDI input port: {port_name}");

        let connection = midi_in.connect(
            port,
            MIDI_CONNECTION_NAME,
            move |_stamp, message, sender| {
                if let Some(event) = parse_midi_message(message) {
                    sender.send(event);
                }
            },
            sender,
        )?;

        Ok(Self {
            _connection: Some(connection),
            port_name: Some(port_name),
        })
    }

    /// Returns the name of the connected port, or `None` if no MIDI
    /// device was available at startup.
    #[must_use]
    pub fn port_name(&self) -> Option<&str> {
        self.port_name.as_deref()
    }
}

/// Parses a single MIDI message into an [`EngineEvent`], or returns
/// `None` if the message is empty, malformed, or one we don't yet
/// handle (CCs, pitch bend, aftertouch — all M3.3 scope).
///
/// Channel filtering is omnidirectional in M3.2: the channel nibble
/// is masked off and ignored.
fn parse_midi_message(message: &[u8]) -> Option<EngineEvent> {
    let status_byte = *message.first()?;
    let status = status_byte & 0xF0;
    match status {
        // Note On (0x9n).
        0x90 => {
            let note = *message.get(1)?;
            let velocity = *message.get(2)?;
            // Velocity-0 Note On is the running-status idiom for
            // Note Off — every hardware controller produces it.
            if velocity == 0 {
                Some(EngineEvent::NoteOff { note_midi: note })
            } else {
                Some(EngineEvent::NoteOn {
                    note_midi: note,
                    velocity,
                })
            }
        }
        // Note Off (0x8n). Release velocity (byte 2) is ignored in v1.
        0x80 => {
            let note = *message.get(1)?;
            Some(EngineEvent::NoteOff { note_midi: note })
        }
        // CCs, program change, pitch bend, aftertouch — M3.3.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_on_message_is_parsed_with_velocity() {
        let event = parse_midi_message(&[0x90, 60, 100]);
        assert!(matches!(
            event,
            Some(EngineEvent::NoteOn {
                note_midi: 60,
                velocity: 100
            })
        ));
    }

    #[test]
    fn note_off_message_is_parsed() {
        let event = parse_midi_message(&[0x80, 60, 64]);
        assert!(matches!(event, Some(EngineEvent::NoteOff { note_midi: 60 })));
    }

    #[test]
    fn velocity_zero_note_on_is_treated_as_note_off() {
        // Running-status idiom: hardware controllers send Note On
        // with velocity 0 instead of an explicit Note Off.
        let event = parse_midi_message(&[0x90, 72, 0]);
        assert!(matches!(event, Some(EngineEvent::NoteOff { note_midi: 72 })));
    }

    #[test]
    fn channel_nibble_is_ignored() {
        // 0x91 = Note On, channel 2. Should still parse.
        let event = parse_midi_message(&[0x91, 60, 100]);
        assert!(matches!(
            event,
            Some(EngineEvent::NoteOn {
                note_midi: 60,
                velocity: 100
            })
        ));
        // 0x8F = Note Off, channel 16. Same.
        let event = parse_midi_message(&[0x8F, 60, 64]);
        assert!(matches!(event, Some(EngineEvent::NoteOff { note_midi: 60 })));
    }

    #[test]
    fn empty_message_returns_none() {
        assert!(parse_midi_message(&[]).is_none());
    }

    #[test]
    fn truncated_note_on_returns_none() {
        // Note On status byte but missing velocity byte.
        assert!(parse_midi_message(&[0x90, 60]).is_none());
        // Even shorter.
        assert!(parse_midi_message(&[0x90]).is_none());
    }

    #[test]
    fn unhandled_status_returns_none() {
        // 0xB0 = CC, will be handled in M3.3 but ignored for now.
        assert!(parse_midi_message(&[0xB0, 7, 100]).is_none());
        // 0xE0 = pitch bend, also M3.3.
        assert!(parse_midi_message(&[0xE0, 0, 64]).is_none());
    }

    #[test]
    fn note_on_routes_into_engine_event_sender() {
        // End-to-end check that parse_midi_message + EngineEventSender
        // delivers a NoteOn the receiver actually sees. This mirrors
        // what the midir callback does in production.
        let (tx, rx, _slot) = synth_engine::param_bus::new_param_bus();
        if let Some(event) = parse_midi_message(&[0x90, 64, 110]) {
            tx.send(event);
        }
        let received = rx.try_recv().expect("event should be queued");
        assert!(matches!(
            received,
            EngineEvent::NoteOn {
                note_midi: 64,
                velocity: 110
            }
        ));
    }
}
