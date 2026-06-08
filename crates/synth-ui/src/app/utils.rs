use synth_engine::ParamId;

/// Maps a preset parameter key (as used in `snapshot_to_map`) to the
/// `(ParamId, range_min, range_max)` tuple needed to convert a CC value
/// (0..=1) to a ParameterChange. Only covers the most common MIDI-learnable
/// destinations; unlisted keys are silently ignored.
pub(crate) fn param_range_for_key(key: &str) -> Option<(ParamId, f32, f32)> {
    Some(match key {
        "filter_cutoff_hz" => (ParamId::FilterCutoffHz, 20.0, 20_000.0),
        "filter_resonance" => (ParamId::FilterResonance, 0.0, 1.0),
        "master_volume" => (ParamId::MasterVolume, 0.0, 1.0),
        "pitch_offset_semis" => (ParamId::PitchOffsetSemis, -24.0, 24.0),
        "amp_attack_secs" => (ParamId::AmpAttackSecs, 0.001, 10.0),
        "amp_decay_secs" => (ParamId::AmpDecaySecs, 0.001, 10.0),
        "amp_sustain_level" => (ParamId::AmpSustainLevel, 0.0, 1.0),
        "amp_release_secs" => (ParamId::AmpReleaseSecs, 0.001, 10.0),
        "osc_1_level" => (ParamId::Osc1Level, 0.0, 1.0),
        "osc_2_level" => (ParamId::Osc2Level, 0.0, 1.0),
        "osc_3_level" => (ParamId::Osc3Level, 0.0, 1.0),
        "sub_level" => (ParamId::SubLevel, 0.0, 1.0),
        "lfo1_rate_hz" => (ParamId::Lfo1RateHz, 0.01, 20.0),
        "lfo2_rate_hz" => (ParamId::Lfo2RateHz, 0.01, 20.0),
        "fx_chorus_mix" => (ParamId::FxChorusMix, 0.0, 1.0),
        "fx_delay_mix" => (ParamId::FxDelayMix, 0.0, 1.0),
        "fx_reverb_mix" => (ParamId::FxReverbMix, 0.0, 1.0),
        "bpm" => (ParamId::Bpm, 20.0, 300.0),
        _ => return None,
    })
}

/// Formats seconds as `"N ms"` below 1 s or `"N.NN s"` at or above 1 s.
pub(crate) fn secs_format(v: f32) -> String {
    if v < 1.0 {
        format!("{:.0} ms", v * 1000.0)
    } else {
        format!("{:.2} s", v)
    }
}

/// Formats a MIDI note number as scientific pitch notation (`C4` = 60).
pub(crate) fn midi_note_label(note_midi: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = i32::from(note_midi / 12) - 1;
    let name = NAMES[usize::from(note_midi % 12)];
    format!("{name}{octave}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_60_is_c4() {
        assert_eq!(midi_note_label(60), "C4");
    }

    #[test]
    fn midi_48_is_c3() {
        assert_eq!(midi_note_label(48), "C3");
    }

    #[test]
    fn midi_69_is_a4() {
        assert_eq!(midi_note_label(69), "A4");
    }

    #[test]
    fn secs_format_below_one_second_shows_ms() {
        assert_eq!(secs_format(0.010), "10 ms");
        assert_eq!(secs_format(0.200), "200 ms");
    }

    #[test]
    fn secs_format_at_or_above_one_second_shows_s() {
        assert!(secs_format(1.0).contains('s'));
        assert!(secs_format(3.5).contains('s'));
    }
}
