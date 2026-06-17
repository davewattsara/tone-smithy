# M22 plan — Engine additions

Two independent engine additions: a second sequencer mod lane and user-editable FM operator
routing. Both touch the same layers (engine, mod matrix, params, presets, UI) but are fully
independent of each other — commit them separately.

**Target version:** v1.2
**Estimate:** 2–3 weeks
**Branch:** `milestone/m22-engine-additions`

---

## Overview

| Phase | Feature | Key constraint |
|---|---|---|
| 1 | Second sequencer mod lane (`Seq2`) | Append-only — no preset index renumbering |
| 2 | Editable FM operator routing | High→low direction constraint maintained; factory algos unchanged |

---

## Phase 1 — Second sequencer mod lane (`Seq2`)

### Background

The step sequencer currently has one mod lane per step (`SeqStep.mod_value`), exposed as
mod source `Seq` (index 11, the last entry in `ModSource`). This phase adds a second
independent lane (`mod2_value`), exposed as source `Seq2` — the same pattern as `LFO1`/`LFO2`.

Everything follows the exact same plumbing path as the original `Seq` source from M18.

### Engine — `crates/synth-engine/src/seq.rs`

Add `mod2_value: f32` to `SeqStep`:

```rust
pub struct SeqStep {
    pub note_offset: i8,
    pub velocity: u8,
    pub gate: f32,
    pub rest: bool,
    pub tie: bool,
    pub mod_value: f32,
    pub mod2_value: f32,   // NEW
}
```

Default: `0.0`. Existing presets that omit `seq_step{i}_mod2` get `0.0` — no audible change.

### Params — `crates/synth-engine/src/params/ids.rs`

Append after `SeqStepMod(u8)`:

```rust
SeqStepMod2(u8),   // index 0..=15
```

Append to `tree.rs` / `snapshot.rs` following the exact pattern of `SeqStepMod`.

### Mod matrix — `crates/synth-engine/src/mod_matrix.rs`

Append `Seq2` to the **end** of `ModSource` (after `Seq`). The enum order is the on-the-wire
index, so appending gives `Seq2` index 12 with no renumbering.

```rust
pub enum ModSource {
    // … existing …
    Seq,   // index 11
    Seq2,  // index 12  ← NEW
}
```

Update `ModSource::COUNT` from 12 to 13. Add `seq2: f32` to `ModSources`. Handle
`ModSource::Seq2 => self.seq2` in `get()`. Add `12 => Some(Self::Seq2)` to `from_index`.

### Voice manager — `crates/synth-engine/src/voice_manager.rs`

Add `global_seq_mod2: f32` alongside the existing `global_seq_mod: f32`. Set it from the
current step's `mod2_value` each block, exactly mirroring the `global_seq_mod` path.

In `advance_modulators`, set `sources.seq2 = self.global_seq_mod2`.

### Engine — `crates/synth-engine/src/engine.rs`

Copy current step's `mod2_value` to `voices.set_global_seq_mod2(..)` each block, alongside
the existing `mod_value` copy.

### Presets — `crates/synth-presets/src/preset_params.rs`

Add `seq_step{i}_mod2` keys (0..=15) to `snapshot_to_map` and `snapshot_from_map`, following
the `seq_step{i}_mod` pattern. Old presets omit the keys → default `0.0` → no change.

### UI — `crates/synth-ui/src/app/state.rs`

Append `"Seq2"` to `MOD_SOURCE_LABELS` at index 12.
Add a `seq_step_mod2: [f32; 16]` state mirror alongside `seq_step_mod`.
Sync it in `sync_from_snapshot`.

### UI — `crates/synth-ui/src/sections/seq.rs`

Add a second Mod row per step in the step grid, below the existing Mod row. Label it
"Mod 2" (or "M2") to distinguish. Emits `ParamId::SeqStepMod2(i)` changes.

### Done when

- `Seq2` is assignable as a mod source; routing it to a different destination than `Seq`
  produces two independent CV lanes stepping in time.
- `Seq2` values survive a preset round-trip.
- Old presets load without change.
- `fmt` / `clippy -D warnings` clean.

---

## Phase 2 — Editable FM operator routing

### Background

FM algorithms are currently defined as a `const ALGORITHMS: [Algorithm; 8]` in `seq.rs` or
`fm.rs`. An `Algorithm` is:

```rust
pub struct Algorithm {
    pub mod_sources: [u8; 4],   // per-op bitmask: which ops modulate it
    pub is_carrier: [bool; 4],  // per-op: does it output to the slot?
}
```

The evaluation order is fixed at op 3 → 2 → 1 → 0, meaning only higher-index operators
can modulate lower-index ones. This constraint is maintained for user algorithms — no cycle
detection or topo-sort needed.

This phase adds a ninth "Custom" algorithm (index 8) where the six valid mod connections
and four carrier flags are stored as parameters, editable from a routing grid in the UI.

### The six valid connections

With the high→low constraint, the valid `src → dest` pairs are:

| Index | Connection |
|---|---|
| 0 | Op 1 → Op 0 |
| 1 | Op 2 → Op 0 |
| 2 | Op 2 → Op 1 |
| 3 | Op 3 → Op 0 |
| 4 | Op 3 → Op 1 |
| 5 | Op 3 → Op 2 |

Self-feedback (Op 3 → Op 3) is already handled by the existing `feedback_amount` parameter
and is not part of the custom routing.

### New param IDs — `crates/synth-engine/src/params/ids.rs`

Append after the existing FM param block:

