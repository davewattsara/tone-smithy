# M18 plan — Step sequencer (+ bundled engine additions)

The headline feature is a 16-step melodic/modulation sequencer that lives alongside the
arpeggiator and shares its audio-thread clock pattern. Two small, backward-compatible engine
additions are bundled in because they touch the same layers (matrix, params, LFO/voice) and are
cheap to land here: a **global (mono) LFO mode** and **per-oscillator detune mod destinations**.

**Target version:** v1.1
**Estimate:** 2–3 weeks (sequencer) + ~1 week (bundled additions)
**Branch:** `milestone/m18-step-sequencer` — *create off `development` after M17 is merged.*

> ⚠ Prerequisite: M17 must be closed out first (merge `milestone/m17-engine-expansion` →
> `development` → `main`, tag `m17`, mark complete in `milestones.md`). M18's branch should fork
> from a `development` that already contains the M17 engine work.

---

## Overview

| Phase | Feature | Notes |
|---|---|---|
| 1 | Sequencer engine core (`SeqEngine`) | Mirrors `ArpEngine`; clock + 16 steps + playback modes |
| 2 | Sequencer mod lane | New global `Seq` mod **source**; per-step CV reuses the whole matrix |
| 3 | Sequencer UI (step grid) | New `Seq` tab; step-grid widget |
| 4 | Global (mono) LFO mode | Bundled; per-LFO `Global` toggle, shared `Lfo` on `VoiceManager` |
| 5 | Per-oscillator detune dests | Bundled; append `Osc2Det` / `Osc3Det` to `ModDest` |

Phases 1→3 are the sequencer and should land in order. Phases 4 and 5 are independent and can be
done at any point (each is its own commit). Settle the **Open questions** below before starting
Phase 1 — they change the engine's shape.

---

## Open questions (confirm before Phase 1)

1. **MIDI-clock sync is out of scope unless we build it.** There is currently *no* MIDI real-time
   clock handling anywhere — `crates/synth-host/src/midi.rs` does not parse `0xF8`/`0xFA`/`0xFC`,
   and the arp runs purely off its internal `bpm`. The milestone line "sync to arp BPM / MIDI clock"
   therefore can't mean external clock without new transport infrastructure (clock-pulse counting,
   start/stop, tempo estimation). **Recommendation:** scope M18 to *internal* BPM sync (the
   sequencer shares the same clock the arp uses) and split external MIDI-clock sync into its own
   later item. This plan assumes that.
