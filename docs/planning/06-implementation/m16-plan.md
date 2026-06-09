# M16 plan — Quick wins

Three self-contained UX improvements that can land back-to-back before the heavier v1.1 engine
work begins. Each is independent and can be committed separately.

**Target version:** v1.1  
**Estimate:** 1–3 days  
**Branch:** `milestone/m16-quick-wins`

---

## Phase 1 — K=C keyboard note

**Goal:** Add K as the 13th computer-keyboard note: C one octave above J=B.

### Background

The current `KEY_LAYOUT` in `crates/synth-ui/src/computer_keyboard.rs` maps 12 keys
(A=C through J=B — one full octave). Adding K=C (semitone offset 12) completes the octave span
so the top note of the range is reachable without shifting octave.

`OCTAVE_BASE_MAX = 96` already keeps the highest mapped key (J=B = 96+11=107) inside the valid
MIDI range. With K added, the new ceiling note is 96+12=108 — still inside 0–127, so no clamp
change is needed.

### Changes

**`crates/synth-ui/src/computer_keyboard.rs`**

- Change `const NUM_KEYS: usize = 12;` → `13`.
- Append `(egui::Key::K, 12),  // C (octave above J)` to `KEY_LAYOUT`.
- Update the module doc comment layout ASCII art and the key-note list to include K.
- Update the comment on `OCTAVE_BASE_MAX` (currently says "J key — an octave plus a B above the
  base"; update to include the K key — "the K key is a C one octave above that: 96+12=108").
- Update the existing test `assert_eq!(note_for(egui::Key::J, DEFAULT_OCTAVE_BASE), Some(59))` to
  add a corresponding assertion for K: `assert_eq!(note_for(egui::Key::K, DEFAULT_OCTAVE_BASE), Some(60))`.
- Check that the virtual keyboard highlight loop (`KEY_LAYOUT.iter().enumerate()`) does not need
  changes — because `K` is just appended to the same array it will be highlighted automatically.

**`docs/user-manual.md`**

Update the computer keyboard layout diagram and the key-note reference table to include K=C.

The ASCII piano diagram currently:

```
     W   E       T   Y   U
   A   S   D   F   G   H   J
```

Should become:

```
     W   E       T   Y   U
   A   S   D   F   G   H   J   K
```

(K is a white key — no black key between J=B and K=C.)

Also update the prose: "A through K" instead of "A through J".

### Done when

- K plays C60 (one octave above J=B at default octave base).
- Virtual keyboard highlights K when pressed.
- New test assertion passes.
- `cargo fmt` and `cargo clippy` both clean.

---

## Phase 2 — Alphabetical preset ordering

**Goal:** Preset browser lists presets A-Z by name within the factory and user groups.

### Background

`refresh_preset_list()` in `crates/synth-ui/src/app/state.rs:478` builds `preset_entries` by
calling `factory_entries()` then extending with `scan_dir()`. No sort is applied; the factory
presets arrive in embed declaration order, user presets in filesystem order.

The browser splits the list into factory / user at display time
(`crates/synth-ui/src/sections/browser.rs:68`) using a `partition`/filter on `is_factory`.

### Changes

**`crates/synth-ui/src/app/state.rs`** — in `refresh_preset_list()`, after building `entries`:

```rust
// Sort each group alphabetically by preset name.
entries.sort_by(|a, b| {
    // Factory before user, then A-Z within each group.
    a.is_factory
        .cmp(&b.is_factory)
        .reverse()
        .then_with(|| a.metadata.name.to_lowercase().cmp(&b.metadata.name.to_lowercase()))
});
```

This keeps factory presets listed before user presets (matching the current display order) and
sorts each group A-Z case-insensitively.

Check whether `PresetEntry` has an `is_factory` field, or whether the factory / user split is done
by another means. If the field name differs, adjust accordingly.

### Done when

- Factory presets appear A-Z in the browser.
- User presets appear A-Z below the factory section.
- The ordering survives a preset save + reload.
- `cargo fmt` and `cargo clippy` clean.

---

## Phase 3 — Conditional OSC/Sub panel

**Goal:** OSC 1/2/3 + Sub controls only appear when at least one slot is in Sub mode; when both
slots are FM, the oscillator section is hidden (since it has no effect on an all-FM patch).

### Background

`osc_tab()` (`crates/synth-ui/src/sections/osc.rs:11`) renders:
1. A waveform selector row.
2. Three side-by-side columns: OSC 1, OSC 2, OSC 3 + Sub.
3. The "SLOTS / FM" section (slot foldouts with mode toggles and FM operator grids).

The SubtractiveBank is **shared across both slots** — OSC 1/2/3 are not per-slot controls. So
it is not meaningful to put OSC controls inside each individual slot foldout (that would show the
same knobs twice if both slots are Sub). The correct conditional is: show OSC controls when
**either** slot is in Sub mode; hide them when both are FM.

The user asked for the controls to appear "inside a slot foldout when that slot is in Sub mode",
mirroring FM operators. Because the oscillator bank is shared, the simplest implementation that
achieves the same visual effect is: **move the waveform selector + three OSC columns inside the
foldout of the first Sub-mode slot**. If both slots are Sub, the controls appear in Slot 1's
foldout only (Slot 2's foldout would show just level/pan and the mode toggle). If only Slot 2 is
Sub, the controls appear inside Slot 2's foldout.

This mirrors exactly how FM operators work: FM controls appear inside the foldout of a slot that
is in FM mode.

### Changes

**`crates/synth-ui/src/sections/fm_slots.rs`**

Inside `fm_slots_section()`, within the per-slot `CollapsingHeader` loop, add a Sub-mode branch
after the level/pan knobs, symmetric with the existing FM branch:

```rust
if self.slot_mode[slot_idx] == 0 {
    // Only render OSC controls for the *first* Sub-mode slot to avoid
    // showing the shared subtractive bank controls twice.
    let first_sub = self.slot_mode.iter().position(|&m| m == 0) == Some(slot_idx);
    if first_sub {
        ui.add_space(4.0);
        // Call the extracted helper (see osc.rs changes).
        self.osc_sub_controls_inline(ui, md);
    }
}
```

**`crates/synth-ui/src/sections/osc.rs`**

- Extract the waveform selector row + three OSC columns into a new method
  `osc_sub_controls_inline(&mut self, ui: &mut Ui, md: ModDisplay)`.
- Remove that block from `osc_tab()`.
- Pass `ModDisplay` down from `osc_tab()` through `fm_slots_section()` to
  `osc_sub_controls_inline()`. This may require threading `md` through the call chain, or making
  it a field on `ToneSmithyApp` if that is simpler.

If threading `ModDisplay` is awkward, an alternative is to keep the OSC columns in `osc_tab()`
but wrap the entire block in:

```rust
let any_sub = self.slot_mode.iter().any(|&m| m == 0);
if any_sub { /* waveform + columns */ }
```

This is simpler and has the same practical effect — use whichever approach is cleaner at
implementation time.

> **Open question:** The user might expect OSC controls inside the slot foldout even when that
> foldout is collapsed. Collapsing works as usual — the foldout hides all its contents including
> OSC controls. Verify with the user after the first implementation if the collapsed behaviour is
> acceptable.

### Done when

- Switching both slots to FM collapses / hides the OSC 1/2/3 + Sub controls entirely.
- Switching one slot back to Sub reveals the OSC controls inside that slot's foldout.
- FM operator grids are unaffected.
- `cargo fmt` and `cargo clippy` clean.

---

## Order of work

1. Phase 1 (K key) — smallest, no dependencies.
2. Phase 2 (alphabetical sort) — no dependencies on Phase 1.
3. Phase 3 (conditional OSC panel) — no dependencies on 1 or 2; do last because it involves
   the most refactoring.

Commit each phase separately so each one is bisectable and reviewable on its own.

## Files touched (summary)

| File | Phase |
|---|---|
| `crates/synth-ui/src/computer_keyboard.rs` | 1 |
| `docs/user-manual.md` | 1 |
| `crates/synth-ui/src/app/state.rs` | 2 |
| `crates/synth-ui/src/sections/fm_slots.rs` | 3 |
| `crates/synth-ui/src/sections/osc.rs` | 3 |

---

## Progress

- [ ] Phase 1 — K=C keyboard note
- [ ] Phase 2 — Alphabetical preset ordering
- [ ] Phase 3 — Conditional OSC/Sub panel
