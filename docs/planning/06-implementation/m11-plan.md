# M11 — UI v1 Polish

**Status:** Planning

## Goal

Transform the functional-but-rough developer UI into something that looks and feels like a professional plugin. Every parameter reachable, every knob labelled and tooltip'd, a locked visual identity, and the structural layout that will carry through to v1.0 release.

---

## What the current UI actually is

- Single 2100-line `app.rs` with all panel functions inline.
- Layout: vertically stacked `ui.columns()` blocks, no proper Header / Main / Footer structure.
- Colours: egui defaults (no custom theme at all).
- Knob: functional custom widget — drag, right-click reset, value arc, label. Missing: Shift fine-drag, double-click reset, tooltips, modulation ring.
- No tooltips, no right-click menus beyond knob reset, no section headers with visual weight.
- Virtual keyboard and footer already exist but are visually plain.

---

## In scope

### 1. Code restructuring
Split `app.rs` into a module tree before any visual work. A 2100-line file is not workable for a polish pass.

```
crates/synth-ui/src/
  app.rs              — ToneSmithyApp struct + eframe::App impl only (~200 lines)
  theme.rs            — Theme struct, palette, type scale, spacing tokens
  sections/
    mod.rs
    osc.rs            — osc1_panel
    filter.rs         — filter_panel
    amp_env.rs        — amp_env_panel
    lfo.rs            — lfo_panel
    env2.rs           — env2_panel
    mod_matrix.rs     — mod_matrix_panel
    fm.rs             — fm_panel
    arp.rs            — arp_panel
    fx.rs             — fx_panel
    master.rs         — master row (volume, pitch offset, BPM)
  widgets/
    mod.rs
    knob.rs           — (moved from top-level, enhanced)
    toggle.rs         — on/off pill replacing checkboxes
    dropdown.rs       — enum picker replacing combo_box
    meter.rs          — VU meter (peak + RMS)
```

### 2. Theme system
A `Theme` struct defined in `theme.rs`, passed by reference to every panel and widget. Replaces all hardcoded colors and font sizes.

```rust
pub struct Theme {
    // Palette
    pub bg0: Color32,       // #0E1013 window background
    pub bg1: Color32,       // #171A1F panel background
    pub bg2: Color32,       // #1F232A control well
    pub fg0: Color32,       // #E6E8EB primary text
    pub fg1: Color32,       // #8A929E secondary text
    pub fg2: Color32,       // #525964 muted / tertiary
    pub accent: Color32,    // #5BC8DE active/focus/selection
    pub warn: Color32,      // #E0795B clip/destructive
    pub mod_pos: Color32,   // modulation positive
    pub mod_neg: Color32,   // modulation negative

    // Spacing
    pub panel_padding: f32,
    pub group_gap: f32,
    pub knob_diameter: f32,

    // Type scale (FontId)
    pub font_display: FontId,   // 18px — preset name, section titles
    pub font_body: FontId,      // 14px — param labels, values
    pub font_small: FontId,     // 12px — units, hints
    pub font_micro: FontId,     // 10px — footer, tooltips
}
```

The palette values above are the locked choices — final during M11, not placeholders.

Applied to egui via `ctx.set_visuals(theme.to_visuals())` once per frame from `update()`.

### 3. Window layout
Implement the proper chrome:

```
┌──────────────────────────────────────────────────────────────────┐
│ HEADER  48px  — preset name | Save | Load | [modified indicator] │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  MAIN AREA  (ScrollArea, stacked sections — tabs in a later      │
│              milestone when the section count justifies it)      │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│ VIRTUAL KEYBOARD  ~80px                                          │
├──────────────────────────────────────────────────────────────────┤
│ FOOTER  24px  — voice count | CPU% | MIDI indicator | device     │
└──────────────────────────────────────────────────────────────────┘
```

- Header and footer are `TopBottomPanel` (already partially true for footer).
- Virtual keyboard is a `TopBottomPanel` immediately above the footer.
- Central panel holds the scrollable section stack.
- Minimum 1280×720 already enforced.

### 4. Knob enhancements
The existing knob is 80% of the way there. Additions:

- **Shift + drag** → fine mode (1/10th sensitivity).
- **Double-click** → reset to default (currently only right-click does this; design doc specifies double-click).
- **Tooltip** → egui `response.on_hover_text(formatted_value_with_unit)`.
- **Modulation ring** → an arc drawn *outside* the value arc, coloured `accent`, showing the current modulated offset. Only drawn when the param has an active mod slot pointing to it. Requires the knob to accept an optional `mod_offset: Option<f32>` (a normalised -1..=1 value).

Right-click currently resets to default. In M11 it opens a **context menu** with:
- Reset to default
- Copy value (clipboard)
- Paste value (clipboard)
- *(MIDI Learn — greyed out stub; wired in M13)*

### 5. Custom widgets

**Toggle** (`widgets/toggle.rs`)
Replaces all `ui.checkbox()` calls. A pill-shaped on/off with the accent colour when on.

