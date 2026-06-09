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
    /// Opens the first available MIDI input port (if any) and routes events
    /// into `sender`. Returns with `port_name == None` if no port is present.
    ///
    /// # Errors
    ///
    /// Returns [`MidiError`] if `midir` cannot initialise or the port refuses
    /// to connect.
    pub fn start(sender: EngineEventSender) -> Result<Self, MidiError> {
        Self::start_on_port(None, sender)
    }

    /// Like [`start`] but opens the port whose name equals `port_name`.
    /// Falls back to the first available port if `port_name` is `None` or
    /// cannot be matched.
    ///
    /// # Errors
    ///
    /// Returns [`MidiError`] if `midir` cannot initialise or the chosen port
    /// refuses to connect.
    pub fn start_on_port(port_name: Option<&str>, sender: EngineEventSender) -> Result<Self, MidiError> {
        let mut midi_in = MidiInput::new(MIDI_CLIENT_NAME)?;
        midi_in.ignore(Ignore::All);

        let ports = midi_in.ports();
        if ports.is_empty() {
            return Ok(Self {
                _connection: None,
                port_name: None,
            });
        }

        // Try to match by name; fall back to the first port.
        let port = if let Some(target) = port_name {
            ports
                .iter()
                .find(|p| midi_in.port_name(p).as_deref() == Ok(target))
                .unwrap_or(&ports[0])
        } else {
            &ports[0]
        };

        let name = midi_in.port_name(port)?;
        tracing::info!("opening MIDI input port: {name}");

        let connection = midi_in.connect(
            port,
            MIDI_CONNECTION_NAME,
            move |_stamp, message, sender| {
                // Quiet diagnostic: silent by default, surfaced with
                // `RUST_LOG=synth_host=debug`. Lets us confirm whether the
                // controller is actually delivering bytes to the open port
                // when troubleshooting "connected but no notes" reports (a
                // stuck WinMM/USB routing state, cleared by a device replug).
                tracing::debug!("MIDI in: {message:02X?}");
                if let Some(event) = parse_midi_message(message) {
                    sender.send(event);
                }
            },
            sender,
        )?;

        Ok(Self {
            _connection: Some(connection),
            port_name: Some(name),
        })
    }

    /// Returns the name of the connected port, or `None` if no MIDI device
    /// was available at startup.
    #[must_use]
    pub fn port_name(&self) -> Option<&str> {
        self.port_name.as_deref()
    }
}