2. **Which BPM does the sequencer follow?** Two tempos exist today: `ArpBpm` (lives on `ArpEngine`)
   and a separate global `Bpm` (param tree, drives LFO sync). **Recommendation:** the sequencer
   follows the **arp BPM** (it's the existing step-clock tempo and matches the milestone wording);
   note the longer-term cleanup of unifying both into one transport BPM, but don't do that refactor
   here.
3. **Sequencer ↔ arp interaction.** Both generate notes from held keys. **Recommendation:** make
   them **mutually exclusive** — enabling the sequencer disables the arp and vice-versa (a single
   "note engine: Off / Arp / Seq" choice), which avoids two clocks fighting over the voice pool.
   The alternative (both running) needs a defined precedence and is more work for little benefit.
4. **What is a step's note relative to?** "Per-step note offset (±24 semitones)" implies a
   reference pitch. **Recommendation:** offset is relative to the **lowest currently-held note**
   (hold a key, the sequence transposes to it), matching a typical mono step sequencer. A "rest"
   step plays nothing; an empty held-note set silences the sequencer.

---

## Phase 1 — Sequencer engine core (`SeqEngine`)

### Background

`ArpEngine` (`crates/synth-engine/src/arp.rs`) is the template: it runs on the audio thread with no
alloc/lock, owns a phase accumulator and a held-note list, and on each `process(n_frames)` returns a
fixed-size `ArpEvents` list of `NoteOn`/`NoteOff` that `engine.rs` injects before the voice loop
(see `engine.rs:427` `self.arp.process(frames)`). The sequencer is a sibling of this — same
clock/gate/swing machinery, but instead of arpeggiating the held set it walks a fixed 16-step
pattern transposed by the held root note.

### New module — `crates/synth-engine/src/seq.rs`

Mirror `arp.rs`. Reuse `ArpEvent`/`ArpEvents` (or a `SeqEvents` clone of the same shape).

```rust
/// Maximum sequencer steps.
pub const SEQ_MAX_STEPS: usize = 16;

/// Per-step data. `rest` mutes the step; `note_offset` is semitones from the
/// held root; `gate` is the fraction of the step the note sounds; `mod_value`
/// is the mod-lane CV (-1..=1) exposed as the `Seq` mod source.
#[derive(Debug, Clone, Copy)]
pub struct SeqStep {
    pub note_offset: i8,   // -24..=24
    pub velocity: u8,      // 0..=127
    pub gate: f32,         // 0.0..=1.0
    pub rest: bool,
    pub mod_value: f32,    // -1.0..=1.0  (Phase 2)
}

/// Playback order across the active step range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeqMode { Forward, Reverse, PingPong, Random }

pub struct SeqEngine {
    pub enabled: bool,
    pub length: usize,     // 1..=SEQ_MAX_STEPS active steps
    pub mode: SeqMode,
    pub rate: ArpRate,     // reuse arp's note-value enum (1/32..1/2)
    pub bpm: f32,          // follows arp BPM (Open Q #2)
    pub swing: f32,
    pub steps: [SeqStep; SEQ_MAX_STEPS],
    // runtime: held root, phase, step cursor, direction, gate_open, current_note, rng
    // — same shape as ArpEngine's runtime block.
}
```

`process(n_frames) -> ArpEvents` is a near-copy of `ArpEngine::process`: advance phase by
`n_frames / step_samples()`, on a step boundary close the previous gate, pick the next step index
per `SeqMode`, skip `rest` steps, and emit `NoteOn { note: root + offset, velocity }`. `step_samples`
reuses the arp formula `(60/bpm) * rate.beats_per_step() * sample_rate_hz` with swing. Keep
`note_on`/`note_off` to track the held root (lowest held note) and a `clear()` for the stuck-note
recovery path.

### New `ParamId` variants — `crates/synth-engine/src/params/ids.rs`

Append after the `ArpSwing` block (appending keeps existing indices stable for presets):

```rust
// ── Step sequencer ────────────────────────────────────────────────────
SeqEnabled,
SeqLength,
SeqMode,
SeqRate,
SeqSwing,
// Per-step params take a u8 step index, like ModSlot*(u8):
SeqStepNote(u8),
SeqStepVelocity(u8),
SeqStepGate(u8),
SeqStepRest(u8),
SeqStepMod(u8),     // Phase 2
```

`SeqStep*(u8)` mirrors the existing `ParamId::ModSlot*(u8)` pattern, so a single variant covers all
16 steps and no per-step enum explosion is needed.

### Engine integration — `crates/synth-engine/src/engine.rs`

- Add `seq: SeqEngine` field; construct in `Engine::new`.
- In the per-block tick, call `self.seq.process(frames)` next to the arp and inject its events the
  same way. Honour Open Q #3: route note input to *either* arp or seq based on the active note
  engine; don't clock both.
- Handle the new `ParamId::Seq*` arms in `apply_event` (clamp like the arp params; `SeqStep*(i)`
  writes into `seq.steps[i]`).

### Param tree / snapshot — `tree.rs`, `snapshot.rs`

- `ParamSnapshot`: add `seq_enabled`, `seq_length`, `seq_mode`, `seq_rate`, `seq_swing`, and
  `[SeqStep; SEQ_MAX_STEPS]` (or parallel `[T; 16]` arrays to match the existing mod-slot mirror
  style). Defaults: disabled, length 16, Forward, 1/16, swing 0.5; per-step offset 0, velocity 100,
  gate 0.5, rest false, mod 0.0.
- `ParameterTree`: matching fields + `apply_event`/`to_snapshot` arms.

### Preset round-trip — `crates/synth-presets/src/preset_params.rs`

- Serialize `seq_enabled`, `seq_length`, `seq_mode`, `seq_rate`, `seq_swing`, and per-step keys
  (`seq_step{i}_note`, `_velocity`, `_gate`, `_rest`, `_mod`) over `0..SEQ_MAX_STEPS`.
- Deserialize the same. **Old presets** omit every `seq_*` key, so they fall back to defaults
  (sequencer disabled) — no migration, identical playback. (Same sparse-overlay behaviour as the
  mod-slot keys.)

### Done when

- A 16-step pattern clocks `NoteOn`/`NoteOff` at the set BPM/rate; per-step offset, velocity, gate,
  and rest all audibly take effect.
- Forward / Reverse / PingPong / Random walk the active range correctly.
- Enabling the sequencer disables the arp (Open Q #3) with no stuck notes.
- Sequencer state round-trips through a preset.
- A `no_alloc` test covers `SeqEngine::process` (mirror the arp's RT-safety coverage).
- `cargo fmt` and `cargo clippy -D warnings` clean.

---

## Phase 2 — Sequencer mod lane

### Background

The lane gives each step a CV value (-1..=1) that should be able to drive *any* mod destination.
Rather than teaching the sequencer about destinations directly, expose the lane's current value as a
**new global mod source** and let the existing matrix route it anywhere — this reuses all dest
plumbing, amount scaling, and the `Via` path for free.

### Changes

- **`crates/synth-engine/src/mod_matrix.rs`** — append `Seq` to the *end* of the `ModSource` enum
  (after `Env3`), exactly as Env3 was appended in M17: the enum order is the on-the-wire preset
  index, so appending gives `Seq` the next index with no renumbering. Add `seq: f32` to
  `ModSources`, a `ModSource::Seq => self.seq` arm in `get()`, and the `from_index` arm.
- **`crates/synth-ui/src/app/state.rs`** — append `"Seq"` to `MOD_SOURCE_LABELS` (and it inherits
  the `MOD_SOURCE_ORDER` display list — decide placement; likely at the end after Env3).
- **`crates/synth-engine/src/voice_manager.rs`** — `advance_modulators` builds `ModSources`; set
  `seq: self.global_seq_mod` from a value pushed by the engine each block (the active step's
  `mod_value`, held across the step). This mirrors how `global_mod_wheel`/`global_aftertouch` are
  stored and read.
- **`engine.rs`** — each block, after `seq.process`, copy the current step's `mod_value` into
  `voices.set_global_seq_mod(..)`.
- Per-step `SeqStepMod(u8)` param already added in Phase 1.

### Done when

- Routing matrix source `Seq` → any destination (e.g. `F1 Cut`) makes the destination step through
  the lane's per-step values in time with the sequence.
- The lane value round-trips in presets (covered by Phase 1's `seq_step{i}_mod` key).
- `cargo fmt` / `clippy -D warnings` clean.

---

## Phase 3 — Sequencer UI

### Background

The arp UI (`crates/synth-ui/src/sections/arp.rs`, rendered via `Tab::Arp` in `app/mod.rs:105`) is a
flat row of toggles/combos/knobs. The sequencer needs a **step-grid** widget — 16 columns, each with
note-offset, velocity, gate, rest, and mod-lane controls — plus length/rate/mode/swing selectors.

### Changes

- **New tab.** Add `Tab::Seq` to the `Tab` enum and `Tab::ALL` (`app/state.rs`), a `seq_tab(ui)`
  renderer (`sections/seq.rs`), and the dispatch arm in `app/mod.rs`. (Alternatively fold into the
  Arp tab; a dedicated tab is cleaner given the grid's size — confirm with the note-engine choice
  from Open Q #3, e.g. a shared "Sequencer/Arp" tab with a mode switch at the top.)
- **State mirrors** in `app/state.rs`: `seq_enabled`, `seq_length`, `seq_mode`, `seq_rate`,
  `seq_swing`, and per-step arrays; sync them in `sync_from_snapshot`.
- **Step-grid widget.** 16 columns. Per column: a small note-offset control (drag or stepper,
  ±24), a velocity slider, a gate slider, a rest toggle, and a mod-lane value. A playhead highlight
  driven by the snapshot's current step (add a `seq_current_step` live field to the snapshot, like
  `lfo1_out`, so the UI can light the active column).
- **Transport controls:** Enabled toggle, Length (1–16), Rate combo (reuse the arp's `1/32…1/2`
  labels), Mode combo (Forward/Reverse/PingPong/Random), Swing knob. Emit `ParamId::Seq*` changes
  exactly like the arp section emits `ParamId::Arp*`.

### Done when

- All 16 steps editable; the grid shows a moving playhead while running.
- Length/rate/mode/swing controls work and round-trip.
- `cargo fmt` / `clippy -D warnings` clean.

---

## Phase 4 — Global (mono) LFO mode (bundled)

### Background

LFOs are per-voice today: each `Voice` owns `lfo1`/`lfo2` (`voice.rs`), advanced in
`Voice::advance_modulators`, and `VoiceManager::advance_modulators` reads `v.lfo1_out()` per voice.
`features-v1.md` specs "per-voice or global mode" but only per-voice shipped. Global mode shares one
LFO across all held voices so chords stay phase-locked.

### Changes

- **`crates/synth-engine/src/params/ids.rs`** — add `Lfo1Global`, `Lfo2Global` (bool params; reuse
  the generic `ParameterChange` event, like `Lfo1ResetOnNoteOn`).
- **`tree.rs` / `snapshot.rs`** — field, default (`false`), set-handler, getter, snapshot mirror.
- **`crates/synth-engine/src/voice_manager.rs`** — add one shared `Lfo` per global LFO as a field;
  advance it **once** per block at the top of `advance_modulators`; when `lfo{1,2}_global` is set,
  use the shared LFO's output for `sources.lfo{1,2}` instead of `v.lfo{1,2}_out()`. The existing
  `set_lfo1_rate_hz`/`set_lfo1_shape`/sync setters must also drive the shared instance so it tracks
  rate/shape/BPM-sync.
- **`engine.rs`** — push the global flags to `VoiceManager` in the per-block LFO sync block
  (alongside the existing `set_lfo1_*` calls near `engine.rs:76`).
- **`crates/synth-ui/src/sections/envelopes.rs`** — add a `Global` toggle next to the existing
  `Reset` selectable; grey out `Reset` when `Global` is on (no per-note phase to reset).
- **`app/state.rs`** mirrors + **`preset_params.rs`** keys (`lfo1_global` / `lfo2_global`).

No new DSP math, no new event type. Old presets omit the keys → default per-voice → unchanged.

### Done when

- Switching an LFO to Global locks all held voices to one shared phase; per-voice stays the default.
- `Reset` is disabled while Global is active.
- Round-trips in presets; `fmt`/`clippy` clean.

---

## Phase 5 — Per-oscillator detune mod destinations (bundled)

### Background

Only OSC1 is mod-addressable (`Osc1Det` / `Osc1Pan`), though the engine has three equal main
oscillators (`osc_main_detune_cents[3]`, applied in `voice_manager.rs:617`). Add OSC2/OSC3 detune as
destinations so each oscillator's tuning can be modulated independently (movement the global `Pitch`
dest can't give, since it shifts all oscillators together). Pan for 2/3 is an optional add.

### Changes (per new destination — append only)

- **`crates/synth-engine/src/mod_matrix.rs`** — append `Osc2DetuneCents`, `Osc3DetuneCents` to the
  *end* of `ModDest` (as `Filter2*` were in M17 — appended indices keep preset dest indices stable).
  Add `osc2_detune_cents` / `osc3_detune_cents` to `DestOffsets`, the `compute_offsets` arms, and
  the `from_index` arms.
- **`crates/synth-engine/src/voice_manager.rs`** — apply the offsets:
  `vp.osc_main_detune_cents[1] += off.osc2_detune_cents;` and `[2] += off.osc3_detune_cents;`
  (next to the existing `[0] += off.osc1_detune_cents`).
- **`crates/synth-ui/src/app/state.rs`** — append `"Osc2Det"`, `"Osc3Det"` to `MOD_DEST_LABELS`
  **and** add matching `MOD_AMOUNT_RANGES` entries (`2400.0` cents each, like `Osc1Det`). ⚠ This is
  the array whose length mismatch silently broke `F2 Cut` during M17 testing — the two slices must
  stay the same length and order as `ModDest`.

(Optional: `Osc2Pan` / `Osc3Pan` the same way with `1.0` ranges.)

### Done when

- An LFO routed to `Osc2Det` audibly detunes only OSC2; OSC1/OSC3 unaffected.
- Dest indices unchanged for existing presets; new dest round-trips.
- `fmt`/`clippy` clean.

---

## Milestone done when

A 16-step melodic line plays with independent velocity and gate per step; the mod lane drives a
destination audibly; switching an LFO to global mode locks all held voices to one shared phase; an
LFO routed to `Osc2Det` detunes only OSC2; and the sequencer, LFO mode, and new oscillator dests all
survive preset save/load round-trips.

1. Sequencer: 16 editable steps, four playback modes, internal BPM sync, moving playhead.
2. Sequencer mod lane drives any matrix destination via the new `Seq` source.
3. Sequencer/arp mutual exclusion with no stuck notes.
4. Global LFO mode phase-locks chords; `Reset` greyed out when global.
5. `Osc2Det` / `Osc3Det` mod destinations work and are append-only (no preset migration).
6. Full preset round-trip for every new parameter.
7. All phases pass `cargo fmt --check` and `cargo clippy -D warnings`, and the `no_alloc` test
   covers the new audio-thread code.

---

## Files touched (summary)

| File | Phase(s) |
|---|---|
| `crates/synth-engine/src/seq.rs` (new) | 1, 2 |
| `crates/synth-engine/src/params/ids.rs` | 1, 4 |
| `crates/synth-engine/src/params/snapshot.rs` | 1, 3, 4 |
| `crates/synth-engine/src/params/tree.rs` | 1, 4 |
| `crates/synth-engine/src/engine.rs` | 1, 2, 4 |
| `crates/synth-engine/src/voice_manager.rs` | 2, 4, 5 |
| `crates/synth-engine/src/voice.rs` | 4 |
| `crates/synth-engine/src/mod_matrix.rs` | 2, 5 |
| `crates/synth-engine/src/lib.rs` | 1 (re-export `SEQ_MAX_STEPS`) |
| `crates/synth-presets/src/preset_params.rs` | 1, 2, 4, 5 |
| `crates/synth-ui/src/app/state.rs` | 1, 2, 3, 5 |
| `crates/synth-ui/src/app/mod.rs` | 3 |
| `crates/synth-ui/src/sections/seq.rs` (new) | 3 |
| `crates/synth-ui/src/sections/envelopes.rs` | 4 |
| `crates/synth-engine/tests/no_alloc.rs` | 1 |

---

## Progress

- [ ] Open questions confirmed with user
- [ ] Phase 1 — Sequencer engine core
- [ ] Phase 2 — Sequencer mod lane
- [ ] Phase 3 — Sequencer UI
- [ ] Phase 4 — Global (mono) LFO mode
- [ ] Phase 5 — Per-oscillator detune dests

Not started — plan drafted on the M17 branch ahead of M17 close-out. Do not begin implementation
until the user gives the go-ahead and M17 is merged to `development`.
