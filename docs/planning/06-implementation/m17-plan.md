# M17 plan — Engine expansion

Four tightly related v1.1 engine additions. Each is self-contained enough to commit separately,
but they're grouped into one milestone because they all touch the same layers (engine voice,
mod matrix, param system, preset serialization, UI) and the "done when" criterion requires all
four working together.

**Target version:** v1.1  
**Estimate:** 4–6 weeks  
**Branch:** `milestone/m17-engine-expansion`

---

## Overview

| Phase | Feature | Key constraint |
|---|---|---|
| 1 | Mod matrix 8 → 16 slots | Must land before Env3; gives the new source somewhere to route |
| 2 | Env3 (second mod envelope) | Needs Phase 1 done; otherwise identical to Env2 |
| 3 | Second filter + serial/parallel routing | Independent; large voice change |
| 4 | 24 dB/oct slope option | Builds on Phase 3; adds a slope field to both filters |

Phase 1 first (it's the smallest and the other three all benefit from having more matrix slots).
Phases 3 and 4 can be developed together since they both live in `filter/`.

---

## Phase 1 — Mod matrix 8 → 16 slots

### Background

The matrix is currently sized at 8 slots in every layer: `ModMatrix`, `ParamSnapshot`,
`ParameterTree`, the UI section, and preset serialization. The magic number `8` is scattered
across several files — there is no shared constant. The `ParamId::ModSlot*(u8)` variants already
take an arbitrary `u8` index, so no new variants are needed; only the array sizes change.

### Changes

**`crates/synth-engine/src/mod_matrix.rs`**

- Add `pub const MOD_MATRIX_SLOTS: usize = 16;` near the top of the file.
- Change `pub slots: [ModSlot; 8]` → `[ModSlot; MOD_MATRIX_SLOTS]` in `ModMatrix`.
- Update `Default` impl: `slots: [ModSlot::default(); MOD_MATRIX_SLOTS]`.
- Re-export the constant from `synth-engine/src/lib.rs` so downstream crates can use it.

**`crates/synth-engine/src/params/snapshot.rs`**

- Import `MOD_MATRIX_SLOTS` from `crate::mod_matrix`.
- Change all five `mod_slot_*: [T; 8]` fields to `[T; MOD_MATRIX_SLOTS]`.
- Update `Default` init to use `MOD_MATRIX_SLOTS` in the array literals.

**`crates/synth-engine/src/params/tree.rs`**

- Import `MOD_MATRIX_SLOTS`.
- Change the five `mod_slot_*: [T; 8]` fields and their defaults to `MOD_MATRIX_SLOTS`.

**`crates/synth-presets/src/preset_params.rs`**

- Import `MOD_MATRIX_SLOTS` from `synth_engine`.
- Change `for i in 0..8usize` loops (serialization and deserialization) to
  `for i in 0..MOD_MATRIX_SLOTS`.
- Change `for i in 0..8u8` (param routing) similarly.
- Old presets with only 8 slots deserialize cleanly: slots 8–15 stay at their `Default` values
  (disabled, source=Off, dest=FilterCutoffHz, amount=0, via=Off). No migration needed.

**`crates/synth-ui/src/app/state.rs`**

- Import `MOD_MATRIX_SLOTS`.
- Change the five `mod_slot_*: [T; 8]` fields and their defaults/sync sites.

**`crates/synth-ui/src/sections/modulation.rs`**

- Import `MOD_MATRIX_SLOTS`.
- Remove the static `[&str; 8]` key arrays — replace with inline `format!()` calls or a
  dynamic array sized by `MOD_MATRIX_SLOTS`. The static approach requires touching the const
  anyway; switching to `format!()` is simpler and the render loop handles any size.
- Change `for i in 0..8usize` to `for i in 0..MOD_MATRIX_SLOTS`.
- The UI grid grows from 8 rows to 16; a `ScrollArea` wrapper may be needed if the panel height
  is fixed. Check whether the existing grid already scrolls or whether it clips.

### Done when

- Matrix has 16 slots; all 16 visible and editable in the UI.
- A preset saved with 16 active slots reloads all 16 correctly.
- `cargo fmt` and `cargo clippy -D warnings` clean.

---

## Phase 2 — Env3 (second mod envelope)

### Background

`ModEnv` (`crates/synth-engine/src/mod_env.rs`) already implements the ADSR-with-curve logic used
by Env2. Env3 is a second independent instance of the same type wired up in parallel. The only
new code is the parameter plumbing and the extra `ModEnv` field on `Voice`.

### Changes

**`crates/synth-engine/src/params/ids.rs`**

Add seven new variants after the Env2 block:

```rust
// ── Env3 (second modulation envelope) ─────────────────────────────────
Env3AttackSecs,
Env3DecaySecs,
Env3SustainLevel,
Env3ReleaseSecs,
Env3AttackCurve,
Env3DecayCurve,
Env3ReleaseCurve,
```

**`crates/synth-engine/src/params/snapshot.rs`**

Add after the Env2 mirror block:

```rust
// ── Env3 parameter mirrors ─────────────────────────────────────────
pub env3_attack_secs: f32,
pub env3_decay_secs: f32,
pub env3_sustain_level: f32,
pub env3_release_secs: f32,
pub env3_attack_curve: f32,
pub env3_decay_curve: f32,
pub env3_release_curve: f32,
```

And in the live outputs block:

```rust
pub env3_out: f32,
```

Defaults: same as Env2 (0.010 / 0.200 / 0.8 / 0.200 / 0.0 / 0.0 / 0.0; `env3_out: 0.0`).

**`crates/synth-engine/src/params/tree.rs`**

- Add seven Env3 fields (matching Env2's layout) and their `Default` values.
- Add `ParamId::Env3*` match arms in `apply_event()` and `to_snapshot()`.

**`crates/synth-engine/src/voice.rs`**

- Add `mod_env3: ModEnv` field to `Voice`.
- Initialise it with the same defaults as `mod_env` in `Voice::new()`.
- In the per-sample loop, advance `mod_env3` alongside `mod_env` (same note-on/off hooks).
- Add `env3_out` to the block snapshot: `self.last_env3_out = ...` (or however Env2 does it).

**`crates/synth-engine/src/mod_matrix.rs`**

- Add `Env3` to `ModSource` enum (after `Env2`).
- Update `ModSource::COUNT` from 10 to 11.
- Add `env3: f32` to `ModSources`.
- Handle `ModSource::Env3 => self.env3` in `get()`.
- Update `from_index` / `to_index` (Env3 gets the next sequential index after the current 9).

**`crates/synth-presets/src/preset_params.rs`**

- Add `env3_*` keys to `snapshot_to_map()` (serialization).
- Add `ParamId::Env3*` to the param routing block.
- Add `get_f32!(env3_*)` calls in `snapshot_from_map()` (deserialization).

**`crates/synth-ui/src/app/state.rs`**

- Add seven `env3_*` mirror fields (matching the Env2 mirrors).
- Sync them in `sync_from_snapshot()`.

**`crates/synth-ui/src/sections/`** (whichever file renders Env2)

- Add an Env3 panel alongside Env2 — same knob layout (A/D/S/R + curve knobs).
- The two envelopes can share a tab with two side-by-side groups, or Env3 gets its own small
  header. Check the existing tab layout to decide.

**`crates/synth-ui/src/sections/modulation.rs`**

- Add `"Env3"` to `MOD_SOURCE_LABELS` at the index matching `ModSource::Env3::to_index()`.

### Done when

- Env3 advances independently of Env2.
- Routing Env3 → filter cutoff with a long attack produces an audible sweep distinct from Env2.
- Env3 parameters survive a preset round-trip.
- `cargo fmt` and `cargo clippy -D warnings` clean.

---

## Phase 3 — Second filter + serial/parallel routing

### Background

`Voice` currently holds `filter_l: StateVariableFilter` and `filter_r: StateVariableFilter`
(one per channel, 12 dB/oct SVF). Adding a second filter means two more SVF instances and a
routing switch. The signal flow changes from:

```
slot mix → F1 → amp
```

to one of:

```
slot mix → F1 → F2 → amp          (serial)
slot mix → (F1 + F2) × 0.5 → amp  (parallel, equal-power average to preserve level)
```

F1 and F2 are fully independent: each has its own cutoff, resonance, mode, and (after Phase 4)
slope. Mod matrix can address both.

### New types

**`crates/synth-engine/src/filter/mod.rs`**

Add:

```rust
/// How the two per-voice filters are connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterRouting {
    #[default]
    Serial,    // F1 feeds F2
    Parallel,  // F1 and F2 run in parallel; outputs averaged
}
```

Add a `SetFilterRouting(FilterRouting)` event variant in `events.rs`.

### New `ParamId` variants

Append to `ids.rs`:

```rust
// ── Filter 2 ──────────────────────────────────────────────────────────
Filter2CutoffHz,
Filter2Resonance,
```

Routing and mode use typed events (`SetFilterRouting`, `SetFilter2Mode`) rather than `f32`
parameters, matching how `SetOscillatorWaveform` and `SetFilterMode` work today.

### New event variants (`events.rs`)

```rust
/// Sets the filter routing between serial and parallel.
SetFilterRouting { routing: FilterRouting },
/// Sets the mode (LP/HP/BP/Notch) on filter 2.
SetFilter2Mode { mode: FilterMode },
```

### Changes

**`crates/synth-engine/src/voice.rs`**

- Add `filter2_l: StateVariableFilter`, `filter2_r: StateVariableFilter`.
- Add `filter_routing: FilterRouting` field.
- In `Voice::new()`: init filter2 with the same defaults as filter1.
- In the per-sample processing:

  ```rust
  let (out_l, out_r) = match self.filter_routing {
      FilterRouting::Serial => {
          let f1_l = self.filter_l.process_sample(slot_mix_l, sp);
          let f1_r = self.filter_r.process_sample(slot_mix_r, sp);
          let f2_l = self.filter2_l.process_sample(f1_l, sp2);
          let f2_r = self.filter2_r.process_sample(f1_r, sp2);
          (f2_l, f2_r)
      }
      FilterRouting::Parallel => {
          let f1_l = self.filter_l.process_sample(slot_mix_l, sp);
          let f1_r = self.filter_r.process_sample(slot_mix_r, sp);
          let f2_l = self.filter2_l.process_sample(slot_mix_l, sp2);
          let f2_r = self.filter2_r.process_sample(slot_mix_r, sp2);
          ((f1_l + f2_l) * 0.5, (f1_r + f2_r) * 0.5)
      }
  };
  ```

  where `sp2` is a `SampleParams`-like struct carrying filter2's smoothed cutoff/resonance.
  The simplest approach: add `filter2_cutoff_hz` and `filter2_resonance` as smoothed fields to
  `SampleParams`; they're consumed by the voice just like the existing filter fields.

- Handle `SetFilterRouting` and `SetFilter2Mode` in `Voice::set_mode()` / a new dispatch method.

**`crates/synth-engine/src/params/snapshot.rs`**

Add:

```rust
pub filter2_cutoff_hz: f32,
pub filter2_resonance: f32,
pub filter2_mode: FilterMode,
pub filter_routing: FilterRouting,
```

Defaults: `filter2_cutoff_hz: DEFAULT_FILTER_CUTOFF_HZ`, `filter2_resonance: DEFAULT_FILTER_RESONANCE`,
`filter2_mode: FilterMode::LowPass`, `filter_routing: FilterRouting::Serial`.

**`crates/synth-engine/src/params/tree.rs`**

- Add smoothed `filter2_cutoff_hz` and `filter2_resonance`.
- Add stepped `filter2_mode` and `filter_routing`.
- Handle the two new `EngineEvent` variants.

**`crates/synth-engine/src/mod_matrix.rs`**

Add to `ModDest`:

```rust
Filter2CutoffHz,
Filter2Resonance,
```

Update `ModDest::COUNT` (6 → 8). Update `from_index`, `to_index`, `compute_offsets`, and
`DestOffsets`.

**`crates/synth-presets/src/preset_params.rs`**

- Serialize `filter2_cutoff_hz`, `filter2_resonance`, `filter2_mode`, `filter_routing`.
- Deserialize same; old presets default to `filter2_cutoff_hz = 20000.0` (open) so they sound
  identical to before.

**`crates/synth-ui/src/sections/filter.rs`** (or wherever filter UI lives)

- Add a "Filter 2" group below Filter 1: cutoff, resonance, mode — same layout.
- Add a routing selector: "Series" / "Parallel" toggle at the top of the filter panel.
- The mod-destination dropdowns in `modulation.rs` gain two new entries: "F2 Cutoff", "F2 Res".

### Done when

- A patch can use both filters in series (F1 LP 500 Hz → F2 HP 200 Hz) and parallel
  (F1 LP + F2 HP blending).
- Filter 2 cutoff is mod-matrix addressable.
- Settings survive a preset round-trip.
- `cargo fmt` and `cargo clippy -D warnings` clean.

---

## Phase 4 — 24 dB/oct slope option

### Background

The `StateVariableFilter` is a 2-pole TPT design (12 dB/oct). A 24 dB/oct option can be
implemented by cascading two 2-pole SVFs. The cleanest approach is to carry the second-pass state
inside `StateVariableFilter` itself, enabled by a `slope` field, so the voice code stays
unchanged.

### New type

**`crates/synth-engine/src/filter/mod.rs`**

```rust
/// Slope of the filter rolloff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterSlope {
    #[default]
    TwelveDboct,     // 2-pole; existing behaviour
    TwentyFourDboct, // 4-pole via cascaded 2-pole
}
```

### Changes

**`crates/synth-engine/src/filter/svf.rs`**

- Add `slope: FilterSlope` and a second set of integrators (`ic1_2`, `ic2_2`, `k_2: f32`) for the
  second cascade.
- In `process_sample()`:
  ```rust
  let after_first_pass = /* existing SVF calculation */;
  if self.slope == FilterSlope::TwentyFourDboct {
      // Run a second SVF pass with the same cutoff/resonance.
      // Use a reduced resonance for the second pass (self.k * 0.5) to
      // avoid double-peaking at high Q settings.
      /* second SVF calculation on after_first_pass */
  } else {
      after_first_pass
  }
  ```
- Add `pub fn set_slope(&mut self, slope: FilterSlope)`. `StateVariableFilter::new()` defaults
  to `FilterSlope::TwelveDboct`.
- The second-pass integrators are zeroed on `reset()`.

Both `filter_l` / `filter_r` (Filter 1) and `filter2_l` / `filter2_r` (Filter 2) are
`StateVariableFilter` instances and pick up the slope support automatically.

**`crates/synth-engine/src/events.rs`**

Add:

```rust
/// Sets the slope (12 or 24 dB/oct) on the specified filter.
SetFilterSlope { filter_idx: u8, slope: FilterSlope },
```

**`crates/synth-engine/src/voice.rs`**

Handle `SetFilterSlope` by calling `set_slope()` on all four SVF instances for the given
`filter_idx` (0 = F1, 1 = F2).

**`crates/synth-engine/src/params/snapshot.rs`**

Add:

```rust
pub filter_slope: [FilterSlope; 2],  // index 0 = F1, 1 = F2
```

Default: `[FilterSlope::TwelveDboct; 2]`.

**`crates/synth-presets/src/preset_params.rs`**

- Serialize `filter_slope_0` / `filter_slope_1` as 0.0/1.0.
- Deserialize; old presets default to 0.0 (12 dB/oct = existing behaviour).

**`crates/synth-ui/src/sections/filter.rs`**

- Add a "12 dB" / "24 dB" toggle for each filter (alongside its mode selector).

### Done when

- 24 dB/oct produces a noticeably steeper rolloff than 12 dB/oct on the same patch.
- High-resonance 24 dB/oct does not produce runaway self-oscillation from second-pass accumulation.
- Both filters independently switchable between slopes.
- Slope setting survives preset round-trip.
- `cargo fmt` and `cargo clippy -D warnings` clean.

---

## Milestone done when

A patch can route Env3 through two filters in series at 24 dB/oct with all 16 mod slots in use;
all paths audio-tested and round-trip serialised in the preset format.

Specifically:
1. Mod matrix shows 16 rows; slots 9–16 assignable.
2. Env3 available as a mod source; independent of Env2.
3. Both filters independently controllable; serial/parallel routing audibly distinct.
4. 24 dB/oct steeper than 12 dB/oct on representative patches.
5. Full preset round-trip for all new parameters.
6. All 4 phases pass `cargo fmt --check` and `cargo clippy -D warnings`.

---

## Files touched (summary)

| File | Phase(s) |
|---|---|
| `crates/synth-engine/src/mod_matrix.rs` | 1, 2 |
| `crates/synth-engine/src/params/ids.rs` | 2, 3 |
| `crates/synth-engine/src/params/snapshot.rs` | 1, 2, 3, 4 |
| `crates/synth-engine/src/params/tree.rs` | 1, 2, 3 |
| `crates/synth-engine/src/events.rs` | 3, 4 |
| `crates/synth-engine/src/engine.rs` | 3, 4 |
| `crates/synth-engine/src/voice.rs` | 2, 3, 4 |
| `crates/synth-engine/src/filter/mod.rs` | 3, 4 |
| `crates/synth-engine/src/filter/svf.rs` | 4 |
| `crates/synth-presets/src/preset_params.rs` | 1, 2, 3, 4 |
| `crates/synth-ui/src/app/state.rs` | 1, 2, 3 |
| `crates/synth-ui/src/sections/modulation.rs` | 1, 2, 3 |
| `crates/synth-ui/src/sections/filter.rs` | 3, 4 |
| `crates/synth-ui/src/sections/` (envelopes) | 2 |
| `crates/synth-engine/src/lib.rs` | 1 |

---

## Progress

- [ ] Phase 1 — Mod matrix 8 → 16 slots
- [ ] Phase 2 — Env3
- [ ] Phase 3 — Second filter + routing
- [ ] Phase 4 — 24 dB/oct slope
