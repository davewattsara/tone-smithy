# M23 plan — Oscillator phase consistency

Two coupled changes. Part 1 (per-oscillator phase mode) is independent and backward-compatible.
Part 2 (default OSC2/3 level change) requires a factory preset migration and must be done
carefully — the migration runs first, then the default is changed.

**Target version:** v1.2
**Estimate:** 2–3 weeks
**Branch:** `milestone/m23-osc-phase`

---

## Overview

| Phase | Feature | Risk |
|---|---|---|
| 1 | Per-oscillator phase mode (Free / Retrig) | Low — backward-compatible new param |
| 2 | `xtask migrate-osc-defaults` audit tool | Medium — must correctly identify affected presets |
| 3 | Default OSC2/3 level 1.0 → 0.0 | High — must run after Phase 2 migration is verified |

**Order is strict:** complete and verify Phase 2 (migration audit) before committing Phase 3
(the default change). Committing Phase 3 without a complete migration will silently mute
oscillators in factory presets.

---

## Phase 1 — Per-oscillator phase mode

### Background

Currently all three main oscillators use random phase on note-on (the "Free" behaviour).
This adds a `Free` / `Retrig` toggle per oscillator. `Retrig` resets the oscillator phase
to 0 on every note-on, producing a deterministic, tight attack — useful for punchy basses,
plucks, and any patch where phase variation between notes is unwanted.

The sub oscillator is excluded for now (sub oscillators rarely benefit from phase retrigger
and adding four toggles rather than three keeps the UI simpler).

### New params — `crates/synth-engine/src/params/ids.rs`

Append:

```rust
Osc1PhaseMode,   // 0.0 = Free (random), 1.0 = Retrig (reset to 0)
Osc2PhaseMode,
Osc3PhaseMode,
```

These are plain `f32` params treated as booleans (> 0.5 = Retrig). Default: `0.0` (Free).
Old presets omit the keys → `0.0` → unchanged behaviour.

### Snapshot / tree — `snapshot.rs`, `tree.rs`

Add:

```rust
pub osc1_phase_mode: f32,   // defaults 0.0
pub osc2_phase_mode: f32,
pub osc3_phase_mode: f32,
```

Handle `ParamId::Osc{1,2,3}PhaseMode` in `apply_event` and `to_snapshot`.

### Engine — `crates/synth-engine/src/voice.rs` or oscillator module

On note-on, for each main oscillator, check the phase mode before randomising:

```rust
fn note_on(&mut self, note: u8, velocity: u8, params: &ParamSnapshot) {
    // existing note-on logic …
    if params.osc1_phase_mode > 0.5 {
        self.osc1.set_phase(0.0);
    } else {
        self.osc1.set_phase(rng.gen());
    }
    // same for osc2 and osc3
}
```

If the oscillator doesn't expose `set_phase` yet, add it. The phase field is already there
(it advances every sample); resetting it to `0.0` is a one-line change.

### Presets — `crates/synth-presets/src/preset_params.rs`

Add `osc1_phase_mode`, `osc2_phase_mode`, `osc3_phase_mode` to `snapshot_to_map` and
`snapshot_from_map`. Values are `0.0` / `1.0`. Old presets omit → `get_f32!` default of
`0.0` (Free) → no change.

### UI — `crates/synth-ui/src/sections/osc.rs`

Add a "Retrig" toggle per oscillator column, below or beside the existing controls. Same
style as the LFO `Reset` toggle. Label: "Retrig". Tooltip: "Reset oscillator phase to 0 on
every note-on for a tighter, more consistent attack."

Emit `ParamId::Osc{1,2,3}PhaseMode` with value `0.0` or `1.0`.

### Done when

- `Retrig` on OSC 1: consecutive identical notes produce identical waveform starts (no
  phase variation between notes). Verify by playing a percussive patch and listening for
  consistent attack transients.
- `Free` (default): behaviour unchanged from v1.1.
- All three presets round-trip: `osc1_phase_mode = 1.0` reloads as Retrig.
- `fmt` / `clippy -D warnings` clean.

---

## Phase 2 — Migration audit (`xtask migrate-osc-defaults`)

### Why this phase comes before the default change

Preset files are *sparse*: `snapshot_to_map` only writes a key when its value differs from
the default at save time. Currently `osc2_level` and `osc3_level` default to `1.0`, so any
preset where they happen to be `1.0` omits them from the file. After Phase 3 changes the
default to `0.0`, those same presets will load with `osc2_level = 0.0` — silently muting
OSC 2 and OSC 3.

The migration must write `osc2_level = 1.0` and `osc3_level = 1.0` explicitly into every
factory preset where these keys are currently absent **and the slot is in Sub mode**.

FM-mode slots are immune: when a slot is in FM mode, the subtractive oscillator levels do
nothing. Only Sub-mode slots need the explicit write-in.

### New xtask subcommand

Add `cargo xtask migrate-osc-defaults` in `xtask/src/main.rs` (or a new module
`xtask/src/migrate.rs`). The subcommand:

