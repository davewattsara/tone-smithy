//! Per-frame MIDI Learn tick: detect CC movement, bind it, apply mappings.

use eframe::egui;
use synth_engine::{EngineEvent, ParamId, ParamSnapshot};
use synth_presets::MidiLearnEntry;

use crate::app::state::ToneSmithyApp;

impl ToneSmithyApp {
    /// Called once per frame from `update()`:
    /// 1. Reads any pending learn intent (key + range) deposited by a knob.
    /// 2. If in learn mode, waits for the first CC that moves and binds it.
    /// 3. Routes each active mapping when the CC moves; syncs the UI field.
    pub(crate) fn tick_midi_learn(&mut self, ctx: &egui::Context, snapshot: &ParamSnapshot) {
        // Step 1 — pick up any learn intent deposited by a knob context menu.
        let intent_key: Option<String> = ctx.memory_mut(|m| m.data.remove_temp(egui::Id::new("ts_ml_key")));
        if let Some(key) = intent_key {
            let start: f32 = ctx
                .memory_mut(|m| m.data.remove_temp(egui::Id::new("ts_ml_start")))
                .unwrap_or(0.0);
            let end: f32 = ctx
                .memory_mut(|m| m.data.remove_temp(egui::Id::new("ts_ml_end")))
                .unwrap_or(1.0);
            self.midi_learn_target = Some((key, start, end));
        }

        // Step 2 — detect CC movement while in learn mode.
        if let Some((key, start, end)) = self.midi_learn_target.clone() {
            let mut found_cc: Option<u8> = None;
            for cc in 0..128usize {
                if (snapshot.cc_values[cc] - self.prev_cc_values[cc]).abs() > 0.02 {
                    found_cc = Some(cc as u8);
                    break;
                }
            }
            if let Some(cc) = found_cc {
                self.midi_learn_target = None;
                self.midi_learn_mappings.retain(|e| e.cc != cc && e.parameter != key);
                self.midi_learn_mappings.push(MidiLearnEntry {
                    cc,
                    parameter: key,
                    range_start: start,
                    range_end: end,
                });
            }
        }

        // Step 3 — apply mappings on CC movement; sync UI so the knob moves.
        let mappings: Vec<MidiLearnEntry> = self.midi_learn_mappings.clone();
        for entry in &mappings {
            let cc_idx = entry.cc as usize;
            let cc_val = snapshot.cc_values[cc_idx];
            let prev_val = self.prev_cc_values[cc_idx];
            if (cc_val - prev_val).abs() > 1e-4 {
                let value = entry.range_start + cc_val * (entry.range_end - entry.range_start);
                if let Some(param_id) = key_to_param_id(&entry.parameter) {
                    self.events.send(EngineEvent::ParameterChange { id: param_id, value });
                }
                self.sync_cc_to_ui(&entry.parameter, value);
            }
        }

        self.prev_cc_values = snapshot.cc_values;
    }

