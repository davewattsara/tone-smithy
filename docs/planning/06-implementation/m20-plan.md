# M20 plan — v1.1 factory expansion + release

The headline deliverable is **content**: grow the factory bank from 61 to ~120
presets, with a deliberate emphasis on patches that *show off the v1.1 engine* —
the second filter (off / serial / parallel, 12 & 24 dB/oct), Env3, the 16-slot
mod matrix, the step sequencer, and the OSC2/OSC3 pan/detune destinations. The
milestone ends with the **v1.1.0 release cut**, which publishes the three-platform
installers built in M19.

**Target version:** v1.1
**Estimate:** 2–4 weeks (mostly sound-design time, not code)
**Branch:** `milestone/m20-content` — *create off `development`.*

> **Prerequisites (hard):** M16, M17, M18, **and M19** must be merged to
> `development` first. M20 authors against the full v1.1 engine, and M19 in
> particular lands two things this milestone depends on: filter 2 defaulting to
> **off** (so new presets opt *in* to the second filter explicitly) and the
> `Osc2Pan` / `Osc3Pan` destinations. Do not start M20 authoring until M19 is
> closed out and the user has signed off on its installers.

> **Open question to resolve first:** *Factory content authoring approach*
> ([`../01-vision/open-questions.md`](../01-vision/open-questions.md)). This plan
> assumes **solo authoring (developer + Claude)**; confirm that and update the
> open-questions file before Phase 2.

---

## Overview

| Phase | Theme | Notes |
|---|---|---|
| 1 | Authoring system + validation guards | Template, QA checklist, coverage tests in `factory.rs` |
| 2 | Revise the original prefabs | Enrich M0-era patches with v1.1 features, keep their identity |
| 3 | Bass batch — incl. grimy/filthy DnB | Reese/neuro/wobble; drive + parallel filters + seq/LFO mod |
| 4 | Keys batch — incl. Hammond organ + Leslie | Drawbars, percussion, rotary-speaker via multi-slot modulation |
| 5 | Lead / Pluck / Pad / FX batches | Multi-filter (12 vs 24 dB) showcases; deep >8-slot evolving patches |
| 6 | QA & polish | Audition all, normalise levels, names/tags/descriptions, ordering |
| 7 | Release cut (v1.1.0) | CHANGELOG, version bump, merge to `main`, tag → release workflow |

The factory bank is **embedded at compile time**: each `.tsmith` lives in
`crates/synth-presets/factory/` and is registered with an `include_str!` line in
the `FACTORY_RAWS` array in `crates/synth-presets/src/factory.rs`, grouped by
category in display order. Every new preset is **two edits** — the file plus its
`FACTORY_RAWS` entry — and is then automatically exercised by the existing
factory tests (parse, unique name, valid category, audible-finite-bounded,
releases to near-silence). New presets must pass those tests as-is.

---

## The five showcase requirements (acceptance criteria)

These come directly from the milestone brief and are tracked as hard
acceptance criteria, not nice-to-haves:

1. **Hammond organ.** At least one convincing tonewheel-organ family in Keys,
   including a rotary-speaker (Leslie) treatment (Phase 4).
2. **Genuinely deep modulation.** Several patches use **more than 8** of the 16
   mod slots — but every enabled slot must earn its place (an audible, musical
   job). No filler slots. Each >8-slot preset documents, in its `description`
   (or an authoring note), what each extra slot is doing.
3. **Multiple-filter showcases.** Patches that demonstrate filter 1 + filter 2
   in **serial** and **parallel**, and that contrast **12 dB/oct vs 24 dB/oct**
   slopes (e.g. a 24 dB ladder bass, a parallel LP+HP or dual-band-pass formant).
4. **Grimy/filthy modern DnB.** Reese, neuro, growl, and wobble basses using
   drive (with asymmetry), parallel/serial filters, and fast LFO/seq-driven
   filter movement — while staying finite, bounded (≤ the test ceiling), and
   releasing cleanly.
5. **Improved originals.** Revisit the M0-era prefabs now that the engine is
   richer, enhancing them without losing the sound that made them recognisable.

Phase 1 adds **automated coverage guards** so these can't silently regress (see
below).

---

## Phase 1 — Authoring system + validation guards

Set up so the content phases are mechanical and self-checking.

- **Confirm the authoring approach** (open question above) and update
  `open-questions.md`.
- **Authoring template + checklist.** A short doc (under `docs/planning/05-design/`
  or a comment block) capturing the house style: sparse params (only store what
  differs from `Init` defaults — see [[feedback-preset-default-coupling]]),
  required metadata (name, author `"Factory"`, category, tags, one-line
  description), loudness target, and the mod-slot justification rule.
