# M18 plan — Step sequencer (+ bundled engine additions)

The headline feature is a 16-step melodic/modulation sequencer that lives alongside the
arpeggiator and shares its audio-thread clock pattern. The milestone also unifies the synth's two
tempo controls into one transport BPM, and bundles two small backward-compatible engine additions
that touch the same layers (matrix, params, LFO/voice): a **global (mono) LFO mode** and
**per-oscillator detune mod destinations**.

**Target version:** v1.1
**Estimate:** 2–3 weeks (sequencer) + ~1 week (BPM unify + bundled additions)
**Branch:** `milestone/m18-step-sequencer` — *create off `development`.*

> Prerequisite met: M17 is closed out (merged to `main`, tag `m17`, 2026-06-15). Branch M18 off the
> current `development`.

---

## Overview

| Phase | Feature | Notes |
|---|---|---|
| 1 | Unify transport BPM | Merge `ArpBpm` + global `Bpm` into one Master-tab BPM; prerequisite for the seq clock |
| 2 | Sequencer engine core (`SeqEngine`) | Mirrors `ArpEngine`; clock + 16 steps + playback modes |
| 3 | Sequencer mod lane | New global `Seq` mod **source**; per-step CV reuses the whole matrix |
| 4 | Sequencer UI (step grid) | New `Seq` tab; step-grid widget |
| 5 | Global (mono) LFO mode | Bundled; per-LFO `Global` toggle, shared `Lfo` on `VoiceManager` |
| 6 | Per-oscillator detune dests | Bundled; append `Osc2Det` / `Osc3Det` to `ModDest` |

Phase 1 lands first (the sequencer clock reads the unified BPM). Phases 2→4 are the sequencer and
land in order. Phases 5 and 6 are independent and can be done at any point (each is its own commit).

---

## Decisions (confirmed with user, 2026-06-15)

These were the plan's open questions; now settled:

1. **Clock source — single internal transport BPM, MIDI clock deferred.** There is no MIDI
   real-time clock handling today (`crates/synth-host/src/midi.rs` doesn't parse
   `0xF8`/`0xFA`/`0xFC`). M18 syncs everything to one *internal* BPM (Phase 1); external MIDI-clock
   sync is split into its own later item.
2. **One unified BPM in the Master tab (Phase 1).** Today two independent tempos exist — global
   `Bpm` (drives LFO sync; already has a knob in the Master tab, `master.rs:66`) and `ArpBpm` (drives
   the arp; separate knob in the Arp tab, `arp.rs:93`). They are merged into a single transport BPM
   in the Master tab that the arp, sequencer, and LFO sync all read. This also answers "which BPM
   does the sequencer follow" — the one unified BPM.
3. **Sequencer and arp are mutually exclusive.** Enabling the sequencer disables the arp and
   vice-versa (a single "note engine: Off / Arp / Seq" choice), so two clocks never fight over the
   voice pool.
4. **Step note offset is relative to the lowest currently-held note.** Hold a key and the sequence
   transposes to it. A `rest` step plays nothing; an empty held-note set silences the sequencer.

---

## Phase 1 — Unify transport BPM (Master tab)

### Background

Two BPMs exist and can disagree: global `Bpm` (`ParamId::Bpm`, param tree — drives LFO tempo-sync,
knob in the Master tab `master.rs:66`) and `ArpBpm` (`ParamId::ArpBpm` — sets `ArpEngine::bpm`, a
separate knob in the Arp tab `arp.rs:93`). A third clock (the sequencer) would make the split worse.
Collapse to one transport BPM, surfaced by the existing Master-tab control, read by the arp, the
sequencer, and LFO sync.

### Changes

- **`crates/synth-engine/src/engine.rs`** — make `ParamId::Bpm` the single source of truth: in its
  `apply_event` arm, also set `self.arp.bpm` (and, after Phase 2, `self.seq.bpm`). Remove the
  `ParamId::ArpBpm` arm (or map it to `Bpm` for back-compat — see MIDI-learn below).
- **LFO sync** already reads `tree.bpm` (`tree.rs` `sync_rate_hz`) — unchanged.
- **`crates/synth-ui/src/sections/arp.rs`** — remove the Arp-tab BPM knob; the Master-tab knob
  (`master.rs`) becomes the one control. Drop the now-unused `arp_bpm` state mirror in
  `app/state.rs` (or repoint it at `bpm`).
- **`crates/synth-ui/src/app/midi_learn.rs`** — it maps the `"arp_bpm"` key → `ParamId::ArpBpm`
  (lines ~207/391). Keep an alias so any existing learned mapping resolves to `ParamId::Bpm`, or
  drop it with a note that re-learning is needed.
