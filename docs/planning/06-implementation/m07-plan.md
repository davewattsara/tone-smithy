# M7 — FM engine

Branch: `milestone/m07-fm-engine`
Status: **Draft** (no sub-milestones implemented yet)

## Done-when

All 8 algorithms produce expected sounds; combining subtractive slot 0
with FM slot 1 in one patch sounds coherent; no audible aliasing at
high modulation indices; CPU stays well below 50% with 32 active voices.

The canonical end-to-end test: a patch with slot 0 set to subtractive
(warm saw) and slot 1 set to FM (clean bell on algorithm 1) plays as a
single voice with both layers audible and panable.

---

## Design decisions

### Voice architecture: two slots, mode-tagged

Each voice owns two `Slot` values. A `Slot` is a struct containing
*both* subtractive and FM state and a `mode: SlotMode` selector — only
the active side is advanced each sample. This keeps the whole voice
stack-allocated (no heap, no trait objects) at the cost of carrying the
inactive bank's state in RAM. With 32 voices × 2 slots × (subtractive
+ FM) the total per-voice footprint stays well under a few KB.

```rust
pub enum SlotMode { Subtractive, Fm }

pub struct Slot {
    mode: SlotMode,
    subtractive: SubtractiveBank,  // 3 unison osc + 1 sub
    fm: FmBank,                    // 4 operators
    level: f32,                    // per-slot mix level
    pan:   f32,                    // per-slot mix pan
}
```

The slot bank that is *not* selected does not advance its phase
accumulators — switching mode mid-note retriggers nothing, and the
previously-inactive bank picks up from rest. This means a mode switch
during a held note clicks unless the amp envelope is silent; in v1
we accept that.

### Operator model

Each FM operator is:

```rust
pub struct Operator {
    phase: f32,                     // 0..1
    sample_rate_hz: f32,
    envelope: Adsr,                 // shared with amp env type
    ratio_integer: u8,              // 0..=15
    ratio_fine_cents: f32,          // -100..=+100
    level: f32,                     // 0..=1; envelope output is scaled by this
    feedback_amount: f32,           // op 3 only; -1..=1
    feedback_prev_output: f32,      // op 3 only; one-sample memory
}
```

- Operators 0/1/2 have no feedback (the `feedback_*` fields are unused for them).
- Operator 3 is the canonical feedback operator per DX7 convention. Its
  self-feedback is one-sample-delayed, scaled by `feedback_amount`.
- Output of each operator is `sin(2π × (phase + modulation_in)) × env_level × level`.

### Effective operator frequency

```text
f_op_hz = note_hz × (ratio_integer as f32 + ratio_fine_cents / 1200.0_to_ratio)
```

Ratio = 0 is treated as a fixed-frequency mode at a default 1 Hz (so the
operator becomes a slow vibrato source rather than going silent). Fixed
frequency is a v1.2 feature; for M7 ratio = 0 simply behaves as ratio = 1.

### Algorithm definitions (8 starter algorithms)

Each algorithm is a static routing graph. For M7, hardcoded as a
`const ALGORITHMS: [Algorithm; 8]`. An `Algorithm` is:

```rust
pub struct Algorithm {
    /// For each operator (0..4), which other operators FM its phase.
    /// `mod_sources[i]` is a bitmask: bit j set means op j modulates op i.
    pub mod_sources: [u8; 4],
    /// For each operator, whether it contributes to the slot output.
    /// `is_carrier[i]` true means op i's output is summed into the slot.
    pub is_carrier: [bool; 4],
}
```

The 8 starter algorithms (using DX7-family numbering; op 3 has the
optional self-feedback in all of them):

| # | Topology | mod_sources | carriers |
|---|---|---|---|
| 1 | Stack 4→3→2→1 | `[0b0010, 0b0100, 0b1000, 0]` | op 0 |
| 2 | Stack 4→3→2→1, op 3 self-feedback | `[0b0010, 0b0100, 0b1000, 0b1000]` | op 0 |
| 3 | Two stacks (4→3, 2→1) mixed | `[0b0010, 0, 0b1000, 0]` | op 0, op 2 |
| 4 | 4 modulates 1, 2, 3 (parallel mod) | `[0b1000, 0b1000, 0b1000, 0]` | op 0, op 1, op 2 |
| 5 | 4→3, 3→2, 2 and 3 to 1 | `[0b1100, 0b0100, 0b1000, 0]` | op 0 |
| 6 | 3+2 mod 1, op 4 separate carrier | `[0b0110, 0, 0, 0]` | op 0, op 3 |
| 7 | All four parallel (additive) | `[0, 0, 0, 0]` | op 0, op 1, op 2, op 3 |
| 8 | 4→1, 3→2, both carriers (paired) | `[0b1000, 0b0100, 0, 0]` | op 0, op 1 |

