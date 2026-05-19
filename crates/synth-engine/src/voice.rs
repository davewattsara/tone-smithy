//! A single synth voice.
//!
//! A voice owns the subtractive slot's four oscillators (three main
//! oscillators sharing a waveform, plus a dedicated sub that is always
//! a sine an octave below the held pitch), one filter, and one amp
//! envelope. Smoothed parameters (pitch offset, filter cutoff,
//! resonance, …) live in the engine's [`ParameterTree`] — the voice
//! is a pure consumer that takes the current per-sample values as a
//! [`SampleParams`] reference passed to [`Voice::next_sample`]. The
//! engine owns a single voice for M2; a polyphonic voice manager
//! joins at M3.
//!
//! Signal flow per sample: each oscillator generates a sample, the
//! four are summed and scaled, the slot mix goes into the filter, and
//! the filter output is gated by the amp envelope.
//!
//! All four oscillators sum equally (× 0.25) at this stage. Per-osc
//! level / pan / detune land in M2.3 along with the real stereo slot
//! mixer.
//!
//! [`ParameterTree`]: crate::params::ParameterTree
//! [`SampleParams`]: crate::params::SampleParams

use crate::envelope::Adsr;
use crate::filter::{FilterMode, StateVariableFilter};
use crate::oscillator::{Oscillator, Waveform};
use crate::params::SampleParams;

/// How many main oscillators (excluding the sub) each subtractive
/// voice carries.
pub const MAIN_OSCILLATOR_COUNT: usize = 3;

/// Equal-weight mixing scale for the four-oscillator subtractive sum
/// (`1 / (MAIN_OSCILLATOR_COUNT + 1 sub)`). Keeps a worst-case
/// constructive sum at unity so the rest of the chain isn't fighting
/// 4× gain headroom before per-osc levels arrive in M2.3.
const SLOT_MIX_SCALE: f32 = 1.0 / 4.0;

/// One synth voice: three main oscillators + a sub oscillator, fed
/// through one filter, gated by one amp envelope.
pub struct Voice {
    main_oscillators: [Oscillator; MAIN_OSCILLATOR_COUNT],
    sub_oscillator: Oscillator,
    filter: StateVariableFilter,
    amp_envelope: Adsr,

    /// MIDI note currently being held by the voice, if any. Used so
    /// `note_off` only releases the matching note.
    held_note_midi: Option<u8>,
}

