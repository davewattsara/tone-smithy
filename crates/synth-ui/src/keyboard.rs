//! On-screen virtual keyboard widget.
//!
//! Draws two octaves of piano keys and converts mouse interaction into
//! `EngineEvent::NoteOn` / `NoteOff` events on the parameter bus.
//! M1-scope and intentionally minimal — the polished keyboard panel
//! ships at M4 (per `docs/planning/06-implementation/project-structure.md`
//! the file moves to `panels/virtual_keyboard.rs` at that point).
//!
//! The widget is mono: only one note is held at a time, matching the
//! engine's single-voice mono behaviour for M1. Mouse-down on a key
//! triggers NoteOn; dragging onto a different key sends NoteOff for
//! the previous note and NoteOn for the new one (legato glide).
//! Releasing the mouse sends NoteOff.

use eframe::egui;
use synth_engine::EngineEvent;
use synth_engine::param_bus::EngineEventSender;

/// Pixel width of a white key.
const WHITE_KEY_WIDTH: f32 = 32.0;
/// Pixel height of a white key.
const WHITE_KEY_HEIGHT: f32 = 120.0;
/// Pixel width of a black key.
const BLACK_KEY_WIDTH: f32 = 20.0;
/// Pixel height of a black key.
const BLACK_KEY_HEIGHT: f32 = 76.0;

/// Index pattern for white-key MIDI notes within an octave starting at C.
/// C=0, D=2, E=4, F=5, G=7, A=9, B=11.
const WHITE_KEY_SEMITONE_OFFSETS: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

/// For each black key in an octave: its semitone offset from C, and the
/// white-key column boundary it's centred on. C# is centred between
/// columns 0 (C) and 1 (D), so its centring boundary is `1`.
const BLACK_KEYS: [(u8, u8); 5] = [
    (1, 1),  // C#
    (3, 2),  // D#
    (6, 4),  // F#
    (8, 5),  // G#
    (10, 6), // A#
];

/// The on-screen keyboard.
pub struct VirtualKeyboard {
    /// MIDI note of the leftmost white key. Defaults to C3 (48).
    start_note_midi: u8,

    /// How many octaves of keys to draw.
    octaves: u8,

    /// The currently held note, if any. Single-voice mono.
    held_note: Option<u8>,
}

impl Default for VirtualKeyboard {
    fn default() -> Self {
        Self {
            start_note_midi: 48,
            octaves: 2,
            held_note: None,
        }
    }
}

impl VirtualKeyboard {
    /// Sets the MIDI note of the leftmost white key. Call before [`show`]
    /// each frame to keep the visible range in sync with the computer
    /// keyboard's octave base.
    ///
    /// Returns the MIDI note that was being held by the mouse if the
    /// range actually changed, so the caller can send a `NoteOff` event
    /// to release it. If the range is unchanged, or no note was held,
    /// returns `None`.
    ///
    /// [`show`]: Self::show
    pub fn set_start_note(&mut self, note_midi: u8) -> Option<u8> {
        if self.start_note_midi != note_midi {
            self.start_note_midi = note_midi;
            self.held_note.take()
        } else {
            None
        }
    }

    /// Renders the keyboard and pumps mouse interaction through `sender`.
    ///
    /// `keyboard_lit` is a slice of MIDI notes currently held by the
    /// computer-keyboard layer; those keys are highlighted in the same
    /// blue as mouse-held keys so the visual feedback matches the sound.
    pub fn show(&mut self, ui: &mut egui::Ui, sender: &EngineEventSender, keyboard_lit: &[Option<u8>]) {
        let total_white_keys = u32::from(self.octaves) * 7;
        let desired_width = white_key_column_x_offset(total_white_keys);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(desired_width, WHITE_KEY_HEIGHT),
            egui::Sense::click_and_drag(),
        );

        // Compute the active note (if any) under the pointer when held.
        let active_note = if response.is_pointer_button_down_on() {
            response.interact_pointer_pos().and_then(|pos| self.note_at(rect, pos))
        } else {
            None
        };

        // Drive note-on / note-off transitions from the change in held note.
        match (self.held_note, active_note) {
            (Some(old), Some(new)) if old != new => {
                sender.send(EngineEvent::NoteOff { note_midi: old });
                sender.send(EngineEvent::NoteOn {
                    note_midi: new,
                    velocity: 100,
                });
                self.held_note = Some(new);
            }
            (None, Some(new)) => {
                sender.send(EngineEvent::NoteOn {
                    note_midi: new,
                    velocity: 100,
                });
                self.held_note = Some(new);
            }
            (Some(old), None) => {
                sender.send(EngineEvent::NoteOff { note_midi: old });
                self.held_note = None;
            }
            _ => {}
        }

