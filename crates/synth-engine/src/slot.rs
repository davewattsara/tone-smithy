//! A voice slot — one of the two synthesis lanes per voice.
//!
//! Per `docs/planning/02-scope/features-v1.md`, each voice has two
//! oscillator slots; each slot is independently configured as either
//! subtractive (3 unison main oscillators + 1 sub) or FM (4 operators).
//! Slot outputs are mixed before the per-voice filter.
//!
//! M7.0 introduced the two-slot architecture in subtractive-only form;
//! M7.2 wires the FM bank into [`Slot`] so a slot in [`SlotMode::Fm`]
//! actually emits the FM bank's output. The mode dispatch is a single
//! match per sample — no trait objects, no heap, all stack-allocated.
//!
//! Slot 0 defaults to subtractive mix level 1.0; slot 1 starts silent
//! (mix level 0.0). Per-slot parameters (mode, level, pan, slot-1
//! oscillator settings) reach the parameter bus in M7.3.

use crate::MAIN_OSCILLATOR_COUNT;
use crate::fm::FmBank;
use crate::oscillator::{Oscillator, UnisonOscillator, Waveform};
use crate::panning::equal_power_pan;
use crate::params::SampleParams;

/// Which synthesis bank produces audio for a slot. The unused bank's
/// state is carried in RAM but does not advance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SlotMode {
    /// 3 unison main oscillators + 1 sub.
    #[default]
    Subtractive,
    /// 4-operator FM. M7.1/M7.2 — not yet implemented; current
    /// behaviour is to emit silence.
    Fm,
}

/// Subtractive synthesis bank: three [`UnisonOscillator`] main banks
/// plus one [`Oscillator`] sub. Reads its per-oscillator parameters
/// (level / pan / detune / unison count and spread) from the global
/// [`SampleParams`] — for M7.0 both slots read the same params, so a
/// non-zero slot-1 level would produce a duplicate of slot 0. The
/// per-slot oscillator parameter family that decouples them is M7.3.
pub struct SubtractiveBank {
    main_oscillators: [UnisonOscillator; MAIN_OSCILLATOR_COUNT],
    sub_oscillator: Oscillator,
}

impl SubtractiveBank {
    /// Creates a fresh bank at the given sample rate. Oscillator
    /// phases start at zero; call [`SubtractiveBank::reset_phases`]
    /// at the first note-on from idle.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            main_oscillators: [(); MAIN_OSCILLATOR_COUNT].map(|()| UnisonOscillator::new(sample_rate_hz)),
            sub_oscillator: Oscillator::new(sample_rate_hz),
        }
    }

    /// Pseudo-randomises every main bank's voice phases and resets the
    /// sub oscillator's phase to zero. Call on the idle-to-attack
    /// transition so a fresh note never comb-filters against itself.
    pub fn reset_phases(&mut self) {
        for bank in &mut self.main_oscillators {
            bank.randomize_phases();
        }
        self.sub_oscillator.reset_phase();
    }

    /// Sets the waveform on every voice of all three main oscillator
    /// banks. The sub oscillator is always a sine per
    /// `docs/planning/05-design/dsp-and-sound.md` and is unaffected.
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        for bank in &mut self.main_oscillators {
            bank.set_waveform(waveform);
        }
    }

    /// Per sample: clamp unison voice counts then re-derive each
    /// oscillator's frequencies. `held_note_midi` is `None` while a
    /// voice is releasing — frequencies stay where they were last set.
    pub fn update_voice_counts_and_frequencies(&mut self, params: &SampleParams, held_note_midi: Option<u8>) {
        for (i, bank) in self.main_oscillators.iter_mut().enumerate() {
            let count = round_voice_count(params.osc_main_unison_voices[i]);
            bank.set_voice_count(count);
        }
        if let Some(note) = held_note_midi {
            let base_semis = f32::from(note) + params.pitch_offset_semis + params.pitch_bend_semis;
            for (i, bank) in self.main_oscillators.iter_mut().enumerate() {
                let detune_semis = params.osc_main_detune_cents[i] * (1.0 / 100.0);
                let semis = base_semis + detune_semis;
                let osc_base_hz = 440.0 * 2.0_f32.powf((semis - 69.0) / 12.0);
                bank.set_base_frequency(osc_base_hz, params.osc_main_unison_detune_cents[i]);
            }
            // Sub: one octave below the base, no detune.
            let sub_hz = 440.0 * 2.0_f32.powf((base_semis - 81.0) / 12.0);
            self.sub_oscillator.set_frequency_hz(sub_hz);
        }
    }

    /// Produces one stereo sample. The caller scales by the slot's mix
    /// level and applies any slot-mix headroom on top.
    pub fn next_sample(&mut self, params: &SampleParams) -> (f32, f32) {
        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for (i, bank) in self.main_oscillators.iter_mut().enumerate() {
            let level = params.osc_main_levels[i];
            let (l, r) = bank.next_sample_stereo(params.osc_main_unison_spreads[i], params.osc_main_pans[i]);
            sum_l += l * level;
            sum_r += r * level;
        }
        let sub = self.sub_oscillator.next_sample();
        let (sub_pl, sub_pr) = equal_power_pan(params.sub_pan);
        sum_l += sub * params.sub_level * sub_pl;
        sum_r += sub * params.sub_level * sub_pr;
        (sum_l, sum_r)
    }
}

