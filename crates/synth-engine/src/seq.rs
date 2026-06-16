//! Step sequencer engine.
//!
//! A sibling of [`crate::arp::ArpEngine`]: it runs entirely on the audio
//! thread (no allocation, no locking), owns a clock and a fixed 16-step
//! pattern, and on each [`SeqEngine::process`] call writes any pending
//! [`ArpEvent`]s into a caller-supplied buffer. Where the arp walks the
//! *held* note set, the sequencer walks a fixed pattern of note offsets
//! transposed by the lowest currently-held note (the "root").
//!
//! The two engines reuse the same [`ArpEvent`] / [`ArpEvents`] output shape
//! and the same [`ArpRate`] note-value enum; the engine treats them
//! interchangeably and only ever clocks one at a time (they are mutually
//! exclusive — see `engine.rs`).

use crate::arp::{ArpEvent, ArpEvents, ArpRate};

/// Maximum sequencer steps.
pub const SEQ_MAX_STEPS: usize = 16;

/// Maximum simultaneous held notes the sequencer tracks (for the root note).
const MAX_HELD: usize = 32;

/// Per-step data.
///
/// `rest` mutes the step (no NoteOn fires, but the step still consumes time
/// and still advances the mod lane); `tie` extends the previously sounding
/// note across this step instead of retriggering; `note_offset` is semitones
/// from the held root; `velocity` is the step's MIDI velocity; `gate` is the
/// fraction of the step the note sounds; `mod_value` is the mod-lane CV
/// (-1..=1) exposed as the `Seq` mod source.
#[derive(Debug, Clone, Copy)]
pub struct SeqStep {
    /// Semitone offset from the held root, -24..=24.
    pub note_offset: i8,
    /// Step velocity, 0..=127.
    pub velocity: u8,
    /// Gate fraction of step duration, 0.0..=1.0.
    pub gate: f32,
    /// When true the step is silent.
    pub rest: bool,
    /// When true the step's note extends forward into the following step(s)
    /// instead of releasing — the step itself still plays its own note, but the
    /// *next* step does not retrigger (its note is consumed by the held one).
    /// Ties chain: a run of tie steps lengthens the note's slot to span the
    /// whole run plus the first step after it. The **originating** step's own
    /// `gate` then governs the note, scaled across that longer span: gate 1.0
    /// rings the full tied duration (legato), gate 0.5 sounds the first half and
    /// is silent for the rest. The consumed steps' note/velocity/gate are unused
    /// (their mod lane still advances).
    pub tie: bool,
    /// Mod-lane CV value, -1.0..=1.0.
    pub mod_value: f32,
}

impl Default for SeqStep {
    fn default() -> Self {
        Self {
            note_offset: 0,
            velocity: 100,
            gate: 0.5,
            rest: false,
            tie: false,
            mod_value: 0.0,
        }
    }
}

/// Playback order across the active step range `0..length`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeqMode {
    Forward,
    Reverse,
    PingPong,
    Random,
}

impl SeqMode {
    /// Decode the discrete UI/param index (0=Forward … 3=Random).
    pub fn from_f32(v: f32) -> Self {
        match v as u8 {
            0 => Self::Forward,
            1 => Self::Reverse,
            2 => Self::PingPong,
            _ => Self::Random,
        }
    }
}

// ── SeqEngine ────────────────────────────────────────────────────────────────

/// Step-sequencer clock and note scheduler.
pub struct SeqEngine {
    // ── Config ────────────────────────────────────────────────────────────
    pub enabled: bool,
    /// Active step count, 1..=`SEQ_MAX_STEPS`.
    pub length: usize,
    pub mode: SeqMode,
    /// Step rate (note value per step) — shares the arp's enum.
    pub rate: ArpRate,
    /// Transport BPM, set from the unified Master-tab tempo.
    pub bpm: f32,
    /// Swing fraction (0.5 = straight, 0.75 = maximum).
    pub swing: f32,
    pub steps: [SeqStep; SEQ_MAX_STEPS],

    // ── Held notes ────────────────────────────────────────────────────────
    /// MIDI note numbers, sorted ascending. `held[0]` is the root.
    held: [u8; MAX_HELD],
    held_count: usize,

