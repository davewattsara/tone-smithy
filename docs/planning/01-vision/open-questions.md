# Open questions

Decisions deliberately deferred from this round of planning. Each one is logged here with the trigger that should force a decision.

## Product name

- **Status:** **Resolved (2026-05-17): "Tone Smithy"**.
  - Display name: Tone Smithy
  - Binary: `tonesmithy.exe`
  - User data folder: `%APPDATA%\Tone Smithy\`
  - Install dir: `%ProgramFiles%\Tone Smithy\`
  - Preset file extension: `.tsmith` (provisional — revisit if a better one emerges)
  - Internal Rust crate names keep the `synth-*` prefix for brevity in imports.

## Open-source licence

- **Status:** **Resolved (2026-05-17): dual-licensed `MIT OR Apache-2.0`** (the Rust ecosystem convention).
- **Rationale:** permissive licence supports the user's preferences — others can fork freely, contributor friction is low, and future commercial paths (support, signed builds, expansion packs) stay open.
- **See:** [`../07-distribution/licensing.md`](../07-distribution/licensing.md) for the full rationale and implications.

## Plugin formats (CLAP / VST3)

- **Status:** Out of scope for v1, planned for v2.
- **Architectural impact:** The audio engine and parameter model must be designed so that a plugin wrapper can be added without rewriting either. `nih-plug` is the most likely host.
- **Decision needed by:** Architecture lock-in at end of M2.

## MPE (MIDI Polyphonic Expression)

- **Status:** Deferred. The voice manager will be aware of per-voice modulation sources to leave the door open, but MPE channel handling is not in v1.
- **Decision needed by:** v1.2 planning.

## Microtuning / alternative scales

- **Status:** Deferred to v1.2 or later. Scala (`.scl` / `.kbm`) is the obvious format.
- **Decision needed by:** v1.x planning.

## Oversampling

- **Status:** No global oversampling in v1. FM operators may oversample internally where audible aliasing is unacceptable.
- **Decision needed by:** During M5 (FM engine).

## Factory content authoring

- **Status:** Sound design is a major effort and the developer may not be the right person for all categories. Options: solo, recruit a community sound designer, run an open call.
- **Decision needed by:** Start of M14 (factory bank).

## Code signing certificate

- **Status:** Deferred — non-trivial cost ($300–$500/year for an EV cert).
- **Mitigation:** First public release may ship unsigned with a clear SmartScreen warning explanation.
- **Decision needed by:** Before M15.

## Auto-update mechanism

- **Status:** Not in v1. v1.1 candidate.
- **Likely approach:** Lightweight check against GitHub Releases.
