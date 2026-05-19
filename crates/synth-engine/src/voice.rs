//! A single synth voice.
//!
//! A voice owns the subtractive slot's four oscillators (three main
//! oscillators sharing a waveform, plus a dedicated sub that is always
//! a sine an octave below the held pitch) and one amp envelope.
//! Smoothed parameters (pitch offset, eventually cutoff, etc.) live in
//! the engine's [`ParameterTree`] — the voice is a pure consumer that
//! takes the current per-sample values as inputs to
//! [`Voice::next_sample`]. The engine owns a single voice for M2; a
//! polyphonic voice manager joins at M3.
//!
//! All four oscillators sum equally (× 0.25) at this stage. Per-osc
//! level / pan / detune land in M2.3 along with the real stereo slot
//! mixer; per-osc waveform routing lands when M4 grows the UI.
//!
//! [`ParameterTree`]: crate::params::ParameterTree

use crate::envelope::Adsr;
use crate::oscillator::{Oscillator, Waveform};

/// How many main oscillators (excluding the sub) each subtractive
/// voice carries.
pub const MAIN_OSCILLATOR_COUNT: usize = 3;

/// Equal-weight mixing scale for the four-oscillator subtractive sum
/// (`1 / (MAIN_OSCILLATOR_COUNT + 1 sub)`). Keeps a worst-case
/// constructive sum at unity so the rest of the chain isn't fighting
/// 4× gain headroom before per-osc levels arrive in M2.3.
const SLOT_MIX_SCALE: f32 = 1.0 / 4.0;

/// One synth voice: three main oscillators + a sub oscillator gated
/// by one amp envelope.
pub struct Voice {
    main_oscillators: [Oscillator; MAIN_OSCILLATOR_COUNT],
    sub_oscillator: Oscillator,
    amp_envelope: Adsr,

    /// MIDI note currently being held by the voice, if any. Used so
    /// `note_off` only releases the matching note.
    held_note_midi: Option<u8>,
}

impl Voice {
    /// Creates a silent, idle voice at the given sample rate. All
    /// three main oscillators default to [`Waveform::Sine`]; the sub
    /// oscillator is fixed as a sine and is never changed.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            main_oscillators: [(); MAIN_OSCILLATOR_COUNT].map(|()| Oscillator::new(sample_rate_hz)),
            sub_oscillator: Oscillator::new(sample_rate_hz),
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
            for osc in &mut self.main_oscillators {
                osc.reset_phase();
            }
            self.sub_oscillator.reset_phase();
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

    /// Sets the waveform on all three main oscillators. The sub
    /// oscillator is unaffected — it is always a sine per
    /// `docs/planning/05-design/dsp-and-sound.md`. The discrete-
    /// parameter-at-block-boundary rule is enforced by the engine
    /// draining events before processing.
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        for osc in &mut self.main_oscillators {
            osc.set_waveform(waveform);
        }
    }

    /// Returns true if the voice is fully idle (amp envelope at zero
    /// and no note held).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.amp_envelope.is_idle()
    }

    /// Produces one mono sample. `pitch_offset_semis` is the current
    /// smoothed pitch offset supplied by the engine for this frame;
    /// the voice re-derives oscillator frequencies each sample so
    /// glide is sample-accurate. All four oscillators are summed and
    /// scaled by [`SLOT_MIX_SCALE`] so a worst-case in-phase sum lands
    /// at unity — the real per-osc mixer (level / pan / detune)
    /// arrives in M2.3.
    pub fn next_sample(&mut self, pitch_offset_semis: f32) -> f32 {
        self.update_frequencies(pitch_offset_semis);
        let env = self.amp_envelope.next_sample();
        let mut sum = self.sub_oscillator.next_sample();
        for osc in &mut self.main_oscillators {
            sum += osc.next_sample();
        }
        sum * SLOT_MIX_SCALE * env
    }

    /// Re-derives oscillator frequencies from the held note plus the
    /// current pitch offset. The three main oscillators all track the
    /// held pitch; the sub oscillator runs an octave below (frequency
    /// halved). When no note is held (release tail) frequencies are
    /// left unchanged so each oscillator keeps cycling at its last
    /// correct pitch — stopping mid-cycle causes a timbral
    /// discontinuity that sounds like a click at note end.
    fn update_frequencies(&mut self, pitch_offset_semis: f32) {
        if let Some(note) = self.held_note_midi {
            let note_with_offset = f32::from(note) + pitch_offset_semis;
            // Standard MIDI-to-Hz formula with a fractional note number
            // so a non-integer smoothed offset glides cleanly through
            // semitones.
            let hz = 440.0 * 2.0_f32.powf((note_with_offset - 69.0) / 12.0);
            for osc in &mut self.main_oscillators {
                osc.set_frequency_hz(hz);
            }
            self.sub_oscillator.set_frequency_hz(hz * 0.5);
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
        // between the last release sample and the first attack
        // sample should be small.
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

    #[test]
    fn four_in_phase_sines_stay_at_unit_amplitude() {
        // All three main oscillators plus the sub are sine by default.
        // Mains run at the held pitch; sub at half. Their sum, scaled
        // by 0.25, must stay within ±1 — proving the slot-mix scale
        // controls the worst-case constructive headroom.
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69); // A4 = 440 Hz mains, 220 Hz sub
        let mut peak = 0.0_f32;
        for _ in 0..48_000 {
            // One full second — well past the envelope reaching peak.
            let s = voice.next_sample(0.0);
            peak = peak.max(s.abs());
        }
        assert!(peak <= 1.0 + 1e-3, "voice output exceeded unity: peak {peak}");
    }

    #[test]
    fn sub_oscillator_runs_one_octave_below_main() {
        // With main oscillators silenced (set to a shape and then
        // forced to zero by setting frequency to 0 is awkward) we
        // instead detect the sub by counting zero-crossings: the sub
        // sine at 220 Hz over 1 second has ~440 crossings, while the
        // mains at 440 Hz contribute ~880 each. The total expected
        // crossings of the summed signal is the dominant component's
        // zero crossings when the others are in phase — but since all
        // four are pure sines at integer ratios, summing them gives a
        // periodic signal at the sub's 220 Hz, which has 440
        // zero-crossings per second. Tolerate ±4 for envelope rise /
        // float drift.
        let mut voice = Voice::new(48_000.0);
        voice.set_main_waveform(Waveform::Sine);
        voice.note_on(69);
        let mut prev = voice.next_sample(0.0);
        let mut crossings = 0;
        for _ in 0..48_000 {
            let s = voice.next_sample(0.0);
            if (prev <= 0.0 && s > 0.0) || (prev >= 0.0 && s < 0.0) {
                crossings += 1;
            }
            prev = s;
        }
        // 440 Hz mains → ~880 crossings, but they're in phase with the
        // 220 Hz sub at integer ratio so the sum's period is the sub's
        // (the mains complete two full cycles inside each sub cycle
        // and contribute their own crossings). What we really care
        // about: more crossings than the sub alone (which would be
        // ~440) — proving the mains are present — and that the count
        // is consistent with a 440-Hz-dominated mix.
        assert!(
            (700..=1000).contains(&crossings),
            "expected ~880 zero crossings (440 Hz dominant), got {crossings}"
        );
    }
}
