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

## Phase 3 — Move OSC/Sub controls into Slot 1 foldout

**Goal:** Move the waveform selector and OSC 1/2/3 + Sub columns inside the Slot 1 foldout, so
the layout mirrors Slot 2 (where FM operators already live inside the foldout). The OSC controls
have always been Slot-1-only; putting them in the foldout makes that structural relationship
visible in the UI.

### Background

`osc_tab()` (`crates/synth-ui/src/sections/osc.rs:11`) currently renders:
1. A waveform selector row — always visible.
2. Three side-by-side columns: OSC 1, OSC 2, OSC 3 + Sub — always visible.
3. The "SLOTS / FM" section (two slot foldouts).

With fixed slot roles (Slot 1 = Sub, Slot 2 = FM), the OSC controls belong structurally inside
the Slot 1 foldout, just as the FM operator grid lives inside the Slot 2 foldout. There is no
conditionality needed — the controls just move.

### Changes

**`crates/synth-ui/src/sections/osc.rs`**

- Extract the waveform selector row + three OSC columns + sub controls into a new method
  `osc_sub_controls_inline(&mut self, ui: &mut Ui, md: ModDisplay)`.
- Remove those blocks from `osc_tab()`, leaving `osc_tab()` as just a thin wrapper that calls
  `fm_slots_section()`.

**`crates/synth-ui/src/sections/fm_slots.rs`**

- Pass `md: ModDisplay` through to `fm_slots_section()`.
- Inside the `slot_idx == 0` branch of the `CollapsingHeader`, after the level/pan knobs, call
  `self.osc_sub_controls_inline(ui, md)`.

If threading `ModDisplay` is awkward, store it as a field set at the top of each frame rather
than passing it through the call chain.

### Done when

- Slot 1 foldout shows level/pan then waveform selector + OSC 1/2/3 + Sub columns.
- Slot 2 foldout shows level/pan then algorithm + operator grid (unchanged).
- The top-level OSC tab no longer shows the waveform/OSC columns outside the foldouts.
- `cargo fmt` and `cargo clippy` clean.

---

## Order of work

1. Phase 1 (K key) — smallest, no dependencies.
2. Phase 2 (alphabetical sort) — no dependencies on Phase 1.
3. Phase 3 (OSC panel into Slot 1 foldout) — no dependencies on 1 or 2; do last.

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
- [ ] Phase 3 — OSC/Sub panel into Slot 1 foldout
