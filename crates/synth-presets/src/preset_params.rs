//! Conversion between [`ParamSnapshot`] and the preset parameter map.
//!
//! `snapshot_to_map` serialises the saveable portion of a snapshot into
//! a `BTreeMap<String, f32>`.  `map_to_events` deserialises it back into
//! a `Vec<EngineEvent>` ready to push through the parameter bus.
//! `map_to_snapshot` builds a full `ParamSnapshot` from the map (used to
//! sync UI local fields immediately on preset load).

use std::collections::BTreeMap;

use synth_engine::EngineEvent;
use synth_engine::{
    EngineEvent as Ev, FilterMode, FilterRouting, FilterSlope, MOD_MATRIX_SLOTS, ParamId, ParamSnapshot, SEQ_MAX_STEPS,
    Waveform,
};

/// Serialises all saveable params from `snap` into a stable string-keyed
/// map. Keys use snake_case matching the `ParamSnapshot` field names;
/// indexed fields append `_<index>` suffixes.
#[must_use]
pub fn snapshot_to_map(snap: &ParamSnapshot) -> BTreeMap<String, f32> {
    let mut m = BTreeMap::new();

    // Discrete: waveform and filter modes/routing encoded as index
    m.insert("waveform".into(), snap.waveform.index() as f32);
    m.insert("filter_mode".into(), snap.filter_mode.index() as f32);
    m.insert("filter2_mode".into(), snap.filter2_mode.index() as f32);
    m.insert("filter_routing".into(), snap.filter_routing.index() as f32);
    m.insert("filter_slope_0".into(), snap.filter_slope[0].index() as f32);
    m.insert("filter_slope_1".into(), snap.filter_slope[1].index() as f32);

    // Global
    m.insert("pitch_offset_semis".into(), snap.pitch_offset_semis);
    m.insert("master_volume".into(), snap.master_volume);
    m.insert("bpm".into(), snap.bpm);

    // Amp envelope
    m.insert("amp_attack_secs".into(), snap.amp_attack_secs);
    m.insert("amp_decay_secs".into(), snap.amp_decay_secs);
    m.insert("amp_sustain_level".into(), snap.amp_sustain_level);
    m.insert("amp_release_secs".into(), snap.amp_release_secs);

    // Filter
    m.insert("filter_cutoff_hz".into(), snap.filter_cutoff_hz);
    m.insert("filter_resonance".into(), snap.filter_resonance);
    m.insert("filter2_cutoff_hz".into(), snap.filter2_cutoff_hz);
    m.insert("filter2_resonance".into(), snap.filter2_resonance);

    // Osc 1 (index 0)
    for i in 0..3usize {
        let n = i + 1;
        m.insert(format!("osc{n}_level"), snap.osc_main_levels[i]);
        m.insert(format!("osc{n}_detune_cents"), snap.osc_main_detune_cents[i]);
        m.insert(format!("osc{n}_pan"), snap.osc_main_pans[i]);
        m.insert(format!("osc{n}_unison_voices"), snap.osc_main_unison_voices[i]);
        m.insert(
            format!("osc{n}_unison_detune_cents"),
            snap.osc_main_unison_detune_cents[i],
        );
        m.insert(format!("osc{n}_unison_spread"), snap.osc_main_unison_spreads[i]);
    }
    m.insert("sub_level".into(), snap.sub_level);
    m.insert("sub_pan".into(), snap.sub_pan);

    // LFO 1
    m.insert("lfo1_rate_hz".into(), snap.lfo1_rate_hz);
    m.insert("lfo1_shape_index".into(), snap.lfo1_shape_index as f32);
    m.insert("lfo1_reset_on_note_on".into(), f32::from(snap.lfo1_reset_on_note_on));
    m.insert("lfo1_sync_enabled".into(), f32::from(snap.lfo1_sync_enabled));
    m.insert("lfo1_sync_division_index".into(), snap.lfo1_sync_division_index as f32);

    // LFO 2
    m.insert("lfo2_rate_hz".into(), snap.lfo2_rate_hz);
    m.insert("lfo2_shape_index".into(), snap.lfo2_shape_index as f32);
    m.insert("lfo2_reset_on_note_on".into(), f32::from(snap.lfo2_reset_on_note_on));
    m.insert("lfo2_sync_enabled".into(), f32::from(snap.lfo2_sync_enabled));
    m.insert("lfo2_sync_division_index".into(), snap.lfo2_sync_division_index as f32);

    // Env2
    m.insert("env2_attack_secs".into(), snap.env2_attack_secs);
    m.insert("env2_decay_secs".into(), snap.env2_decay_secs);
    m.insert("env2_sustain_level".into(), snap.env2_sustain_level);
    m.insert("env2_release_secs".into(), snap.env2_release_secs);
    m.insert("env2_attack_curve".into(), snap.env2_attack_curve);
    m.insert("env2_decay_curve".into(), snap.env2_decay_curve);
    m.insert("env2_release_curve".into(), snap.env2_release_curve);

    // Env3
    m.insert("env3_attack_secs".into(), snap.env3_attack_secs);
    m.insert("env3_decay_secs".into(), snap.env3_decay_secs);
    m.insert("env3_sustain_level".into(), snap.env3_sustain_level);
    m.insert("env3_release_secs".into(), snap.env3_release_secs);
    m.insert("env3_attack_curve".into(), snap.env3_attack_curve);
    m.insert("env3_decay_curve".into(), snap.env3_decay_curve);
    m.insert("env3_release_curve".into(), snap.env3_release_curve);

    // Mod matrix (MOD_MATRIX_SLOTS slots)
    for i in 0..MOD_MATRIX_SLOTS {
        m.insert(format!("mod_slot_enabled_{i}"), f32::from(snap.mod_slot_enabled[i]));
        m.insert(format!("mod_slot_source_{i}"), f32::from(snap.mod_slot_source[i]));
        m.insert(format!("mod_slot_dest_{i}"), f32::from(snap.mod_slot_dest[i]));
        m.insert(format!("mod_slot_amount_{i}"), snap.mod_slot_amount[i]);
        m.insert(format!("mod_slot_via_{i}"), f32::from(snap.mod_slot_via[i]));
    }

    // FM synthesis (2 slots × 4 ops).
    // slot_mode is no longer serialised — slot 0 is always Subtractive,
    // slot 1 is always FM.
    for s in 0..2usize {
        m.insert(format!("slot_level_{s}"), snap.slot_level[s]);
        m.insert(format!("slot_pan_{s}"), snap.slot_pan[s]);
        m.insert(format!("fm_algorithm_{s}"), f32::from(snap.fm_algorithm[s]));
        for op in 0..4usize {
            m.insert(
                format!("fm_op_ratio_integer_{s}_{op}"),
                f32::from(snap.fm_op_ratio_integer[s][op]),
            );
            m.insert(format!("fm_op_ratio_fine_{s}_{op}"), snap.fm_op_ratio_fine_cents[s][op]);
            m.insert(format!("fm_op_level_{s}_{op}"), snap.fm_op_level[s][op]);
            m.insert(format!("fm_op_attack_secs_{s}_{op}"), snap.fm_op_attack_secs[s][op]);
            m.insert(format!("fm_op_decay_secs_{s}_{op}"), snap.fm_op_decay_secs[s][op]);
            m.insert(format!("fm_op_sustain_level_{s}_{op}"), snap.fm_op_sustain_level[s][op]);
            m.insert(format!("fm_op_release_secs_{s}_{op}"), snap.fm_op_release_secs[s][op]);
            m.insert(format!("fm_op_feedback_{s}_{op}"), snap.fm_op_feedback[s][op]);
        }
    }

    // FX chain
    m.insert("fx_eq_enabled".into(), f32::from(snap.fx_eq_enabled));
    m.insert("fx_eq_low_gain_db".into(), snap.fx_eq_low_gain_db);
    m.insert("fx_eq_low_freq_hz".into(), snap.fx_eq_low_freq_hz);
    m.insert("fx_eq_mid_gain_db".into(), snap.fx_eq_mid_gain_db);
    m.insert("fx_eq_mid_freq_hz".into(), snap.fx_eq_mid_freq_hz);
    m.insert("fx_eq_mid_q".into(), snap.fx_eq_mid_q);
    m.insert("fx_eq_high_gain_db".into(), snap.fx_eq_high_gain_db);
    m.insert("fx_eq_high_freq_hz".into(), snap.fx_eq_high_freq_hz);
    m.insert("fx_drive_enabled".into(), f32::from(snap.fx_drive_enabled));
    m.insert("fx_drive_drive".into(), snap.fx_drive_drive);
    m.insert("fx_drive_asymmetry".into(), snap.fx_drive_asymmetry);
    m.insert("fx_chorus_enabled".into(), f32::from(snap.fx_chorus_enabled));
    m.insert("fx_chorus_rate_hz".into(), snap.fx_chorus_rate_hz);
    m.insert("fx_chorus_depth_ms".into(), snap.fx_chorus_depth_ms);
    m.insert("fx_chorus_mix".into(), snap.fx_chorus_mix);
    m.insert("fx_chorus_spread".into(), snap.fx_chorus_spread);
    m.insert("fx_delay_enabled".into(), f32::from(snap.fx_delay_enabled));
    m.insert("fx_delay_time_secs".into(), snap.fx_delay_time_secs);
    m.insert("fx_delay_feedback".into(), snap.fx_delay_feedback);
    m.insert("fx_delay_mix".into(), snap.fx_delay_mix);
    m.insert("fx_delay_lowcut_hz".into(), snap.fx_delay_lowcut_hz);
    m.insert("fx_delay_ping_pong".into(), f32::from(snap.fx_delay_ping_pong));
    m.insert("fx_reverb_enabled".into(), f32::from(snap.fx_reverb_enabled));
    m.insert("fx_reverb_predelay_ms".into(), snap.fx_reverb_predelay_ms);
    m.insert("fx_reverb_decay_secs".into(), snap.fx_reverb_decay_secs);
    m.insert("fx_reverb_size".into(), snap.fx_reverb_size);
    m.insert("fx_reverb_damping".into(), snap.fx_reverb_damping);
    m.insert("fx_reverb_mix".into(), snap.fx_reverb_mix);

    // Arpeggiator
    m.insert("arp_enabled".into(), f32::from(snap.arp_enabled));
    m.insert("arp_mode".into(), f32::from(snap.arp_mode));
    m.insert("arp_octaves".into(), f32::from(snap.arp_octaves));
    m.insert("arp_rate".into(), f32::from(snap.arp_rate));
    m.insert("arp_gate".into(), snap.arp_gate);
    m.insert("arp_swing".into(), snap.arp_swing);

    // Step sequencer
    m.insert("seq_enabled".into(), f32::from(snap.seq_enabled));
    m.insert("seq_length".into(), f32::from(snap.seq_length));
    m.insert("seq_mode".into(), f32::from(snap.seq_mode));
    m.insert("seq_rate".into(), f32::from(snap.seq_rate));
    m.insert("seq_swing".into(), snap.seq_swing);
    for i in 0..SEQ_MAX_STEPS {
        m.insert(format!("seq_step{i}_note"), f32::from(snap.seq_step_note[i]));
        m.insert(format!("seq_step{i}_velocity"), f32::from(snap.seq_step_velocity[i]));
        m.insert(format!("seq_step{i}_gate"), snap.seq_step_gate[i]);
        m.insert(format!("seq_step{i}_rest"), f32::from(snap.seq_step_rest[i]));
        m.insert(format!("seq_step{i}_mod"), snap.seq_step_mod[i]);
    }

    m
}