impl Voice {
    /// Creates a silent, idle voice at the given sample rate. All
    /// three main oscillators default to [`Waveform::Sine`]; the sub
    /// oscillator is fixed as a sine and is never changed. The filter
    /// defaults to a wide-open low-pass.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            main_oscillators: [(); MAIN_OSCILLATOR_COUNT].map(|()| Oscillator::new(sample_rate_hz)),
            sub_oscillator: Oscillator::new(sample_rate_hz),
            filter: StateVariableFilter::new(sample_rate_hz),
            amp_envelope: Adsr::new(sample_rate_hz),
            held_note_midi: None,
        }
    }

    /// Triggers a note. The oscillator phase is only reset when the
    /// envelope was idle (first note from silence); on retrigger the
    /// phase continues uninterrupted so there is no discontinuity in
    /// the waveform output while the envelope level is non-zero. The
    /// filter's integrator state is reset on the same idle condition
    /// so the new note does not inherit a ringing tail. The caller
    /// (the engine) is responsible for snapping any per-voice
    /// smoothed parameters before calling this so the first sample
    /// plays exactly at the target value.
    pub fn note_on(&mut self, note_midi: u8) {
        self.held_note_midi = Some(note_midi);
        if self.amp_envelope.is_idle() {
            for osc in &mut self.main_oscillators {
                osc.reset_phase();
            }
            self.sub_oscillator.reset_phase();
            self.filter.reset();
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

    /// Sets the filter output mode. Discrete; the integrator state is
    /// preserved so the change is click-free.
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter.set_mode(mode);
    }

    /// Returns true if the voice is fully idle (amp envelope at zero
    /// and no note held).
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.amp_envelope.is_idle()
    }

    /// Produces one mono sample. Reads every per-sample smoothed
    /// parameter from `params` so the engine has a single point of
    /// fan-out; the voice itself is stateless with respect to
    /// parameter sources.
    pub fn next_sample(&mut self, params: &SampleParams) -> f32 {
        self.update_frequencies(params.pitch_offset_semis);
        let env = self.amp_envelope.next_sample();
        let mut sum = self.sub_oscillator.next_sample();
        for osc in &mut self.main_oscillators {
            sum += osc.next_sample();
        }
        let mixed = sum * SLOT_MIX_SCALE;
        self.filter.set_params(params.filter_cutoff_hz, params.filter_resonance);
        let filtered = self.filter.next_sample(mixed);
        filtered * env
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

    /// Open-filter sample params: filter at near-Nyquist and zero
    /// resonance, so the filter passes input untouched. Used by the
    /// oscillator-only voice tests to keep them focused on the
    /// generators.
    fn open_filter_params(pitch_offset_semis: f32) -> SampleParams {
        SampleParams {
            pitch_offset_semis,
            filter_cutoff_hz: 22_000.0,
            filter_resonance: 0.0,
        }
    }

    #[test]
    fn fresh_voice_is_idle_and_silent() {
        let mut voice = Voice::new(48_000.0);
        assert!(voice.is_idle());
        let params = open_filter_params(0.0);
        for _ in 0..256 {
            assert_eq!(voice.next_sample(&params), 0.0);
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
        let sample_rate = 48_000.0;
        let mut voice = Voice::new(sample_rate);
        let params = open_filter_params(0.0);

        voice.note_on(60);
        for _ in 0..4_800 {
            voice.next_sample(&params);
        }
        voice.note_off(60);

        let mut last_sample = 0.0;
        for _ in 0..480 {
            last_sample = voice.next_sample(&params);
        }

        voice.note_on(62);
        let first_retrigger_sample = voice.next_sample(&params);

        let jump = (first_retrigger_sample - last_sample).abs();
        assert!(
            jump < 0.05,
            "output jumped by {jump:.4} on retrigger — phase reset caused a click"
        );
    }

    #[test]
    fn four_in_phase_sines_stay_at_unit_amplitude() {
        let mut voice = Voice::new(48_000.0);
        voice.note_on(69);
        let params = open_filter_params(0.0);
        let mut peak = 0.0_f32;
        for _ in 0..48_000 {
            let s = voice.next_sample(&params);
            peak = peak.max(s.abs());
        }
        assert!(peak <= 1.0 + 1e-3, "voice output exceeded unity: peak {peak}");
    }

    #[test]
    fn sub_oscillator_runs_one_octave_below_main() {
        let mut voice = Voice::new(48_000.0);
        voice.set_main_waveform(Waveform::Sine);
        voice.note_on(69);
        let params = open_filter_params(0.0);
        let mut prev = voice.next_sample(&params);
        let mut crossings = 0;
        for _ in 0..48_000 {
            let s = voice.next_sample(&params);
            if (prev <= 0.0 && s > 0.0) || (prev >= 0.0 && s < 0.0) {
                crossings += 1;
            }
            prev = s;
        }
        assert!(
            (700..=1000).contains(&crossings),
            "expected ~880 zero crossings (440 Hz dominant), got {crossings}"
        );
    }

    #[test]
    fn closed_low_pass_silences_the_voice() {
        // A saw at full mix, with LP cutoff well below the
        // fundamental, must come out essentially silent.
        let mut voice = Voice::new(48_000.0);
        voice.set_main_waveform(Waveform::Saw);
        voice.set_filter_mode(FilterMode::LowPass);
        voice.note_on(69); // 440 Hz
        let closed = SampleParams {
            pitch_offset_semis: 0.0,
            filter_cutoff_hz: 30.0,
            filter_resonance: 0.0,
        };
        // Let the envelope reach sustain.
        for _ in 0..4_800 {
            voice.next_sample(&closed);
        }
        let mut peak = 0.0_f32;
        for _ in 0..4_800 {
            peak = peak.max(voice.next_sample(&closed).abs());
        }
        assert!(peak < 0.05, "expected LP to silence saw, peak {peak}");
    }
}
