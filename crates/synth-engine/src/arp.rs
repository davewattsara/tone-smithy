//! Arpeggiator engine.
//!
//! Runs entirely on the audio thread — no allocation, no locking. Owns a
//! held-note list and a phase accumulator. On each [`ArpEngine::process`]
//! call it advances the clock and writes any pending [`ArpEvent`]s into the
//! caller-supplied output buffer; the engine turns those into [`crate::events::EngineEvent`]
//! NoteOn / NoteOff calls before the voice loop.

/// Maximum simultaneous held notes the arp tracks.
const MAX_HELD: usize = 32;

/// Output event produced by the arpeggiator for one audio block.
#[derive(Debug, Clone, Copy)]
pub enum ArpEvent {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
}

/// Fixed-size event list returned by [`ArpEngine::process`].
///
/// Up to 4 events can fire in one block (gate-off + step-advance + new NoteOn
/// is the worst case). Caller iterates with `iter()`.
pub struct ArpEvents {
    buf: [ArpEvent; 4],
    count: usize,
}

impl ArpEvents {
    pub(crate) fn new() -> Self {
        Self {
            buf: [ArpEvent::NoteOff { note: 0 }; 4],
            count: 0,
        }
    }

    pub(crate) fn push(&mut self, ev: ArpEvent) {
        if self.count < self.buf.len() {
            self.buf[self.count] = ev;
            self.count += 1;
        }
    }

    /// Iterate over events produced this block.
    pub fn iter(&self) -> impl Iterator<Item = &ArpEvent> {
        self.buf[..self.count].iter()
    }
}

// ── Mode / Rate enums ──────────────────────────────────────────────────────

/// Arpeggiator play mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArpMode {
    Up,
    Down,
    UpDown,
    Random,
    Played,
}

impl ArpMode {
    pub fn from_f32(v: f32) -> Self {
        match v as u8 {
            0 => Self::Up,
            1 => Self::Down,
            2 => Self::UpDown,
            3 => Self::Random,
            _ => Self::Played,
        }
    }
}

/// Arpeggiator step rate (note value per step).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArpRate {
    /// 1/32 note — 0.125 beats per step
    R32,
    /// 1/16 note — 0.25 beats per step
    R16,
    /// 1/8 note — 0.5 beats per step
    R8,
    /// 1/4 note — 1.0 beats per step
    R4,
    /// 1/2 note — 2.0 beats per step
    R2,
}

impl ArpRate {
    pub fn beats_per_step(self) -> f32 {
        match self {
            Self::R32 => 0.125,
            Self::R16 => 0.25,
            Self::R8 => 0.5,
            Self::R4 => 1.0,
            Self::R2 => 2.0,
        }
    }

    pub fn from_f32(v: f32) -> Self {
        match v as u8 {
            0 => Self::R32,
            1 => Self::R16,
            2 => Self::R8,
            3 => Self::R4,
            _ => Self::R2,
        }
    }
}

// ── ArpEngine ──────────────────────────────────────────────────────────────

/// Arpeggiator clock and note scheduler.
pub struct ArpEngine {
    // ── Config ────────────────────────────────────────────────────────────
    pub enabled: bool,
    pub mode: ArpMode,
    /// Octave expansion range, 1–4.
    pub octaves: u8,
    pub rate: ArpRate,
    pub bpm: f32,
    /// Gate fraction of step duration (0.01–1.0).
    pub gate: f32,
    /// Swing fraction (0.5 = straight, 0.75 = maximum).
    pub swing: f32,
    pub velocity: u8,

    // ── Held notes ────────────────────────────────────────────────────────
    /// MIDI note numbers in Up/Down/UpDown order (sorted) or insertion order (Played).
    held: [u8; MAX_HELD],
    held_count: usize,

    // ── Runtime state ─────────────────────────────────────────────────────
    /// Index into the expanded sequence (notes × octaves).
    /// `usize::MAX` is the "not started" sentinel; `advance_step` clears it
    /// on the first call so Up starts at 0 and Down starts at seq_len-1.
    step_index: usize,
    /// Phase within the current step, 0.0–1.0.
    phase: f32,
    /// Whether the gate is currently open (NoteOn sent, NoteOff not yet).
    gate_open: bool,
    /// Currently sounding MIDI note (for sending the NoteOff).
    pub current_note: u8,
    /// Direction flag for UpDown mode (true = going up).
    going_up: bool,
    /// Whether this is an even step in the pair (for swing).
    even_step: bool,