    // ── Runtime state ─────────────────────────────────────────────────────
    /// Cursor into `0..length`. `usize::MAX` is the "not started" sentinel.
    step_index: usize,
    /// Phase within the current step, 0.0–1.0.
    phase: f32,
    /// Whether the gate is currently open (NoteOn sent, NoteOff not yet).
    gate_open: bool,
    /// Currently sounding MIDI note (for sending the NoteOff).
    pub current_note: u8,
    /// Velocity of the currently sounding step (used when the engine fires
    /// the very first note directly on key-down).
    pub current_velocity: u8,
    /// Release point of the currently sounding note, in step-units measured from
    /// its articulation: `gate * tie_span`. For an untied note this is just the
    /// step's gate (0.0–1.0); a tie lengthens the slot so it can exceed 1.0.
    release_at: f32,
    /// Step-units elapsed since the current note was articulated. Compared
    /// against `release_at` to fire the NoteOff. Accumulates across tied steps.
    note_elapsed: f32,
    /// Mod-lane value of the current step, held across the step.
    current_mod: f32,
    /// Direction flag for PingPong (true = ascending).
    going_up: bool,
    /// Whether this is an even step in the pair (for swing).
    even_step: bool,

    sample_rate_hz: f32,
    /// xorshift32 state for Random mode — no std::rand on audio thread.
    rng: u32,
}

