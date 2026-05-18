//! A single synth voice.
//!
//! A voice owns one oscillator, one amp envelope, and the smoothed
//! parameters that feed them. For now the engine owns a single voice
//! and re-triggers it on every `NoteOn` (mono behaviour); the
//! polyphonic voice manager joins later.

use crate::envelope::Adsr;
use crate::oscillator::{Oscillator, Waveform};
use crate::smoothing::SmoothedParam;

/// One synth voice: one oscillator gated by one amp envelope.
pub struct Voice {
    oscillator: Oscillator,
    amp_envelope: Adsr,

    /// MIDI note currently being held by the voice, if any. Used so
    /// `note_off` only releases the matching note.
    held_note_midi: Option<u8>,

    /// Smoothed pitch offset, in semitones. UI sets the target; the
    /// audio thread advances `current` toward it each sample so that
    /// dragging the slider doesn't introduce zipper noise on the
    /// oscillator frequency (design-patterns.md §2.6).
    pitch_offset_semis: SmoothedParam,
}

impl Voice {
    /// Creates a silent, idle voice at the given sample rate.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            oscillator: Oscillator::new(sample_rate_hz),
            amp_envelope: Adsr::new(sample_rate_hz),
            held_note_midi: None,
            pitch_offset_semis: SmoothedParam::new(0.0, sample_rate_hz),
        }
    }

    /// Triggers a note. Snaps the smoothed pitch to its target so the
    /// first sample of the new note plays exactly on pitch, resets the
    /// oscillator phase, and starts the amp envelope.
    pub fn note_on(&mut self, note_midi: u8) {
        self.held_note_midi = Some(note_midi);
        self.pitch_offset_semis.snap_to_target();
        self.update_frequency();
        self.oscillator.reset_phase();
        self.amp_envelope.note_on();
    }

    /// Releases the held note. Ignored if a different note is currently
    /// held or if the voice is already idle — this matches what
    /// polyphonic hardware does and avoids drop-outs from out-of-order
    /// note-off events.
    pub fn note_off(&mut self, note_midi: u8) {
        if self.held_note_midi == Some(note_midi) {
            self.amp_envelope.note_off();
            self.held_note_midi = None;
        }
    }

    /// Sets the target pitch offset in semitones. The audio thread
    /// glides `current` toward this each sample.
    pub fn set_pitch_offset_semis(&mut self, pitch_offset_semis: f32) {
        self.pitch_offset_semis.set_target(pitch_offset_semis);
    }

    /// Sets the amp envelope release time in seconds.
    pub fn set_release_secs(&mut self, release_secs: f32) {
        self.amp_envelope.set_release_secs(release_secs);
    }

    /// Sets the oscillator waveform. Routed straight through to the
    /// oscillator; the discrete-parameter-at-block-boundary rule is
    /// enforced by the engine draining events before processing.
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.oscillator.set_waveform(waveform);
    }

    /// Returns true if the voice is fully idle (amp envelope at zero
    /// and no note held).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.amp_envelope.is_idle()
    }

    /// Produces one mono sample. Advances the smoothed pitch offset
    /// and re-derives the oscillator frequency each sample so glide
    /// is sample-accurate.
    pub fn next_sample(&mut self) -> f32 {
        self.pitch_offset_semis.next_sample();
        self.update_frequency();
        let env = self.amp_envelope.next_sample();
        let osc = self.oscillator.next_sample();
        osc * env
    }

    /// Re-derives the oscillator frequency from the held note plus the
    /// current smoothed pitch offset.
    fn update_frequency(&mut self) {
        if let Some(note) = self.held_note_midi {
            let note_with_offset = f32::from(note) + self.pitch_offset_semis.current();
            // Standard MIDI-to-Hz formula with a fractional note number
            // so a non-integer smoothed offset glides cleanly through
            // semitones.
            let hz = 440.0 * 2.0_f32.powf((note_with_offset - 69.0) / 12.0);
            self.oscillator.set_frequency_hz(hz);
        } else {
            self.oscillator.set_frequency_hz(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_voice_is_idle_and_silent() {
        let mut voice = Voice::new(48_000.0);
        assert!(voice.is_idle());
        for _ in 0..256 {
            assert_eq!(voice.next_sample(), 0.0);
        }
    }

    #[test]
    fn note_off_for_unrelated_note_is_ignored() {
        let mut voice = Voice::new(48_000.0);
        voice.note_on(60);
        voice.note_off(72);
        assert!(!voice.is_idle(), "voice should still be running");
    }
}