    sample_rate_hz: f32,
    /// xorshift32 state for Random mode — no std::rand on audio thread.
    rng: u32,
}

impl ArpEngine {
    pub fn new(sample_rate_hz: f32) -> Self {
        Self {
            enabled: false,
            mode: ArpMode::Up,
            octaves: 1,
            rate: ArpRate::R8,
            bpm: 120.0,
            gate: 0.5,
            swing: 0.5,
            velocity: 100,
            held: [0; MAX_HELD],
            held_count: 0,
            step_index: usize::MAX,
            phase: 0.0,
            gate_open: false,
            current_note: 0,
            going_up: true,
            even_step: true,
            sample_rate_hz,
            rng: 0xDEAD_BEEF,
        }
    }

    // ── Note list ──────────────────────────────────────────────────────────

    /// Record a held note. Maintains sorted order for Up/Down/UpDown; appends for Played.
    ///
    /// Returns `true` if this is the first note into a previously-empty arp.
    /// When `true` the caller should fire a `NoteOn` immediately (before the
    /// next `process()` call) so there is no extra-block delay on the first
    /// step. The arp sets `gate_open = true` and `phase = 0.0` so subsequent
    /// `process()` calls handle gate-off and the next step boundary normally.
    pub fn note_on(&mut self, midi_note: u8) -> bool {
        if self.held_count >= MAX_HELD {
            return false;
        }
        let was_empty = self.held_count == 0;
        if self.mode == ArpMode::Played {
            self.held[self.held_count] = midi_note;
            self.held_count += 1;
        } else {
            // Sorted insert (ascending)
            let pos = self.held[..self.held_count].partition_point(|&n| n < midi_note);
            // Shift right to make room
            if pos < self.held_count {
                self.held.copy_within(pos..self.held_count, pos + 1);
            }
            self.held[pos] = midi_note;
            self.held_count += 1;
        }
        if was_empty && self.enabled {
            // Advance to the canonical first step so process() knows which
            // note is sounding, then open the gate at phase=0 so the caller
            // can fire NoteOn immediately and process() handles gate-off and
            // subsequent step boundaries without any special-casing.
            self.advance_step();
            self.gate_open = true;
            self.phase = 0.0;
            return true;
        }
        false
    }

