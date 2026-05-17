# Roadmap

Indicative versions. The list is intentionally short — large additions should be discussed and promoted into the plan rather than accumulated here as a wish list.

## v1.0 — Initial release

Everything in [`features-v1.md`](features-v1.md). Windows-only standalone. Free download.

## v1.1 — Polish & quality of life

- Auto-update check (GitHub Releases).
- Drag-and-drop modulation assignment (drag a source onto a knob).
- Editable FM operator routing (user algorithms in addition to the 8 factory algorithms).
- An additional mod lane in the step sequencer.
- Improved factory bank ("v2"): refresh based on user feedback, add 30+ new presets.
- Bug-fix backlog from v1.0 reports.

## v1.2 — Expression & tuning

- **MPE input** (per-note pitch, timbre, pressure).
- **Microtuning** via Scala (`.scl`) and keyboard maps (`.kbm`).
- Per-voice pan modulation.
- Optional internal oversampling (2×, 4×) as a global setting.

## v1.x — Audio features

- Built-in audio recorder / bouncing to WAV.
- Multi-out routing for advanced patches.
- Sidechain input (only meaningful once plugin formats exist; see v2).

## v2.0 — Plugin formats & cross-platform

- **CLAP plugin build** (first target — newest, cleanest API).
- **VST3 plugin build**.
- **macOS support** (Intel + Apple Silicon), including AU plugin.
- **Linux support** (CLAP + VST3 only).
- Architecture: `nih-plug` is the leading host candidate; the engine and parameter model are designed in v1 to make this addition possible without a rewrite.

## v2.x and beyond — Speculative

- A second engine family (wavetable or sample-based).
- Theming / user themes.
- Cloud preset sharing (opt-in).
- AAX / Pro Tools support (requires Avid developer agreement).

## What this roadmap is not

- A commitment. Versions and contents will shift based on what we learn from v1.0.
- Exhaustive. Items can be added once they are concrete enough to estimate.
