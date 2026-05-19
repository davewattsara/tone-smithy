//! Computer-keyboard note input.
//!
//! Maps the home row plus the row above into one chromatic octave so
//! the synth is playable without MIDI hardware. The layout is the
//! Vital / Serum convention:
//!
//! ```text
//!     W   E       T   Y   U
//!   A   S   D   F   G   H   J
//! ```
//!
//! - `A` = C, `W` = C#, `S` = D, `E` = D#, `D` = E, `F` = F, `T` = F#,
//!   `G` = G, `Y` = G#, `H` = A, `U` = A#, `J` = B.
//! - `Z` shifts the octave down, `X` shifts it up. The default octave
//!   base is MIDI 48 (C3) so `A` plays middle-low C out of the box.
//! - The octave base is clamped to `[0, 96]` so the J key — an octave
//!   plus a B above the base — never escapes the valid MIDI range
//!   `[0, 127]`.
//!
//! The struct remembers which exact MIDI note each piano key is
//! currently sustaining. That way an octave shift mid-hold sends the
//! matching note-off on the next release, instead of leaking a stuck
//! note on the engine side. Focus loss is handled implicitly: egui
//! reports the key as up while the window doesn't have focus, so the
//! note-off path fires on the next frame.

use eframe::egui;
use synth_engine::EngineEvent;
use synth_engine::param_bus::EngineEventSender;

/// Number of chromatic keys mapped (one full octave).
const NUM_KEYS: usize = 12;

/// Key → semitone offset from the octave base. Index 0 is the lowest
/// note of the octave (C); index 11 is B.
const KEY_LAYOUT: [(egui::Key, u8); NUM_KEYS] = [
    (egui::Key::A, 0),  // C
    (egui::Key::W, 1),  // C#
    (egui::Key::S, 2),  // D
    (egui::Key::E, 3),  // D#
    (egui::Key::D, 4),  // E
    (egui::Key::F, 5),  // F
    (egui::Key::T, 6),  // F#
    (egui::Key::G, 7),  // G
    (egui::Key::Y, 8),  // G#
    (egui::Key::H, 9),  // A
    (egui::Key::U, 10), // A#
    (egui::Key::J, 11), // B
];

/// Default octave base MIDI note (C3 = 48).
const DEFAULT_OCTAVE_BASE: u8 = 48;
/// Lower clamp on the octave base. Below this `Z` is a no-op.
const OCTAVE_BASE_MIN: u8 = 0;
/// Upper clamp on the octave base. The B key one octave above this
/// (96 + 11 = 107) is the highest playable note.
const OCTAVE_BASE_MAX: u8 = 96;
/// Velocity sent on every note-on. The computer keyboard can't sense
/// velocity; MIDI hardware (M3.2) provides the real value.
const COMPUTER_KEYBOARD_VELOCITY: u8 = 100;

/// One-octave chromatic keyboard driven by the computer keyboard.
pub struct ComputerKeyboard {
    /// MIDI note of the `A` key at the current octave shift.
    octave_base: u8,

    /// For each entry in [`KEY_LAYOUT`], the MIDI note that key is
    /// currently sustaining, or `None` if the key is up. Storing the
    /// exact triggered note (not just "is this key held?") means an
    /// octave shift between note-on and note-off still releases the
    /// correct voice on the engine side.
    held_notes: [Option<u8>; NUM_KEYS],
}

impl Default for ComputerKeyboard {
    fn default() -> Self {
        Self {
            octave_base: DEFAULT_OCTAVE_BASE,
            held_notes: [None; NUM_KEYS],
        }
    }
}

impl ComputerKeyboard {
    /// Returns the MIDI note the `A` key would currently play.
    #[must_use]
    pub fn octave_base(&self) -> u8 {
        self.octave_base
    }

    /// Returns the per-key held-note state. Each entry is the MIDI note
    /// the corresponding key is currently sustaining, or `None` if the
    /// key is up. Used by the virtual keyboard to highlight active keys.
    #[must_use]
    pub fn held_notes(&self) -> &[Option<u8>] {
        &self.held_notes
    }

    /// Shifts the octave base down by 12 semitones, clamped at
    /// [`OCTAVE_BASE_MIN`]. Bound to `Z` in [`Self::handle_input`].
    pub fn shift_octave_down(&mut self) {
        if self.octave_base >= OCTAVE_BASE_MIN + 12 {
            self.octave_base -= 12;
        }
    }