```rust
// ── FM custom algorithm ─────────────────────────────────────────────────
// u8 = slot * 6 + conn_idx (0..=5) → total range 0..=11
FmCustomConn(u8),
// u8 = slot * 4 + op_idx (0..=3) → total range 0..=7
FmCustomCarrier(u8),
```

Value range: 0.0 = off, 1.0 = on (treated as booleans via `> 0.5`).

### Engine storage — `crates/synth-engine/src/params/snapshot.rs`

Add:

```rust
pub fm_custom_conn: [[bool; 6]; 2],      // [slot][conn_idx]
pub fm_custom_carrier: [[bool; 4]; 2],   // [slot][op_idx]
```

Defaults: `[[false; 6]; 2]` and `[[false; 4]; 2]` (all connections off, no carriers).
Old presets omit these keys → all-false → silence if algorithm is Custom, which is fine
because old presets never set algorithm to Custom (Custom is index 8; existing presets use 0–7).

### ParameterTree — `crates/synth-engine/src/params/tree.rs`

Add the matching fields and `apply_event` / `to_snapshot` arms for `FmCustomConn` and
`FmCustomCarrier`.

### FM evaluation — `crates/synth-engine/src/fm.rs` (or `slot.rs`)

Where the `Algorithm` is currently selected from `CONST_ALGORITHMS[algo_idx]`, add a branch
for `algo_idx == 8`:

```rust
let algorithm = if algo_idx < 8 {
    CONST_ALGORITHMS[algo_idx]
} else {
    // Build Algorithm from the custom params
    let slot = self.slot_idx;
    let mut mod_sources = [0u8; 4];
    for (conn_idx, &enabled) in snapshot.fm_custom_conn[slot].iter().enumerate() {
        if enabled {
            let (src, dest) = CONN_TABLE[conn_idx];
            mod_sources[dest] |= 1 << src;
        }
    }
    Algorithm {
        mod_sources,
        is_carrier: snapshot.fm_custom_carrier[slot],
    }
};
```

Where `CONN_TABLE: [(usize, usize); 6]` maps each connection index to `(src_op, dest_op)`:

```rust
const CONN_TABLE: [(usize, usize); 6] = [
    (1, 0), (2, 0), (2, 1), (3, 0), (3, 1), (3, 2),
];
```

### Algorithm selector — `crates/synth-engine/src/params/ids.rs`

`FmAlgorithm(u8)` already exists with values 0–7. Values 0–7 remain factory algorithms;
value 8 selects Custom. No change to the param ID; just the range extends to 8.

When switching to Custom (value 8) from a factory algorithm, the UI should initialise the
custom routing from the current factory algorithm before emitting `FmAlgorithm(slot)=8`.
This is a UI-side concern: read the current factory `Algorithm` struct, convert it to the
6-bit connection mask and 4-bit carrier mask, and emit `FmCustomConn`/`FmCustomCarrier`
events for each bit before switching the algorithm index.

### Presets — `crates/synth-presets/src/preset_params.rs`

Serialize `fm_custom_conn_s{slot}_c{conn}` (12 keys) and `fm_custom_carrier_s{slot}_op{op}`
(8 keys) with 0.0/1.0 values. Deserialize the same. Old presets omit all → defaults (all
false) → Custom algorithm is silent, but old presets never use algorithm 8, so no impact.

### UI — algorithm selector

In `crates/synth-ui/src/sections/osc.rs` (or wherever the FM controls are rendered), extend
the algorithm dropdown to include a ninth option: "Custom". Label the factory algorithms
"1 (Stack)" through "8 (Paired)" or use their existing names.

When "Custom" is selected, initialise by reading the current factory algorithm and emitting
the connection/carrier params before switching the index. (Avoid emitting 20 events on every
frame — only emit on the frame the switch happens.)

### UI — routing grid

When algorithm == Custom (index 8), show a routing grid below the algorithm selector.
When algorithm < 8, hide the grid (or show it read-only as a visualisation of the factory
algorithm — read-only is optional for MVP).

**Grid layout:**

Four rows, one per operator. Each row:

```
Carrier [x]  OP 0  |  (modulated by)  Op1 [x]  Op2 [x]  Op3 [x]
Carrier [ ]  OP 1  |  (modulated by)  Op2 [ ]  Op3 [ ]
Carrier [ ]  OP 2  |  (modulated by)  Op3 [x]
Carrier [x]  OP 3  |  (self-feedback handled by existing Feedback knob)
```

Each checkbox emits a `FmCustomCarrier` or `FmCustomConn` param event. Checkboxes are only
interactive when algorithm == 8; grey them out otherwise.

This layout is compact (4 rows × up to 4 checkboxes) and fits within the existing slot
foldout without a scroll area.

### Done when

- Selecting Custom and enabling op 3 as carrier + op 3 → op 2 connection (a one-carrier
  one-modulator patch) produces an audible FM tone.
- The full stack (3→2→1→0) reproduced as Custom sounds identical to factory algorithm 1.
- Switching from a factory algorithm to Custom initialises correctly from that algorithm.
- Custom routing survives a preset round-trip.
- Factory algorithms 0–7 are unaffected.
- `fmt` / `clippy -D warnings` clean.

---

## Milestone done when

1. `Seq2` drives a destination independently of `Seq`; both survive preset round-trips.
2. Custom FM routing produces expected sounds; connection grid is editable when in Custom mode.
3. Switching factory → Custom initialises from the factory algorithm.
4. Old presets unaffected.
5. All phases pass `cargo fmt --check` and `cargo clippy -D warnings`.

---

## Progress

- [ ] Phase 1 — Second sequencer mod lane (Seq2)
- [ ] Phase 2 — Editable FM operator routing