/// Returns the names of all currently visible MIDI input ports.
///
/// # Errors
///
/// Returns [`MidiError`] if `midir` cannot initialise.
pub fn list_ports() -> Result<Vec<String>, MidiError> {
    let midi_in = MidiInput::new(MIDI_CLIENT_NAME)?;
    let names = midi_in
        .ports()
        .iter()
        .filter_map(|p| midi_in.port_name(p).ok())
        .collect();
    Ok(names)
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
        // Control change (0xBn): mod wheel, sustain pedal, arbitrary CC.
        0xB0 => {
            let cc = *message.get(1)?;
            let raw = *message.get(2)?;
            if cc == 64 {
                // Sustain pedal: threshold at 64, matching GM spec.
                Some(EngineEvent::Sustain { held: raw >= 64 })
            } else if cc == 120 || cc == 123 {
                // Channel-mode messages: 120 = All Sound Off,
                // 123 = All Notes Off. Both map to our panic so a
                // controller's panic button (or a DAW's) stops stuck
                // notes.
                Some(EngineEvent::AllNotesOff)
            } else {
                Some(EngineEvent::ControlChange {
                    cc,
                    value_normalised: f32::from(raw) / 127.0,
                })
            }
        }
        // Pitch bend (0xEn): 14-bit value across two 7-bit bytes.
        0xE0 => {
            let lsb = *message.get(1)?;
            let msb = *message.get(2)?;
            let raw = (u16::from(msb) << 7) | u16::from(lsb);
            // Centre = 8192; divide by 8192 so full-down = -1.0 exactly.
            let value_normalised = (raw as f32 - 8192.0) / 8192.0;
            Some(EngineEvent::PitchBend { value_normalised })
        }
        // Channel aftertouch (0xDn): single pressure byte.
        0xD0 => {
            let raw = *message.get(1)?;
            Some(EngineEvent::ChannelAftertouch {
                value_normalised: f32::from(raw) / 127.0,
            })
        }
        // Program change, SysEx, etc. — not handled in M3.
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
        // Program change (0xC0) is not handled.
        assert!(parse_midi_message(&[0xC0, 5]).is_none());
    }

    #[test]
    fn control_change_is_normalised_to_0_1() {
        // CC 7 (volume), value 127 → 1.0.
        let event = parse_midi_message(&[0xB0, 7, 127]);
        assert!(matches!(
            event,
            Some(EngineEvent::ControlChange {
                cc: 7,
                value_normalised,
            }) if (value_normalised - 1.0).abs() < 1e-4
        ));
        // CC 1 (mod wheel), value 64 → ~0.504.
        let event = parse_midi_message(&[0xB0, 1, 64]);
        assert!(matches!(
            event,
            Some(EngineEvent::ControlChange {
                cc: 1,
                value_normalised,
            }) if value_normalised > 0.49 && value_normalised < 0.51
        ));
    }

    #[test]
    fn sustain_cc_produces_sustain_variant() {
        // CC 64 ≥ 64 → pedal down.
        let event = parse_midi_message(&[0xB0, 64, 127]);
        assert!(matches!(event, Some(EngineEvent::Sustain { held: true })));
        // CC 64 < 64 → pedal up.
        let event = parse_midi_message(&[0xB0, 64, 0]);
        assert!(matches!(event, Some(EngineEvent::Sustain { held: false })));
        // Threshold at 64.
        let event = parse_midi_message(&[0xB0, 64, 63]);
        assert!(matches!(event, Some(EngineEvent::Sustain { held: false })));
        let event = parse_midi_message(&[0xB0, 64, 64]);
        assert!(matches!(event, Some(EngineEvent::Sustain { held: true })));
    }

    #[test]
    fn all_sound_off_and_all_notes_off_ccs_map_to_panic() {
        // CC 120 (All Sound Off) and CC 123 (All Notes Off) both panic.
        assert!(matches!(
            parse_midi_message(&[0xB0, 120, 0]),
            Some(EngineEvent::AllNotesOff)
        ));
        assert!(matches!(
            parse_midi_message(&[0xB0, 123, 0]),
            Some(EngineEvent::AllNotesOff)
        ));
    }

    #[test]
    fn pitch_bend_centre_is_zero() {
        // Raw 8192 = centre → 0.0.
        let lsb: u8 = (8192u16 & 0x7F) as u8;
        let msb: u8 = ((8192u16 >> 7) & 0x7F) as u8;
        let event = parse_midi_message(&[0xE0, lsb, msb]);
        assert!(matches!(
            event,
            Some(EngineEvent::PitchBend { value_normalised })
                if value_normalised.abs() < 1e-4
        ));
    }

    #[test]
    fn pitch_bend_full_down_is_minus_one() {
        // Raw 0 → -1.0 exactly.
        let event = parse_midi_message(&[0xE0, 0, 0]);
        assert!(matches!(
            event,
            Some(EngineEvent::PitchBend { value_normalised })
                if (value_normalised - (-1.0)).abs() < 1e-4
        ));
    }

    #[test]
    fn pitch_bend_full_up_is_near_plus_one() {
        // Raw 16383 → (16383 - 8192) / 8192 ≈ +0.9999.
        let event = parse_midi_message(&[0xE0, 0x7F, 0x7F]);
        assert!(matches!(
            event,
            Some(EngineEvent::PitchBend { value_normalised })
                if value_normalised > 0.99
        ));
    }

    #[test]
    fn channel_aftertouch_is_normalised() {
        let event = parse_midi_message(&[0xD0, 127]);
        assert!(matches!(
            event,
            Some(EngineEvent::ChannelAftertouch { value_normalised })
                if (value_normalised - 1.0).abs() < 1e-4
        ));
        let event = parse_midi_message(&[0xD0, 0]);
        assert!(matches!(
            event,
            Some(EngineEvent::ChannelAftertouch { value_normalised })
                if value_normalised.abs() < 1e-4
        ));
    }

    #[test]
    fn truncated_cc_returns_none() {
        assert!(parse_midi_message(&[0xB0, 7]).is_none());
        assert!(parse_midi_message(&[0xB0]).is_none());
    }

    #[test]
    fn truncated_pitch_bend_returns_none() {
        assert!(parse_midi_message(&[0xE0, 0]).is_none());
        assert!(parse_midi_message(&[0xE0]).is_none());
    }

    #[test]
    fn truncated_aftertouch_returns_none() {
        assert!(parse_midi_message(&[0xD0]).is_none());
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
