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

pub use subtractive::{Oscillator, UnisonOscillator, Waveform};

mod polyblep;
mod subtractive;

/// Converts a MIDI note number to a frequency in Hz using equal
/// temperament with A4 (note 69) at 440 Hz. Accepts the standard MIDI
/// range and beyond; the caller is responsible for clamping if needed.
#[must_use]
pub fn midi_note_to_hz(note_midi: u8) -> f32 {
    // f = 440 * 2^((n - 69) / 12)
    440.0 * 2.0_f32.powf((f32::from(note_midi) - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_note_69_is_440_hz() {
        let hz = midi_note_to_hz(69);
        assert!((hz - 440.0).abs() < 1e-3, "got {hz}");
    }

    #[test]
    fn midi_note_60_is_middle_c() {
        // Middle C in equal temperament with A4=440 is ~261.626 Hz.
        let hz = midi_note_to_hz(60);
        assert!((hz - 261.625_55).abs() < 1e-2, "got {hz}");
    }
}