- **Target distribution.** Grow each category roughly proportionally while
  leaving room for the showcase families:

  | Category | Now | Target | New |
  |---|---:|---:|---:|
  | Bass | 15 | 30 | +15 |
  | Lead | 15 | 26 | +11 |
  | Pad | 12 | 22 | +10 |
  | Pluck | 8 | 14 | +6 |
  | Keys | 6 | 16 | +10 |
  | FX | 4 | 12 | +8 |
  | **Total (+ Init)** | **61** | **121** | **+60** |

  Counts are targets, adjustable during QA; "~120 total" is the bar.

- **Coverage guard tests** in `crates/synth-presets/src/factory.rs` (extend the
  existing test module): assert the new total and per-category distribution, and
  add feature-coverage assertions that decode each preset's param map and count
  how many presets:
  - set `filter_routing` to **Serial** and to **Parallel** (≥ a small threshold each);
  - set either `filter_slope_*` to **24 dB/oct** (index 1);
  - use the **Seq** mod source (index 11) and **Env3** (index 10);
  - enable **> 8** mod slots.

  These turn the showcase requirements into CI-enforced invariants.

- **Loudness check.** Reuse the audibility harness (`run_for`); optionally add a
  loose peak-in-range assertion so a wildly hot or near-silent patch fails fast.
  Final level-matching is by ear in Phase 6.

---

## Phase 2 — Revise the original prefabs

The earliest presets (`init`, `sub_bass`, `saw_lead`, `analog_pad`, `pluck`,
`keys`, and any thin M0/M1-era patches) predate filter 2, Env3, the 16-slot
matrix, the sequencer, and per-osc pan/detune mod. Enrich them **without changing
their core identity** — they should still read as "the saw lead" / "the analog
pad", just fuller and more alive.

- Examples: `analog_pad` gains a slow second-filter movement and gentle
  Env3-driven OSC2/OSC3 pan drift for width; `saw_lead` gains an optional 24 dB
  filter and subtle unison; `pluck` gains a touch of Env3 on filter 2. Keep
  `init` minimal — it's the blank-slate patch; leave it as the engine defaults.
- **A/B every revision** against the current version. Because factory presets are
  read-only embedded RON (not user data), editing the files in place is safe — no
  migration — but it *does* change what ships, so identity preservation is the QA
  bar here.

---

## Phase 3 — Bass batch (incl. grimy/filthy DnB)

~15 new basses plus a heavier rework of `Reese`. The DnB grime is the spotlight:

- **Reese family:** an improved `Reese`, plus `Reese Mk2` / `Distorted Reese`
  with parallel filters and drive asymmetry.
- **Neuro / growl / wobble:** rhythmic filter movement from the **step
  sequencer** mod lane (Seq → Filter1 cutoff) and/or sync'd LFO; `Env2` →
  cutoff for the per-note "ow"; `ModWheel`/`Velocity` → filter for playability.
- **Filter-slope showcases here too:** a **24 dB ladder bass** (serial, filter 1
  LP 24 dB), a **parallel LP+HP** split bass.
- All grime patches must pass the bounded/finite/release tests — drive + high
  resonance is where runaway lurks; keep `master_volume` and drive in check and
  level-match in Phase 6.

---

## Phase 4 — Keys batch (incl. Hammond organ + Leslie)

~10 new keys, anchored by a **tonewheel-organ family** — the clearest home for
both the "Hammond" and the ">8 mods, all justified" requirements.

- **Drawbars:** parallel-carrier FM (slot 1), op ratios voiced as harmonic
  drawbars (e.g. 1 / 2 / 3 / 4 with tapered levels), near-instant attack, flat
  sustain — building on the existing `FM Organ`.
- **Percussion + key click:** a high-ratio operator with a fast decay for the
  classic 2nd/3rd-harmonic percussion tab, and a short transient for click.
- **Leslie / rotary speaker** (the deep-modulation showpiece): approximate the
  rotating horn + drum with **chorus** plus a cluster of mod slots — e.g.
  `LFO1 → Osc1Pan` and `LFO1 → Osc2Pan` in opposition (rotation in the stereo
  field), `LFO1 → Volume` (amplitude tremolo), a small `LFO2 → PitchSemis`
  (doppler vibrato), with horn and drum on slightly different LFO rates. An
  **overdriven rock-organ** variant adds the **drive** FX and a 24 dB filter.
  Each of these slots has a named acoustic job — exactly the "no filler" rule.
- Round out Keys with other instruments (clav, extra Rhodes/Wurli voicings,
  pipe/combo organ) as count allows.

