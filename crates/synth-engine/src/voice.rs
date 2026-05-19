//! A single synth voice.
//!
//! A voice owns one oscillator and one amp envelope. Smoothed
//! parameters (pitch offset, eventually cutoff, etc.) live in the
//! engine's [`ParameterTree`] — the voice is a pure consumer that takes
//! the current per-sample values as inputs to
//! [`Voice::next_sample`]. The engine owns a single voice for M1/M2; a
//! polyphonic voice manager joins at M3.
//!
//! [`ParameterTree`]: crate::params::ParameterTree

use crate::envelope::Adsr;
use crate::oscillator::{Oscillator, Waveform};

/// One synth voice: one oscillator gated by one amp envelope.
pub struct Voice {
    oscillator: Oscillator,
    amp_envelope: Adsr,

    /// MIDI note currently being held by the voice, if any. Used so
    /// `note_off` only releases the matching note.
    held_note_midi: Option<u8>,
}

impl Voice {
    /// Creates a silent, idle voice at the given sample rate.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            oscillator: Oscillator::new(sample_rate_hz),
            amp_envelope: Adsr::new(sample_rate_hz),
            held_note_midi: None,
        }
    }

    /// Triggers a note. The oscillator phase is only reset when the
    /// envelope was idle (first note from silence); on retrigger the
    /// phase continues uninterrupted so there is no discontinuity in
    /// the waveform output while the envelope level is non-zero. The
    /// caller (the engine) is responsible for snapping any per-voice
    /// smoothed parameters before calling this so the first sample
    /// plays exactly at the target value.
    pub fn note_on(&mut self, note_midi: u8) {
        self.held_note_midi = Some(note_midi);
        if self.amp_envelope.is_idle() {
            self.oscillator.reset_phase();
        }
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

    /// Produces one mono sample. `pitch_offset_semis` is the current
    /// smoothed pitch offset supplied by the engine for this frame; the
    /// voice re-derives the oscillator frequency each sample so glide
    /// is sample-accurate.
    pub fn next_sample(&mut self, pitch_offset_semis: f32) -> f32 {
        self.update_frequency(pitch_offset_semis);
        let env = self.amp_envelope.next_sample();
        let osc = self.oscillator.next_sample();
        osc * env
    }

    /// Re-derives the oscillator frequency from the held note plus the
    /// current pitch offset. When no note is held (release tail) the
    /// frequency is left unchanged so the oscillator keeps cycling at
    /// the correct pitch — stopping it causes a timbral discontinuity
    /// that sounds like a click at note end.
    fn update_frequency(&mut self, pitch_offset_semis: f32) {
        if let Some(note) = self.held_note_midi {
            let note_with_offset = f32::from(note) + pitch_offset_semis;
            // Standard MIDI-to-Hz formula with a fractional note number
            // so a non-integer smoothed offset glides cleanly through
            // semitones.
            let hz = 440.0 * 2.0_f32.powf((note_with_offset - 69.0) / 12.0);
            self.oscillator.set_frequency_hz(hz);
        }
        // No else: when held_note_midi is None the oscillator retains
        // its last phase_increment and rings through the release at
        // correct pitch.
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
            assert_eq!(voice.next_sample(0.0), 0.0);
        }
    }

    #[test]
    fn note_off_for_unrelated_note_is_ignored() {
        let mut voice = Voice::new(48_000.0);
        voice.note_on(60);
        voice.note_off(72);
        assert!(!voice.is_idle(), "voice should still be running");
    }

    #[test]
    fn retrigger_during_release_produces_no_output_discontinuity() {
        // If a new note-on arrives while the envelope is still in
        // release the output must not jump — the amplitude step
        // between the last release sample and the first attack sample
        // should be small.
        let sample_rate = 48_000.0;
        let mut voice = Voice::new(sample_rate);

        // Play note long enough to reach sustain.
        voice.note_on(60);
        for _ in 0..4_800 {
            voice.next_sample(0.0);
        }

        // Begin release.
        voice.note_off(60);

        // Let a short portion of the release run so level is well
        // above zero.
        let mut last_sample = 0.0;
        for _ in 0..480 {
            last_sample = voice.next_sample(0.0);
        }

        // Retrigger. The output must not jump by more than one attack
        // step.
        voice.note_on(62);
        let first_retrigger_sample = voice.next_sample(0.0);

        let jump = (first_retrigger_sample - last_sample).abs();
        assert!(
            jump < 0.05,
            "output jumped by {jump:.4} on retrigger — phase reset caused a click"
        );
    }
}
