//! A voice slot — one of the two synthesis lanes per voice.
//!
//! Per `docs/planning/02-scope/features-v1.md`, each voice has two
//! oscillator slots; each slot is independently configured as either
//! subtractive (3 unison main oscillators + 1 sub) or FM (4 operators).
//! Slot outputs are mixed before the per-voice filter.
//!
//! M7.0 introduces the two-slot architecture in subtractive-only form:
//! a [`Slot`] owns a [`SubtractiveBank`] and a [`SlotMode`] flag. The
//! FM bank arrives in M7.1/M7.2. The mode dispatch is a single match
//! per sample — no trait objects, no heap, all stack-allocated.
//!
//! For M7.0, slot 0 carries the existing subtractive behaviour at
//! mix level 1.0 and slot 1 is present at mix level 0.0 (silent).
//! Per-slot parameters (mode, level, pan, slot-1 oscillator settings)
//! land on the parameter bus in M7.3.

use crate::MAIN_OSCILLATOR_COUNT;
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

/// One voice slot. Owns the subtractive bank and (M7.1+) the FM bank;
/// the `mode` flag selects which is active.
pub struct Slot {
    /// Selects which synthesis bank produces audio.
    pub mode: SlotMode,
    /// Per-slot mix level applied to the bank's stereo output. M7.0
    /// uses static values (slot 0 = 1.0, slot 1 = 0.0); the parameter
    /// bus surface arrives in M7.3.
    pub level: f32,
    /// Subtractive bank. Always allocated — the FM mode does not free
    /// or replace it, so a mode switch is just a flag flip.
    pub subtractive: SubtractiveBank,
}

impl Slot {
    /// Creates a slot at the given sample rate in subtractive mode at
    /// the given default mix level.
    #[must_use]
    pub fn new(sample_rate_hz: f32, default_level: f32) -> Self {
        Self {
            mode: SlotMode::Subtractive,
            level: default_level,
            subtractive: SubtractiveBank::new(sample_rate_hz),
        }
    }

    /// Forwards a phase reset to whichever bank is currently active.
    /// Called by the voice on the idle-to-attack transition.
    pub fn reset_phases(&mut self) {
        match self.mode {
            SlotMode::Subtractive => self.subtractive.reset_phases(),
            SlotMode::Fm => {} // M7.1
        }
    }

    /// Forwards a waveform change to the subtractive bank. The FM bank
    /// has no waveform concept (operators are always sine).
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        self.subtractive.set_main_waveform(waveform);
    }

    /// Produces one stereo sample, pre-scaled by `level`. Returns
    /// `(0.0, 0.0)` immediately when `level` is exactly zero so a
    /// silent slot costs no per-oscillator work. The phase accumulators
    /// of a level-0 slot do not advance; if the level later rises from
    /// zero the next note-on will re-randomise them.
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
            SlotMode::Fm => (0.0, 0.0), // M7.1
        };
        (l * self.level, r * self.level)
    }
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
    fn slot_in_fm_mode_is_silent_until_m7_2() {
        let mut slot = Slot::new(48_000.0, 1.0);
        slot.mode = SlotMode::Fm;
        let params = default_params();
        assert_eq!(slot.next_sample(&params, Some(60)), (0.0, 0.0));
    }

    #[test]
    fn subtractive_slot_with_unit_level_produces_audio() {
        let mut slot = Slot::new(48_000.0, 1.0);
        slot.reset_phases();
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
}
