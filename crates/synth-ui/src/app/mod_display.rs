use synth_engine::{MOD_MATRIX_SLOTS, ParamSnapshot};

use super::state::{CUTOFF_MAX_HZ, CUTOFF_MIN_HZ, OSC_DETUNE_MAX_CENTS, PITCH_OFFSET_RANGE};

/// Normalised (-1..=1) modulation offsets for every destination that has a
/// knob in the UI. Computed once per frame from the engine snapshot and
/// passed to section panels so they can drive the mod ring on knobs.
///
/// Only sources with live values in the snapshot (LFO1, LFO2, Env2, Env3)
/// produce visible rings; the rest are left at 0.0.
#[derive(Default, Clone, Copy)]
pub(crate) struct ModDisplay {
    pub cutoff: f32,
    pub resonance: f32,
    pub pitch: f32,
    pub volume: f32,
    pub osc1_detune: f32,
    pub osc1_pan: f32,
    pub filter2_cutoff: f32,
    pub filter2_resonance: f32,
    pub osc2_detune: f32,
    pub osc3_detune: f32,
}

impl ModDisplay {
    /// Derives the display offsets from `snap`.
    pub(crate) fn from_snapshot(snap: &ParamSnapshot) -> Self {
        let source_live = |src: u8| -> f32 {
            match src {
                1 => snap.lfo1_out,
                2 => snap.lfo2_out,
                3 => snap.env2_out,
                10 => snap.env3_out,
                _ => 0.0,
            }
        };

        let mut d = ModDisplay::default();
        for i in 0..MOD_MATRIX_SLOTS {
            if !snap.mod_slot_enabled[i] {
                continue;
            }
            let live = source_live(snap.mod_slot_source[i]);
            if live == 0.0 {
                continue;
            }
            let amt = snap.mod_slot_amount[i];
            // Normalise: contribution in destination-natural units → -1..=1
            match snap.mod_slot_dest[i] {
                0 => d.cutoff += live * amt / (CUTOFF_MAX_HZ - CUTOFF_MIN_HZ),
                1 => d.resonance += live * amt / 1.0,
                2 => d.pitch += live * amt / (2.0 * PITCH_OFFSET_RANGE),
                3 => d.volume += live * amt / 1.0,
                4 => d.osc1_detune += live * amt / (2.0 * OSC_DETUNE_MAX_CENTS),
                5 => d.osc1_pan += live * amt / 2.0,
                6 => d.filter2_cutoff += live * amt / (CUTOFF_MAX_HZ - CUTOFF_MIN_HZ),
                7 => d.filter2_resonance += live * amt / 1.0,
                8 => d.osc2_detune += live * amt / (2.0 * OSC_DETUNE_MAX_CENTS),
                9 => d.osc3_detune += live * amt / (2.0 * OSC_DETUNE_MAX_CENTS),
                _ => {}
            }
        }
        // Clamp so the ring never wraps past the knob's endpoints.
        d.cutoff = d.cutoff.clamp(-1.0, 1.0);
        d.resonance = d.resonance.clamp(-1.0, 1.0);
        d.pitch = d.pitch.clamp(-1.0, 1.0);
        d.volume = d.volume.clamp(-1.0, 1.0);
        d.osc1_detune = d.osc1_detune.clamp(-1.0, 1.0);
        d.osc1_pan = d.osc1_pan.clamp(-1.0, 1.0);
        d.filter2_cutoff = d.filter2_cutoff.clamp(-1.0, 1.0);
        d.filter2_resonance = d.filter2_resonance.clamp(-1.0, 1.0);
        d.osc2_detune = d.osc2_detune.clamp(-1.0, 1.0);
        d.osc3_detune = d.osc3_detune.clamp(-1.0, 1.0);
        d
    }
}