- **Presets:** BPM is **not** serialized per-preset today (0 of 61 factory presets carry `bpm` or
  `arp_bpm` — tempo is a global/session setting, not patch state). So **no preset migration** is
  needed. Before deleting `ArpBpm`, just confirm `snapshot_to_map` doesn't emit either key.

### Done when

- A single BPM control (Master tab) retempos the arp, LFO sync, and (after Phase 2) the sequencer.
- No separate Arp-tab BPM remains; nothing reads `ArpBpm`.
- `cargo fmt` / `clippy -D warnings` clean.

---

## Phase 2 — Sequencer engine core (`SeqEngine`)

### Background

`ArpEngine` (`crates/synth-engine/src/arp.rs`) is the template: it runs on the audio thread with no
alloc/lock, owns a phase accumulator and a held-note list, and on each `process(n_frames)` returns a
fixed-size `ArpEvents` list of `NoteOn`/`NoteOff` that `engine.rs` injects before the voice loop
(see `engine.rs:427` `self.arp.process(frames)`). The sequencer is a sibling — same
clock/gate/swing machinery, but instead of arpeggiating the held set it walks a fixed 16-step
pattern transposed by the held root note (Decision 4).

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
    pub mod_value: f32,    // -1.0..=1.0  (Phase 3)
}

/// Playback order across the active step range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SeqMode { Forward, Reverse, PingPong, Random }

