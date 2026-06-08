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