/// One voice slot. Owns both the subtractive and the FM bank; the
/// `mode` flag selects which is active.
pub struct Slot {
    /// Selects which synthesis bank produces audio.
    pub mode: SlotMode,
    /// Per-slot mix level applied to the bank's stereo output. M7.0
    /// uses static values (slot 0 = 1.0, slot 1 = 0.0); the parameter
    /// bus surface arrives in M7.3.
    pub level: f32,
    /// Per-slot pan applied to the bank's stereo output, -1..=1. A
    /// center-unity law (`pan=0 → L=R=1`) keeps subtractive volume
    /// identical to pre-M7. Surfaced on the bus in M7.3.
    pub pan: f32,
    /// Subtractive bank. Always allocated — a mode switch is a flag
    /// flip, not a heap operation.
    pub subtractive: SubtractiveBank,
    /// FM bank. Always allocated; advances only when `mode` is `Fm`.
    pub fm: FmBank,
    /// Cached base note frequency in Hz, used by the FM bank during the
    /// release phase when the voice no longer holds a note but the
    /// operator envelopes are still draining. The subtractive bank
    /// caches frequencies inside each unison bank, so this field is
    /// only consulted by the FM path.
    last_base_note_hz: f32,
}

impl Slot {
    /// Creates a slot at the given sample rate in subtractive mode at
    /// the given default mix level, centred pan.
    #[must_use]
    pub fn new(sample_rate_hz: f32, default_level: f32) -> Self {
        Self {
            mode: SlotMode::Subtractive,
            level: default_level,
            pan: 0.0,
            subtractive: SubtractiveBank::new(sample_rate_hz),
            fm: FmBank::new(sample_rate_hz),
            // A4 as a benign default; only used by the FM bank in
            // release mode if no note has ever been played.
            last_base_note_hz: 440.0,
        }
    }

    /// Called by the voice on every note-on. `is_first_note` is `true`
    /// when the voice's amp envelope was idle (the slot resets its
    /// subtractive phases in that case); the FM bank retriggers its
    /// operator envelopes on every note-on regardless, per DX7
    /// convention.
    pub fn note_on(&mut self, is_first_note: bool) {
        match self.mode {
            SlotMode::Subtractive => {
                if is_first_note {
                    self.subtractive.reset_phases();
                }
            }
            SlotMode::Fm => {
                self.fm.note_on();
            }
        }
    }

    /// Called by the voice on every note-off. Releases slot-internal
    /// envelopes (FM operators); the voice's amp envelope and Env2 are
    /// the primary release gates.
    pub fn note_off(&mut self) {
        match self.mode {
            SlotMode::Subtractive => {}
            SlotMode::Fm => self.fm.note_off(),
        }
    }

    /// Forwards a waveform change to the subtractive bank. The FM bank
    /// has no waveform concept (operators are always sine).
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        self.subtractive.set_main_waveform(waveform);
    }

    /// Produces one stereo sample, pre-scaled by `level` and panned by
    /// `pan`. Returns `(0.0, 0.0)` immediately when `level` is exactly
    /// zero so a silent slot costs no per-bank work.
    pub fn next_sample(&mut self, params: &SampleParams, held_note_midi: Option<u8>) -> (f32, f32) {
        if self.level == 0.0 {
            return (0.0, 0.0);
        }
        let (l, r) = match self.mode {
            SlotMode::Subtractive => {
                self.subtractive
                    .update_voice_counts_and_frequencies(params, held_note_midi);
                self.subtractive.next_sample(params)
            }
            SlotMode::Fm => {
                if let Some(note) = held_note_midi {
                    let semis = f32::from(note) + params.pitch_offset_semis + params.pitch_bend_semis;
                    self.last_base_note_hz = 440.0 * 2.0_f32.powf((semis - 69.0) / 12.0);
                }
                let mono = self.fm.next_sample(self.last_base_note_hz);
                (mono, mono)
            }
        };
        let (lp, rp) = center_unity_pan(self.pan);
        (l * lp * self.level, r * rp * self.level)
    }
}

