# UI layer

The user interface lives in the `synth-ui` crate. It uses **egui** via **eframe**, the immediate-mode UI framework chosen for its speed of iteration, suitability for dense parameter panels, and active ecosystem.

## High-level structure

```
┌─────────────────────────────────────────────────────────────┐
│ HEADER  preset name | < prev | next > | browser ▾ | settings │
├──────────────────────────────────────────┬──────────────────┤
│                                          │                  │
│  MAIN PANEL                              │  PRESET BROWSER  │
│  (tabs or stacked sections):             │  (collapsible    │
│   • Oscillators                          │   side panel)    │
│   • Filter                               │                  │
│   • Envelopes & LFOs                     │                  │
│   • Modulation matrix                    │                  │
│   • Arp / Sequencer                      │                  │
│   • Effects                              │                  │
│   • Master                               │                  │
│                                          │                  │
├──────────────────────────────────────────┴──────────────────┤
│ VIRTUAL KEYBOARD  (always visible, configurable octave range)│
├─────────────────────────────────────────────────────────────┤
│ FOOTER  CPU% | voice count | MIDI activity | audio device   │
└─────────────────────────────────────────────────────────────┘
```

Layout adapts at a small set of width breakpoints; below 1280×720 the window is not allowed to shrink further (enforced minimum).

## Parameter binding

The UI does not own audio state. Instead, every frame:

1. Read the latest **parameter snapshot** (atomic pointer swap published by the engine each block).
2. Render widgets using the snapshot values.
3. Where a user interacts, push `(param_id, new_value)` events to the engine's parameter ring buffer.
4. For meters, voice count, and modulated parameter display, read from the engine's telemetry ring buffer (oldest entries dropped if the UI falls behind).

This pattern keeps the audio and UI threads decoupled while presenting a coherent view.

## Custom widgets

Stock egui widgets are functional but not synth-grade. We will build a small custom widget library in `synth-ui/widgets/`:

- **Knob** — rotary control. Vertical drag changes value (configurable sensitivity). Shift = fine, double-click = reset. Tooltip shows value with unit. Modulation ring around the perimeter shows modulated value vs base.
- **Slider** — vertical or horizontal, with the same drag/shift/double-click behaviour.
- **Toggle / button** — flat, on/off, with hover state.
- **Dropdown** — for enum parameters (osc waveform, filter mode, FM algorithm).
- **Step grid** — for the sequencer; per-cell note, velocity, gate.
- **Mod matrix row** — source dropdown, destination dropdown, bipolar amount knob, "via" source dropdown.
- **Patch name editor** — single-line text with edit/save flow.
- **VU meter / oscilloscope** — for the master output footer.

Each widget exposes a small builder API and renders with consistent typography and spacing tokens.

## Theming

Single dark theme in v1. A small `Theme` struct holds the palette, type scale, knob radius, padding tokens; widgets consume tokens rather than hardcoding values. This makes a future light/custom theme tractable without rewriting widgets.

Visual direction details: [`../05-design/ui-design.md`](../05-design/ui-design.md).

## Interaction patterns

- **Right-click on any parameter** opens a context menu: `MIDI Learn`, `Reset to default`, `Copy value`, `Paste value`, `Unassign modulation`.
- **Double-click** resets to default.
- **Shift + drag** = fine adjustment.
- **Ctrl + drag** (or middle-click drag) = coarse adjustment.
- **Keyboard navigation** between major sections via Tab / arrow keys.
- **Computer keyboard as MIDI** is toggled in the footer; when active, focus traps note input rather than triggering shortcuts.

## State persisted by the UI

- Window size and position.
- Last-used preset path.
- Browser filter and search state.
- Open/closed state of the preset browser panel.

Persisted to a small `ui_state.ron` file in the user data directory, separate from synth presets.

## Performance

- egui repaints only when the UI requests it (interaction or telemetry change). The default frame rate is throttled to **30 fps when idle**, **60 fps during interaction**.
- Meters and the oscilloscope are time-bounded: they will skip rendering if a frame is over budget.
- The parameter snapshot read is O(parameter count); the snapshot is a flat array of small values, kept under 8 KB.

## Accessibility (v1)

- All controls reachable via keyboard.
- Visible focus ring.
- Sufficient contrast for the default theme (verify against WCAG AA where possible).
- Screen-reader support is **deferred to v2** — egui's accessibility story is improving but not mature enough to rely on for v1.

## Error surfaces

User-facing errors (failed preset load, missing audio device, etc.) appear as transient toasts in the bottom-right plus a permanent record in a "Messages" panel accessible from the footer. The UI never silently drops an error.