These are picked for variety — algorithm 1 is the canonical FM stack
(modulator-to-carrier), algorithm 7 is pure additive synthesis, and the
rest fall between. They will be tuned during M7.1 implementation if any
sound unmusical.

### Per-sample evaluation

Evaluation order is fixed: op 3 → op 2 → op 1 → op 0. Operators with
higher index can modulate operators with lower index but not vice
versa — this is sufficient for all 8 algorithms above and avoids the
need for dependency sorting at runtime.

```text
out_3 = sin(2π × (phase_3 + feedback_amount × prev_out_3)) × env_3 × level_3
out_2 = sin(2π × (phase_2 + Σ out_j for j in mod_sources[2])) × env_2 × level_2
out_1 = sin(2π × (phase_1 + Σ out_j for j in mod_sources[1])) × env_1 × level_1
out_0 = sin(2π × (phase_0 + Σ out_j for j in mod_sources[0])) × env_0 × level_0

slot_mono = Σ out_i for i where is_carrier[i]
```

### Oversampling

Each FM slot runs 2× oversampled, always (not conditional on modulation
index). The simpler rule keeps CPU cost flat and predictable. Strategy:

1. The slot internally advances at `2 × sample_rate_hz`.
2. Per output sample at base rate, the slot computes two FM samples.
3. A symmetric FIR half-band filter (31 taps, ≈16 multiplies after
   exploiting zero coefficients) low-passes the upsampled stream.
4. The decimated output feeds the slot mix → filter.

Operator envelopes and ratios are sampled at base rate (block-rate
parameter smoothing already handles audio-rate updates); only the
oscillator phases run at 2× rate. This keeps the cost of oversampling
to roughly 2× the per-operator phase + sin work.

