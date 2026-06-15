# Roadmap

Indicative versions. The list is intentionally short — large additions should be discussed and promoted into the plan rather than accumulated here as a wish list.

## v1.0 — Initial release

Everything in [`features-v1.md`](features-v1.md). Windows-only standalone. Free download.

## v1.1 — Engine completion + cross-platform

Restores the engine features deferred from v1 to ship faster (Path B scope trim), plus quick UX
improvements and platform expansion:

### Quick UX wins
- **Extra keyboard note** — K key maps to C one octave above J=B, completing a full octave span on
  the computer keyboard.
- **Alphabetical preset ordering** — preset browser lists presets A-Z within each category by
  default.
- **Conditional OSC/Sub panel** — OSC 1, OSC 2, OSC 3 + Sub controls only appear inside a slot
  foldout when that slot is in Sub mode; FM slots show only FM operator controls (mirrors the
  existing FM panel behaviour).

### Engine expansion
- **Second filter** per voice with serial (`F1 → F2`) and parallel (`F1 ∥ F2 summed`) routing.
- **24 dB/oct filter** option (4-pole ZDF ladder or cascaded SVF, chosen by listening tests).
- **Second mod envelope** (Env3) — ADSR with curve shaping, freely assignable.
- **Modulation matrix expanded to 16 slots** (Env3 added as a source).
- **Global (mono) LFO mode** — per-LFO toggle to share one LFO instance across all voices so chords
  stay phase-locked; completes the "per-voice or global mode" LFO spec deferred from v1.0. Delivered
  in the M18 milestone.

### Step sequencer
- **Step sequencer** — 16 steps with note offset, velocity, gate, and one assignable mod lane.

### Platform
- **Linux installer** — AppImage or tarball build via CI on `ubuntu-latest`; attached to the
  GitHub Release alongside the Windows installer.
- **macOS installer** — DMG build via CI on `macos-latest`; notarized if a Developer ID
  certificate is available, otherwise unsigned with a Gatekeeper note in docs.

### Content
- **Factory bank expansion** — add 40–60 new presets to reach ~120 across all categories,
  including patches that showcase the new second filter and Env3.
- Bug-fix backlog from v1.0 reports.

## v1.2 — Polish & quality of life

- Auto-update check (GitHub Releases).
- Drag-and-drop modulation assignment (drag a source onto a knob).
- Editable FM operator routing (user algorithms in addition to the 8 factory algorithms).
- An additional mod lane in the step sequencer.
- Bug-fix backlog from v1.1 reports.

## v1.3 — Expression & tuning

- **MPE input** (per-note pitch, timbre, pressure).
- **Microtuning** via Scala (`.scl`) and keyboard maps (`.kbm`).
- Per-voice pan modulation.
- Optional internal oversampling (2×, 4×) as a global setting.

## v1.x — Audio features

- Built-in audio recorder / bouncing to WAV.
- Multi-out routing for advanced patches.
- Sidechain input (only meaningful once plugin formats exist; see v2).

## v2.0 — Plugin formats

- **CLAP plugin build** (first target — newest, cleanest API).
- **VST3 plugin build**.
- **macOS AU plugin** (in addition to the standalone added in v1.1).
- **Linux CLAP + VST3 plugin** (standalone is already in v1.1).
- Architecture: `nih-plug` is the leading host candidate; the engine and parameter model are designed in v1 to make this addition possible without a rewrite.

## v2.x and beyond — Speculative

- A second engine family (wavetable or sample-based).
- Theming / user themes.
- Cloud preset sharing (opt-in).
- AAX / Pro Tools support (requires Avid developer agreement).

## What this roadmap is not

- A commitment. Versions and contents will shift based on what we learn from v1.0.
- Exhaustive. Items can be added once they are concrete enough to estimate.
