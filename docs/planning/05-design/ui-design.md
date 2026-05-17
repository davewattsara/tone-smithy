# UI design

The visual and interaction design. The implementation contract for these decisions lives in [`../03-architecture/ui-layer.md`](../03-architecture/ui-layer.md).

## Visual direction

**Modern, flat, dark, restrained.** Think Vital, Pigments, modern Ableton devices — clean panels, generous spacing, one accent colour, type that does most of the heavy lifting.

What we are explicitly **not** going for:
- Skeuomorphic knobs with screws, brushed metal, simulated LCDs.
- Heavy gradients, glassmorphism, drop shadows used as decoration.
- Cluttered "every parameter visible at once" hardware-style layouts.

## Palette

A small palette, defined once as theme tokens and consumed everywhere:

- `bg.0` — window background (near-black, e.g. `#0E1013`).
- `bg.1` — panel background (slightly lighter, e.g. `#171A1F`).
- `bg.2` — control well / inset (e.g. `#1F232A`).
- `fg.0` — primary text (`#E6E8EB`).
- `fg.1` — secondary text (`#8A929E`).
- `fg.2` — tertiary text / muted (`#525964`).
- `accent` — single accent colour for active modulation, focus, selection (e.g. a desaturated cyan, `#5BC8DE`).
- `warn` — for clipping indicators and destructive actions (e.g. `#E0795B`).
- `mod_pos` — bipolar modulation positive (a green tone).
- `mod_neg` — bipolar modulation negative (a magenta tone).

All values above are placeholders; the actual palette is locked during M11.

## Typography

- **One sans-serif typeface** for everything (default candidate: Inter; backup: system UI sans).
- Type scale:
  - Display 18px (preset name, section titles)
  - Body 14px (parameter labels, browser items)
  - Small 12px (units, hints, status)
  - Micro 10px (footer, tooltips)
- Numerals: tabular figures for parameter values (so digits don't jiggle as values change).

## Layout

Single window, fixed structure with a small set of responsive breakpoints. From top to bottom:

1. **Header** (~48px) — current preset name (clickable for rename), prev / next buttons, browser toggle, settings button, "modified" indicator.
2. **Main area** — synth sections, either as stacked panels (wide window) or tabs (narrow window). Sections: Oscillators, Filter, Envelopes & LFOs, Modulation Matrix, Arp / Sequencer, Effects, Master.
3. **Preset browser** — collapsible side panel on the right. Categories / tag filter / search at the top, list below.
4. **Virtual keyboard** (~80px) — always visible. Configurable octave range.
5. **Footer** (~24px) — CPU%, voice count, MIDI activity indicator, audio device summary, QWERTY-input toggle.

Minimum window size **1280×720**. Scales up cleanly to 4K via egui's pixels-per-point setting.

## Component vocabulary

A small, opinionated set of widgets — see [`../03-architecture/ui-layer.md`](../03-architecture/ui-layer.md) for the engineering contract.

- **Knob** — the default for continuous parameters. Vertical drag, fine with Shift, reset with double-click. Modulated value rendered as a faint ring offset from the base value.
- **Slider** — used where direction matters (envelope times, fade-style parameters).
- **Toggle** — clean on/off pill.
- **Dropdown** — for enumerated parameters; opens upward when near the bottom of the window.
- **Step grid** — sequencer; each cell is a toggle plus a velocity bar.
- **Mod matrix row** — source ▾ → destination ▾ — bipolar amount knob — via ▾.
- **Patch name editor** — single-line with edit / save flow; Enter commits, Esc cancels.
- **Meter** — peak + RMS bars; clipping flashes `warn`.
- **Oscilloscope** — small XY scope in the master section (optional, can be hidden).

## Interaction

- **Direct manipulation** is the default; drag a knob, see the value change live.
- **Right-click** any parameter for a context menu: `MIDI Learn`, `Reset to default`, `Copy value`, `Paste value`, `Modulation…`.
- **Hover** shows a tooltip with the value, unit, and a one-line description.
- **Shift** = fine, **Ctrl/Cmd** = coarse, **Double-click** = reset.
- **Modulation visualisation** — every modulated parameter shows two indicators: the base value (where you set it) and the current modulated value (where it is right now). On knobs, this is a coloured arc inside the perimeter ring.
- **Drag-and-drop modulation assignment** is a v1.1 addition; v1 uses the matrix view exclusively.

## States

Every interactive control has these states explicitly designed:

- Default
- Hover
- Active (mouse down / being dragged)
- Focused (keyboard)
- Disabled (rare; reserved for parameters genuinely inapplicable)
- Modulated (overlay; orthogonal to the above)

Focus rings use the `accent` colour; never rely on contrast alone for the focus signal.

## Motion

- Restrained. Transitions are short (60–120 ms) and used for state changes (panel collapse, preset load flash, error toast in/out).
- Avoid spring physics. Avoid hover-driven micro-animations on every control.
- VU meters and oscilloscopes update at 30 fps idle / 60 fps active.

## Empty / error states

- Preset browser with no matching results → friendly message and a "clear filters" button.
- No audio device available → modal at startup pointing to the device picker.
- Preset failed to load → toast plus an entry in the persistent message panel.
- Settings file corrupted → start with defaults, surface a warning, do **not** overwrite the user's file until they confirm.

## Inspirational anchors

The user explicitly opted not to anchor to any single reference synth. The "modern flat / minimal" direction reflects this: it borrows the general visual idiom common across recent flagship plugins without copying any one of them. Specific design choices (knob proportions, panel grouping, type scale) are decided fresh during M11.