/// Converts a preset parameter map back into engine events. The caller
/// should push all returned events through the event bus.  A preceding
/// `AllNotesOff` is the caller's responsibility.
///
/// Unknown keys are silently ignored (forward-compatibility).
#[must_use]
pub fn map_to_events(m: &BTreeMap<String, f32>) -> Vec<EngineEvent> {
    let mut ev = Vec::with_capacity(m.len() + 2);

    macro_rules! pc {
        ($key:expr, $id:expr) => {
            if let Some(&v) = m.get($key) {
                ev.push(Ev::ParameterChange { id: $id, value: v });
            }
        };
    }

    // Discrete events with dedicated EngineEvent variants
    if let Some(&v) = m.get("waveform") {
        ev.push(Ev::SetOscillatorWaveform {
            waveform: Waveform::from_index(v as usize),
        });
    }
    if let Some(&v) = m.get("filter_mode") {
        ev.push(Ev::SetFilterMode {
            mode: FilterMode::from_index(v as usize),
        });
    }
    if let Some(&v) = m.get("filter2_mode") {
        ev.push(Ev::SetFilter2Mode {
            mode: FilterMode::from_index(v as usize),
        });
    }
    if let Some(&v) = m.get("filter_routing") {
        ev.push(Ev::SetFilterRouting {
            routing: FilterRouting::from_index(v as usize),
        });
    }
    if let Some(&v) = m.get("filter_slope_0") {
        ev.push(Ev::SetFilterSlope {
            filter_idx: 0,
            slope: FilterSlope::from_index(v as usize),
        });
    }
    if let Some(&v) = m.get("filter_slope_1") {
        ev.push(Ev::SetFilterSlope {
            filter_idx: 1,
            slope: FilterSlope::from_index(v as usize),
        });
    }

    // Global
    pc!("pitch_offset_semis", ParamId::PitchOffsetSemis);
    pc!("master_volume", ParamId::MasterVolume);
    pc!("bpm", ParamId::Bpm);

    // Amp envelope
    pc!("amp_attack_secs", ParamId::AmpAttackSecs);
    pc!("amp_decay_secs", ParamId::AmpDecaySecs);
    pc!("amp_sustain_level", ParamId::AmpSustainLevel);
    pc!("amp_release_secs", ParamId::AmpReleaseSecs);

    // Filter
    pc!("filter_cutoff_hz", ParamId::FilterCutoffHz);
    pc!("filter_resonance", ParamId::FilterResonance);
    pc!("filter2_cutoff_hz", ParamId::Filter2CutoffHz);
    pc!("filter2_resonance", ParamId::Filter2Resonance);

    // Osc arrays
    for i in 0..3usize {
        let n = i + 1;
        pc!(
            &format!("osc{n}_level"),
            match i {
                0 => ParamId::Osc1Level,
                1 => ParamId::Osc2Level,
                _ => ParamId::Osc3Level,
            }
        );
        pc!(
            &format!("osc{n}_detune_cents"),
            match i {
                0 => ParamId::Osc1DetuneCents,
                1 => ParamId::Osc2DetuneCents,
                _ => ParamId::Osc3DetuneCents,
            }
        );
        pc!(
            &format!("osc{n}_pan"),
            match i {
                0 => ParamId::Osc1Pan,
                1 => ParamId::Osc2Pan,
                _ => ParamId::Osc3Pan,
            }
        );
        pc!(
            &format!("osc{n}_unison_voices"),
            match i {
                0 => ParamId::Osc1UnisonVoices,
                1 => ParamId::Osc2UnisonVoices,
                _ => ParamId::Osc3UnisonVoices,
            }
        );
        pc!(
            &format!("osc{n}_unison_detune_cents"),
            match i {
                0 => ParamId::Osc1UnisonDetuneCents,
                1 => ParamId::Osc2UnisonDetuneCents,
                _ => ParamId::Osc3UnisonDetuneCents,
            }
        );
        pc!(
            &format!("osc{n}_unison_spread"),
            match i {
                0 => ParamId::Osc1UnisonSpread,
                1 => ParamId::Osc2UnisonSpread,
                _ => ParamId::Osc3UnisonSpread,
            }
        );
    }
    pc!("sub_level", ParamId::SubLevel);
    pc!("sub_pan", ParamId::SubPan);

    // LFO 1
    pc!("lfo1_rate_hz", ParamId::Lfo1RateHz);
    pc!("lfo1_shape_index", ParamId::Lfo1Shape);
    pc!("lfo1_reset_on_note_on", ParamId::Lfo1ResetOnNoteOn);
    pc!("lfo1_sync_enabled", ParamId::Lfo1SyncEnabled);
    pc!("lfo1_sync_division_index", ParamId::Lfo1SyncDivision);

    // LFO 2
    pc!("lfo2_rate_hz", ParamId::Lfo2RateHz);
    pc!("lfo2_shape_index", ParamId::Lfo2Shape);
    pc!("lfo2_reset_on_note_on", ParamId::Lfo2ResetOnNoteOn);
    pc!("lfo2_sync_enabled", ParamId::Lfo2SyncEnabled);
    pc!("lfo2_sync_division_index", ParamId::Lfo2SyncDivision);

    // Env2
    pc!("env2_attack_secs", ParamId::Env2AttackSecs);
    pc!("env2_decay_secs", ParamId::Env2DecaySecs);
    pc!("env2_sustain_level", ParamId::Env2SustainLevel);
    pc!("env2_release_secs", ParamId::Env2ReleaseSecs);
    pc!("env2_attack_curve", ParamId::Env2AttackCurve);
    pc!("env2_decay_curve", ParamId::Env2DecayCurve);
    pc!("env2_release_curve", ParamId::Env2ReleaseCurve);
    pc!("env3_attack_secs", ParamId::Env3AttackSecs);
    pc!("env3_decay_secs", ParamId::Env3DecaySecs);
    pc!("env3_sustain_level", ParamId::Env3SustainLevel);
    pc!("env3_release_secs", ParamId::Env3ReleaseSecs);
    pc!("env3_attack_curve", ParamId::Env3AttackCurve);
    pc!("env3_decay_curve", ParamId::Env3DecayCurve);
    pc!("env3_release_curve", ParamId::Env3ReleaseCurve);

    // Mod matrix
    for i in 0..MOD_MATRIX_SLOTS as u8 {
        let ii = i as usize;
        pc!(&format!("mod_slot_enabled_{ii}"), ParamId::ModSlotEnabled(i));
        pc!(&format!("mod_slot_source_{ii}"), ParamId::ModSlotSource(i));
        pc!(&format!("mod_slot_dest_{ii}"), ParamId::ModSlotDest(i));
        pc!(&format!("mod_slot_amount_{ii}"), ParamId::ModSlotAmount(i));
        pc!(&format!("mod_slot_via_{ii}"), ParamId::ModSlotVia(i));
    }

    // FM synthesis (slot_mode is no longer a runtime parameter)
    for s in 0..2u8 {
        let si = s as usize;
        pc!(&format!("slot_level_{si}"), ParamId::SlotLevel(s));
        pc!(&format!("slot_pan_{si}"), ParamId::SlotPan(s));
        pc!(&format!("fm_algorithm_{si}"), ParamId::FmAlgorithm(s));
        for op in 0..4u8 {
            let oi = op as usize;
            let packed = (s << 4) | op;
            pc!(
                &format!("fm_op_ratio_integer_{si}_{oi}"),
                ParamId::FmOpRatioInteger(packed)
            );
            pc!(&format!("fm_op_ratio_fine_{si}_{oi}"), ParamId::FmOpRatioFine(packed));
            pc!(&format!("fm_op_level_{si}_{oi}"), ParamId::FmOpLevel(packed));
            pc!(&format!("fm_op_attack_secs_{si}_{oi}"), ParamId::FmOpAttackSecs(packed));
            pc!(&format!("fm_op_decay_secs_{si}_{oi}"), ParamId::FmOpDecaySecs(packed));
            pc!(
                &format!("fm_op_sustain_level_{si}_{oi}"),
                ParamId::FmOpSustainLevel(packed)
            );
            pc!(
                &format!("fm_op_release_secs_{si}_{oi}"),
                ParamId::FmOpReleaseSecs(packed)
            );
            pc!(&format!("fm_op_feedback_{si}_{oi}"), ParamId::FmOpFeedback(packed));
        }
    }

    // FX chain
    pc!("fx_eq_enabled", ParamId::FxEqEnabled);
    pc!("fx_eq_low_gain_db", ParamId::FxEqLowGainDb);
    pc!("fx_eq_low_freq_hz", ParamId::FxEqLowFreqHz);
    pc!("fx_eq_mid_gain_db", ParamId::FxEqMidGainDb);
    pc!("fx_eq_mid_freq_hz", ParamId::FxEqMidFreqHz);
    pc!("fx_eq_mid_q", ParamId::FxEqMidQ);
    pc!("fx_eq_high_gain_db", ParamId::FxEqHighGainDb);
    pc!("fx_eq_high_freq_hz", ParamId::FxEqHighFreqHz);
    pc!("fx_drive_enabled", ParamId::FxDriveEnabled);
    pc!("fx_drive_drive", ParamId::FxDriveDrive);
    pc!("fx_drive_asymmetry", ParamId::FxDriveAsymmetry);
    pc!("fx_chorus_enabled", ParamId::FxChorusEnabled);
    pc!("fx_chorus_rate_hz", ParamId::FxChorusRateHz);
    pc!("fx_chorus_depth_ms", ParamId::FxChorusDepthMs);
    pc!("fx_chorus_mix", ParamId::FxChorusMix);
    pc!("fx_chorus_spread", ParamId::FxChorusSpread);
    pc!("fx_delay_enabled", ParamId::FxDelayEnabled);
    pc!("fx_delay_time_secs", ParamId::FxDelayTimeSecs);
    pc!("fx_delay_feedback", ParamId::FxDelayFeedback);
    pc!("fx_delay_mix", ParamId::FxDelayMix);
    pc!("fx_delay_lowcut_hz", ParamId::FxDelayLowcutHz);
    pc!("fx_delay_ping_pong", ParamId::FxDelayPingPong);
    pc!("fx_reverb_enabled", ParamId::FxReverbEnabled);
    pc!("fx_reverb_predelay_ms", ParamId::FxReverbPredelayMs);
    pc!("fx_reverb_decay_secs", ParamId::FxReverbDecaySecs);
    pc!("fx_reverb_size", ParamId::FxReverbSize);
    pc!("fx_reverb_damping", ParamId::FxReverbDamping);
    pc!("fx_reverb_mix", ParamId::FxReverbMix);

    // Arpeggiator
    pc!("arp_enabled", ParamId::ArpEnabled);
    pc!("arp_mode", ParamId::ArpMode);
    pc!("arp_octaves", ParamId::ArpOctaves);
    pc!("arp_rate", ParamId::ArpRate);
    // Back-compat: legacy presets stored a separate arp BPM. The transport
    // tempo is now unified, so an old `arp_bpm` key drives the global Bpm.
    pc!("arp_bpm", ParamId::Bpm);
    pc!("arp_gate", ParamId::ArpGate);
    pc!("arp_swing", ParamId::ArpSwing);

    // Step sequencer
    pc!("seq_enabled", ParamId::SeqEnabled);
    pc!("seq_length", ParamId::SeqLength);
    pc!("seq_mode", ParamId::SeqMode);
    pc!("seq_rate", ParamId::SeqRate);
    pc!("seq_swing", ParamId::SeqSwing);
    for i in 0..SEQ_MAX_STEPS as u8 {
        let ii = i as usize;
        pc!(&format!("seq_step{ii}_note"), ParamId::SeqStepNote(i));
        pc!(&format!("seq_step{ii}_velocity"), ParamId::SeqStepVelocity(i));
        pc!(&format!("seq_step{ii}_gate"), ParamId::SeqStepGate(i));
        pc!(&format!("seq_step{ii}_rest"), ParamId::SeqStepRest(i));
        pc!(&format!("seq_step{ii}_mod"), ParamId::SeqStepMod(i));
    }

    ev
}

