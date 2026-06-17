# M21 plan — UX polish

Five self-contained UX improvements, all UI-only. No engine or parameter-system changes.

**Target version:** v1.2
**Estimate:** 1–2 weeks
**Branch:** `milestone/m21-ux-polish`

---

## Overview

| Phase | Feature | Touches |
|---|---|---|
| 1 | Tooltips | `sections/envelopes.rs`, `sections/modulation.rs`, `sections/filter.rs`, knob widget |
| 2 | In-app help | `synth-app/Cargo.toml`, menu bar |
| 3 | Unsaved-changes warning | `app/state.rs`, `app/mod.rs`, preset load path |
| 4 | Slot foldout behaviour | `sections/osc.rs`, `app/state.rs` |
| 5 | Preset description in main view | `sections/header.rs`, `app/state.rs` |

Phases are independent — commit each separately.

---

## Phase 1 — Tooltips

### LFO Sync and Reset

In `crates/synth-ui/src/sections/envelopes.rs`, the `Sync` and `Reset` controls are
rendered as checkbox or toggle widgets. Append `.on_hover_text("…")` to each response:

```rust
// Reset
let r = ui.checkbox(&mut state.lfo1_reset, "Reset");
r.on_hover_text("Restart LFO phase on every note-on.");

// Sync
let r = ui.checkbox(&mut state.lfo1_sync, "Sync");
r.on_hover_text("Lock LFO rate to the Master BPM instead of running free.");
```

Apply the same pattern to LFO 2.

### Mod matrix dropdown items

The source and destination dropdowns in `crates/synth-ui/src/sections/modulation.rs` use
`egui::ComboBox`. Individual selectable values can receive tooltips by capturing the response
from `selectable_value` and chaining `.on_hover_text()`:

```rust
egui::ComboBox::from_id_source(src_id)
    .selected_text(MOD_SOURCE_LABELS[src_idx])
    .show_ui(ui, |ui| {
        for (i, label) in MOD_SOURCE_LABELS.iter().enumerate() {
            let r = ui.selectable_value(&mut src_idx, i, *label);
            r.on_hover_text(MOD_SOURCE_TOOLTIPS[i]);
        }
    });
```

Add two new `const` slices — `MOD_SOURCE_TOOLTIPS: [&str; N]` and
`MOD_DEST_TOOLTIPS: [&str; N]` — in `app/state.rs` alongside the existing label slices.
Their lengths must match `ModSource::COUNT` and `ModDest::COUNT` (the existing guard test
covers this if we extend it to include the tooltip slices).

Suggested tooltip text:

| Source | Tooltip |
|---|---|
| LFO1 | "LFO 1 output (-1 to +1)." |
| LFO2 | "LFO 2 output (-1 to +1)." |
| Env2 | "Mod envelope 2 output (0 to 1)." |
| AmpEnv | "Amplitude envelope output (0 to 1). Follows the note's volume shape." |
| Vel | "MIDI note velocity (0 to 1). Higher velocity = louder note." |
| Key | "Key tracking — 0 at C-1, 1 at G9. Useful for filter opening with pitch." |
| ModWhl | "MIDI mod wheel / CC 1 (0 to 1)." |
| AfterT | "MIDI channel aftertouch (0 to 1)." |
| Bend | "MIDI pitch bend (-1 to +1)." |
| Env3 | "Mod envelope 3 output (0 to 1)." |
| Seq | "Step sequencer mod lane 1 — current step CV (-1 to +1)." |
| Seq2 | "Step sequencer mod lane 2 — current step CV (-1 to +1)." |

| Destination | Tooltip |
|---|---|
| Cutoff | "Filter 1 cutoff frequency. Amount in Hz." |
| Reso | "Filter 1 resonance (0 to 1)." |
| Pitch | "Global pitch offset. Amount in semitones." |
| Vol | "Master output level (0 to 1)." |
| Osc1Det | "OSC 1 detune. Amount in cents." |
| Osc1Pan | "OSC 1 stereo pan (-1 = left, +1 = right)." |
| F2Cut | "Filter 2 cutoff frequency. Amount in Hz." |
| F2Res | "Filter 2 resonance (0 to 1)." |
| Osc2Det | "OSC 2 detune. Amount in cents." |
| Osc3Det | "OSC 3 detune. Amount in cents." |
| Osc2Pan | "OSC 2 stereo pan (-1 = left, +1 = right)." |
| Osc3Pan | "OSC 3 stereo pan (-1 = left, +1 = right)." |

### Filter tab