    /// Updates the UI-local mirror field for `key` after a CC routing event
    /// so the knob display matches what the engine will play.
    fn sync_cc_to_ui(&mut self, key: &str, value: f32) {
        match key {
            // Filter
            "filter_cutoff_hz" => self.filter_cutoff_hz = value,
            "filter_resonance" => self.filter_resonance = value,
            // Global
            "master_volume" => self.master_volume = value,
            "pitch_offset_semis" => self.pitch_offset_semis = value,
            "bpm" => self.bpm = value,
            // Amp envelope
            "amp_attack_secs" => self.amp_attack_secs = value,
            "amp_decay_secs" => self.amp_decay_secs = value,
            "amp_sustain_level" => self.amp_sustain_level = value,
            "amp_release_secs" => self.amp_release_secs = value,
            // Env2
            "env2_attack_secs" => self.env2_attack_secs = value,
            "env2_decay_secs" => self.env2_decay_secs = value,
            "env2_sustain_level" => self.env2_sustain_level = value,
            "env2_release_secs" => self.env2_release_secs = value,
            "env2_attack_curve" => self.env2_attack_curve = value,
            "env2_decay_curve" => self.env2_decay_curve = value,
            "env2_release_curve" => self.env2_release_curve = value,
            // LFOs
            "lfo1_rate_hz" => self.lfo1_rate_hz = value,
            "lfo2_rate_hz" => self.lfo2_rate_hz = value,
            // Oscillators (note: preset keys use "osc1_" not "osc_1_")
            "osc1_level" => self.osc_level[0] = value,
            "osc2_level" => self.osc_level[1] = value,
            "osc3_level" => self.osc_level[2] = value,
            "sub_level" => self.sub_level = value,
            "sub_pan" => self.sub_pan = value,
            "osc1_detune_cents" => self.osc_detune_cents[0] = value,
            "osc2_detune_cents" => self.osc_detune_cents[1] = value,
            "osc3_detune_cents" => self.osc_detune_cents[2] = value,
            "osc1_pan" => self.osc_pan[0] = value,
            "osc2_pan" => self.osc_pan[1] = value,
            "osc3_pan" => self.osc_pan[2] = value,
            "osc1_unison_voices" => self.osc_unison_voices[0] = value,
            "osc2_unison_voices" => self.osc_unison_voices[1] = value,
            "osc3_unison_voices" => self.osc_unison_voices[2] = value,
            "osc1_unison_detune_cents" => self.osc_unison_detune_cents[0] = value,
            "osc2_unison_detune_cents" => self.osc_unison_detune_cents[1] = value,
            "osc3_unison_detune_cents" => self.osc_unison_detune_cents[2] = value,
            "osc1_unison_spread" => self.osc_unison_spread[0] = value,
            "osc2_unison_spread" => self.osc_unison_spread[1] = value,
            "osc3_unison_spread" => self.osc_unison_spread[2] = value,
            // Mod matrix amounts
            "mod_slot_amount_0" => self.mod_slot_amount[0] = value,
            "mod_slot_amount_1" => self.mod_slot_amount[1] = value,
            "mod_slot_amount_2" => self.mod_slot_amount[2] = value,
            "mod_slot_amount_3" => self.mod_slot_amount[3] = value,
            "mod_slot_amount_4" => self.mod_slot_amount[4] = value,
            "mod_slot_amount_5" => self.mod_slot_amount[5] = value,
            "mod_slot_amount_6" => self.mod_slot_amount[6] = value,
            "mod_slot_amount_7" => self.mod_slot_amount[7] = value,
            // FM slots
            "slot_level_0" => self.slot_level[0] = value,
            "slot_level_1" => self.slot_level[1] = value,
            "slot_pan_0" => self.slot_pan[0] = value,
            "slot_pan_1" => self.slot_pan[1] = value,
            // FM operator params
            "fm_op_level_0_0" => self.fm_op_level[0][0] = value,
            "fm_op_level_0_1" => self.fm_op_level[0][1] = value,
            "fm_op_level_0_2" => self.fm_op_level[0][2] = value,
            "fm_op_level_0_3" => self.fm_op_level[0][3] = value,
            "fm_op_level_1_0" => self.fm_op_level[1][0] = value,
            "fm_op_level_1_1" => self.fm_op_level[1][1] = value,
            "fm_op_level_1_2" => self.fm_op_level[1][2] = value,
            "fm_op_level_1_3" => self.fm_op_level[1][3] = value,
            "fm_op_ratio_fine_0_0" => self.fm_op_ratio_fine[0][0] = value,
            "fm_op_ratio_fine_0_1" => self.fm_op_ratio_fine[0][1] = value,
            "fm_op_ratio_fine_0_2" => self.fm_op_ratio_fine[0][2] = value,
            "fm_op_ratio_fine_0_3" => self.fm_op_ratio_fine[0][3] = value,
            "fm_op_ratio_fine_1_0" => self.fm_op_ratio_fine[1][0] = value,
            "fm_op_ratio_fine_1_1" => self.fm_op_ratio_fine[1][1] = value,
            "fm_op_ratio_fine_1_2" => self.fm_op_ratio_fine[1][2] = value,
            "fm_op_ratio_fine_1_3" => self.fm_op_ratio_fine[1][3] = value,
            "fm_op_attack_secs_0_0" => self.fm_op_attack_secs[0][0] = value,
            "fm_op_attack_secs_0_1" => self.fm_op_attack_secs[0][1] = value,
            "fm_op_attack_secs_0_2" => self.fm_op_attack_secs[0][2] = value,
            "fm_op_attack_secs_0_3" => self.fm_op_attack_secs[0][3] = value,
            "fm_op_attack_secs_1_0" => self.fm_op_attack_secs[1][0] = value,
            "fm_op_attack_secs_1_1" => self.fm_op_attack_secs[1][1] = value,
            "fm_op_attack_secs_1_2" => self.fm_op_attack_secs[1][2] = value,
            "fm_op_attack_secs_1_3" => self.fm_op_attack_secs[1][3] = value,
            "fm_op_decay_secs_0_0" => self.fm_op_decay_secs[0][0] = value,
            "fm_op_decay_secs_0_1" => self.fm_op_decay_secs[0][1] = value,
            "fm_op_decay_secs_0_2" => self.fm_op_decay_secs[0][2] = value,
            "fm_op_decay_secs_0_3" => self.fm_op_decay_secs[0][3] = value,
            "fm_op_decay_secs_1_0" => self.fm_op_decay_secs[1][0] = value,
            "fm_op_decay_secs_1_1" => self.fm_op_decay_secs[1][1] = value,
            "fm_op_decay_secs_1_2" => self.fm_op_decay_secs[1][2] = value,
            "fm_op_decay_secs_1_3" => self.fm_op_decay_secs[1][3] = value,
            "fm_op_sustain_level_0_0" => self.fm_op_sustain_level[0][0] = value,
            "fm_op_sustain_level_0_1" => self.fm_op_sustain_level[0][1] = value,
            "fm_op_sustain_level_0_2" => self.fm_op_sustain_level[0][2] = value,
            "fm_op_sustain_level_0_3" => self.fm_op_sustain_level[0][3] = value,
            "fm_op_sustain_level_1_0" => self.fm_op_sustain_level[1][0] = value,
            "fm_op_sustain_level_1_1" => self.fm_op_sustain_level[1][1] = value,
            "fm_op_sustain_level_1_2" => self.fm_op_sustain_level[1][2] = value,
            "fm_op_sustain_level_1_3" => self.fm_op_sustain_level[1][3] = value,
            "fm_op_release_secs_0_0" => self.fm_op_release_secs[0][0] = value,
            "fm_op_release_secs_0_1" => self.fm_op_release_secs[0][1] = value,
            "fm_op_release_secs_0_2" => self.fm_op_release_secs[0][2] = value,
            "fm_op_release_secs_0_3" => self.fm_op_release_secs[0][3] = value,
            "fm_op_release_secs_1_0" => self.fm_op_release_secs[1][0] = value,
            "fm_op_release_secs_1_1" => self.fm_op_release_secs[1][1] = value,
            "fm_op_release_secs_1_2" => self.fm_op_release_secs[1][2] = value,
            "fm_op_release_secs_1_3" => self.fm_op_release_secs[1][3] = value,
            "fm_op_feedback_0_3" => self.fm_op_feedback[0][3] = value,
            "fm_op_feedback_1_3" => self.fm_op_feedback[1][3] = value,
            // FX — EQ
            "fx_eq_low_gain_db" => self.fx_eq_low_gain_db = value,
            "fx_eq_low_freq_hz" => self.fx_eq_low_freq_hz = value,
            "fx_eq_mid_gain_db" => self.fx_eq_mid_gain_db = value,
            "fx_eq_mid_freq_hz" => self.fx_eq_mid_freq_hz = value,
            "fx_eq_mid_q" => self.fx_eq_mid_q = value,
            "fx_eq_high_gain_db" => self.fx_eq_high_gain_db = value,
            "fx_eq_high_freq_hz" => self.fx_eq_high_freq_hz = value,
            // FX — drive
            "fx_drive_drive" => self.fx_drive_drive = value,
            "fx_drive_asymmetry" => self.fx_drive_asymmetry = value,
            // FX — chorus
            "fx_chorus_rate_hz" => self.fx_chorus_rate_hz = value,
            "fx_chorus_depth_ms" => self.fx_chorus_depth_ms = value,
            "fx_chorus_mix" => self.fx_chorus_mix = value,
            "fx_chorus_spread" => self.fx_chorus_spread = value,
            // FX — delay
            "fx_delay_time_secs" => self.fx_delay_time_secs = value,
            "fx_delay_feedback" => self.fx_delay_feedback = value,
            "fx_delay_mix" => self.fx_delay_mix = value,
            "fx_delay_lowcut_hz" => self.fx_delay_lowcut_hz = value,
            // FX — reverb
            "fx_reverb_predelay_ms" => self.fx_reverb_predelay_ms = value,
            "fx_reverb_decay_secs" => self.fx_reverb_decay_secs = value,
            "fx_reverb_size" => self.fx_reverb_size = value,
            "fx_reverb_damping" => self.fx_reverb_damping = value,
            "fx_reverb_mix" => self.fx_reverb_mix = value,
            // Arp
            "arp_bpm" => self.arp_bpm = value,
            "arp_gate" => self.arp_gate = value,
            "arp_swing" => self.arp_swing = value,
            _ => {}
        }
    }
}