---

## Phase 5 — Lead / Pluck / Pad / FX batches

The remaining ~35 presets, carrying the rest of the multi-filter and
deep-modulation showcases:

- **Leads (~11):** a **parallel split-filter** lead (LP + HP blended), a
  **dual-band-pass formant/vocal** lead (filter 1 BP + filter 2 BP at different
  frequencies), a 24 dB screaming acid lead, FM bells/feedback voices.
- **Pads (~10):** at least one **deep evolving pad** using **> 8 justified
  slots** — e.g. `LFO1 → Filter1`, slow `LFO2 → Filter2` (counter-motion),
  `Env3 → Osc2Det` and `Env3 → Osc3Det` (opposed, for a slow chorus bloom),
  `LFO1 → Osc1Pan` / `LFO2 → Osc2Pan` / `Seq → Osc3Pan` (drifting width),
  `KeyTracking → Filter1`, `ModWheel → Filter2Res`, `Aftertouch → Volume`. Each
  slot is doing real work; document the rationale in the description.
- **Pluck (~6):** mallet/string/ethnic voices; per-osc pan for stereo body.
- **FX (~8):** risers, drops, drones, and textures driven by the **sequencer**
  (rhythmic gate/CV) and **Env3** for long automated sweeps.

---

## Phase 6 — QA & polish

- **Audition every preset** (all new + all revised) across the keyboard range and
  in a short musical context — not just "does it make noise".
- **Level-match** the bank by ear so switching presets doesn't jump in loudness;
  keep peaks well under the test ceiling.
- **Names, tags, descriptions:** consistent, searchable, one good sentence each;
  names chosen to sort sensibly (the browser supports alphabetical ordering as of
  M16).
- **Ordering:** keep `FACTORY_RAWS` grouped by category in display order, original
  patch first within each group (matches the existing comment structure).
- **Update the coverage/distribution tests** to the final counts; run
  `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, and `cargo run -p xtask -- check-deps`.

---

## Phase 7 — Release cut (v1.1.0)

- **CHANGELOG.md:** add the v1.1 entry (M16–M20 highlights: K=C keyboard note,
  alphabetical presets, conditional OSC/Sub panel, second filter + 24 dB + Env3 +
  16-slot matrix, step sequencer, Linux + macOS installers, ~120-preset bank).
- **Version bump:** workspace `Cargo.toml` `1.0.0 → 1.1.0` (xtask reads it for
  artefact naming).
- **README:** flip the active-milestone/status line and the "deferred to v1.1"
  scope note; confirm the feature list reflects the shipped engine.
- **Merge & tag:** `milestone/m20-content → development`, then
  `development → main` with `git merge --no-ff`, tag **`v1.1.0`** on `main`.
  Pushing the tag triggers `release.yml`, which publishes the Windows, Linux, and
  macOS packages (the M19 pipeline) to one GitHub Release.
- **User testing & sign-off** on at least Windows + one other platform
  ([[feedback-milestone-user-testing]]) **before** tagging.

---

## Sequencing & risks

- **Hard dependency on M19** (and M16–M18) being merged to `development`. M20 is
  almost entirely content + the release cut; very little new code (the test
  guards in Phase 1 and the version/CHANGELOG bumps in Phase 7).
- **Loudness / runaway:** the grimy/drive and high-resonance patches are the most
  likely to trip the bounded-output test or to be uncomfortably hot. Budget QA
  time; level-match late.
- **CPU:** > 8 active mod slots × 32 voices is well within budget, but spot-check
  the heaviest patches (deep pad, Leslie organ, unison + parallel filters) don't
  regress the audio thread.
- **Editing originals changes shipped sound.** Identity preservation + A/B is the
  guard (Phase 2). Sparse-preset coupling still applies — only store params that
  differ from defaults ([[feedback-preset-default-coupling]]).
- **Scope:** ~60 new presets is real sound-design effort; the 2–4 week estimate
  reflects authoring + QA, not engineering.

## Done when (milestone)

The factory bank reaches ~120 presets (target distribution met); the five
showcase requirements are satisfied and enforced by the coverage tests (Hammond
organ family incl. Leslie; several justified > 8-slot patches; serial/parallel
and 12/24 dB showcases; grimy DnB basses; revised originals); the full test
suite, `fmt`, `clippy -D warnings`, and `check-deps` are clean; `CHANGELOG.md`
and the version are bumped to v1.1.0; and the `v1.1.0` tag publishes the Windows,
Linux, and macOS installers to a GitHub Release after the user has tested on at
least two platforms and signed off.