    /// Shifts the octave base up by 12 semitones, clamped at
    /// [`OCTAVE_BASE_MAX`]. Bound to `X` in [`Self::handle_input`].
    pub fn shift_octave_up(&mut self) {
        if self.octave_base <= OCTAVE_BASE_MAX - 12 {
            self.octave_base += 12;
        }
    }

    /// Reads keyboard input from `ctx` and pushes resulting note
    /// events into `sender`. Call once per frame from the eframe
    /// update loop.
    pub fn handle_input(&mut self, ctx: &egui::Context, sender: &EngineEventSender) {
        ctx.input(|input| {
            // Octave shifts are edge-triggered so holding Z or X
            // doesn't drift the octave by one per frame.
            if input.key_pressed(egui::Key::Z) {
                self.shift_octave_down();
            }
            if input.key_pressed(egui::Key::X) {
                self.shift_octave_up();
            }

            // Piano keys: drive transitions from `key_down` against
            // our remembered note. Continuous polling handles focus
            // loss implicitly — when the window isn't focused egui
            // reports `key_down == false`, which triggers note-off
            // for any held key.
            for (i, (key, semitone)) in KEY_LAYOUT.iter().enumerate() {
                let is_down = input.key_down(*key);
                match (is_down, self.held_notes[i]) {
                    (true, None) => {
                        let note = self.octave_base + semitone;
                        sender.send(EngineEvent::NoteOn {
                            note_midi: note,
                            velocity: COMPUTER_KEYBOARD_VELOCITY,
                        });
                        self.held_notes[i] = Some(note);
                    }
                    (false, Some(note)) => {
                        sender.send(EngineEvent::NoteOff { note_midi: note });
                        self.held_notes[i] = None;
                    }
                    _ => {}
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pure helper for testing the layout map without an egui context.
    fn note_for(key: egui::Key, octave_base: u8) -> Option<u8> {
        KEY_LAYOUT
            .iter()
            .find_map(|&(k, semitone)| if k == key { Some(octave_base + semitone) } else { None })
    }

    #[test]
    fn key_a_at_default_octave_is_c3() {
        assert_eq!(note_for(egui::Key::A, DEFAULT_OCTAVE_BASE), Some(48));
    }

    #[test]
    fn key_j_at_default_octave_is_b3() {
        assert_eq!(note_for(egui::Key::J, DEFAULT_OCTAVE_BASE), Some(59));
    }

    #[test]
    fn whole_chromatic_octave_is_mapped() {
        let base = 60;
        // Each semitone 0..=11 should be reachable from some key.
        let mut seen = [false; 12];
        for (key, _) in KEY_LAYOUT {
            let note = note_for(key, base).unwrap();
            seen[(note - base) as usize] = true;
        }
        assert!(seen.iter().all(|&b| b), "missing semitones: {seen:?}");
    }

    #[test]
    fn unmapped_key_returns_none() {
        assert_eq!(note_for(egui::Key::Q, DEFAULT_OCTAVE_BASE), None);
    }

    #[test]
    fn default_keyboard_holds_no_notes() {
        let kb = ComputerKeyboard::default();
        assert_eq!(kb.octave_base(), DEFAULT_OCTAVE_BASE);
        assert!(kb.held_notes.iter().all(Option::is_none));
    }

    #[test]
    fn shift_octave_down_clamps_at_floor() {
        let mut kb = ComputerKeyboard::default();
        for _ in 0..10 {
            kb.shift_octave_down();
        }
        assert_eq!(kb.octave_base(), OCTAVE_BASE_MIN);
    }

    #[test]
    fn shift_octave_up_clamps_at_ceiling() {
        let mut kb = ComputerKeyboard::default();
        for _ in 0..10 {
            kb.shift_octave_up();
        }
        assert_eq!(kb.octave_base(), OCTAVE_BASE_MAX);
    }

    #[test]
    fn shift_octave_round_trip_returns_to_default() {
        let mut kb = ComputerKeyboard::default();
        kb.shift_octave_up();
        assert_eq!(kb.octave_base(), DEFAULT_OCTAVE_BASE + 12);
        kb.shift_octave_down();
        assert_eq!(kb.octave_base(), DEFAULT_OCTAVE_BASE);
    }
}