In `crates/synth-ui/src/sections/filter.rs`, add `.on_hover_text()` after each control:

- Routing selector (Off/Serial/Parallel): `"Off: bypass Filter 2. Serial: signal passes F1 then F2. Parallel: F1 and F2 run in parallel, outputs averaged."`
- Mode (LP/HP/BP/Notch): `"Low-pass / High-pass / Band-pass / Notch."` (or per-button)
- Slope (12 dB/24 dB): `"12 dB/oct: 2-pole. 24 dB/oct: steeper rolloff via cascaded 2-pole."` 
- Cutoff knob: `"Filter cutoff frequency (Hz)."` 
- Resonance knob: `"Resonance (0–1). Higher values add a peak at the cutoff; approaching 1 self-oscillates."`

### Knob full name in tooltip

The custom knob widget (`crates/synth-ui/src/widgets/knob.rs` or similar) currently shows
the abbreviated label. Extend it to accept an optional full-name string:

```rust
pub fn knob(ui: &mut Ui, label: &str, full_name: &str, value: &mut f32, …) -> Response {
    …
    resp.on_hover_text(full_name)
}
```

At each call site, pass the full name (e.g. `"Attack"` instead of `"A"`, `"Decay"` instead
of `"D"`). Where the label is already the full name, pass the same string for both.

If the knob widget is constructed differently (e.g. builder pattern), add a `.tooltip(str)`
method that sets an inner field and calls `.on_hover_text()` on the final response.

### Done when

All listed controls have non-empty tooltip text. No control has a tooltip that duplicates its
visible label verbatim (e.g. a knob labelled "Cutoff" with tooltip "Cutoff" adds no value —
use "Filter cutoff frequency (Hz)." instead).

---

## Phase 2 — In-app help

### Dependency

Add the `open` crate to `crates/synth-app/Cargo.toml`:

```toml
[dependencies]
open = "5"
```

### Menu item

In the top menu bar (wherever `ui.menu_button` is called, likely `app/mod.rs` or
`sections/header.rs`), add a Help entry:

```rust
ui.menu_button("Help", |ui| {
    if ui.button("User manual").clicked() {
        open::that("https://github.com/davewattsara/tone-smithy/blob/main/docs/user-manual.md").ok();
        ui.close_menu();
    }
});
```

`open::that` is fire-and-forget — it calls the OS default browser and returns immediately.
The `.ok()` silently ignores failures (browser not found, etc.); no error handling needed.

### Done when

Clicking Help → User manual opens the manual in the default browser. No crash if the OS
browser is unavailable.

---

## Phase 3 — Unsaved-changes warning

### Dirty flag

Add to `crates/synth-ui/src/app/state.rs`:

```rust
pub is_dirty: bool,
pub pending_preset_load: Option<PresetId>,  // or whatever type identifies a preset
```

Set `is_dirty = true` whenever a param event is dispatched from a user interaction (not
from a preset load path). The cleanest hook is wherever `AppState::emit_param` (or equivalent)
is called — add `self.is_dirty = true` there.

Set `is_dirty = false` and `pending_preset_load = None` in two places:
- After a preset is successfully loaded.
- After a preset is successfully saved.

### Intercepting preset loads

In the preset-browser click handler (wherever `load_preset(id)` is called), change from:

```rust
self.load_preset(id);
```

to:

```rust
if self.state.is_dirty {
    self.state.pending_preset_load = Some(id);
    // dialog renders next frame
} else {
    self.load_preset(id);
}
```

### Modal dialog

In the main `update()` loop (or a dedicated `show_modals` helper), render the dialog when
`pending_preset_load` is `Some`:

```rust
if let Some(pending_id) = self.state.pending_preset_load {
    egui::Window::new("Unsaved changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("You have unsaved changes. What would you like to do?");
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.save_current_preset();
                    self.load_preset(pending_id);
                    self.state.pending_preset_load = None;
                }
                if ui.button("Discard").clicked() {
                    self.load_preset(pending_id);
                    self.state.pending_preset_load = None;
                }
                if ui.button("Cancel").clicked() {
                    self.state.pending_preset_load = None;
                }
            });
        });
}
```