impl SeqEngine {
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            enabled: false,
            length: SEQ_MAX_STEPS,
            mode: SeqMode::Forward,
            rate: ArpRate::R16,
            bpm: 120.0,
            swing: 0.5,
            steps: [SeqStep::default(); SEQ_MAX_STEPS],
            held: [0; MAX_HELD],
            held_count: 0,
            step_index: usize::MAX,
            phase: 0.0,
            gate_open: false,
            current_note: 0,
            current_velocity: 100,
            release_at: 0.5,
            note_elapsed: 0.0,
            current_mod: 0.0,
            going_up: true,
            even_step: true,
            sample_rate_hz,
            rng: 0x1234_5678,
        }
    }

    /// Current mod-lane value (the active step's `mod_value`), or 0.0 when the
    /// sequencer is not running. Read by the engine and published as the
    /// `Seq` mod source.
    #[must_use]
    pub fn mod_value(&self) -> f32 {
        if self.enabled && self.held_count > 0 {
            self.current_mod
        } else {
            0.0
        }
    }

    /// Index of the step currently under the cursor, or `None` when idle.
    /// Drives the UI playhead.
    #[must_use]
    pub fn current_step(&self) -> Option<usize> {
        if self.enabled && self.held_count > 0 && self.step_index != usize::MAX {
            Some(self.step_index)
        } else {
            None
        }
    }

    /// Active number of steps, clamped to the valid range.
    fn active_len(&self) -> usize {
        self.length.clamp(1, SEQ_MAX_STEPS)
    }

    // ── Note list ──────────────────────────────────────────────────────────

    /// Record a held note (sorted ascending so `held[0]` is the root).
    ///
    /// Returns `true` if the caller should fire a `NoteOn` immediately: this
    /// is the first note into a previously-empty enabled sequencer *and* the
    /// canonical first step is not a rest. In that case the engine fires
    /// `current_note` / `current_velocity` directly so there is no extra-block
    /// delay, and `process()` then handles gate-off and later steps normally.
    pub fn note_on(&mut self, midi_note: u8) -> bool {
        if self.held_count >= MAX_HELD {
            return false;
        }
        let was_empty = self.held_count == 0;
        let pos = self.held[..self.held_count].partition_point(|&n| n < midi_note);
        if pos < self.held_count {
            self.held.copy_within(pos..self.held_count, pos + 1);
        }
        self.held[pos] = midi_note;
        self.held_count += 1;

        if was_empty && self.enabled {
            // Jump to the canonical first step, prime gate/phase so the caller
            // can fire NoteOn immediately and process() takes over cleanly.
            self.phase = 0.0;
            self.even_step = true;
            self.advance_step();
            let step = self.steps[self.step_index];
            // A rest is silent. A tie step still plays its own note (the tie
            // only extends it forward at the next boundary).
            if step.rest {
                self.gate_open = false;
                return false;
            }
            self.current_note = self.note_at(self.step_index);
            self.current_velocity = step.velocity;
            self.release_at = step.gate * self.tie_span(self.step_index) as f32;
            self.note_elapsed = 0.0;
            self.gate_open = true;
            return true;
        }
        false
    }

    /// Remove a released note.
    pub fn note_off(&mut self, midi_note: u8) {
        if let Some(pos) = self.held[..self.held_count].iter().position(|&n| n == midi_note) {
            self.held.copy_within(pos + 1..self.held_count, pos);
            self.held_count -= 1;
        }
    }

    /// If the held set is now empty but a note is still sounding from the
    /// key-down immediate-fire, close the gate and return that note so the
    /// caller can release its voice now rather than at the next `process()`.
    ///
    /// Without this, a fast release→re-press cycle (e.g. dragging across the
    /// on-screen keyboard) refills the held set before `process()` runs its
    /// empty-state cleanup, so the previously fired voice never receives a
    /// NoteOff and stays stuck. Returns `None` when notes are still held or no
    /// gate is open.
    #[must_use]
    pub fn take_idle_note_off(&mut self) -> Option<u8> {
        if self.held_count == 0 && self.gate_open {
            self.gate_open = false;
            Some(self.current_note)
        } else {
            None
        }
    }

    /// Clear all held notes and close the gate. Used by the engine's
    /// panic / all-notes-off path.
    pub fn clear(&mut self) {
        self.held_count = 0;
        self.gate_open = false;
        self.step_index = usize::MAX;
        self.phase = 0.0;
        self.current_mod = 0.0;
    }

    /// Reset the clock so the next `process()` fires the first step with a
    /// canonical fresh-start position. Called when the sequencer is toggled on.
    pub fn reset_clock(&mut self) {
        self.phase = 1.0;
        self.step_index = usize::MAX;
        self.gate_open = false;
        self.even_step = true;
        self.going_up = true;
    }

    // ── Audio-thread process ───────────────────────────────────────────────

    /// Advance the sequencer clock by `n_frames` samples. Returns events to inject.
    pub fn process(&mut self, n_frames: usize) -> ArpEvents {
        let mut out = ArpEvents::new();

        if !self.enabled || self.held_count == 0 {
            if self.gate_open {
                out.push(ArpEvent::NoteOff {
                    note: self.current_note,
                });
                self.gate_open = false;
            }
            self.phase = 0.0;
            self.step_index = usize::MAX;
            self.current_mod = 0.0;
            return out;
        }

        let step_samples = self.step_samples();
        let phase_advance = n_frames as f32 / step_samples;

        self.phase += phase_advance;
        self.note_elapsed += phase_advance;

        // Release the current note once it has sounded for its gated length.
        // `release_at` is `gate * tie_span` in step-units, so a tie raises it
        // above 1.0 and the note simply rings on across the consumed step(s)
        // until this threshold is reached.
        if self.gate_open && self.note_elapsed >= self.release_at {
            out.push(ArpEvent::NoteOff {
                note: self.current_note,
            });
            self.gate_open = false;
        }

        // Step boundary crossed?
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.even_step = !self.even_step;

            // Did the step we are leaving tie its note forward into this one?
            let extend = self.step_index != usize::MAX && self.steps[self.step_index].tie;

            self.advance_step();
            let step = self.steps[self.step_index];

            if extend {
                // Consumed step: the held note's slot continues across it — no
                // retrigger, no new gate. Its note/velocity/gate are unused; the
                // mod lane (refreshed in advance_step) still applies. The note
                // may already have released early within the slot for a short
                // gate — that is fine, it stays silent for the rest of the slot.
            } else {
                // Silence any still-open gate from the previous step.
                if self.gate_open {
                    out.push(ArpEvent::NoteOff {
                        note: self.current_note,
                    });
                    self.gate_open = false;
                }

                // A rest stays silent; otherwise articulate this step's note. A
                // tie step still plays its own note here — the tie lengthens the
                // slot it occupies, via `release_at` below.
                if !step.rest {
                    let note = self.note_at(self.step_index);
                    self.current_note = note;
                    self.current_velocity = step.velocity;
                    self.release_at = step.gate * self.tie_span(self.step_index) as f32;
                    self.note_elapsed = self.phase;
                    out.push(ArpEvent::NoteOn {
                        note,
                        velocity: step.velocity,
                    });
                    self.gate_open = true;

                    // Very short gate that closes before the next block: fire
                    // off now.
                    if self.note_elapsed >= self.release_at {
                        out.push(ArpEvent::NoteOff { note });
                        self.gate_open = false;
                    }
                }
            }
        }

        out
    }

    // ── Helpers ────────────────────────────────────────────────────────────

    /// Step duration in samples, with swing applied (mirrors the arp).
    fn step_samples(&self) -> f32 {
        let base = (60.0 / self.bpm) * self.rate.beats_per_step() * self.sample_rate_hz;
        let pair_total = 2.0 * base;
        if self.even_step {
            pair_total * self.swing
        } else {
            pair_total * (1.0 - self.swing)
        }
    }

    /// Advance the cursor across `0..length` per the playback mode, then
    /// update the held mod-lane value for the new step.
    fn advance_step(&mut self) {
        let len = self.active_len();
        let fresh = self.step_index == usize::MAX;

        match self.mode {
            SeqMode::Forward => {
                self.step_index = if fresh { 0 } else { (self.step_index + 1) % len };
            }
            SeqMode::Reverse => {
                self.step_index = if fresh {
                    len - 1
                } else {
                    self.step_index.checked_sub(1).unwrap_or(len - 1)
                };
            }
            SeqMode::PingPong => {
                if len <= 1 {
                    self.step_index = 0;
                } else if fresh {
                    self.step_index = 0;
                    self.going_up = true;
                } else if self.going_up {
                    if self.step_index + 1 >= len {
                        self.going_up = false;
                        self.step_index = len - 2;
                    } else {
                        self.step_index += 1;
                    }
                } else if self.step_index == 0 {
                    self.going_up = true;
                    self.step_index = 1.min(len - 1);
                } else {
                    self.step_index -= 1;
                }
            }
            SeqMode::Random => {
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 17;
                self.rng ^= self.rng << 5;
                self.step_index = (self.rng as usize) % len;
            }
        }

        // The cursor may land outside a freshly-shortened length; clamp.
        if self.step_index >= len {
            self.step_index = len - 1;
        }
        self.current_mod = self.steps[self.step_index].mod_value;
    }

    /// Number of steps a note articulated at `start` occupies, following the
    /// tie run forward in index order: 1 for an untied step, +1 for each
    /// consecutive tie step in the run (the run ends at, and includes, the first
    /// non-tie step). The originating step's gate is scaled by this so the note
    /// fills its lengthened slot. Bounded by the active length to stay finite
    /// when every step is tied.
    fn tie_span(&self, start: usize) -> usize {
        let len = self.active_len();
        let mut span = 1;
        let mut idx = start;
        while self.steps[idx].tie && span < len {
            idx = (idx + 1) % len;
            span += 1;
        }
        span
    }

    /// MIDI note for a step: the held root transposed by the step's offset,
    /// clamped to the valid MIDI range.
    fn note_at(&self, idx: usize) -> u8 {
        let root = self.held[0] as i16;
        (root + self.steps[idx].note_offset as i16).clamp(0, 127) as u8
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a sequencer with ascending offsets 0,1,2,… so each step's pitch
    /// reveals its index, *without* pressing a key yet. Use this when a test
    /// needs to configure steps (e.g. ties) before the first articulation, since
    /// `release_at` is snapshotted at the moment a note is articulated.
    fn make_seq_idle(mode: SeqMode, length: usize) -> SeqEngine {
        let mut s = SeqEngine::new(48_000.0);
        s.enabled = true;
        s.mode = mode;
        s.length = length;
        s.bpm = 120.0;
        s.rate = ArpRate::R8;
        s.swing = 0.5;
        for (i, step) in s.steps.iter_mut().enumerate() {
            step.note_offset = i as i8;
            step.velocity = 100;
            step.gate = 0.5;
        }
        s
    }

    /// `make_seq_idle` plus an immediate key-down on `root`.
    fn make_seq(mode: SeqMode, length: usize, root: u8) -> SeqEngine {
        let mut s = make_seq_idle(mode, length);
        s.note_on(root);
        s
    }

    /// Collect NoteOn pitches over `steps` boundaries. The first note was
    /// already dispatched on key-down (read from `current_note`).
    fn collect_notes(seq: &mut SeqEngine, steps: usize) -> Vec<u8> {
        let step_samples = 12_000usize; // 120 BPM, 1/8 note
        let mut notes = vec![seq.current_note];
        for _ in 1..steps {
            let evs = seq.process(step_samples);
            for ev in evs.iter() {
                if let ArpEvent::NoteOn { note, .. } = *ev {
                    notes.push(note);
                }
            }
        }
        notes
    }

    #[test]
    fn forward_walks_and_wraps() {
        let mut s = make_seq(SeqMode::Forward, 4, 60);
        let notes = collect_notes(&mut s, 6);
        assert_eq!(notes, vec![60, 61, 62, 63, 60, 61]);
    }

    #[test]
    fn reverse_walks_and_wraps() {
        let mut s = make_seq(SeqMode::Reverse, 4, 60);
        // Reverse starts at the last step (offset 3 = 63) and walks down,
        // wrapping back to the top.
        let notes = collect_notes(&mut s, 6);
        assert_eq!(notes, vec![63, 62, 61, 60, 63, 62]);
    }

    #[test]
    fn pingpong_does_not_repeat_endpoints() {
        let mut s = make_seq(SeqMode::PingPong, 4, 60);
        let notes = collect_notes(&mut s, 8);
        for pair in notes.windows(2) {
            assert_ne!(pair[0], pair[1], "consecutive notes should differ: {:?}", notes);
        }
    }

    #[test]
    fn rest_step_emits_no_note_on() {
        let mut s = make_seq(SeqMode::Forward, 4, 60);
        s.steps[1].rest = true;
        // Step 0 fired on key-down. Step into index 1 (rest) — expect no NoteOn.
        let evs = s.process(12_000);
        assert!(
            !evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { .. })),
            "rest step must not fire a NoteOn"
        );
        // Step into index 2 — a NoteOn for 62 should fire.
        let evs = s.process(12_000);
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { note: 62, .. })),
            "step after the rest should sound"
        );
    }

    #[test]
    fn tie_extends_note_forward_over_next_step() {
        let mut s = make_seq_idle(SeqMode::Forward, 2);
        // Step 0 ties its note (60) forward, consuming step 1. Full gate so the
        // note rings the whole of step 0.
        s.steps[0].tie = true;
        s.steps[0].gate = 1.0;
        s.note_on(60);
        // Step 0 (note 60) was dispatched on key-down. Extending into step 1
        // must not retrigger or release — the note rings on.
        let evs = s.process(12_000);
        assert_eq!(evs.iter().count(), 0, "tied note must ring on with no events");
        // Back to step 0: the held note ends and re-articulates.
        let evs = s.process(12_000);
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOff { note: 60 })),
            "held note should release when the tie chain ends"
        );
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { note: 60, .. })),
            "step 0 should re-articulate after the tie"
        );
    }

    #[test]
    fn tie_step_plays_its_note_then_consumes_the_next() {
        let mut s = make_seq(SeqMode::Forward, 3, 60); // offsets 0,1,2 -> 60,61,62
        s.steps[1].tie = true;
        // Into step 1: it still plays its own note (61) before extending.
        let evs = s.process(12_000);
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { note: 61, .. })),
            "a tie step still articulates its own note"
        );
        // Into step 2: step 1's tie holds 61, so step 2's note is consumed.
        let evs = s.process(12_000);
        assert!(
            !evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { .. })),
            "the step after a tie is consumed by the held note"
        );
    }

    #[test]
    fn tie_scales_originating_gate_over_the_longer_span() {
        // Step 0 ties forward (2-step slot). Its own gate of 0.5 should make the
        // note sound for half of the *two-step* span — i.e. one full step — then
        // release. Step 1's gate is irrelevant (its note is consumed).
        let mut s = make_seq_idle(SeqMode::Forward, 2);
        s.steps[0].tie = true;
        s.steps[0].gate = 0.5;
        s.steps[1].gate = 1.0; // must have no effect
        s.note_on(60);
        // First half-step: note still ringing.
        let evs = s.process(6_000);
        assert_eq!(evs.iter().count(), 0, "note should still ring at quarter span");
        // Second half-step completes step 0 -> note has sounded 1 of 2 steps.
        let evs = s.process(6_000);
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOff { note: 60 })),
            "tied note should release at gate*span (one full step in)"
        );
        // The consumed step must not retrigger.
        assert!(
            !evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { .. })),
            "consumed step must not articulate"
        );
    }

    #[test]
    fn tie_with_full_gate_rings_the_whole_span() {
        // gate 1.0 + tie => legato across the full two-step slot, releasing only
        // when the slot ends (and re-articulating step 0).
        let mut s = make_seq_idle(SeqMode::Forward, 2);
        s.steps[0].tie = true;
        s.steps[0].gate = 1.0;
        s.note_on(60);
        // Across the whole of step 0 and the consumed step 1: no release.
        for _ in 0..3 {
            let evs = s.process(6_000);
            assert!(
                !evs.iter().any(|e| matches!(e, ArpEvent::NoteOff { .. })),
                "full-gate tie must not release before the slot ends"
            );
        }
        // Final half-step of the slot: release and re-articulate.
        let evs = s.process(6_000);
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOff { note: 60 })),
            "tied note should release at the end of its slot"
        );
    }

    #[test]
    fn per_step_velocity_is_used() {
        let mut s = make_seq(SeqMode::Forward, 2, 60);
        s.steps[1].velocity = 42;
        let evs = s.process(12_000);
        let on = evs.iter().find_map(|e| match e {
            ArpEvent::NoteOn { velocity, .. } => Some(*velocity),
            _ => None,
        });
        assert_eq!(on, Some(42));
    }

    #[test]
    fn gate_off_fires_within_step() {
        let mut s = make_seq(SeqMode::Forward, 1, 60);
        s.steps[0].gate = 0.5;
        // Re-prime the single-step sequencer for a clean run.
        let evs = s.process(6_000); // half a step at gate 0.5 -> NoteOff
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOff { .. })),
            "expected NoteOff at the gate boundary"
        );
    }

    #[test]
    fn offset_transposes_to_root() {
        // Root 72 instead of 60 shifts every pitch up an octave.
        let mut s = make_seq(SeqMode::Forward, 4, 72);
        let notes = collect_notes(&mut s, 4);
        assert_eq!(notes, vec![72, 73, 74, 75]);
    }

    #[test]
    fn lowest_held_note_is_the_root() {
        let mut s = make_seq(SeqMode::Forward, 2, 64);
        // Press a lower note: root drops to 60, so subsequent steps transpose.
        s.note_on(60);
        let evs = s.process(12_000);
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { note: 61, .. })),
            "root should follow the lowest held note (60 + offset 1 = 61)"
        );
    }

    #[test]
    fn no_events_when_no_notes_held() {
        let mut s = SeqEngine::new(48_000.0);
        s.enabled = true;
        let evs = s.process(12_000);
        assert_eq!(evs.iter().count(), 0);
    }

    #[test]
    fn mod_value_tracks_current_step() {
        let mut s = make_seq(SeqMode::Forward, 4, 60);
        s.steps[0].mod_value = -0.5;
        s.steps[1].mod_value = 0.75;
        // Re-arm so step 0 is the current step with its mod value.
        s.clear();
        s.note_on(60);
        assert!((s.mod_value() - (-0.5)).abs() < 1e-6);
        s.process(12_000); // advance to step 1
        assert!((s.mod_value() - 0.75).abs() < 1e-6);
    }
}
