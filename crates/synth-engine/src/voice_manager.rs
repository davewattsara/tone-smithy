//! Polyphonic voice pool.
//!
//! Owns a fixed-size array of [`Voice`]s, hands one out per incoming
//! note-on (stealing when all are in use), and sums their stereo
//! outputs per frame. Note-off finds the voice holding the given MIDI
//! note and routes the release to it.
//!
//! Architecture context: [`audio-engine.md`](../../../../docs/planning/03-architecture/audio-engine.md)
//! §"Voice management".
//!
//! Real-time safety: every voice is pre-allocated in [`VoiceManager::new`].
//! No allocation, locks, or syscalls on the audio path
//! ([`design-patterns.md`](../../../../docs/planning/03-architecture/design-patterns.md)
//! §2.1).
//!
//! [`Voice`]: crate::voice::Voice

use crate::POLYPHONY;
use crate::filter::FilterMode;
use crate::oscillator::Waveform;
use crate::params::SampleParams;
use crate::voice::Voice;

/// Fixed-size pool of [`Voice`]s with note allocation and stealing.
///
/// **Allocation policy** (on note-on, in order):
///
/// 1. **Idle voice.** The first voice whose envelope is fully at zero.
/// 2. **Oldest releasing voice.** If no voice is idle, the voice in
///    its release phase whose note-off was issued the longest ago is
///    stolen — its release tail is the cheapest sound to lose.
/// 3. **Quietest voice.** If no voice is in release either, the voice
///    with the lowest current envelope level is stolen. Ties are
///    broken by oldest note-on, so very-recent attacks survive over
///    long-held sustain.
///
/// **Note-off** finds the voice currently holding the matching MIDI
/// note and releases it. If multiple voices play the same note (from
/// rapid retriggering during release), the oldest is released first.
///
/// **Sustain pedal** defers note-offs while the pedal is held. The
/// deferred set is a fixed `[bool; 128]` array indexed by MIDI note
/// number — no allocation. On pedal release all deferred notes fire.
/// A retriggered note while the pedal is held cancels its deferral.
///
/// **Polyphony summing** is intentionally unscaled: thirty-two
/// in-phase voices can exceed unity per channel. A soft limiter on
/// the global mix is M8 effect-chain scope; in the meantime, master
/// volume (M4) is the user-side knob.
pub struct VoiceManager {
    voices: [Voice; POLYPHONY],

    /// Monotonic counter incremented on every note-on and note-off so
    /// "oldest" / "newest" can be ranked without a wall clock. A `u64`
    /// at one tick per event survives a session longer than the heat
    /// death of any laptop.
    next_tick: u64,

    /// Tick at which each voice's most recent note started. `None`
    /// for voices that have never been triggered. Used as the
    /// tiebreaker in the third-pass steal.
    note_on_tick: [Option<u64>; POLYPHONY],

    /// Tick at which each voice's most recent note-off was issued.
    /// `None` for voices still in attack/decay/sustain, or that have
    /// never run at all. Used to rank voices in the second-pass
    /// (oldest-releasing) steal.
    note_off_tick: [Option<u64>; POLYPHONY],

    /// True while the sustain pedal (CC #64) is held down.
    sustain_held: bool,

    /// Per-MIDI-note flag: `true` means a NoteOff arrived while the
    /// sustain pedal was held and should fire when the pedal releases.
    /// Indexed by MIDI note number (0..=127).
    deferred_note_offs: [bool; 128],
}