The existing `save_current_preset` path already exists (it's what Save does in the browser).
If it requires a name or confirmation, use Save As instead and skip it — keep the dialog
simple: Save overwrites the current file, Discard abandons, Cancel aborts.

### Intercepting app quit

In egui, the close button sets a flag on the viewport. Check it each frame and, if dirty,
cancel the close and show the dialog instead:

```rust
// In update(), before rendering anything else
if ctx.input(|i| i.viewport().close_requested()) {
    if self.state.is_dirty {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.state.pending_quit = true;
    }
    // if not dirty, the close proceeds normally
}
```

Add `pending_quit: bool` to `AppState`. When `pending_quit` is true, render the same
Save / Discard / Cancel dialog but with quit-specific wording ("You have unsaved changes.
Quit anyway?"):

```rust
if self.state.pending_quit {
    egui::Window::new("Unsaved changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("You have unsaved changes. Quit anyway?");
            ui.horizontal(|ui| {
                if ui.button("Save and quit").clicked() {
                    self.save_current_preset();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Discard and quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Cancel").clicked() {
                    self.state.pending_quit = false;
                }
            });
        });
}
```

`ViewportCommand::Close` (issued after Save or Discard) triggers the actual OS-level close,
bypassing the `close_requested` check that would normally re-cancel it.

### Done when

Loading a preset after making a parameter change shows the dialog. Save saves and loads.
Discard loads without saving. Cancel returns to the current patch unchanged. Closing the
app window while dirty shows "Quit anyway?" — Save and quit saves first; Discard and quit
closes without saving; Cancel keeps the app open. `is_dirty` is correctly false after a
clean load or save.

---

## Phase 4 — Slot foldout behaviour

### State

Add to `crates/synth-ui/src/app/state.rs`:

```rust
pub slot_foldout_open: [bool; 2],
```

Default: `[true, false]` — Slot 1 expanded, Slot 2 collapsed on fresh launch.

### On preset load

After `sync_from_snapshot` runs, set:

```rust
self.state.slot_foldout_open[0] = snapshot.slot_level[0] > 0.0;
self.state.slot_foldout_open[1] = snapshot.slot_level[1] > 0.0;
```

(`slot_level` is already in the snapshot.)

### In the OSC tab

In `crates/synth-ui/src/sections/osc.rs`, the slot foldouts are likely rendered with
`egui::CollapsingHeader`. Pass the `open` override:

```rust
egui::CollapsingHeader::new("Slot 1")
    .open(Some(state.slot_foldout_open[0]))
    .show(ui, |ui| { … });
```

`open(Some(true/false))` overrides the header's internal toggle. The user can still
click to collapse/expand after load — `open` is just the initial state on this frame.
To keep user interaction working after the load frame, set `slot_foldout_open` only
on the first frame after a load (use a one-frame flag like `just_loaded_preset: bool`),
then let CollapsingHeader manage its own state thereafter.

### Done when

Fresh launch: Slot 1 open, Slot 2 closed. Loading a preset with both slots in use: both
open. Loading a preset with only Slot 1 in use (Slot 2 level = 0): Slot 1 open, Slot 2
closed. User can manually toggle after load without the foldout resetting.

---

## Phase 5 — Preset description in main view

### State

Add to `crates/synth-ui/src/app/state.rs`:

```rust
pub current_preset_description: String,
```

Populate it from the preset metadata when a preset is loaded. The description is already
stored in the preset file's metadata block; `synth-presets` exposes it on load.

### Header bar

In `crates/synth-ui/src/sections/header.rs` (or wherever the patch-name label is rendered),
below the patch name:

```rust
if !state.current_preset_description.is_empty() {
    ui.add(
        egui::Label::new(
            egui::RichText::new(&state.current_preset_description)
                .italics()
                .small()
        )
        .wrap(true)
    );
}
```

If the header bar is a fixed-height strip, this will require making the description area
a separate row below the main header controls, or allowing the header to grow. Check the
current layout and adjust accordingly — the description should not push other controls
around; a dedicated description row underneath the main header strip is the least disruptive.

### Done when

Loading a preset with a non-empty description shows it below the patch name in small italic
text. Loading a preset with no description shows nothing. The header layout does not break
for either case.

---

## Milestone done when

1. Tooltips present on all listed controls; knobs show full names.
2. Help → User manual opens the browser.
3. Loading a preset while dirty shows Save / Discard / Cancel; all three options behave correctly.
4. Slot foldouts auto-expand/collapse on patch load; Slot 1 is open by default on fresh launch.
5. Preset description appears below patch name when non-empty.
6. `cargo fmt --check` and `cargo clippy -D warnings` clean.

---

## Progress

- [ ] Phase 1 — Tooltips
- [ ] Phase 2 — In-app help
- [ ] Phase 3 — Unsaved-changes warning
- [ ] Phase 4 — Slot foldout behaviour
- [ ] Phase 5 — Preset description in main view