        self.paint(ui, rect, keyboard_lit);
    }

    /// Returns the MIDI note under `pos`, or `None` if the pointer is
    /// outside the keyboard rect. Black keys are tested first because
    /// they overlay white keys.
    fn note_at(&self, rect: egui::Rect, pos: egui::Pos2) -> Option<u8> {
        if !rect.contains(pos) {
            return None;
        }
        // Black keys first.
        for octave in 0..self.octaves {
            for (semitone_offset, boundary_column) in BLACK_KEYS {
                let key_rect = self.black_key_rect(rect, octave, boundary_column);
                if key_rect.contains(pos) {
                    return Some(self.start_note_midi + octave * 12 + semitone_offset);
                }
            }
        }
        // White keys: figure out which column the pointer is over.
        // The pointer-x check above guarantees `pos.x >= rect.left()`
        // implicitly only if rect.contains(pos) — which we asserted at
        // the top of the function — so the subtraction is non-negative.
        let column_f = (pos.x - rect.left()) / WHITE_KEY_WIDTH;
        let total_white_keys = u32::from(self.octaves) * 7;
        // `column_f` is in [0, num_white_keys) by construction of
        // `rect.contains(pos)`; sign-loss / truncation are impossible.
        if !column_f.is_finite() || column_f < 0.0 || column_f >= total_white_keys as f32 {
            return None;
        }
        let column = column_f as u32;
        let octave = u8::try_from(column / 7).expect("octave fits in u8: octaves is u8 to begin with");
        let key_within_octave = (column % 7) as usize;
        let semitone_offset = WHITE_KEY_SEMITONE_OFFSETS[key_within_octave];
        Some(self.start_note_midi + octave * 12 + semitone_offset)
    }

    fn paint(&self, ui: &mut egui::Ui, rect: egui::Rect, keyboard_lit: &[Option<u8>]) {
        let painter = ui.painter_at(rect);
        let stroke = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color);

        // White keys.
        for octave in 0..self.octaves {
            for (key_within_octave, &semitone_offset) in WHITE_KEY_SEMITONE_OFFSETS.iter().enumerate() {
                let column = u32::from(octave) * 7 + key_within_octave as u32;
                let x_left = rect.left() + white_key_column_x_offset(column);
                let key_rect = egui::Rect::from_min_size(
                    egui::pos2(x_left, rect.top()),
                    egui::vec2(WHITE_KEY_WIDTH, WHITE_KEY_HEIGHT),
                );
                let note = self.start_note_midi + octave * 12 + semitone_offset;

                let fill = if self.held_note == Some(note) || keyboard_lit.contains(&Some(note)) {
                    egui::Color32::from_rgb(180, 210, 255)
                } else {
                    egui::Color32::WHITE
                };
                painter.rect(key_rect, 2.0, fill, stroke);
            }
        }

        // Black keys overlay.
        let black_stroke = egui::Stroke::new(1.0, egui::Color32::BLACK);
        for octave in 0..self.octaves {
            for (semitone_offset, boundary_column) in BLACK_KEYS {
                let key_rect = self.black_key_rect(rect, octave, boundary_column);
                let note = self.start_note_midi + octave * 12 + semitone_offset;
                let fill = if self.held_note == Some(note) || keyboard_lit.contains(&Some(note)) {
                    egui::Color32::from_rgb(80, 110, 180)
                } else {
                    egui::Color32::BLACK
                };
                painter.rect(key_rect, 2.0, fill, black_stroke);
            }
        }
    }

    /// Computes the screen rect of a black key, centred on the boundary
    /// between two white-key columns.
    fn black_key_rect(&self, rect: egui::Rect, octave: u8, boundary_column: u8) -> egui::Rect {
        let boundary_x = rect.left() + (f32::from(octave) * 7.0 + f32::from(boundary_column)) * WHITE_KEY_WIDTH;
        egui::Rect::from_min_size(
            egui::pos2(boundary_x - BLACK_KEY_WIDTH / 2.0, rect.top()),
            egui::vec2(BLACK_KEY_WIDTH, BLACK_KEY_HEIGHT),
        )
    }
}

/// X offset (in pixels, from the left edge of the keyboard) of the
/// white-key column at `column`. Column counts are bounded by the
/// keyboard's `octaves * 7` (a few dozen at most for any sensible
/// keyboard size), so the `u32 -> f32` cast is exact.
#[allow(clippy::cast_precision_loss)]
fn white_key_column_x_offset(column: u32) -> f32 {
    (column as f32) * WHITE_KEY_WIDTH
}