pub struct SeqEngine {
    pub enabled: bool,
    pub length: usize,     // 1..=SEQ_MAX_STEPS active steps
    pub mode: SeqMode,
    pub rate: ArpRate,     // reuse arp's note-value enum (1/32..1/2)
    pub bpm: f32,          // set from the unified transport BPM (Decision 2)
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
SeqStepMod(u8),     // Phase 3
```

`SeqStep*(u8)` mirrors the existing `ParamId::ModSlot*(u8)` pattern, so a single variant covers all
16 steps and no per-step enum explosion is needed.

### Engine integration — `crates/synth-engine/src/engine.rs`

- Add `seq: SeqEngine` field; construct in `Engine::new`.
- In the per-block tick, call `self.seq.process(frames)` next to the arp and inject its events the
  same way. Honour Decision 3: route note input to *either* arp or seq based on the active note
  engine; don't clock both.
- Handle the new `ParamId::Seq*` arms in `apply_event` (clamp like the arp params; `SeqStep*(i)`
  writes into `seq.steps[i]`). Mutual exclusion: enabling `SeqEnabled` forces the arp off and
  vice-versa (mirror the existing `ArpEnabled` note-handoff logic at `engine.rs:387`).

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

- A 16-step pattern clocks `NoteOn`/`NoteOff` at the unified BPM / set rate; per-step offset,
  velocity, gate, and rest all audibly take effect.
- Forward / Reverse / PingPong / Random walk the active range correctly.
- Enabling the sequencer disables the arp (Decision 3) with no stuck notes.
- Sequencer state round-trips through a preset.
- A `no_alloc` test covers `SeqEngine::process` (mirror the arp's RT-safety coverage).
- `cargo fmt` and `cargo clippy -D warnings` clean.

---

## Phase 3 — Sequencer mod lane

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
- **`crates/synth-ui/src/app/state.rs`** — append `"Seq"` to `MOD_SOURCE_LABELS` (and add it to the
  `MOD_SOURCE_ORDER` display list — likely at the end after Env3).
- **`crates/synth-engine/src/voice_manager.rs`** — `advance_modulators` builds `ModSources`; set
  `seq: self.global_seq_mod` from a value pushed by the engine each block (the active step's
  `mod_value`, held across the step). This mirrors how `global_mod_wheel`/`global_aftertouch` are
  stored and read.
- **`engine.rs`** — each block, after `seq.process`, copy the current step's `mod_value` into
  `voices.set_global_seq_mod(..)`.
- Per-step `SeqStepMod(u8)` param already added in Phase 2.

### Done when

- Routing matrix source `Seq` → any destination (e.g. `F1 Cut`) makes the destination step through
  the lane's per-step values in time with the sequence.
- The lane value round-trips in presets (covered by Phase 2's `seq_step{i}_mod` key).
- `cargo fmt` / `clippy -D warnings` clean.

---

## Phase 4 — Sequencer UI

### Background

The arp UI (`crates/synth-ui/src/sections/arp.rs`, rendered via `Tab::Arp` in `app/mod.rs:105`) is a
flat row of toggles/combos/knobs. The sequencer needs a **step-grid** widget — 16 columns, each with
note-offset, velocity, gate, rest, and mod-lane controls — plus length/rate/mode controls.

### Changes

- **New tab.** Add `Tab::Seq` to the `Tab` enum and `Tab::ALL` (`app/state.rs`), a `seq_tab(ui)`
  renderer (`sections/seq.rs`), and the dispatch arm in `app/mod.rs`. (Given Decision 3, a shared
  "Sequencer/Arp" presentation with a note-engine switch at the top is an option; a dedicated Seq
  tab is cleaner given the grid's size.)
- **State mirrors** in `app/state.rs`: `seq_enabled`, `seq_length`, `seq_mode`, `seq_rate`,
  `seq_swing`, and per-step arrays; sync them in `sync_from_snapshot`.
- **Step-grid widget.** 16 columns. Per column: a small note-offset control (drag or stepper,
  ±24), a velocity slider, a gate slider, a rest toggle, and a mod-lane value. A playhead highlight
  driven by the snapshot's current step (add a `seq_current_step` live field to the snapshot, like
  `lfo1_out`, so the UI can light the active column).
- **Transport controls:** Enabled toggle, Length (1–16), Rate combo (reuse the arp's `1/32…1/2`
  labels), Mode combo (Forward/Reverse/PingPong/Random), Swing knob. Emit `ParamId::Seq*` changes
  exactly like the arp section emits `ParamId::Arp*`. BPM lives in the Master tab now (Phase 1), not
  here.

### Done when

- All 16 steps editable; the grid shows a moving playhead while running.
- Length/rate/mode/swing controls work and round-trip.
- `cargo fmt` / `clippy -D warnings` clean.

---

## Phase 5 — Global (mono) LFO mode (bundled)

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

## Phase 6 — Per-oscillator detune mod destinations (bundled)

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

One Master-tab BPM drives the arp, LFO sync, and sequencer; a 16-step melodic line plays with
independent velocity and gate per step; the mod lane drives a destination audibly; switching an LFO
to global mode locks all held voices to one shared phase; an LFO routed to `Osc2Det` detunes only
OSC2; and the sequencer, LFO mode, and new oscillator dests all survive preset save/load round-trips.

1. Single unified transport BPM in the Master tab; no separate Arp BPM.
2. Sequencer: 16 editable steps, four playback modes, internal BPM sync, moving playhead.
3. Sequencer mod lane drives any matrix destination via the new `Seq` source.
4. Sequencer/arp mutual exclusion with no stuck notes.
5. Global LFO mode phase-locks chords; `Reset` greyed out when global.
6. `Osc2Det` / `Osc3Det` mod destinations work and are append-only (no preset migration).
7. Full preset round-trip for every new parameter.
8. All phases pass `cargo fmt --check` and `cargo clippy -D warnings`, and the `no_alloc` test
   covers the new audio-thread code.

---

## Files touched (summary)

| File | Phase(s) |
|---|---|
| `crates/synth-engine/src/arp.rs` | 1 (BPM source) |
| `crates/synth-engine/src/seq.rs` (new) | 2, 3 |
| `crates/synth-engine/src/params/ids.rs` | 2, 5 |
| `crates/synth-engine/src/params/snapshot.rs` | 2, 4, 5 |
| `crates/synth-engine/src/params/tree.rs` | 2, 5 |
| `crates/synth-engine/src/engine.rs` | 1, 2, 3, 5 |
| `crates/synth-engine/src/voice_manager.rs` | 3, 5, 6 |
| `crates/synth-engine/src/voice.rs` | 5 |
| `crates/synth-engine/src/mod_matrix.rs` | 3, 6 |
| `crates/synth-engine/src/lib.rs` | 2 (re-export `SEQ_MAX_STEPS`) |
| `crates/synth-presets/src/preset_params.rs` | 2, 3, 5, 6 |
| `crates/synth-ui/src/sections/master.rs` | 1 (sole BPM control) |
| `crates/synth-ui/src/sections/arp.rs` | 1 (remove BPM knob) |
| `crates/synth-ui/src/app/midi_learn.rs` | 1 (arp_bpm alias) |
| `crates/synth-ui/src/app/state.rs` | 1, 2, 3, 4, 6 |
| `crates/synth-ui/src/app/mod.rs` | 4 |
| `crates/synth-ui/src/sections/seq.rs` (new) | 4 |
| `crates/synth-ui/src/sections/envelopes.rs` | 5 |
| `crates/synth-engine/tests/no_alloc.rs` | 2 |

---

## Progress

- [x] Open questions resolved with user (2026-06-15) — see Decisions
- [ ] Phase 1 — Unify transport BPM
- [ ] Phase 2 — Sequencer engine core
- [ ] Phase 3 — Sequencer mod lane
- [ ] Phase 4 — Sequencer UI
- [ ] Phase 5 — Global (mono) LFO mode
- [ ] Phase 6 — Per-oscillator detune dests

Not started — decisions confirmed; awaiting explicit go-ahead to begin implementation on a
`milestone/m18-step-sequencer` branch off `development`.