If CPU profiling at M7.5 close-out shows the half-band filter is the
bottleneck, swap to a polyphase IIR half-band (cheaper but with mild
phase distortion in the passband — acceptable for FM where phase
linearity isn't audible).

### Parameter representation

Indexed tuple variants in `ParamId` to keep the bus tidy:

```rust
// Slot config
SlotMode(u8),               // index 0..=1; value 0 = subtractive, 1 = FM
SlotLevel(u8),              // 0..=1
SlotPan(u8),                // -1..=1

// FM per-operator (operator index encoded high-nibble, slot index low-nibble)
FmOpRatioInteger(u8),       // packed (slot << 4) | op; value 0..=15
FmOpRatioFine(u8),          // value -100..=100
FmOpLevel(u8),              // 0..=1
FmOpAttackSecs(u8),
FmOpDecaySecs(u8),
FmOpSustainLevel(u8),
FmOpReleaseSecs(u8),
FmOpFeedback(u8),           // op 3 only; 0 for others (no-op)

// Algorithm
FmAlgorithm(u8),            // slot index; value 0..=7
```

Packing slot and op indices into a single `u8` keeps the ParamId enum
small. The decoder is:

```rust
let slot = (packed >> 4) & 0x0F;
let op   =  packed       & 0x0F;
```

### Hybrid voice integration

`Voice` gains:

```rust
slots: [Slot; 2],
```

and replaces the current `main_oscillators` / `sub_oscillator` /
`filter_l` / `filter_r` fields. The filter stays once per voice, fed by
the *sum* of `slot[0].next_sample() + slot[1].next_sample()` (after
per-slot level and pan).

The 8 amp envelopes (4 ops × 2 slots) are independent of the voice's
single amp envelope, which still gates the final output. Operator
envelopes shape the FM timbre over time but do not control the voice's
amp envelope.

---

## Sub-milestones

### M7.0 — Slot refactor (no behaviour change)

Extract the current 3-osc + sub subtractive bank into a `SubtractiveBank`
struct in a new `slot.rs`. Voice gains `slots: [Slot; 2]`. Slot 0 keeps
the current subtractive defaults; slot 1 defaults to subtractive at
level = 0 (silent). All existing M2/M3 tests continue to pass without
modification.

**Files:**
- `crates/synth-engine/src/slot.rs` (new) — `Slot`, `SlotMode`, `SubtractiveBank`
- `crates/synth-engine/src/voice.rs` — replace ops/sub fields with `slots: [Slot; 2]`
- `crates/synth-engine/src/voice_manager.rs` — re-wire osc-related setters
  to operate on slot 0 only (preserves existing `Osc1*` parameter names)

**Done when:** every test from M0–M6 still passes; subtractive sound is
unchanged when slot 0 is the only audible slot.

---

### M7.1 — Operator + algorithm data types

New file `crates/synth-engine/src/fm.rs`:

```rust
pub struct Operator { … }
pub const ALGORITHMS: [Algorithm; 8] = [ … ];
pub struct FmBank { operators: [Operator; 4], algorithm_index: u8 }
impl FmBank {
    pub fn next_sample(&mut self, base_phase_hz: f32) -> f32 { … }
}
```

Unit tests for:
- Each algorithm's mod_sources mask and carrier set
- Operator phase wraps correctly
- Single-carrier algorithm at zero modulation = pure sine at base frequency
- Self-feedback on op 3 does not blow up at maximum amount

**Files:** `crates/synth-engine/src/fm.rs` (new), `lib.rs` re-exports

**Done when:** `cargo test fm::` passes; no integration with Voice yet.

---

### M7.2 — FmBank as a slot mode

Wire `FmBank` into the `Slot` struct from M7.0. Switching a slot's
`mode` to `Fm` causes its per-sample call to invoke `FmBank::next_sample`
instead of `SubtractiveBank::next_sample`. The slot's level and pan
apply uniformly to whichever bank is active.

Add a smoke test in `voice.rs`:
- Set slot 1 to FM, algorithm 0, default operator ratios/levels
- Play a note, process 4096 samples
- Assert the audio is non-zero and bounded

**Files:** `crates/synth-engine/src/slot.rs`, `voice.rs`, `voice_manager.rs`

**Done when:** a hardcoded FM patch produces sound through the voice's
filter and amp envelope; no UI yet.

---

### M7.3 — Parameter bus + UI surface

**`params.rs`:** add all new `ParamId` variants from the Design Decisions
section. Snapshot fields for slot mode/level/pan and all FM op params.

**`engine.rs`:** fan-out for new variants to `VoiceManager`.

**`voice_manager.rs`:** `set_slot_mode`, `set_slot_level`, `set_slot_pan`,
`set_fm_op_*`, `set_fm_algorithm`.

**UI (`app.rs`):** new FM panel beside the existing osc1 panel. Layout:

- Slot mode toggle (Subtractive / FM) per slot
- For FM mode: algorithm picker (combo 0..=7), 4 operator rows each
  with ratio (integer + fine), level, ADSR knobs, and feedback (op 3 only)

Polish deferred to M11.

**Done when:** every FM parameter is reachable from the UI and changing
it audibly affects the sound.

---

### M7.4 — 2× oversampling

Implement the half-band FIR in `crates/synth-engine/src/halfband.rs`.
Per-slot oversampling state lives inside `FmBank`:

```rust
upsampled_scratch: [f32; OVERSAMPLE_BLOCK_MAX * 2],
decim_filter: HalfBand,
```

The slot's per-sample call computes two FM samples, feeds them through
the half-band filter, and emits the decimated sample.

**Done when:** at high modulation index (op 3 level = 1.0, ratio = 8,
feedback = 0.7, into op 0 carrier), aliasing artefacts are inaudible
in the output spectrum. Sanity check: spectrum FFT shows no significant
energy reflected above Nyquist.

---

### M7.5 — Close-out / verification

- Play each of the 8 algorithms in isolation — sounds match the
  hand-written description in the algorithm table.
- Hybrid patch: slot 0 subtractive saw, slot 1 FM algorithm 1
  (DX7 stack). Both audible, both panable, both shaped by the single
  voice amp envelope.
- 32-note chord stress test: CPU stays below 50%.
- Mod matrix can address slot 0 vs slot 1 mix levels (rough sanity —
  full FM modulation targets are M11 polish).
- `cargo fmt`, `cargo clippy -D warnings`, full test suite clean.

---

## What M7 does NOT include

- Per-FM-operator level as a mod matrix destination (M11 polish).
- User-editable algorithm routing (v1.2).
- Operator-keyboard scaling / level scaling curves (out of scope; v1.1+).
- Fixed-frequency operator mode (v1.2).
- Operator phase initialisation patterns (always reset to 0 on retrigger).
- Per-slot filter (single filter per voice; v1.1).