/// Builds a `ParamSnapshot` from a preset parameter map by starting
/// from defaults and overlaying the map's values. Used to sync UI
/// local fields immediately after a preset load, before the engine
/// events have been processed.
#[must_use]
pub fn map_to_snapshot(m: &BTreeMap<String, f32>) -> ParamSnapshot {
    let mut s = ParamSnapshot::default();

    macro_rules! get {
        ($key:expr, $field:expr) => {
            if let Some(&v) = m.get($key) {
                $field = v;
            }
        };
    }
    macro_rules! get_bool {
        ($key:expr, $field:expr) => {
            if let Some(&v) = m.get($key) {
                $field = v >= 0.5;
            }
        };
    }
    macro_rules! get_u8 {
        ($key:expr, $field:expr) => {
            if let Some(&v) = m.get($key) {
                $field = v as u8;
            }
        };
    }
    macro_rules! get_i8 {
        ($key:expr, $field:expr) => {
            if let Some(&v) = m.get($key) {
                $field = v as i8;
            }
        };
    }
    macro_rules! get_usize {
        ($key:expr, $field:expr) => {
            if let Some(&v) = m.get($key) {
                $field = v as usize;
            }
        };
    }

    if let Some(&v) = m.get("waveform") {
        s.waveform = Waveform::from_index(v as usize);
    }
    if let Some(&v) = m.get("filter_mode") {
        s.filter_mode = FilterMode::from_index(v as usize);
    }
    if let Some(&v) = m.get("filter2_mode") {
        s.filter2_mode = FilterMode::from_index(v as usize);
    }
    if let Some(&v) = m.get("filter_routing") {
        s.filter_routing = FilterRouting::from_index(v as usize);
    }
    if let Some(&v) = m.get("filter_slope_0") {
        s.filter_slope[0] = FilterSlope::from_index(v as usize);
    }
    if let Some(&v) = m.get("filter_slope_1") {
        s.filter_slope[1] = FilterSlope::from_index(v as usize);
    }

    get!("pitch_offset_semis", s.pitch_offset_semis);
    get!("master_volume", s.master_volume);
    get!("bpm", s.bpm);
    get!("amp_attack_secs", s.amp_attack_secs);
    get!("amp_decay_secs", s.amp_decay_secs);
    get!("amp_sustain_level", s.amp_sustain_level);
    get!("amp_release_secs", s.amp_release_secs);
    get!("filter_cutoff_hz", s.filter_cutoff_hz);
    get!("filter_resonance", s.filter_resonance);
    get!("filter2_cutoff_hz", s.filter2_cutoff_hz);
    get!("filter2_resonance", s.filter2_resonance);

    for i in 0..3usize {
        let n = i + 1;
        get!(&format!("osc{n}_level"), s.osc_main_levels[i]);
        get!(&format!("osc{n}_detune_cents"), s.osc_main_detune_cents[i]);
        get!(&format!("osc{n}_pan"), s.osc_main_pans[i]);
        get!(&format!("osc{n}_unison_voices"), s.osc_main_unison_voices[i]);
        get!(
            &format!("osc{n}_unison_detune_cents"),
            s.osc_main_unison_detune_cents[i]
        );
        get!(&format!("osc{n}_unison_spread"), s.osc_main_unison_spreads[i]);
    }
    get!("sub_level", s.sub_level);
    get!("sub_pan", s.sub_pan);

    get!("lfo1_rate_hz", s.lfo1_rate_hz);
    get_usize!("lfo1_shape_index", s.lfo1_shape_index);
    get_bool!("lfo1_reset_on_note_on", s.lfo1_reset_on_note_on);
    get_bool!("lfo1_sync_enabled", s.lfo1_sync_enabled);
    get_usize!("lfo1_sync_division_index", s.lfo1_sync_division_index);
    get!("lfo2_rate_hz", s.lfo2_rate_hz);
    get_usize!("lfo2_shape_index", s.lfo2_shape_index);
    get_bool!("lfo2_reset_on_note_on", s.lfo2_reset_on_note_on);
    get_bool!("lfo2_sync_enabled", s.lfo2_sync_enabled);
    get_usize!("lfo2_sync_division_index", s.lfo2_sync_division_index);

    get!("env2_attack_secs", s.env2_attack_secs);
    get!("env2_decay_secs", s.env2_decay_secs);
    get!("env2_sustain_level", s.env2_sustain_level);
    get!("env2_release_secs", s.env2_release_secs);
    get!("env2_attack_curve", s.env2_attack_curve);
    get!("env2_decay_curve", s.env2_decay_curve);
    get!("env2_release_curve", s.env2_release_curve);
    get!("env3_attack_secs", s.env3_attack_secs);
    get!("env3_decay_secs", s.env3_decay_secs);
    get!("env3_sustain_level", s.env3_sustain_level);
    get!("env3_release_secs", s.env3_release_secs);
    get!("env3_attack_curve", s.env3_attack_curve);
    get!("env3_decay_curve", s.env3_decay_curve);
    get!("env3_release_curve", s.env3_release_curve);

    for i in 0..MOD_MATRIX_SLOTS {
        get_bool!(&format!("mod_slot_enabled_{i}"), s.mod_slot_enabled[i]);
        get_u8!(&format!("mod_slot_source_{i}"), s.mod_slot_source[i]);
        get_u8!(&format!("mod_slot_dest_{i}"), s.mod_slot_dest[i]);
        get!(&format!("mod_slot_amount_{i}"), s.mod_slot_amount[i]);
        get_u8!(&format!("mod_slot_via_{i}"), s.mod_slot_via[i]);
    }

    // slot_mode is no longer stored — slot 0 is always Sub, slot 1 always FM.
    // Presets saved before this change may carry slot_mode_0 / slot_mode_1;
    // those keys are ignored here and handled by the migration below.
    for si in 0..2usize {
        get!(&format!("slot_level_{si}"), s.slot_level[si]);
        get!(&format!("slot_pan_{si}"), s.slot_pan[si]);
        get_u8!(&format!("fm_algorithm_{si}"), s.fm_algorithm[si]);
        for oi in 0..4usize {
            get_u8!(&format!("fm_op_ratio_integer_{si}_{oi}"), s.fm_op_ratio_integer[si][oi]);
            get!(&format!("fm_op_ratio_fine_{si}_{oi}"), s.fm_op_ratio_fine_cents[si][oi]);
            get!(&format!("fm_op_level_{si}_{oi}"), s.fm_op_level[si][oi]);
            get!(&format!("fm_op_attack_secs_{si}_{oi}"), s.fm_op_attack_secs[si][oi]);
            get!(&format!("fm_op_decay_secs_{si}_{oi}"), s.fm_op_decay_secs[si][oi]);
            get!(&format!("fm_op_sustain_level_{si}_{oi}"), s.fm_op_sustain_level[si][oi]);
            get!(&format!("fm_op_release_secs_{si}_{oi}"), s.fm_op_release_secs[si][oi]);
            get!(&format!("fm_op_feedback_{si}_{oi}"), s.fm_op_feedback[si][oi]);
        }
    }

    // Backward compatibility: v1.0 presets could place the FM bank on slot 0
    // (slot_mode_0 == 1.0).  Remap their FM parameters to the fixed slot 1
    // position so the patch sounds the same under the new fixed-slot layout.
    if m.get("slot_mode_0").copied().unwrap_or(0.0) >= 0.5 {
        let dflt = ParamSnapshot::default();
        s.fm_algorithm[1] = s.fm_algorithm[0];
        s.fm_algorithm[0] = dflt.fm_algorithm[0];
        for op in 0..4 {
            s.fm_op_ratio_integer[1][op] = s.fm_op_ratio_integer[0][op];
            s.fm_op_ratio_fine_cents[1][op] = s.fm_op_ratio_fine_cents[0][op];
            s.fm_op_level[1][op] = s.fm_op_level[0][op];
            s.fm_op_attack_secs[1][op] = s.fm_op_attack_secs[0][op];
            s.fm_op_decay_secs[1][op] = s.fm_op_decay_secs[0][op];
            s.fm_op_sustain_level[1][op] = s.fm_op_sustain_level[0][op];
            s.fm_op_release_secs[1][op] = s.fm_op_release_secs[0][op];
            s.fm_op_feedback[1][op] = s.fm_op_feedback[0][op];
            // Reset slot 0 FM params to defaults (Sub bank; engine ignores
            // them for the Sub path but keeps them for clean state).
            s.fm_op_ratio_integer[0][op] = dflt.fm_op_ratio_integer[0][op];
            s.fm_op_ratio_fine_cents[0][op] = dflt.fm_op_ratio_fine_cents[0][op];
            s.fm_op_level[0][op] = dflt.fm_op_level[0][op];
            s.fm_op_attack_secs[0][op] = dflt.fm_op_attack_secs[0][op];
            s.fm_op_decay_secs[0][op] = dflt.fm_op_decay_secs[0][op];
            s.fm_op_sustain_level[0][op] = dflt.fm_op_sustain_level[0][op];
            s.fm_op_release_secs[0][op] = dflt.fm_op_release_secs[0][op];
            s.fm_op_feedback[0][op] = dflt.fm_op_feedback[0][op];
        }
        // Carry the FM output level to slot 1; silence slot 0 (Sub bank).
        s.slot_level[1] = s.slot_level[0];
        s.slot_pan[1] = s.slot_pan[0];
        s.slot_level[0] = 0.0;
        s.slot_pan[0] = 0.0;
    }

    get_bool!("fx_eq_enabled", s.fx_eq_enabled);
    get!("fx_eq_low_gain_db", s.fx_eq_low_gain_db);
    get!("fx_eq_low_freq_hz", s.fx_eq_low_freq_hz);
    get!("fx_eq_mid_gain_db", s.fx_eq_mid_gain_db);
    get!("fx_eq_mid_freq_hz", s.fx_eq_mid_freq_hz);
    get!("fx_eq_mid_q", s.fx_eq_mid_q);
    get!("fx_eq_high_gain_db", s.fx_eq_high_gain_db);
    get!("fx_eq_high_freq_hz", s.fx_eq_high_freq_hz);
    get_bool!("fx_drive_enabled", s.fx_drive_enabled);
    get!("fx_drive_drive", s.fx_drive_drive);
    get!("fx_drive_asymmetry", s.fx_drive_asymmetry);
    get_bool!("fx_chorus_enabled", s.fx_chorus_enabled);
    get!("fx_chorus_rate_hz", s.fx_chorus_rate_hz);
    get!("fx_chorus_depth_ms", s.fx_chorus_depth_ms);
    get!("fx_chorus_mix", s.fx_chorus_mix);
    get!("fx_chorus_spread", s.fx_chorus_spread);
    get_bool!("fx_delay_enabled", s.fx_delay_enabled);
    get!("fx_delay_time_secs", s.fx_delay_time_secs);
    get!("fx_delay_feedback", s.fx_delay_feedback);
    get!("fx_delay_mix", s.fx_delay_mix);
    get!("fx_delay_lowcut_hz", s.fx_delay_lowcut_hz);
    get_bool!("fx_delay_ping_pong", s.fx_delay_ping_pong);
    get_bool!("fx_reverb_enabled", s.fx_reverb_enabled);
    get!("fx_reverb_predelay_ms", s.fx_reverb_predelay_ms);
    get!("fx_reverb_decay_secs", s.fx_reverb_decay_secs);
    get!("fx_reverb_size", s.fx_reverb_size);
    get!("fx_reverb_damping", s.fx_reverb_damping);
    get!("fx_reverb_mix", s.fx_reverb_mix);

    get_bool!("arp_enabled", s.arp_enabled);
    get_u8!("arp_mode", s.arp_mode);
    get_u8!("arp_octaves", s.arp_octaves);
    get_u8!("arp_rate", s.arp_rate);
    // Back-compat: a legacy `arp_bpm` key feeds the now-unified transport Bpm.
    get!("arp_bpm", s.bpm);
    get!("arp_gate", s.arp_gate);
    get!("arp_swing", s.arp_swing);

    // Step sequencer
    get_bool!("seq_enabled", s.seq_enabled);
    get_u8!("seq_length", s.seq_length);
    get_u8!("seq_mode", s.seq_mode);
    get_u8!("seq_rate", s.seq_rate);
    get!("seq_swing", s.seq_swing);
    for i in 0..SEQ_MAX_STEPS {
        get_i8!(&format!("seq_step{i}_note"), s.seq_step_note[i]);
        get_u8!(&format!("seq_step{i}_velocity"), s.seq_step_velocity[i]);
        get!(&format!("seq_step{i}_gate"), s.seq_step_gate[i]);
        get_bool!(&format!("seq_step{i}_rest"), s.seq_step_rest[i]);
        get!(&format!("seq_step{i}_mod"), s.seq_step_mod[i]);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every saveable field survives a map round-trip: snapshot → map → snapshot.
    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn round_trip_map() {
        let mut orig = ParamSnapshot::default();

        // Set non-default values for every field so the test catches
        // any field that is accidentally omitted from snapshot_to_map or
        // map_to_snapshot.
        orig.waveform = Waveform::Square;
        orig.filter_mode = FilterMode::BandPass;
        orig.pitch_offset_semis = 3.0;
        orig.master_volume = 0.7;
        orig.bpm = 140.0;
        orig.amp_attack_secs = 0.5;
        orig.amp_decay_secs = 0.3;
        orig.amp_sustain_level = 0.6;
        orig.amp_release_secs = 1.2;
        orig.filter_cutoff_hz = 3_000.0;
        orig.filter_resonance = 0.4;
        orig.filter2_cutoff_hz = 1_200.0;
        orig.filter2_resonance = 0.6;
        orig.filter2_mode = FilterMode::HighPass;
        orig.filter_routing = FilterRouting::Parallel;
        orig.filter_slope = [FilterSlope::TwentyFourDbOct, FilterSlope::TwelveDbOct];
        for i in 0..3 {
            orig.osc_main_levels[i] = 0.8 - i as f32 * 0.1;
            orig.osc_main_detune_cents[i] = 5.0 + i as f32;
            orig.osc_main_pans[i] = -0.3 + i as f32 * 0.3;
            orig.osc_main_unison_voices[i] = (i + 1) as f32;
            orig.osc_main_unison_detune_cents[i] = 10.0 + i as f32;
            orig.osc_main_unison_spreads[i] = 0.5;
        }
        orig.sub_level = 0.2;
        orig.sub_pan = 0.1;
        orig.lfo1_rate_hz = 3.5;
        orig.lfo1_shape_index = 2;
        orig.lfo1_reset_on_note_on = true;
        orig.lfo1_sync_enabled = true;
        orig.lfo1_sync_division_index = 3;
        orig.lfo2_rate_hz = 0.5;
        orig.lfo2_shape_index = 1;
        orig.lfo2_sync_division_index = 2;
        orig.env2_attack_secs = 0.02;
        orig.env2_decay_secs = 0.5;
        orig.env2_sustain_level = 0.7;
        orig.env2_release_secs = 0.8;
        orig.env2_attack_curve = 0.3;
        orig.env2_decay_curve = -0.2;
        orig.env2_release_curve = 0.5;
        orig.env3_attack_secs = 0.03;
        orig.env3_decay_secs = 0.6;
        orig.env3_sustain_level = 0.6;
        orig.env3_release_secs = 0.9;
        orig.env3_attack_curve = -0.4;
        orig.env3_decay_curve = 0.25;
        orig.env3_release_curve = -0.5;
        for i in 0..MOD_MATRIX_SLOTS {
            orig.mod_slot_enabled[i] = i % 2 == 0;
            orig.mod_slot_source[i] = i as u8;
            orig.mod_slot_dest[i] = (i + 1) as u8;
            orig.mod_slot_amount[i] = i as f32 * 10.0;
            orig.mod_slot_via[i] = i as u8;
        }
        for s in 0..2 {
            orig.slot_level[s] = 0.9;
            orig.slot_pan[s] = -0.1;
            orig.fm_algorithm[s] = (s + 1) as u8;
            for op in 0..4 {
                orig.fm_op_ratio_integer[s][op] = (op + 1) as u8;
                orig.fm_op_ratio_fine_cents[s][op] = op as f32 * 5.0;
                orig.fm_op_level[s][op] = 0.9 - op as f32 * 0.1;
                orig.fm_op_attack_secs[s][op] = 0.01 + op as f32 * 0.01;
                orig.fm_op_decay_secs[s][op] = 0.1 + op as f32 * 0.05;
                orig.fm_op_sustain_level[s][op] = 0.8 - op as f32 * 0.1;
                orig.fm_op_release_secs[s][op] = 0.2 + op as f32 * 0.1;
                orig.fm_op_feedback[s][op] = op as f32 * 0.1;
            }
        }
        orig.fx_eq_enabled = true;
        orig.fx_eq_low_gain_db = 3.0;
        orig.fx_eq_low_freq_hz = 150.0;
        orig.fx_eq_mid_gain_db = -2.0;
        orig.fx_eq_mid_freq_hz = 2_000.0;
        orig.fx_eq_mid_q = 1.5;
        orig.fx_eq_high_gain_db = 2.0;
        orig.fx_eq_high_freq_hz = 8_000.0;
        orig.fx_drive_enabled = true;
        orig.fx_drive_drive = 4.0;
        orig.fx_drive_asymmetry = 0.3;
        orig.fx_chorus_enabled = true;
        orig.fx_chorus_rate_hz = 1.5;
        orig.fx_chorus_depth_ms = 5.0;
        orig.fx_chorus_mix = 0.4;
        orig.fx_chorus_spread = 0.8;
        orig.fx_delay_enabled = true;
        orig.fx_delay_time_secs = 0.5;
        orig.fx_delay_feedback = 0.5;
        orig.fx_delay_mix = 0.25;
        orig.fx_delay_lowcut_hz = 300.0;
        orig.fx_delay_ping_pong = true;
        orig.fx_reverb_enabled = true;
        orig.fx_reverb_predelay_ms = 20.0;
        orig.fx_reverb_decay_secs = 3.0;
        orig.fx_reverb_size = 0.8;
        orig.fx_reverb_damping = 0.4;
        orig.fx_reverb_mix = 0.3;
        orig.arp_enabled = true;
        orig.arp_mode = 2;
        orig.arp_octaves = 3;
        orig.arp_rate = 1;
        orig.arp_gate = 0.7;
        orig.arp_swing = 0.6;
        orig.seq_enabled = true;
        orig.seq_length = 12;
        orig.seq_mode = 2;
        orig.seq_rate = 3;
        orig.seq_swing = 0.65;
        for i in 0..SEQ_MAX_STEPS {
            orig.seq_step_note[i] = (i as i8) - 5;
            orig.seq_step_velocity[i] = (i as u8) * 4;
            orig.seq_step_gate[i] = (i as f32) / 16.0;
            orig.seq_step_rest[i] = i % 3 == 0;
            orig.seq_step_mod[i] = (i as f32) / 16.0 - 0.5;
        }

        let map = snapshot_to_map(&orig);
        let got = map_to_snapshot(&map);

        // Compare every saveable field
        assert_eq!(orig.waveform, got.waveform);
        assert_eq!(orig.filter_mode, got.filter_mode);
        assert_eq!(orig.pitch_offset_semis, got.pitch_offset_semis);
        assert_eq!(orig.master_volume, got.master_volume);
        assert_eq!(orig.bpm, got.bpm);
        assert_eq!(orig.amp_attack_secs, got.amp_attack_secs);
        assert_eq!(orig.amp_decay_secs, got.amp_decay_secs);
        assert_eq!(orig.amp_sustain_level, got.amp_sustain_level);
        assert_eq!(orig.amp_release_secs, got.amp_release_secs);
        assert_eq!(orig.filter_cutoff_hz, got.filter_cutoff_hz);
        assert_eq!(orig.filter_resonance, got.filter_resonance);
        assert_eq!(orig.filter2_cutoff_hz, got.filter2_cutoff_hz);
        assert_eq!(orig.filter2_resonance, got.filter2_resonance);
        assert_eq!(orig.filter2_mode, got.filter2_mode);
        assert_eq!(orig.filter_routing, got.filter_routing);
        assert_eq!(orig.filter_slope, got.filter_slope);
        assert_eq!(orig.osc_main_levels, got.osc_main_levels);
        assert_eq!(orig.osc_main_detune_cents, got.osc_main_detune_cents);
        assert_eq!(orig.osc_main_pans, got.osc_main_pans);
        assert_eq!(orig.osc_main_unison_voices, got.osc_main_unison_voices);
        assert_eq!(orig.osc_main_unison_detune_cents, got.osc_main_unison_detune_cents);
        assert_eq!(orig.osc_main_unison_spreads, got.osc_main_unison_spreads);
        assert_eq!(orig.sub_level, got.sub_level);
        assert_eq!(orig.sub_pan, got.sub_pan);
        assert_eq!(orig.lfo1_rate_hz, got.lfo1_rate_hz);
        assert_eq!(orig.lfo1_shape_index, got.lfo1_shape_index);
        assert_eq!(orig.lfo1_reset_on_note_on, got.lfo1_reset_on_note_on);
        assert_eq!(orig.lfo1_sync_enabled, got.lfo1_sync_enabled);
        assert_eq!(orig.lfo1_sync_division_index, got.lfo1_sync_division_index);
        assert_eq!(orig.lfo2_rate_hz, got.lfo2_rate_hz);
        assert_eq!(orig.lfo2_shape_index, got.lfo2_shape_index);
        assert_eq!(orig.lfo2_sync_division_index, got.lfo2_sync_division_index);
        assert_eq!(orig.env2_attack_secs, got.env2_attack_secs);
        assert_eq!(orig.env2_decay_secs, got.env2_decay_secs);
        assert_eq!(orig.env2_sustain_level, got.env2_sustain_level);
        assert_eq!(orig.env2_release_secs, got.env2_release_secs);
        assert_eq!(orig.env2_attack_curve, got.env2_attack_curve);
        assert_eq!(orig.env2_decay_curve, got.env2_decay_curve);
        assert_eq!(orig.env2_release_curve, got.env2_release_curve);
        assert_eq!(orig.env3_attack_secs, got.env3_attack_secs);
        assert_eq!(orig.env3_decay_secs, got.env3_decay_secs);
        assert_eq!(orig.env3_sustain_level, got.env3_sustain_level);
        assert_eq!(orig.env3_release_secs, got.env3_release_secs);
        assert_eq!(orig.env3_attack_curve, got.env3_attack_curve);
        assert_eq!(orig.env3_decay_curve, got.env3_decay_curve);
        assert_eq!(orig.env3_release_curve, got.env3_release_curve);
        assert_eq!(orig.mod_slot_enabled, got.mod_slot_enabled);
        assert_eq!(orig.mod_slot_source, got.mod_slot_source);
        assert_eq!(orig.mod_slot_dest, got.mod_slot_dest);
        assert_eq!(orig.mod_slot_amount, got.mod_slot_amount);
        assert_eq!(orig.mod_slot_via, got.mod_slot_via);
        assert_eq!(orig.slot_level, got.slot_level);
        assert_eq!(orig.slot_pan, got.slot_pan);
        assert_eq!(orig.fm_algorithm, got.fm_algorithm);
        assert_eq!(orig.fm_op_ratio_integer, got.fm_op_ratio_integer);
        assert_eq!(orig.fm_op_ratio_fine_cents, got.fm_op_ratio_fine_cents);
        assert_eq!(orig.fm_op_level, got.fm_op_level);
        assert_eq!(orig.fm_op_attack_secs, got.fm_op_attack_secs);
        assert_eq!(orig.fm_op_decay_secs, got.fm_op_decay_secs);
        assert_eq!(orig.fm_op_sustain_level, got.fm_op_sustain_level);
        assert_eq!(orig.fm_op_release_secs, got.fm_op_release_secs);
        assert_eq!(orig.fm_op_feedback, got.fm_op_feedback);
        assert_eq!(orig.fx_eq_enabled, got.fx_eq_enabled);
        assert_eq!(orig.fx_eq_low_gain_db, got.fx_eq_low_gain_db);
        assert_eq!(orig.fx_eq_low_freq_hz, got.fx_eq_low_freq_hz);
        assert_eq!(orig.fx_eq_mid_gain_db, got.fx_eq_mid_gain_db);
        assert_eq!(orig.fx_eq_mid_freq_hz, got.fx_eq_mid_freq_hz);
        assert_eq!(orig.fx_eq_mid_q, got.fx_eq_mid_q);
        assert_eq!(orig.fx_eq_high_gain_db, got.fx_eq_high_gain_db);
        assert_eq!(orig.fx_eq_high_freq_hz, got.fx_eq_high_freq_hz);
        assert_eq!(orig.fx_drive_enabled, got.fx_drive_enabled);
        assert_eq!(orig.fx_drive_drive, got.fx_drive_drive);
        assert_eq!(orig.fx_drive_asymmetry, got.fx_drive_asymmetry);
        assert_eq!(orig.fx_chorus_enabled, got.fx_chorus_enabled);
        assert_eq!(orig.fx_chorus_rate_hz, got.fx_chorus_rate_hz);
        assert_eq!(orig.fx_chorus_depth_ms, got.fx_chorus_depth_ms);
        assert_eq!(orig.fx_chorus_mix, got.fx_chorus_mix);
        assert_eq!(orig.fx_chorus_spread, got.fx_chorus_spread);
        assert_eq!(orig.fx_delay_enabled, got.fx_delay_enabled);
        assert_eq!(orig.fx_delay_time_secs, got.fx_delay_time_secs);
        assert_eq!(orig.fx_delay_feedback, got.fx_delay_feedback);
        assert_eq!(orig.fx_delay_mix, got.fx_delay_mix);
        assert_eq!(orig.fx_delay_lowcut_hz, got.fx_delay_lowcut_hz);
        assert_eq!(orig.fx_delay_ping_pong, got.fx_delay_ping_pong);
        assert_eq!(orig.fx_reverb_enabled, got.fx_reverb_enabled);
        assert_eq!(orig.fx_reverb_predelay_ms, got.fx_reverb_predelay_ms);
        assert_eq!(orig.fx_reverb_decay_secs, got.fx_reverb_decay_secs);
        assert_eq!(orig.fx_reverb_size, got.fx_reverb_size);
        assert_eq!(orig.fx_reverb_damping, got.fx_reverb_damping);
        assert_eq!(orig.fx_reverb_mix, got.fx_reverb_mix);
        assert_eq!(orig.arp_enabled, got.arp_enabled);
        assert_eq!(orig.arp_mode, got.arp_mode);
        assert_eq!(orig.arp_octaves, got.arp_octaves);
        assert_eq!(orig.arp_rate, got.arp_rate);
        assert_eq!(orig.arp_gate, got.arp_gate);
        assert_eq!(orig.arp_swing, got.arp_swing);
        assert_eq!(orig.seq_enabled, got.seq_enabled);
        assert_eq!(orig.seq_length, got.seq_length);
        assert_eq!(orig.seq_mode, got.seq_mode);
        assert_eq!(orig.seq_rate, got.seq_rate);
        assert_eq!(orig.seq_swing, got.seq_swing);
        assert_eq!(orig.seq_step_note, got.seq_step_note);
        assert_eq!(orig.seq_step_velocity, got.seq_step_velocity);
        assert_eq!(orig.seq_step_gate, got.seq_step_gate);
        assert_eq!(orig.seq_step_rest, got.seq_step_rest);
        assert_eq!(orig.seq_step_mod, got.seq_step_mod);
    }

    /// A sparse preset (only a few keys set) must expand to the *full*
    /// parameter set before being applied, so omitted parameters reset to
    /// their defaults rather than retaining the previously loaded preset's
    /// values. Regression test for the "second preset only loads partially"
    /// bug: loaders go through `snapshot_to_map(map_to_snapshot(..))` so a
    /// sparse map yields an event for every parameter.
    #[test]
    fn sparse_map_expands_to_full_parameter_set() {
        let mut sparse = BTreeMap::new();
        sparse.insert("filter_cutoff_hz".to_string(), 300.0);

        let expanded = snapshot_to_map(&map_to_snapshot(&sparse));
        let default_full = snapshot_to_map(&ParamSnapshot::default());

        // The expanded map covers exactly the complete parameter set.
        assert_eq!(
            expanded.keys().collect::<Vec<_>>(),
            default_full.keys().collect::<Vec<_>>(),
            "expanded sparse map must contain every parameter key"
        );
        // The one key the preset set survives…
        assert!((expanded["filter_cutoff_hz"] - 300.0).abs() < 1e-6);
        // …and an omitted key takes its default rather than a stale value.
        assert_eq!(expanded["fx_reverb_enabled"], default_full["fx_reverb_enabled"]);

        // Applying the expanded map emits far more events than the sparse
        // map would, i.e. every parameter is explicitly (re)set on load.
        assert!(map_to_events(&expanded).len() > map_to_events(&sparse).len());
    }
}