/// Center-unity linear pan: `pan == 0` leaves both channels at unit
/// gain. `pan == -1` silences the right channel; `pan == +1` silences
/// the left. Chosen over equal-power so the pre-M7 subtractive volume
/// at the default `pan == 0` is preserved sample-for-sample.
fn center_unity_pan(pan: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    let l = if p > 0.0 { 1.0 - p } else { 1.0 };
    let r = if p < 0.0 { 1.0 + p } else { 1.0 };
    (l, r)
}

/// Rounds an `f32` voice-count parameter to the nearest valid `u8` in
/// `1..=MAX_UNISON_VOICES`. The unison bank clamps internally too, but
/// rounding here keeps `SampleParams`-side and bank-side semantics
/// aligned.
fn round_voice_count(v: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let rounded = v.round().max(1.0) as u32;
    rounded.min(u8::MAX as u32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamSnapshot;

    fn default_params() -> SampleParams {
        let snap = ParamSnapshot::default();
        SampleParams {
            pitch_offset_semis: snap.pitch_offset_semis,
            filter_cutoff_hz: 22_000.0,
            filter_resonance: 0.0,
            osc_main_levels: snap.osc_main_levels,
            sub_level: snap.sub_level,
            osc_main_detune_cents: snap.osc_main_detune_cents,
            osc_main_pans: snap.osc_main_pans,
            sub_pan: snap.sub_pan,
            osc_main_unison_voices: snap.osc_main_unison_voices,
            osc_main_unison_detune_cents: snap.osc_main_unison_detune_cents,
            osc_main_unison_spreads: snap.osc_main_unison_spreads,
            pitch_bend_semis: snap.pitch_bend_semis,
            master_volume: 1.0,
        }
    }

    #[test]
    fn slot_at_level_zero_is_silent_and_skips_work() {
        let mut slot = Slot::new(48_000.0, 0.0);
        let params = default_params();
        for _ in 0..1024 {
            assert_eq!(slot.next_sample(&params, Some(60)), (0.0, 0.0));
        }
    }

    #[test]
    fn subtractive_slot_with_unit_level_produces_audio() {
        let mut slot = Slot::new(48_000.0, 1.0);
        slot.note_on(true);
        let params = default_params();
        let mut peak = 0.0_f32;
        for _ in 0..2048 {
            let (l, r) = slot.next_sample(&params, Some(60));
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(
            peak > 0.01,
            "subtractive slot should produce audible output, peak={peak}"
        );
    }

    #[test]
    fn fm_slot_produces_audio_after_note_on() {
        let mut slot = Slot::new(48_000.0, 1.0);
        slot.mode = SlotMode::Fm;
        // Snap op envelopes so the slot is audible within the test window.
        for i in 0..crate::fm::OPERATOR_COUNT {
            let op = slot.fm.operator_mut(i).unwrap();
            op.set_attack_secs(0.001);
            op.set_decay_secs(0.001);
            op.set_sustain_level(1.0);
        }
        slot.note_on(true);
        let params = default_params();
        let mut peak = 0.0_f32;
        for _ in 0..2048 {
            let (l, r) = slot.next_sample(&params, Some(60));
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 0.001, "FM slot should produce audio after note_on, peak={peak}");
    }

    #[test]
    fn slot_pan_at_minus_one_silences_right_channel() {
        let mut slot = Slot::new(48_000.0, 1.0);
        slot.pan = -1.0;
        slot.note_on(true);
        let params = default_params();
        let mut peak_r = 0.0_f32;
        for _ in 0..1024 {
            let (_, r) = slot.next_sample(&params, Some(60));
            peak_r = peak_r.max(r.abs());
        }
        assert_eq!(peak_r, 0.0, "hard-left pan should silence right channel");
    }

    #[test]
    fn fm_slot_keeps_frequency_through_release() {
        // After note_off, held_note becomes None — the slot must keep
        // generating audio at the same pitch as the operator envelopes
        // drain, rather than stalling on a DC component.
        let mut slot = Slot::new(48_000.0, 1.0);
        slot.mode = SlotMode::Fm;
        for i in 0..crate::fm::OPERATOR_COUNT {
            let op = slot.fm.operator_mut(i).unwrap();
            op.set_attack_secs(0.001);
            op.set_decay_secs(0.001);
            op.set_sustain_level(1.0);
            op.set_release_secs(0.500);
        }
        slot.note_on(true);
        let params = default_params();
        for _ in 0..256 {
            slot.next_sample(&params, Some(60));
        }
        slot.note_off();
        let mut peak = 0.0_f32;
        for _ in 0..256 {
            let (l, _) = slot.next_sample(&params, None);
            peak = peak.max(l.abs());
        }
        assert!(
            peak > 0.001,
            "FM slot should keep producing audio during release, peak={peak}"
        );
    }
}
