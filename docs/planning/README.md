# Planning

This folder is the source of truth for the design and direction of **Tone Smithy** — a hybrid (subtractive + FM) software synthesizer, standalone on Windows, written in Rust.

The plan is organised as a set of small, focused markdown documents. It is intended to contain enough information to build the full product without leaving the spec.

## Status

- **Stage:** Initial draft. Several decisions are intentionally deferred (see `01-vision/open-questions.md`).
- **Last updated:** 2026-05-17

## Index

### 01 — Vision
- [Overview](01-vision/overview.md) — what we're building, for whom, and why.
- [Success criteria](01-vision/success-criteria.md) — measurable definition of "good enough to ship".
- [Open questions](01-vision/open-questions.md) — decisions deferred from this round of planning.

### 02 — Scope
- [v1 features](02-scope/features-v1.md) — the engine, modulation, sequencer, effects, presets, I/O.
- [Out of scope](02-scope/out-of-scope.md) — explicitly excluded from v1.
- [Roadmap](02-scope/roadmap.md) — what comes after v1.

### 03 — Architecture
- [Overview](03-architecture/overview.md) — high-level structure and module split.
- [Design patterns](03-architecture/design-patterns.md) — architectural and real-time safety patterns the codebase commits to.
- [Audio engine](03-architecture/audio-engine.md) — DSP signal flow, voice management, real-time safety.
- [UI layer](03-architecture/ui-layer.md) — egui front-end architecture and parameter binding.
- [MIDI & input](03-architecture/midi-and-input.md) — hardware MIDI, virtual keyboard, computer keyboard.
- [Persistence](03-architecture/persistence.md) — presets and settings on disk.

### 04 — Tech stack
- [Stack](04-tech-stack/stack.md) — language, frameworks, key rationale.
- [Libraries](04-tech-stack/libraries.md) — concrete crate choices and what each is for.
- [Tooling](04-tech-stack/tooling.md) — build, lint, test, CI, profiling, packaging.
- [Code style](04-tech-stack/code-style.md) — naming, file organisation, documentation comments, inline comments, error handling.
- [Unit testing](04-tech-stack/unit-testing.md) — what to test, naming, AAA structure, float comparison, DSP-specific patterns. Sibling docs (integration / snapshot / property / real-time / benchmarks) added per milestone.

### 05 — Design
- [UI design](05-design/ui-design.md) — visual language, layout, component vocabulary, interaction.
- [DSP & sound design](05-design/dsp-and-sound.md) — sonic targets, oscillator/filter/FX choices, factory bank plan.

### 06 — Implementation
- [Project structure](06-implementation/project-structure.md) — Cargo workspace layout and crate boundaries.
- [Milestones](06-implementation/milestones.md) — ordered, sized milestones from scaffold to v1 release.
- [Risks](06-implementation/risks.md) — what could go wrong and how to mitigate.

### 07 — Distribution
- [Licensing](07-distribution/licensing.md) — open-source / proprietary options (decision deferred).
- [Packaging](07-distribution/packaging.md) — installer, code signing, updates, distribution channels.

## How to use this plan

- Treat each document as living. Update it when a decision changes; don't add a contradicting note elsewhere.
- When making a design or implementation decision, cite the relevant plan section in the commit message so the doc and the code stay in sync.
- The roadmap is intentionally compact — large additions belong here, in the plan, before they become work in flight.

## Related docs (outside the plan)

- [`/CLAUDE.md`](../../CLAUDE.md) — agent-facing instructions, auto-loaded by Claude Code.
- [`../working-conventions.md`](../working-conventions.md) — git workflow, conversation logging, commit cadence, and reading order for new contributors.
- [`../conversations/README.md`](../conversations/README.md) — conversation log format.