**Dropdown** (`widgets/dropdown.rs`)
Wraps `egui::ComboBox` with theme-aware styling. Used for waveform selector, filter mode, LFO shape, arp mode, FM algorithm, mod source/dest.

**VU Meter** (`widgets/meter.rs`)
Peak + RMS, vertical bars, `warn` colour flash on clip. Placed in the master section and footer.

### 6. Panel polish
For each section, a single focused pass:

| Section | Current state | M11 changes |
|---|---|---|
| Osc 1 | Knobs + waveform buttons | Styled section header; osc 2 & 3 visible (they exist in the engine but the UI only shows osc 1); unison controls grouped |
| Filter | Knobs + mode buttons | Section header; LP/HP/BP/Notch as a Toggle group |
| Amp Envelope | ADSR knobs | Section header; curve display hint (simple) |
| LFO 1 / LFO 2 | Knobs + shape | Section headers; shape as Dropdown; sync controls cleaner |
| Env2 | Knobs | Section header; curve knobs grouped separately |
| Mod Matrix | Table rows | Each row uses the custom Dropdown widget; amount knob uses themed Knob |
| FM | Slot tabs + op grid | Already reasonably structured; section header polish |
| Arp | Knobs + mode | Section header; mode as Dropdown |
| FX | Per-stage sections | Enable toggles use custom Toggle; section headers |
| Master | Knobs | VU meter added |

**Osc 2 and 3** — the engine supports them fully (levels, detune, pan, unison) but the current UI only exposes osc 1. M11 adds osc 2 and 3 controls.

### 7. Modulation visualisation
Each knob that is the destination of an active mod slot gets its modulation ring drawn. The current snapshot already carries `mod_slot_*` arrays. The approach:

1. Before rendering any panel, compute a `ModMap: HashMap<ModDest, f32>` from the snapshot — for each enabled slot, look up the live modulator output (lfo1_out, lfo2_out, etc. from the snapshot), multiply by amount, accumulate.
2. Pass `ModMap` by reference to each panel function.
3. When rendering a `Knob` that corresponds to a `ModDest`, look up its entry in the map and pass it as `mod_offset`.

This is a read-only pass over already-available snapshot data — no new engine API needed.

---

## Out of scope for M11

| Item | Where it lands |
|---|---|
| Preset browser side panel | M12 |
| MIDI Learn (functional) | M13 |
| Audio / MIDI device picker | M13 |
| Step sequencer grid | v1.1 (arp is not a step sequencer) |
| Oscilloscope / XY scope | Optional, decide at M11 wrap-up |
| Window resizing breakpoints / tab mode | The 1280 minimum covers v1; tabs deferred |
| Drag-and-drop mod assignment | v1.1 |
| Light theme | v1.1 |

---

## Phases

M11 is large enough to warrant sequential phases rather than one monolithic pass:

**Phase A — Foundation (do first, unblocks everything)**
- Restructure code into `sections/` and `widgets/` modules
- Implement `Theme` struct and apply to egui visuals
- Implement proper Header / Main / Keyboard / Footer layout

**Phase B — Widget polish**
- Knob: Shift fine-drag, double-click reset, tooltip, mod ring, right-click context menu
- Toggle, Dropdown, VU Meter widgets

**Phase C — Panel polish**
- Apply new widgets and theme to every panel
- Add osc 2 & 3 to the oscillator section
- Section headers with consistent styling

**Phase D — Modulation visualisation**
- Compute ModMap per frame
- Wire mod_offset into knobs for all ModDest params

**Phase E — Wrap-up**
- Full pass: every param labelled, reachable, tooltip'd
- Check at 100%, 125%, 150% DPI
- Test checklist

---

## Done when

- Every parameter exposed in the engine is reachable in the UI
- Every knob has a tooltip showing value with unit
- Theme is consistent — no egui-default grey anywhere
- Knobs for modulated parameters show the modulation ring
- Right-click on any knob gives Reset / Copy / Paste / (MIDI Learn stub)
- Osc 2 and 3 controls are visible
- VU meter in the master section
- Looks like a professional plugin, not a coder UI

---

## Open questions for Dave

1. **Osc 2 and osc 3** — the engine has them fully wired but the UI only shows osc 1. Should M11 expose all three in the oscillator panel? (Recommended: yes — they're a core feature.)

2. **Section tabs vs. stacked scroll** — the design doc mentions tabs for narrow windows. For M11 at the fixed 1280 width, stacked-and-scrollable is simpler and the design doc allows it. OK to defer tabs?

3. **Palette** — the design doc lists specific hex values as "placeholder candidates". Should I use them as-is (`#0E1013`, `#5BC8DE`, etc.) or do you want to review/adjust the palette before it gets locked?

4. **Font** — design doc recommends Inter but notes "system UI sans" as a fallback. egui ships with its own font (not Inter). Loading a custom font (bundling Inter) adds complexity. Should M11 use egui's built-in font for now and treat font as a v1.1 improvement, or is Inter a v1.0 requirement?