/// Maps a preset parameter key to its `ParamId`. Used by MIDI Learn routing
/// to send a `ParameterChange` event for any learned parameter.
pub(crate) fn key_to_param_id(key: &str) -> Option<ParamId> {
    Some(match key {
        // Filter
        "filter_cutoff_hz" => ParamId::FilterCutoffHz,
        "filter_resonance" => ParamId::FilterResonance,
        // Global
        "master_volume" => ParamId::MasterVolume,
        "pitch_offset_semis" => ParamId::PitchOffsetSemis,
        "bpm" => ParamId::Bpm,
        // Amp envelope
        "amp_attack_secs" => ParamId::AmpAttackSecs,
        "amp_decay_secs" => ParamId::AmpDecaySecs,
        "amp_sustain_level" => ParamId::AmpSustainLevel,
        "amp_release_secs" => ParamId::AmpReleaseSecs,
        // Env2
        "env2_attack_secs" => ParamId::Env2AttackSecs,
        "env2_decay_secs" => ParamId::Env2DecaySecs,
        "env2_sustain_level" => ParamId::Env2SustainLevel,
        "env2_release_secs" => ParamId::Env2ReleaseSecs,
        "env2_attack_curve" => ParamId::Env2AttackCurve,
        "env2_decay_curve" => ParamId::Env2DecayCurve,
        "env2_release_curve" => ParamId::Env2ReleaseCurve,
        // LFOs
        "lfo1_rate_hz" => ParamId::Lfo1RateHz,
        "lfo2_rate_hz" => ParamId::Lfo2RateHz,
        // Oscillators
        "osc1_level" => ParamId::Osc1Level,
        "osc2_level" => ParamId::Osc2Level,
        "osc3_level" => ParamId::Osc3Level,
        "sub_level" => ParamId::SubLevel,
        "sub_pan" => ParamId::SubPan,
        "osc1_detune_cents" => ParamId::Osc1DetuneCents,
        "osc2_detune_cents" => ParamId::Osc2DetuneCents,
        "osc3_detune_cents" => ParamId::Osc3DetuneCents,
        "osc1_pan" => ParamId::Osc1Pan,
        "osc2_pan" => ParamId::Osc2Pan,
        "osc3_pan" => ParamId::Osc3Pan,
        "osc1_unison_voices" => ParamId::Osc1UnisonVoices,
        "osc2_unison_voices" => ParamId::Osc2UnisonVoices,
        "osc3_unison_voices" => ParamId::Osc3UnisonVoices,
        "osc1_unison_detune_cents" => ParamId::Osc1UnisonDetuneCents,
        "osc2_unison_detune_cents" => ParamId::Osc2UnisonDetuneCents,
        "osc3_unison_detune_cents" => ParamId::Osc3UnisonDetuneCents,
        "osc1_unison_spread" => ParamId::Osc1UnisonSpread,
        "osc2_unison_spread" => ParamId::Osc2UnisonSpread,
        "osc3_unison_spread" => ParamId::Osc3UnisonSpread,
        // Mod matrix amounts
        "mod_slot_amount_0" => ParamId::ModSlotAmount(0),
        "mod_slot_amount_1" => ParamId::ModSlotAmount(1),
        "mod_slot_amount_2" => ParamId::ModSlotAmount(2),
        "mod_slot_amount_3" => ParamId::ModSlotAmount(3),
        "mod_slot_amount_4" => ParamId::ModSlotAmount(4),
        "mod_slot_amount_5" => ParamId::ModSlotAmount(5),
        "mod_slot_amount_6" => ParamId::ModSlotAmount(6),
        "mod_slot_amount_7" => ParamId::ModSlotAmount(7),
        // FM slots
        "slot_level_0" => ParamId::SlotLevel(0),
        "slot_level_1" => ParamId::SlotLevel(1),
        "slot_pan_0" => ParamId::SlotPan(0),
        "slot_pan_1" => ParamId::SlotPan(1),
        // FM operator level
        "fm_op_level_0_0" => ParamId::FmOpLevel(0x00),
        "fm_op_level_0_1" => ParamId::FmOpLevel(0x01),
        "fm_op_level_0_2" => ParamId::FmOpLevel(0x02),
        "fm_op_level_0_3" => ParamId::FmOpLevel(0x03),
        "fm_op_level_1_0" => ParamId::FmOpLevel(0x10),
        "fm_op_level_1_1" => ParamId::FmOpLevel(0x11),
        "fm_op_level_1_2" => ParamId::FmOpLevel(0x12),
        "fm_op_level_1_3" => ParamId::FmOpLevel(0x13),
        // FM operator ratio fine
        "fm_op_ratio_fine_0_0" => ParamId::FmOpRatioFine(0x00),
        "fm_op_ratio_fine_0_1" => ParamId::FmOpRatioFine(0x01),
        "fm_op_ratio_fine_0_2" => ParamId::FmOpRatioFine(0x02),
        "fm_op_ratio_fine_0_3" => ParamId::FmOpRatioFine(0x03),
        "fm_op_ratio_fine_1_0" => ParamId::FmOpRatioFine(0x10),
        "fm_op_ratio_fine_1_1" => ParamId::FmOpRatioFine(0x11),
        "fm_op_ratio_fine_1_2" => ParamId::FmOpRatioFine(0x12),
        "fm_op_ratio_fine_1_3" => ParamId::FmOpRatioFine(0x13),
        // FM operator attack
        "fm_op_attack_secs_0_0" => ParamId::FmOpAttackSecs(0x00),
        "fm_op_attack_secs_0_1" => ParamId::FmOpAttackSecs(0x01),
        "fm_op_attack_secs_0_2" => ParamId::FmOpAttackSecs(0x02),
        "fm_op_attack_secs_0_3" => ParamId::FmOpAttackSecs(0x03),
        "fm_op_attack_secs_1_0" => ParamId::FmOpAttackSecs(0x10),
        "fm_op_attack_secs_1_1" => ParamId::FmOpAttackSecs(0x11),
        "fm_op_attack_secs_1_2" => ParamId::FmOpAttackSecs(0x12),
        "fm_op_attack_secs_1_3" => ParamId::FmOpAttackSecs(0x13),
        // FM operator decay
        "fm_op_decay_secs_0_0" => ParamId::FmOpDecaySecs(0x00),
        "fm_op_decay_secs_0_1" => ParamId::FmOpDecaySecs(0x01),
        "fm_op_decay_secs_0_2" => ParamId::FmOpDecaySecs(0x02),
        "fm_op_decay_secs_0_3" => ParamId::FmOpDecaySecs(0x03),
        "fm_op_decay_secs_1_0" => ParamId::FmOpDecaySecs(0x10),
        "fm_op_decay_secs_1_1" => ParamId::FmOpDecaySecs(0x11),
        "fm_op_decay_secs_1_2" => ParamId::FmOpDecaySecs(0x12),
        "fm_op_decay_secs_1_3" => ParamId::FmOpDecaySecs(0x13),
        // FM operator sustain
        "fm_op_sustain_level_0_0" => ParamId::FmOpSustainLevel(0x00),
        "fm_op_sustain_level_0_1" => ParamId::FmOpSustainLevel(0x01),
        "fm_op_sustain_level_0_2" => ParamId::FmOpSustainLevel(0x02),
        "fm_op_sustain_level_0_3" => ParamId::FmOpSustainLevel(0x03),
        "fm_op_sustain_level_1_0" => ParamId::FmOpSustainLevel(0x10),
        "fm_op_sustain_level_1_1" => ParamId::FmOpSustainLevel(0x11),
        "fm_op_sustain_level_1_2" => ParamId::FmOpSustainLevel(0x12),
        "fm_op_sustain_level_1_3" => ParamId::FmOpSustainLevel(0x13),
        // FM operator release
        "fm_op_release_secs_0_0" => ParamId::FmOpReleaseSecs(0x00),
        "fm_op_release_secs_0_1" => ParamId::FmOpReleaseSecs(0x01),
        "fm_op_release_secs_0_2" => ParamId::FmOpReleaseSecs(0x02),
        "fm_op_release_secs_0_3" => ParamId::FmOpReleaseSecs(0x03),
        "fm_op_release_secs_1_0" => ParamId::FmOpReleaseSecs(0x10),
        "fm_op_release_secs_1_1" => ParamId::FmOpReleaseSecs(0x11),
        "fm_op_release_secs_1_2" => ParamId::FmOpReleaseSecs(0x12),
        "fm_op_release_secs_1_3" => ParamId::FmOpReleaseSecs(0x13),
        // FM operator feedback (op 3 only)
        "fm_op_feedback_0_3" => ParamId::FmOpFeedback(0x03),
        "fm_op_feedback_1_3" => ParamId::FmOpFeedback(0x13),
        // FX — EQ
        "fx_eq_low_gain_db" => ParamId::FxEqLowGainDb,
        "fx_eq_low_freq_hz" => ParamId::FxEqLowFreqHz,
        "fx_eq_mid_gain_db" => ParamId::FxEqMidGainDb,
        "fx_eq_mid_freq_hz" => ParamId::FxEqMidFreqHz,
        "fx_eq_mid_q" => ParamId::FxEqMidQ,
        "fx_eq_high_gain_db" => ParamId::FxEqHighGainDb,
        "fx_eq_high_freq_hz" => ParamId::FxEqHighFreqHz,
        // FX — drive
        "fx_drive_drive" => ParamId::FxDriveDrive,
        "fx_drive_asymmetry" => ParamId::FxDriveAsymmetry,
        // FX — chorus
        "fx_chorus_rate_hz" => ParamId::FxChorusRateHz,
        "fx_chorus_depth_ms" => ParamId::FxChorusDepthMs,
        "fx_chorus_mix" => ParamId::FxChorusMix,
        "fx_chorus_spread" => ParamId::FxChorusSpread,
        // FX — delay
        "fx_delay_time_secs" => ParamId::FxDelayTimeSecs,
        "fx_delay_feedback" => ParamId::FxDelayFeedback,
        "fx_delay_mix" => ParamId::FxDelayMix,
        "fx_delay_lowcut_hz" => ParamId::FxDelayLowcutHz,
        // FX — reverb
        "fx_reverb_predelay_ms" => ParamId::FxReverbPredelayMs,
        "fx_reverb_decay_secs" => ParamId::FxReverbDecaySecs,
        "fx_reverb_size" => ParamId::FxReverbSize,
        "fx_reverb_damping" => ParamId::FxReverbDamping,
        "fx_reverb_mix" => ParamId::FxReverbMix,
        // Arp
        "arp_bpm" => ParamId::ArpBpm,
        "arp_gate" => ParamId::ArpGate,
        "arp_swing" => ParamId::ArpSwing,
        _ => return None,
    })
}