    /// Remove a released note.
    pub fn note_off(&mut self, midi_note: u8) {
        if let Some(pos) = self.held[..self.held_count].iter().position(|&n| n == midi_note) {
            self.held.copy_within(pos + 1..self.held_count, pos);
            self.held_count -= 1;
            // Keep step_index in range, but only when it holds a real index —
            // usize::MAX is the "not started" sentinel and must not be touched.
            if self.held_count > 0 && self.step_index != usize::MAX {
                let seq_len = self.held_count * self.octaves as usize;
                self.step_index %= seq_len;
            }
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
    /// panic / all-notes-off path so the arp stops scheduling notes
    /// immediately rather than continuing to clock a stale held set.
    pub fn clear(&mut self) {
        self.held_count = 0;
        self.gate_open = false;
        self.step_index = usize::MAX;
        self.phase = 0.0;
    }

    /// Reset the arp clock so the next `process()` call fires the first step
    /// immediately with a canonical fresh-start position.
    ///
    /// Called by the engine whenever the arp is toggled on, to clear any stale
    /// phase or gate state from when it was disabled.
    pub fn reset_clock(&mut self) {
        self.phase = 1.0;
        self.step_index = usize::MAX;
        self.gate_open = false;
        self.even_step = true;
    }

    // ── Audio-thread process ───────────────────────────────────────────────

    /// Advance the arp clock by `n_frames` samples. Returns events to inject.
    pub fn process(&mut self, n_frames: usize) -> ArpEvents {
        let mut out = ArpEvents::new();

        if !self.enabled || self.held_count == 0 {
            // If arp is off or no notes held but a note is still sounding, kill it
            if self.gate_open {
                out.push(ArpEvent::NoteOff {
                    note: self.current_note,
                });
                self.gate_open = false;
            }
            // Reset clock so the next note_on starts a fresh sequence
            self.phase = 0.0;
            self.step_index = usize::MAX;
            return out;
        }

        let step_samples = self.step_samples();
        let frames_f = n_frames as f32;
        let phase_advance = frames_f / step_samples;

        let prev_phase = self.phase;
        self.phase += phase_advance;

        // Gate-off threshold crossed?
        if self.gate_open && prev_phase < self.gate && self.phase >= self.gate {
            out.push(ArpEvent::NoteOff {
                note: self.current_note,
            });
            self.gate_open = false;
        }

        // Step boundary crossed?
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.even_step = !self.even_step;

            // Silence any still-open gate
            if self.gate_open {
                out.push(ArpEvent::NoteOff {
                    note: self.current_note,
                });
                self.gate_open = false;
            }

            // Advance step
            self.advance_step();

            // Fire NoteOn
            let note = self.current_note;
            out.push(ArpEvent::NoteOn {
                note,
                velocity: self.velocity,
            });
            self.gate_open = true;

            // If gate < phase-advance (very short gate), close it immediately
            if self.gate <= self.phase {
                out.push(ArpEvent::NoteOff { note });
                self.gate_open = false;
            }
        }

        out
    }

    // ── Helpers ────────────────────────────────────────────────────────────

    /// Step duration in samples, with swing applied.
    fn step_samples(&self) -> f32 {
        let base = (60.0 / self.bpm) * self.rate.beats_per_step() * self.sample_rate_hz;
        // Swing pairs steps: even step gets `swing` fraction, odd step gets `1-swing`.
        // At swing=0.5 both are 1.0× — straight time.
        // Multiply pair total (2×base) by the fraction for this step.
        let pair_total = 2.0 * base;
        if self.even_step {
            pair_total * self.swing
        } else {
            pair_total * (1.0 - self.swing)
        }
    }

    /// Advance `step_index` and update `current_note` and `going_up` for the new step.
    fn advance_step(&mut self) {
        let seq_len = self.held_count * self.octaves as usize;

        // usize::MAX is the "not started" sentinel — jump to the canonical
        // start position for the mode rather than incrementing from it.
        let fresh = self.step_index == usize::MAX;

        match self.mode {
            ArpMode::Up | ArpMode::Played => {
                self.step_index = if fresh { 0 } else { (self.step_index + 1) % seq_len };
            }
            ArpMode::Down => {
                self.step_index = if fresh {
                    seq_len - 1
                } else {
                    self.step_index.checked_sub(1).unwrap_or(seq_len - 1)
                };
            }
            ArpMode::UpDown => {
                if seq_len <= 1 {
                    self.step_index = 0;
                } else if fresh {
                    self.step_index = 0;
                    self.going_up = true;
                } else if self.going_up {
                    if self.step_index + 1 >= seq_len {
                        // Reached the top — turn around, skip the top note
                        self.going_up = false;
                        self.step_index = seq_len.saturating_sub(2);
                    } else {
                        self.step_index += 1;
                    }
                } else if self.step_index == 0 {
                    // Reached the bottom — turn around, skip the bottom note
                    self.going_up = true;
                    self.step_index = 1.min(seq_len - 1);
                } else {
                    self.step_index -= 1;
                }
            }
            ArpMode::Random => {
                // xorshift32 — no std::rand on audio thread
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 17;
                self.rng ^= self.rng << 5;
                self.step_index = (self.rng as usize) % seq_len;
            }
        }

        self.current_note = self.note_at(self.step_index);
    }

    /// Map a sequence index to a MIDI note number (note × octave expansion).
    fn note_at(&self, idx: usize) -> u8 {
        let note_idx = idx % self.held_count;
        let oct = (idx / self.held_count) as u8;
        self.held[note_idx].saturating_add(oct * 12)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_arp(mode: ArpMode, octaves: u8, notes: &[u8]) -> ArpEngine {
        let mut a = ArpEngine::new(48_000.0);
        a.enabled = true;
        a.mode = mode;
        a.octaves = octaves;
        a.bpm = 120.0;
        a.rate = ArpRate::R8;
        a.gate = 0.5;
        a.swing = 0.5;
        for &n in notes {
            a.note_on(n);
        }
        a
    }

    /// Collect the NoteOn pitches fired over `steps` step-boundaries.
    ///
    /// The first note is taken from `arp.current_note` directly — it was
    /// already dispatched by the engine in the same block as the key press.
    /// Subsequent notes come from `process()` calls.
    fn collect_notes(arp: &mut ArpEngine, steps: usize) -> Vec<u8> {
        // Step duration at 120 BPM, 1/8 note = 0.5 beats = 0.25 s = 12000 samples
        let step_samples = 12_000usize;
        let mut notes = vec![arp.current_note];
        for _ in 1..steps {
            let evs = arp.process(step_samples);
            for ev in evs.iter() {
                if let ArpEvent::NoteOn { note, .. } = *ev {
                    notes.push(note);
                }
            }
        }
        notes
    }

    #[test]
    fn up_mode_ascends_then_wraps() {
        let mut a = make_arp(ArpMode::Up, 1, &[60, 64, 67]);
        let notes = collect_notes(&mut a, 6);
        assert_eq!(notes, vec![60, 64, 67, 60, 64, 67]);
    }

    #[test]
    fn down_mode_descends_then_wraps() {
        let mut a = make_arp(ArpMode::Down, 1, &[60, 64, 67]);
        let notes = collect_notes(&mut a, 6);
        // First note is the key that was pressed (60, direct dispatch). The arp
        // then wraps to the top (67) and descends from there.
        assert_eq!(notes, vec![60, 67, 64, 60, 67, 64]);
    }

    #[test]
    fn updown_does_not_repeat_endpoints() {
        let mut a = make_arp(ArpMode::UpDown, 1, &[60, 64, 67]);
        // Sequence: 60, 64, 67, 64, 60, 64, 67, ...
        // After advance_step from 0, step_index = 1 (going up)
        let notes = collect_notes(&mut a, 8);
        // Endpoints (60, 67) should each appear once per traversal (not doubled)
        // Pattern after initial: 64, 67, 64, 60, 64, 67, 64, 60
        for pair in notes.windows(2) {
            assert_ne!(pair[0], pair[1], "consecutive notes should differ: {:?}", notes);
        }
    }

    #[test]
    fn octave_expansion() {
        let mut a = make_arp(ArpMode::Up, 2, &[60, 64]);
        // seq_len = 4: 60, 64, 72, 76
        let notes = collect_notes(&mut a, 4);
        assert!(notes.contains(&72), "octave 2 C should appear: {:?}", notes);
        assert!(notes.contains(&76), "octave 2 E should appear: {:?}", notes);
    }

    #[test]
    fn no_events_when_held_empty() {
        let mut a = ArpEngine::new(48_000.0);
        a.enabled = true;
        let evs = a.process(12_000);
        assert_eq!(evs.count, 0);
    }

    #[test]
    fn note_off_fires_before_step_end() {
        // Gate = 0.5: NoteOff fires halfway through the step
        let mut a = make_arp(ArpMode::Up, 1, &[60]);
        a.gate = 0.5;
        let step = 12_000usize;
        // First full step: NoteOn fires and gate opens
        let evs = a.process(step);
        assert!(
            evs.iter().any(|e| matches!(e, ArpEvent::NoteOn { .. })),
            "NoteOn must fire first"
        );
        // Second block: half a step — crosses the gate threshold, NoteOff fires
        let evs = a.process(step / 2);
        let has_off = evs.iter().any(|e| matches!(e, ArpEvent::NoteOff { .. }));
        assert!(has_off, "expected NoteOff at gate boundary");
    }

    #[test]
    fn swing_makes_odd_steps_shorter() {
        let mut a = ArpEngine::new(48_000.0);
        a.enabled = true;
        a.mode = ArpMode::Up;
        a.octaves = 1;
        a.bpm = 120.0;
        a.rate = ArpRate::R8;
        a.gate = 0.9;
        a.swing = 0.75; // even steps = 75% of pair, odd = 25%
        a.note_on(60);

        // Even step: 2 * 12000 * 0.75 = 18000 samples
        // Odd step:  2 * 12000 * 0.25 = 6000 samples
        let evs1 = a.process(18_000);
        let step1_notes: Vec<_> = evs1.iter().filter(|e| matches!(e, ArpEvent::NoteOn { .. })).collect();
        assert_eq!(step1_notes.len(), 1, "expected 1 NoteOn in even step");

        let evs2 = a.process(6_000);
        let step2_notes: Vec<_> = evs2.iter().filter(|e| matches!(e, ArpEvent::NoteOn { .. })).collect();
        assert_eq!(step2_notes.len(), 1, "expected 1 NoteOn in odd (short swing) step");
    }
}
