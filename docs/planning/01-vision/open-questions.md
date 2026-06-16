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

- **Status:** Deferred to v1.3 (see [`../02-scope/roadmap.md`](../02-scope/roadmap.md)). The voice manager will be aware of per-voice modulation sources to leave the door open, but MPE channel handling is not in v1.
- **Decision needed by:** v1.3 planning.

## Microtuning / alternative scales

- **Status:** Deferred to v1.3 (see [`../02-scope/roadmap.md`](../02-scope/roadmap.md)). Scala (`.scl` / `.kbm`) is the obvious format.
- **Decision needed by:** v1.3 planning.

## Oversampling

- **Status:** No global oversampling in v1. FM operators may oversample internally where audible aliasing is unacceptable. A global 2×/4× option is on the v1.3 roadmap.
- **Decision needed by:** During M7 (FM engine) for the internal-only FM case; before v1.3 for the global option.

## Factory content authoring

- **Status:** **Resolved (2026-06-16): solo authoring (developer + Claude).** The v1.0 factory bank (M14) and the v1.1 expansion (M20) are authored in-house rather than recruiting a community sound designer or running an open call.
- **Mitigation:** The M20 plan ([`../06-implementation/m20-plan.md`](../06-implementation/m20-plan.md)) makes the showcase requirements explicit and adds CI coverage-guard tests so the bank's breadth is enforced, not left to chance.
- **Revisit:** Community/open-call sound design can be reconsidered for a later release if the in-house effort proves a bottleneck.

## Code signing certificate

- **Status:** **Resolved (2026-06-09): v1.0 ships unsigned.** A certificate is
  deferred to a later release (non-trivial cost, $300–$500/year for an EV cert).
- **Mitigation:** The README and the bundled `README.txt` carry a "SmartScreen
  warning" explanation; the dist/CI tooling is signing-optional (gated on
  `TONESMITHY_CERT` / a CI secret), so signing can be switched on later with no
  code change.
- **Revisit:** When a certificate is obtained for a future version (likely
  v1.0.1 / v1.1).

## Application icon

- **Status:** **Resolved (2026-06-09): v1.0 ships with the default icon.** A
  custom `assets/icons/tonesmithy.ico` is deferred to a later version.
- **Mitigation:** The installer and exe icon references are guarded with
  `FileExists`, so dropping the `.ico` in and rebuilding is the only step needed
  once the art asset exists.

## Auto-update mechanism

- **Status:** Not in v1. v1.1 candidate.
- **Likely approach:** Lightweight check against GitHub Releases.
