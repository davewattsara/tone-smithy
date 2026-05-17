# Risks

The risks most likely to derail v1, with a mitigation for each. Reviewed at each milestone boundary; new risks added as they emerge.

## R1 — DSP sounds amateurish

**Risk:** Hand-rolled DSP is the central creative bet. If the synth sounds thin, brittle, or aliased, no UI polish will save it.

**Mitigation:**
- Reference comparisons throughout — A/B against known-good synths at each engine milestone.
- Spectrum-snapshot tests catch regressions, but they don't catch "this just sounds bad" — schedule explicit listening sessions at M2, M5, M8.
- Recruit at least one external listener (sound designer or producer) for feedback no later than M5.
- Reserve scope to swap algorithms (e.g. ladder vs cascaded SVF for 4-pole filter) based on what sounds better — decision deferred to M2.

## R2 — Real-time safety regressions

**Risk:** An accidental allocation, lock, or syscall on the audio thread causes dropouts that are hard to reproduce and hard to attribute.

**Mitigation:**
- `assert_no_alloc` test in CI from M3 onward.
- Audio thread API surface kept small; any new code reviewed against the "no alloc / no lock / no syscall" rule.
- Soak test in CI (long render with random automation) on every release-candidate build.
- Tracy / Superluminal sessions before each major engine milestone.

## R3 — Egui performance on dense synth UI

**Risk:** A full screen of knobs, meters, oscilloscope, and a sequencer grid at 60 fps could push the UI thread harder than expected, particularly at 4K.

**Mitigation:**
- Profile at M11 with the full panel layout populated.
- Throttle idle frame rate to 30 fps.
- Cache widget mesh data where egui allows; avoid per-frame allocation in custom widgets.
- Have a fallback plan: reduce oscilloscope rate, simplify mod ring rendering, skip mod-indicator updates above a CPU budget.

## R4 — ASIO support is harder than expected

**Risk:** Users on professional interfaces expect ASIO. `cpal` supports it, but the Steinberg ASIO SDK can't be redistributed, and the build needs the user (or our build pipeline) to vendor it.

**Mitigation:**
- Ship v1 with WASAPI only. WASAPI on Windows 10/11 is good enough for the latency target.
- Document the limitation honestly in the README.
- ASIO is a v1.x or v2 candidate; investigate vendoring during v1.x planning.

## R5 — Plugin formats become urgent after v1

**Risk:** Many target users won't take a standalone-only synth seriously. If demand for VST3/CLAP arrives quickly, the architecture has to accommodate it without a rewrite.

**Mitigation:**
- Engine/host/UI separation in the workspace layout exists specifically to make this possible.
- Parameter model designed to map onto `nih-plug`'s flat-id parameter API.
- Threading model already assumes a "process is called from outside" pattern; replacing `cpal` with a plugin host wrapper should not touch the engine.
- Validate the assumption: prototype a `nih-plug` build during v1.x even if it isn't shipped.

## R6 — Scope creep on the flagship feature set

**Risk:** "Flagship" is a generous label and the feature list is already large. Adding "just one more thing" repeatedly slips v1 indefinitely.

**Mitigation:**
- Milestones are explicit and the features doc is the contract.
- New ideas go to the roadmap, not v1. The bar to promote a v1.x item into v1 is high and requires updating the plan.
- Milestone reviews check for scope drift.

## R7 — Solo factory bank effort underestimated

**Risk:** Authoring 120–150 quality presets is weeks of work by an experienced sound designer, and is often done badly by engineers.

**Mitigation:**
- Recognise this risk early and decide at the start of M14: solo, recruit, or open call.
- A shorter v1.0 bank (e.g. 80 strong presets) is preferable to a longer mediocre one.
- Reserve a "factory bank v2" item in v1.1.

## R8 — No licence chosen before public release

**Risk:** Hesitating on the licence question into M15 leaves us with either an unlicensed public binary (a problem) or a rushed decision.

**Mitigation:**
- The open question is logged with a deadline (start of M15).
- Compare options in advance — see [`../07-distribution/licensing.md`](../07-distribution/licensing.md).
- A minimum decision (even "MIT for now, may change later") is better than no decision.

## R9 — Code signing cost / Windows SmartScreen

**Risk:** First-time users see a SmartScreen warning on an unsigned installer and bounce.

**Mitigation:**
- Plan to obtain an EV cert before v1.0 if budget allows ($300–$500/year).
- If unsigned, add a prominent "How to install" section to the README explaining the SmartScreen prompt.
- Build reputation over multiple releases; eventually SmartScreen reputation can be earned without an EV cert.

## R10 — Sustained developer effort

**Risk:** ~6 months of part-time work needs steady momentum; a long gap mid-project usually means the project dies.

**Mitigation:**
- Small, demoable milestones (M1 produces sound; M5 produces a recognisable hybrid patch).
- Public visibility — early devlogs, a short demo video at M5, a small public alpha at M10 — to build accountability and community.
- The risk is the developer's, not the project's; this risk lives here as a reminder rather than something to "engineer around".

## R11 — Cross-thread parameter consistency bugs

**Risk:** Subtle races between UI changes, MIDI Learn, and modulation cause parameter values to drift or display incorrectly.

**Mitigation:**
- Single source of truth (engine parameter tree); UI reads snapshots, never owns state.
- Property tests for parameter round-trips and snapshot consistency.
- Manual test plan at M11 specifically targets edge cases (rapid clicks, MIDI Learn while modulating, preset change mid-modulation).
