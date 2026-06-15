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
use crate::filter::{FilterMode, FilterRouting, FilterSlope};
use crate::lfo::LfoShape;
use crate::mod_matrix::{ModDest, ModMatrix, ModSource, ModSources};
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
/// number — no allocation. On pedal release all deferred notes fire,
/// releasing every voice that holds the note (not just the oldest) to
/// prevent stuck voices when a note is retriggered while the pedal is
/// held. A retriggered note clears its deferral, but NoteOff for the
/// new attack re-defers it before pedal release cleans up both voices.
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

    /// 8-slot modulation matrix. Evaluated per-voice once per block.
    matrix: ModMatrix,

    /// Global mod sources shared across all voices: mod wheel, aftertouch,
    /// and pitch bend. Updated from the parameter bus; consumed when building
    /// per-voice `ModSources` inside `advance_modulators`.
    global_mod_wheel: f32,
    global_aftertouch: f32,
    global_pitch_bend: f32,
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
            matrix: ModMatrix::default(),
            global_mod_wheel: 0.0,
            global_aftertouch: 0.0,
            global_pitch_bend: 0.0,
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

    /// Release all currently sounding voices immediately (used when the arp is disabled).
    pub fn all_notes_off(&mut self) {
        for note in 0u8..=127 {
            self.release_note(note);
        }
    }

    /// Full panic: release every voice and reset the gating state so no
    /// note can stay latched. Beyond [`all_notes_off`](Self::all_notes_off)
    /// this also clears every sustain-deferred release and lifts the
    /// sustain latch, so a stuck note caused by a lost note-off or a
    /// missed pedal-up is guaranteed to stop. The sustain latch resyncs
    /// on the pedal's next transition.
    pub fn panic(&mut self) {
        self.all_notes_off();
        self.deferred_note_offs = [false; 128];
        self.sustain_held = false;
    }

    /// Updates the sustain-pedal state. When `held` transitions to
    /// `false` all deferred note-offs are fired in MIDI-note order.
    pub fn set_sustain(&mut self, held: bool) {
        self.sustain_held = held;
        if !held {
            for note in 0_u8..=127 {
                if self.deferred_note_offs[note as usize] {
                    self.deferred_note_offs[note as usize] = false;
                    // Release every voice holding this note, not just the
                    // oldest. Multiple voices can hold the same MIDI note when
                    // the note is retriggered while the pedal is down (the new
                    // attack allocates a fresh voice while the old one keeps
                    // sounding). Releasing only the oldest would leave the
                    // newer voice stuck with no path to note-off.
                    for i in 0..self.voices.len() {
                        if self.voices[i].held_note() == Some(note) {
                            self.voices[i].note_off(note);
                            self.note_off_tick[i] = Some(self.next_tick);
                            self.next_tick += 1;
                        }
                    }
                }
            }
        }
    }

    /// Releases `note_midi` immediately, bypassing the sustain pedal.
    /// Used by the arpeggiator so its gate-off events are not deferred.
    pub fn release_note_immediate(&mut self, note_midi: u8) {
        self.deferred_note_offs[note_midi as usize] = false;
        self.release_note(note_midi);
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

    /// Sets the amp-envelope attack time (in seconds) on every voice.
    pub fn set_attack_secs(&mut self, attack_secs: f32) {
        for v in &mut self.voices {
            v.set_attack_secs(attack_secs);
        }
    }

    /// Sets the amp-envelope decay time (in seconds) on every voice.
    pub fn set_decay_secs(&mut self, decay_secs: f32) {
        for v in &mut self.voices {
            v.set_decay_secs(decay_secs);
        }
    }

    /// Sets the amp-envelope sustain level (0..=1) on every voice.
    pub fn set_sustain_level(&mut self, sustain_level: f32) {
        for v in &mut self.voices {
            v.set_sustain_level(sustain_level);
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

    /// Sets the filter-2 output tap on every voice.
    pub fn set_filter2_mode(&mut self, mode: FilterMode) {
        for v in &mut self.voices {
            v.set_filter2_mode(mode);
        }
    }

    /// Sets the filter routing (serial/parallel) on every voice.
    pub fn set_filter_routing(&mut self, routing: FilterRouting) {
        for v in &mut self.voices {
            v.set_filter_routing(routing);
        }
    }

    /// Sets the slope of the given filter on every voice.
    pub fn set_filter_slope(&mut self, filter_idx: u8, slope: FilterSlope) {
        for v in &mut self.voices {
            v.set_filter_slope(filter_idx, slope);
        }
    }

    /// Sets LFO1 rate (Hz) on every voice.
    pub fn set_lfo1_rate_hz(&mut self, rate_hz: f32) {
        for v in &mut self.voices {
            v.set_lfo1_rate_hz(rate_hz);
        }
    }

    /// Sets LFO1 shape on every voice.
    pub fn set_lfo1_shape(&mut self, shape: LfoShape) {
        for v in &mut self.voices {
            v.set_lfo1_shape(shape);
        }
    }

    /// Sets LFO1 phase-reset-on-note-on on every voice.
    pub fn set_lfo1_reset_on_note_on(&mut self, reset: bool) {
        for v in &mut self.voices {
            v.set_lfo1_reset_on_note_on(reset);
        }
    }

    /// Sets LFO2 rate (Hz) on every voice.
    pub fn set_lfo2_rate_hz(&mut self, rate_hz: f32) {
        for v in &mut self.voices {
            v.set_lfo2_rate_hz(rate_hz);
        }
    }

    /// Sets LFO2 shape on every voice.
    pub fn set_lfo2_shape(&mut self, shape: LfoShape) {
        for v in &mut self.voices {
            v.set_lfo2_shape(shape);
        }
    }

    /// Sets LFO2 phase-reset-on-note-on on every voice.
    pub fn set_lfo2_reset_on_note_on(&mut self, reset: bool) {
        for v in &mut self.voices {
            v.set_lfo2_reset_on_note_on(reset);
        }
    }

    /// Sets Env2 attack time (seconds) on every voice.
    pub fn set_env2_attack_secs(&mut self, secs: f32) {
        for v in &mut self.voices {
            v.set_env2_attack_secs(secs);
        }
    }

    /// Sets Env2 decay time (seconds) on every voice.
    pub fn set_env2_decay_secs(&mut self, secs: f32) {
        for v in &mut self.voices {
            v.set_env2_decay_secs(secs);
        }
    }

    /// Sets Env2 sustain level on every voice.
    pub fn set_env2_sustain_level(&mut self, level: f32) {
        for v in &mut self.voices {
            v.set_env2_sustain_level(level);
        }
    }

    /// Sets Env2 release time (seconds) on every voice.
    pub fn set_env2_release_secs(&mut self, secs: f32) {
        for v in &mut self.voices {
            v.set_env2_release_secs(secs);
        }
    }

    /// Sets Env2 Attack curve on every voice.
    pub fn set_env2_attack_curve(&mut self, curve: f32) {
        for v in &mut self.voices {
            v.set_env2_attack_curve(curve);
        }
    }

    /// Sets Env2 Decay curve on every voice.
    pub fn set_env2_decay_curve(&mut self, curve: f32) {
        for v in &mut self.voices {
            v.set_env2_decay_curve(curve);
        }
    }

    /// Sets Env2 Release curve on every voice.
    pub fn set_env2_release_curve(&mut self, curve: f32) {
        for v in &mut self.voices {
            v.set_env2_release_curve(curve);
        }
    }

    /// Sets Env3 attack time (seconds) on every voice.
    pub fn set_env3_attack_secs(&mut self, secs: f32) {
        for v in &mut self.voices {
            v.set_env3_attack_secs(secs);
        }
    }

    /// Sets Env3 decay time (seconds) on every voice.
    pub fn set_env3_decay_secs(&mut self, secs: f32) {
        for v in &mut self.voices {
            v.set_env3_decay_secs(secs);
        }
    }

    /// Sets Env3 sustain level on every voice.
    pub fn set_env3_sustain_level(&mut self, level: f32) {
        for v in &mut self.voices {
            v.set_env3_sustain_level(level);
        }
    }

    /// Sets Env3 release time (seconds) on every voice.
    pub fn set_env3_release_secs(&mut self, secs: f32) {
        for v in &mut self.voices {
            v.set_env3_release_secs(secs);
        }
    }

    /// Sets Env3 Attack curve on every voice.
    pub fn set_env3_attack_curve(&mut self, curve: f32) {
        for v in &mut self.voices {
            v.set_env3_attack_curve(curve);
        }
    }

    /// Sets Env3 Decay curve on every voice.
    pub fn set_env3_decay_curve(&mut self, curve: f32) {
        for v in &mut self.voices {
            v.set_env3_decay_curve(curve);
        }
    }

    /// Sets Env3 Release curve on every voice.
    pub fn set_env3_release_curve(&mut self, curve: f32) {
        for v in &mut self.voices {
            v.set_env3_release_curve(curve);
        }
    }

    // ── Mod matrix ───────────────────────────────────────────────────────────

    /// Enables or disables the slot at `index`.
    pub fn set_mod_slot_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(slot) = self.matrix.slots.get_mut(index) {
            slot.enabled = enabled;
        }
    }

    /// Sets the source for the slot at `index`.
    pub fn set_mod_slot_source(&mut self, index: usize, source: ModSource) {
        if let Some(slot) = self.matrix.slots.get_mut(index) {
            slot.source = source;
        }
    }

    /// Sets the destination for the slot at `index`.
    pub fn set_mod_slot_dest(&mut self, index: usize, dest: ModDest) {
        if let Some(slot) = self.matrix.slots.get_mut(index) {
            slot.dest = dest;
        }
    }

    /// Sets the amount for the slot at `index`.
    pub fn set_mod_slot_amount(&mut self, index: usize, amount: f32) {
        if let Some(slot) = self.matrix.slots.get_mut(index) {
            slot.amount = amount;
        }
    }

    /// Sets the via source for the slot at `index`. Use `ModSource::Off`
    /// to disable via scaling.
    pub fn set_mod_slot_via(&mut self, index: usize, via: ModSource) {
        if let Some(slot) = self.matrix.slots.get_mut(index) {
            slot.via = via;
        }
    }

    /// Updates the global mod wheel value (0..=1) used in per-voice
    /// `ModSources` construction.
    pub fn set_global_mod_wheel(&mut self, value: f32) {
        self.global_mod_wheel = value;
    }

    /// Updates the global channel aftertouch value (0..=1).
    pub fn set_global_aftertouch(&mut self, value: f32) {
        self.global_aftertouch = value;
    }

    /// Updates the global pitch bend value (-1..=1).
    pub fn set_global_pitch_bend(&mut self, value: f32) {
        self.global_pitch_bend = value;
    }

    // ── FM synthesis ─────────────────────────────────────────────────────────

    /// Sets the mix level on slot `slot` of every voice.
    pub fn set_slot_level(&mut self, slot: usize, level: f32) {
        for v in &mut self.voices {
            v.set_slot_level(slot, level);
        }
    }

    /// Sets the mix pan on slot `slot` of every voice.
    pub fn set_slot_pan(&mut self, slot: usize, pan: f32) {
        for v in &mut self.voices {
            v.set_slot_pan(slot, pan);
        }
    }

    /// Sets the FM algorithm on slot `slot` of every voice.
    pub fn set_fm_algorithm(&mut self, slot: usize, index: u8) {
        for v in &mut self.voices {
            v.set_fm_algorithm(slot, index);
        }
    }

    /// Sets an FM operator's integer ratio on every voice.
    pub fn set_fm_op_ratio_integer(&mut self, slot: usize, op: usize, v: u8) {
        for voice in &mut self.voices {
            voice.set_fm_op_ratio_integer(slot, op, v);
        }
    }

    /// Sets an FM operator's fine ratio in cents on every voice.
    pub fn set_fm_op_ratio_fine(&mut self, slot: usize, op: usize, v: f32) {
        for voice in &mut self.voices {
            voice.set_fm_op_ratio_fine(slot, op, v);
        }
    }

    /// Sets an FM operator's output level on every voice.
    pub fn set_fm_op_level(&mut self, slot: usize, op: usize, v: f32) {
        for voice in &mut self.voices {
            voice.set_fm_op_level(slot, op, v);
        }
    }

    /// Sets an FM operator's envelope attack time on every voice.
    pub fn set_fm_op_attack_secs(&mut self, slot: usize, op: usize, v: f32) {
        for voice in &mut self.voices {
            voice.set_fm_op_attack_secs(slot, op, v);
        }
    }

    /// Sets an FM operator's envelope decay time on every voice.
    pub fn set_fm_op_decay_secs(&mut self, slot: usize, op: usize, v: f32) {
        for voice in &mut self.voices {
            voice.set_fm_op_decay_secs(slot, op, v);
        }
    }

    /// Sets an FM operator's envelope sustain level on every voice.
    pub fn set_fm_op_sustain_level(&mut self, slot: usize, op: usize, v: f32) {
        for voice in &mut self.voices {
            voice.set_fm_op_sustain_level(slot, op, v);
        }
    }

    /// Sets an FM operator's envelope release time on every voice.
    pub fn set_fm_op_release_secs(&mut self, slot: usize, op: usize, v: f32) {
        for voice in &mut self.voices {
            voice.set_fm_op_release_secs(slot, op, v);
        }
    }

    /// Sets an FM operator's self-feedback amount on every voice.
    pub fn set_fm_op_feedback(&mut self, slot: usize, op: usize, v: f32) {
        for voice in &mut self.voices {
            voice.set_fm_op_feedback(slot, op, v);
        }
    }

    /// Advances LFO1, LFO2, and Env2 on every active voice by one block,
    /// then evaluates the mod matrix for each voice and stores the resulting
    /// [`DestOffsets`] on the voice for use in the per-sample loop.
    /// Call once per inner block, before the per-sample loop.
    pub fn advance_modulators(&mut self, block_size: usize) {
        for v in &mut self.voices {
            if v.is_idle() {
                continue;
            }
            v.advance_modulators(block_size);

            let key_tracking = v.held_note().map(|n| (f32::from(n) - 60.0) / 60.0).unwrap_or(0.0);

            let sources = ModSources {
                lfo1: v.lfo1_out(),
                lfo2: v.lfo2_out(),
                env2: v.env2_out(),
                amp_env: v.amp_env_level(),
                velocity: v.velocity_scale(),
                key_tracking,
                mod_wheel: self.global_mod_wheel,
                aftertouch: self.global_aftertouch,
                pitch_bend: self.global_pitch_bend,
                env3: v.env3_out(),
            };
            v.mod_offsets = self.matrix.compute_offsets(&sources);
        }
    }

    /// Returns the LFO1/LFO2/Env2/Env3 outputs of the first active voice,
    /// or `(0.0, 0.0, 0.0, 0.0)` if no voice is active. Used for the UI
    /// live readout in the snapshot.
    pub fn first_active_modulator_outputs(&self) -> (f32, f32, f32, f32) {
        for v in &self.voices {
            if !v.is_idle() {
                return (v.lfo1_out(), v.lfo2_out(), v.env2_out(), v.env3_out());
            }
        }
        (0.0, 0.0, 0.0, 0.0)
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
            // Apply per-voice mod matrix offsets to a local copy of the
            // base params. Volume offset is applied inside the voice
            // (on the amp envelope output); all other destinations are
            // patched here before passing to the voice.
            let mut vp = *params;
            let off = &v.mod_offsets;
            vp.filter_cutoff_hz = (vp.filter_cutoff_hz + off.filter_cutoff_hz).clamp(20.0, 20_000.0);
            vp.filter_resonance = (vp.filter_resonance + off.filter_resonance).clamp(0.0, 1.0);
            vp.filter2_cutoff_hz = (vp.filter2_cutoff_hz + off.filter2_cutoff_hz).clamp(20.0, 20_000.0);
            vp.filter2_resonance = (vp.filter2_resonance + off.filter2_resonance).clamp(0.0, 1.0);
            vp.pitch_offset_semis += off.pitch_semis;
            vp.osc_main_detune_cents[0] += off.osc1_detune_cents;
            vp.osc_main_pans[0] = (vp.osc_main_pans[0] + off.osc1_pan).clamp(-1.0, 1.0);

            let (l, r) = v.next_sample(&vp);
            sum_l += l;
            sum_r += r;
        }
        (sum_l, sum_r)
    }

    /// Returns the number of voices currently producing audio — those
    /// whose amp envelope is not yet idle. Voices whose amp is silent
    /// but whose Env2 is still releasing are not counted here; they
    /// contribute nothing to the mix but remain allocated until fully
    /// idle. The engine forwards this into the snapshot for the UI footer.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| !v.is_amp_silent()).count()
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
            filter2_cutoff_hz: 22_000.0,
            filter2_resonance: 0.0,
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
    fn sustain_pedal_release_frees_all_voices_holding_same_note() {
        // Regression: play A, defer NoteOff(A), play B, play A again.
        // NoteOn(A) clears deferred[A] and allocates a second voice for A.
        // NoteOff(A) re-defers, targeting the oldest voice. When the pedal
        // releases, both voices holding A must exit — not just the oldest.
        let mut manager = VoiceManager::new(48_000.0);
        manager.set_sustain(true);

        manager.note_on(60, 100); // voice 0 plays A
        manager.note_off(60); // deferred
        manager.note_on(62, 100); // voice 1 plays B
        manager.note_off(62); // deferred
        manager.note_on(60, 100); // NoteOn(A) clears deferred[A]; voice 2 plays A
        manager.note_off(60); // deferred, targets oldest voice (voice 0)

        manager.set_sustain(false);

        // Every voice should be in release (held_note cleared) — no stuck voice.
        let any_held = (0..POLYPHONY).any(|i| manager.voices[i].held_note().is_some());
        assert!(
            !any_held,
            "all voices should release when pedal lifts; a voice is stuck"
        );
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