1. Reads every `.tsmith` file under `crates/synth-presets/src/factory/` (the directory
   where factory presets are compiled in via `factory.rs`).
2. Parses the RON into a `HashMap<String, ron::Value>` (reuse the existing `snapshot_from_map`
   deserialization path, or parse the RON directly).
3. For each preset, checks `slot_mode` for each slot (key `slot0_mode` / `slot1_mode`;
   value `0.0` = Sub, `1.0` = FM):
   - If slot mode is Sub (or absent, defaulting to Sub): check if `osc2_level` / `osc3_level`
     are absent from the map.
   - If absent: add them at `1.0` and mark the file as needing a rewrite.
   - If FM mode: skip — the osc level keys are irrelevant.
4. Rewrites the modified files in-place, preserving all other keys and the existing
   RON formatting as closely as possible.
5. Prints a summary: how many files were checked, how many were modified, which keys were
   added to which files.

**Dry-run flag:** `cargo xtask migrate-osc-defaults --dry-run` prints the summary without
writing files. Run dry-run first, review the output, then run without the flag.

### Verification

After the migration:

1. Run `cargo test` — the existing round-trip property test should still pass.
2. Load a selection of factory presets in the running app and confirm they sound identical
   to before the migration.
3. Run `cargo xtask migrate-osc-defaults --dry-run` again — it should report 0 files to
   modify (idempotent).

### Done when

- Dry-run produces a plausible list of affected presets (expect ~20–40 of 120 to need the
  write-in, depending on how many are subtractive with default osc levels).
- After a real run, `--dry-run` reports 0 files remaining.
- Factory presets sound identical in the app before and after the migration.
- Round-trip test still passes.

---

## Phase 3 — Change default OSC2/3 level to 0.0

**Prerequisite: Phase 2 migration verified (dry-run shows 0 files remaining).**

### Engine default change

In `crates/synth-engine/src/params/tree.rs` (or wherever the `ParameterTree` defaults are
defined), change the default for OSC 2 and OSC 3 level from `1.0` to `0.0`:

```rust
// Before:
osc2_level: 1.0,
osc3_level: 1.0,

// After:
osc2_level: 0.0,
osc3_level: 0.0,
```

Apply the same change in `snapshot.rs` defaults and any hardcoded init-patch values.

### Init patch

The init patch (the blank patch used on "New" or the factory `_init.tsmith` if one exists)
should now default to a single active oscillator (OSC 1 only). If the init patch is
hardcoded rather than loaded from a file, update the hardcoded defaults to match:
`osc1_level = 1.0`, `osc2_level = 0.0`, `osc3_level = 0.0`.

### Verification

After the change:

- Load the init patch: only OSC 1 is audible. OSC 2 and OSC 3 produce no sound at their
  default level of 0.
- Load every factory preset: all sound identical to before (confirming the migration was
  complete).
- Old user presets that omit `osc2_level` / `osc3_level`: these will now default to `0.0`.
  This is a **user-facing breaking change for user presets** — mention it in the CHANGELOG
  as a known consequence. (Factory presets are safe because Phase 2 wrote them in explicitly.)

### CHANGELOG note

Add to the v1.2 CHANGELOG entry:

> **Init patch now starts with one oscillator.** OSC 2 and OSC 3 default to level 0, matching
> the universal convention. Existing factory presets are unaffected. User-created presets that
> relied on OSC 2/3 being at full level by default may sound different — open them, set OSC 2/3
> levels explicitly, and re-save.

### Done when

- Init patch is one oscillator.
- All 120 factory presets sound identical before and after.
- `fmt` / `clippy -D warnings` clean.

---

## Milestone done when

1. `Retrig` mode produces a consistent, click-free attack; `Free` is unchanged.
2. Phase mode survives preset round-trips.
3. Migration audit reports 0 files remaining after the run.
4. All 120 factory presets sound identical with the new default.
5. Init patch plays with one oscillator.
6. CHANGELOG carries the user-preset breaking-change note.
7. All phases pass `cargo fmt --check` and `cargo clippy -D warnings`.

---

## Progress

- [x] Phase 1 — Per-oscillator phase mode
- [x] Phase 2 — Migration audit (`xtask migrate-osc-defaults`)
  - 8 presets migrated (fewer than the 20-40 estimate). The estimate
    predated the current slot-mix model: the main osc levels feed slot 0
    (subtractive) only, and 26 of the omitting presets are pure-FM
    (`slot_level_0 == 0`), so the osc2/osc3 default change is inaudible
    for them and they are correctly left untouched. `init.tsmith` (empty
    param map) is skipped so Phase 3 can turn it single-oscillator.
- [x] Phase 3 — Default OSC2/3 level 1.0 → 0.0 (run only after Phase 2 verified)
  - Sub oscillator default intentionally left at 1.0 (user decision): the
    init patch is one *main* oscillator (OSC 1) + the sub, not a literal
    single oscillator. Zeroing the sub default would have silently muted
    ~54 sub-audible factory presets that omit `sub_level`, which is out of
    this milestone's scope.