impl VoiceManager {
    /// Creates a manager with all [`POLYPHONY`] voices pre-allocated
    /// at the given sample rate. Every voice starts idle.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            voices: [(); POLYPHONY].map(|()| Voice::new(sample_rate_hz)),
            next_tick: 0,
            note_on_tick: [None; POLYPHONY],
            note_off_tick: [None; POLYPHONY],
            sustain_held: false,
            deferred_note_offs: [false; 128],
        }
    }

    /// Triggers a note on the next available voice. Allocates a fresh
    /// voice if any are idle; otherwise steals per the policy in the
    /// type-level docs.
    ///
    /// If the sustain pedal is held and `note_midi` has a deferred
    /// release, the deferral is cancelled: the new note-on "re-takes"
    /// the voice, so a new attack plays when the user presses again
    /// while the pedal is down.
    pub fn note_on(&mut self, note_midi: u8, velocity: u8) {
        // Cancel any pending deferred release for this note so the
        // re-attack sounds immediately rather than cutting off at pedal
        // release.
        self.deferred_note_offs[note_midi as usize] = false;
        let index = self.allocate_voice();
        self.voices[index].note_on(note_midi, velocity);
        self.note_on_tick[index] = Some(self.next_tick);
        self.note_off_tick[index] = None;
        self.next_tick += 1;
    }

    /// Releases the voice currently holding `note_midi`, if any. If
    /// the sustain pedal is held the release is deferred until the
    /// pedal is lifted. A note-off for a note no voice holds is
    /// silently ignored (same behaviour as polyphonic hardware).
    pub fn note_off(&mut self, note_midi: u8) {
        if self.sustain_held {
            // Only defer if a voice actually holds the note; phantom
            // deferrals for notes that never played would fire
            // spuriously when the pedal releases.
            if self.find_oldest_voice_holding(note_midi).is_some() {
                self.deferred_note_offs[note_midi as usize] = true;
            }
        } else {
            self.release_note(note_midi);
        }
    }

    /// Updates the sustain-pedal state. When `held` transitions to
    /// `false` all deferred note-offs are fired in MIDI-note order.
    pub fn set_sustain(&mut self, held: bool) {
        self.sustain_held = held;
        if !held {
            for note in 0_u8..=127 {
                if self.deferred_note_offs[note as usize] {
                    self.deferred_note_offs[note as usize] = false;
                    self.release_note(note);
                }
            }
        }
    }

    /// Releases the voice holding `note_midi` immediately, without
    /// consulting the sustain state. The `note_off` path goes through
    /// this after the sustain check.
    fn release_note(&mut self, note_midi: u8) {
        let chosen = self.find_oldest_voice_holding(note_midi);
        if let Some(index) = chosen {
            self.voices[index].note_off(note_midi);
            self.note_off_tick[index] = Some(self.next_tick);
            self.next_tick += 1;
        }
    }

    /// Sets the amp-envelope release time (in seconds) on every voice.
    /// Stepped parameter — fans out immediately rather than per-sample.
    pub fn set_release_secs(&mut self, release_secs: f32) {
        for v in &mut self.voices {
            v.set_release_secs(release_secs);
        }
    }

    /// Sets the main-oscillator waveform on every voice. Discrete
    /// parameter; events arrive at block boundaries.
    pub fn set_main_waveform(&mut self, waveform: Waveform) {
        for v in &mut self.voices {
            v.set_main_waveform(waveform);
        }
    }

    /// Sets the filter output tap on every voice.
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        for v in &mut self.voices {
            v.set_filter_mode(mode);
        }
    }

    /// Produces one stereo frame as the sum of every non-idle voice.
    ///
    /// Idle voices skip their per-sample work — the oscillator phase
    /// accumulators don't advance for voices that aren't sounding,
    /// which is correct because the next note-on resets phases on the
    /// idle-to-attack transition inside the voice.
    pub fn next_sample(&mut self, params: &SampleParams) -> (f32, f32) {
        let mut sum_l = 0.0_f32;
        let mut sum_r = 0.0_f32;
        for v in &mut self.voices {
            if v.is_idle() {
                continue;
            }
            let (l, r) = v.next_sample(params);
            sum_l += l;
            sum_r += r;
        }
        (sum_l, sum_r)
    }

    /// Returns the number of voices currently producing audio. The
    /// engine forwards this into the parameter snapshot every block
    /// so the UI footer can show the live count.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| !v.is_idle()).count()
    }

    /// Returns the index of the oldest voice currently holding the
    /// given note, or `None` if no voice does. "Oldest" means smallest
    /// `note_on_tick`. Pulled out so the same logic stays in one
    /// place for tests and so `note_off` reads as a single intent.
    fn find_oldest_voice_holding(&self, note_midi: u8) -> Option<usize> {
        let mut chosen: Option<(usize, u64)> = None;
        for (i, v) in self.voices.iter().enumerate() {
            if v.held_note() != Some(note_midi) {
                continue;
            }
            let on_tick = self.note_on_tick[i].unwrap_or(u64::MAX);
            if chosen.is_none_or(|(_, t)| on_tick < t) {
                chosen = Some((i, on_tick));
            }
        }
        chosen.map(|(i, _)| i)
    }

    /// Picks a voice for a new note. See the type-level docs for the
    /// three-pass policy.
    fn allocate_voice(&self) -> usize {
        // Pass 1: any idle voice.
        for (i, v) in self.voices.iter().enumerate() {
            if v.is_idle() {
                return i;
            }
        }
        // Pass 2: oldest releasing voice. Smallest note_off_tick wins.
        let mut oldest_releasing: Option<(usize, u64)> = None;
        for (i, v) in self.voices.iter().enumerate() {
            if !v.is_releasing() {
                continue;
            }
            let off_tick = self.note_off_tick[i].unwrap_or(u64::MAX);
            if oldest_releasing.is_none_or(|(_, t)| off_tick < t) {
                oldest_releasing = Some((i, off_tick));
            }
        }
        if let Some((i, _)) = oldest_releasing {
            return i;
        }
        // Pass 3: quietest voice. Lowest envelope level wins; ties
        // broken by oldest note-on tick so brand-new attacks survive
        // over long-running sustains at the same level.
        let mut best: (usize, f32, u64) = (0, f32::INFINITY, u64::MAX);
        for (i, v) in self.voices.iter().enumerate() {
            let level = v.envelope_level();
            let on_tick = self.note_on_tick[i].unwrap_or(u64::MAX);
            let cheaper = level < best.1 || (level == best.1 && on_tick < best.2);
            if cheaper {
                best = (i, level, on_tick);
            }
        }
        best.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamSnapshot;

    /// Sample-params with the filter wide open, matching `voice.rs`'s
    /// test helper. Lets us focus voice-manager tests on the manager's
    /// allocation and summing without filter behaviour confounding the
    /// signal.
    fn default_sample_params() -> SampleParams {
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
        }
    }

    #[test]
    fn fresh_manager_has_no_active_voices() {
        let manager = VoiceManager::new(48_000.0);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn note_on_increments_active_count() {
        let mut manager = VoiceManager::new(48_000.0);
        manager.note_on(60, 100);
        assert_eq!(manager.active_count(), 1);
        manager.note_on(64, 100);
        manager.note_on(67, 100);
        assert_eq!(manager.active_count(), 3);
    }

    #[test]
    fn note_off_releases_the_matching_voice() {
        let mut manager = VoiceManager::new(48_000.0);
        manager.note_on(60, 100);
        manager.note_on(64, 100);
        manager.note_off(60);
        // The 60 voice is now in release (not yet idle); the 64 voice
        // is still attacking. Both contribute audio, so active_count
        // is still 2 — release isn't silence.
        assert_eq!(manager.active_count(), 2);
        let still_holding_60 = (0..POLYPHONY).any(|i| manager.voices[i].held_note() == Some(60));
        assert!(!still_holding_60, "note 60 should no longer be held by any voice");
        let still_holding_64 = (0..POLYPHONY).any(|i| manager.voices[i].held_note() == Some(64));
        assert!(still_holding_64, "note 64 should still be held");
    }

    #[test]
    fn note_off_for_unheld_note_is_a_no_op() {
        let mut manager = VoiceManager::new(48_000.0);
        manager.note_on(60, 100);
        manager.note_off(99); // never played
        let still_holding_60 = (0..POLYPHONY).any(|i| manager.voices[i].held_note() == Some(60));
        assert!(still_holding_60, "stray note-off should not affect held notes");
    }

    #[test]
    fn thirty_two_simultaneous_notes_all_sound() {
        let mut manager = VoiceManager::new(48_000.0);
        for n in 0..POLYPHONY {
            #[allow(clippy::cast_possible_truncation)]
            manager.note_on(36 + n as u8, 100);
        }
        assert_eq!(manager.active_count(), POLYPHONY);
        // Render a block and check the sum is audible — confirms
        // every voice actually contributed.
        let params = default_sample_params();
        let mut peak = 0.0_f32;
        for _ in 0..480 {
            let (l, r) = manager.next_sample(&params);
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 0.5, "expected audible polyphonic mix, peak {peak}");
    }

    #[test]
    fn thirty_third_note_steals_oldest_released() {
        let sample_rate = 48_000.0;
        let mut manager = VoiceManager::new(sample_rate);
        // A long release time keeps the released voices in the
        // release phase (not idle) across the few samples we render
        // between events. With the default 200 ms release a barely-
        // attacked voice would idle out before the next note-off.
        manager.set_release_secs(2.0);
        let params = default_sample_params();

        // Fill the pool. Pass 1 of `allocate_voice` picks the first
        // idle voice each time, so voice `i` ends up holding `36 + i`.
        for n in 0..POLYPHONY {
            #[allow(clippy::cast_possible_truncation)]
            manager.note_on(36 + n as u8, 100);
        }
        // Settle envelopes past attack so `release_start_level` is
        // substantial; otherwise the release step is tiny and the
        // voice idles in a few samples.
        for _ in 0..(sample_rate as usize / 4) {
            manager.next_sample(&params);
        }
        // Release voices 0, 1, 2 in order with a handful of samples
        // between so their `note_off_tick`s differ.
        manager.note_off(36);
        for _ in 0..8 {
            manager.next_sample(&params);
        }
        manager.note_off(37);
        for _ in 0..8 {
            manager.next_sample(&params);
        }
        manager.note_off(38);
        for _ in 0..8 {
            manager.next_sample(&params);
        }

        manager.note_on(99, 100);
        // Pass 1 finds no idle voice; pass 2 picks the smallest
        // `note_off_tick`, which is voice 0.
        let holding_99 = (0..POLYPHONY).find(|&i| manager.voices[i].held_note() == Some(99));
        assert_eq!(holding_99, Some(0), "expected voice 0 (oldest released) to be stolen");
    }

    #[test]
    fn thirty_third_note_with_no_release_steals_quietest() {
        let sample_rate = 48_000.0;
        let mut manager = VoiceManager::new(sample_rate);
        let params = default_sample_params();

        // Fill the pool one at a time, processing 16 samples between
        // each note-on so envelopes diverge. The first-allocated
        // voice has the highest envelope level; the last-allocated
        // has the lowest. None of them are in release.
        for n in 0..POLYPHONY {
            #[allow(clippy::cast_possible_truncation)]
            manager.note_on(36 + n as u8, 100);
            for _ in 0..16 {
                manager.next_sample(&params);
            }
        }

        let last_added_index =
            (0..POLYPHONY).find(|&i| manager.voices[i].held_note() == Some(36 + (POLYPHONY as u8) - 1));
        assert!(last_added_index.is_some(), "test setup: last note must be findable");

        manager.note_on(99, 100);
        // The quietest voice was the last-allocated one (it had the
        // shortest attack run). The steal should have put 99 into
        // that slot.
        let holding_99 = (0..POLYPHONY).find(|&i| manager.voices[i].held_note() == Some(99));
        assert_eq!(holding_99, last_added_index, "expected quietest voice to be stolen");
    }

    #[test]
    fn fan_out_release_seconds_reaches_every_voice() {
        let mut manager = VoiceManager::new(48_000.0);
        // Fill the pool so we can render every voice and confirm the
        // longer release actually applies.
        for n in 0..POLYPHONY {
            #[allow(clippy::cast_possible_truncation)]
            manager.note_on(36 + n as u8, 100);
        }
        manager.set_release_secs(3.0);
        // Render to sustain, release them all, render 100 ms of audio.
        // With a 3 s release every voice is still well above zero —
        // active_count must still be POLYPHONY.
        let params = default_sample_params();
        for _ in 0..(48_000 / 5) {
            manager.next_sample(&params);
        }
        for n in 0..POLYPHONY {
            #[allow(clippy::cast_possible_truncation)]
            manager.note_off(36 + n as u8);
        }
        for _ in 0..(48_000 / 10) {
            manager.next_sample(&params);
        }
        assert_eq!(
            manager.active_count(),
            POLYPHONY,
            "long release should keep all voices active 100 ms after note-off"
        );
    }

    #[test]
    fn sustain_defers_note_off_until_pedal_release() {
        let mut manager = VoiceManager::new(48_000.0);
        manager.note_on(60, 100);
        manager.set_sustain(true);
        manager.note_off(60);
        // Voice should still be held (not in release) while pedal is down.
        let held = (0..POLYPHONY).any(|i| manager.voices[i].held_note() == Some(60));
        assert!(held, "note should still be held with sustain down");
        // Release the pedal — deferred note-off fires.
        manager.set_sustain(false);
        let still_held = (0..POLYPHONY).any(|i| manager.voices[i].held_note() == Some(60));
        assert!(!still_held, "note should release when pedal lifts");
    }

    #[test]
    fn retrigger_while_sustained_cancels_deferral() {
        let mut manager = VoiceManager::new(48_000.0);
        manager.note_on(60, 100);
        manager.set_sustain(true);
        manager.note_off(60); // deferred
        // Re-attack the same note — should cancel the deferred release.
        manager.note_on(60, 100);
        manager.set_sustain(false);
        // After pedal release the note should still be held (re-attack
        // cancelled the deferral, so no deferred note-off fires).
        let held = (0..POLYPHONY).any(|i| manager.voices[i].held_note() == Some(60));
        assert!(held, "re-attack should cancel the deferred note-off");
    }

    #[test]
    fn sustain_with_no_deferred_notes_is_harmless() {
        let mut manager = VoiceManager::new(48_000.0);
        manager.note_on(60, 100);
        // Cycle sustain without a note-off — should not change voice state.
        manager.set_sustain(true);
        manager.set_sustain(false);
        let held = (0..POLYPHONY).any(|i| manager.voices[i].held_note() == Some(60));
        assert!(held, "voice should remain held after empty sustain cycle");
    }

    #[test]
    fn idle_voices_silent_after_release_completes() {
        let mut manager = VoiceManager::new(48_000.0);
        manager.set_release_secs(0.005);
        let params = default_sample_params();
        manager.note_on(60, 100);
        // Settle into sustain.
        for _ in 0..4_800 {
            manager.next_sample(&params);
        }
        manager.note_off(60);
        // Run well past the release.
        for _ in 0..4_800 {
            manager.next_sample(&params);
        }
        assert_eq!(manager.active_count(), 0);
        // And the next-sample output is now exactly zero.
        let (l, r) = manager.next_sample(&params);
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);
    }
}
