# M19 plan — Cross-platform installers (+ bundled engine pre-reqs)

The headline feature is extending the release pipeline to **Linux and macOS** so Tone Smithy ships
on all three desktop platforms. The milestone also carries two small, backward-compatible engine
changes that must land **before the M20 factory-preset authoring** (so new presets are designed
against them): making **filter 2 optional** (off / serial / parallel) and adding **OSC2 / OSC3 pan
mod destinations**. Both are bundled here only to gate them ahead of M20 — they are not installer
work.

**Target version:** v1.1
**Estimate:** 2–3 weeks (installers) + ~1 week (bundled engine pre-reqs)
**Branch:** `milestone/m19-installers` — *create off `development`.*

> Prerequisite met: M18 is closed out (merged to `main`, tag `m18`, 2026-06-16). Branch M19 off the
> current `development`.

---

## Overview

| Phase | Feature | Notes |
|---|---|---|
| 1 | Filter 2 optional (off/serial/parallel) | Engine pre-req for M20; default **off** restores v1.0 preset sound |
| 2 | OSC2 / OSC3 pan mod destinations | Engine pre-req for M20; append `Osc2Pan` / `Osc3Pan` to `ModDest` |
| 3 | Linux package + CI | `ubuntu-latest`; AppImage and/or `.tar.gz`; cpal/ALSA/PipeWire, midir |
| 4 | macOS package + CI | `macos-latest` (Apple Silicon); `.dmg`; CoreAudio/CoreMIDI; sign/notarize if cert |
| 5 | `xtask dist` + release workflow + docs | `--target` support; three-platform artefacts; README/getting-started |

Phases 1 and 2 are independent engine changes — land them first (each its own commit) so they are
available for the rest of the milestone and for M20. Phases 3–5 are the installer work proper and
are mostly CI / packaging.

---

## Phase 1 — Make filter 2 optional (off / serial / parallel)

### Background

Filter 2 (added in M17) is always in the signal path: `FilterRouting` is **Serial / Parallel**
only, and default Serial runs `filter1 → filter2` per voice with no bypass (`voice.rs`). At its
defaults (LowPass, 20 kHz, no resonance) it is near-transparent but **not bit-identical**, and it
costs two extra SVFs per voice. Every v1.0 preset predates filter 2 and omits `filter_routing`, so
it currently renders through filter 2.

### Changes

- **`FilterRouting` enum** (engine): add an **`Off` variant at index 2** — keep `Serial = 0`,
  `Parallel = 1`; do **not** renumber. Update `from_f32` / any `from_index` and the variant count.
- **Default routing → `Off`.** Set the `FilterRouting` param default to `Off` so presets that omit
  `filter_routing` (all v1.0 presets) load with filter 2 bypassed — restoring their original sound
  and skipping two SVFs per voice. **This is a default change → audit factory presets** that rely on
  the old default (see [[feedback-preset-default-coupling]]); any factory preset that intends serial
  filter 2 must store `filter_routing` explicitly.
- **`voice.rs`:** when routing is `Off`, output filter 1 only — skip all filter-2 processing (no SVF
  evaluation, no parallel mix).
- **UI** (`filter.rs`): the routing control becomes a three-way **Off / Serial / Parallel** selector
  (display order independent of the stored index, like the mod-source ordering). When `Off`, grey out
  the filter 2 controls (mode, cutoff, resonance, slope) via `add_enabled_ui`.

### Done when

Filter 2 defaults to off; old presets sound like v1.0 again and skip the extra SVFs; the filter 2
panel is disabled while routing is Off; existing serial/parallel presets still load and behave as
before; round-trip preserves the routing value; `fmt` / `clippy -D warnings` / tests clean.

---

## Phase 2 — OSC2 / OSC3 pan mod destinations

### Background

Only OSC1 pan is a mod-matrix destination today (`Osc1Pan`, index 5). OSC2 and OSC3 pan can be set
statically but not modulated. This mirrors M18's `Osc2Det` / `Osc3Det` detune destinations — the
same plumbing path, applied to pan. The pan base params (`osc_main_pans`) already exist, so this is a
**mod destination only**: no new `ParamId`, no preset-format change beyond two appended dest indices.

### Changes (one commit, mirrors the M18 detune work)

1. **`mod_matrix.rs`:**
   - Add `Osc2Pan`, `Osc3Pan` to `ModDest` after `Osc3DetuneCents` (indices **10 / 11**); bump
     `ModDest::COUNT` 10 → 12; add `10 => Osc2Pan`, `11 => Osc3Pan` to `from_index`.
   - Add `osc2_pan`, `osc3_pan` to `DestOffsets`; add the match arms in `compute_offsets`
     (`ModDest::Osc2Pan => out.osc2_pan += contribution`, likewise for 3).
   - Add a `osc2_and_osc3_pan_are_independent` unit test mirroring the detune one.
2. **`voice_manager.rs`** (next to the existing `osc1_pan` line, ~`690`): apply
   `off.osc2_pan` to `osc_main_pans[1]` and `off.osc3_pan` to `osc_main_pans[2]`, each clamped ±1.
3. **`app/state.rs`:** append `"Osc2Pan"`, `"Osc3Pan"` to `MOD_DEST_LABELS` and `1.0`, `1.0` to
   `MOD_AMOUNT_RANGES` (the guard test asserts both stay the same length as `ModDest::COUNT`).
4. **`app/mod_display.rs`:** add `osc2_pan` / `osc3_pan` fields; handle dest indices 10/11
   (normalise by `2.0`, like `osc1_pan`); add their clamps.
5. **`sections/osc.rs`:** fix the pan knob's `.mod_offset` to select the per-oscillator entry
   `[m.osc1_pan, m.osc2_pan, m.osc3_pan][idx]` (it currently hard-codes `osc1_pan` for all three —
   the same bug M18 fixed on the detune knob).

### Done when

Routing an LFO → `Osc2Pan` pans only OSC2 (OSC1/OSC3 stay put); `Osc3Pan` moves OSC3 independently;
the OSC2/OSC3 pan knobs animate their mod ring; old presets load unchanged (no migration); the
mod-table guard test passes; `fmt` / `clippy -D warnings` / tests clean.

---

## Phase 3 — Linux package + CI

- **CI job on `ubuntu-latest`.** Build `--release`; install the system deps a clean build needs
  (ALSA dev headers for `cpal`; any X11/Wayland libs `egui`/`winit` require).
- **Package** as an **AppImage** (self-contained, runs on most distros) and/or a `.tar.gz` of the
  binary + assets. Audio via `cpal` (PipeWire/ALSA); MIDI via `midir`.
- **Test** on a clean **Ubuntu 24.04** VM: download, launch, load and play a preset with no extra
  setup. Confirm audio device enumeration and MIDI input work.
- Verify the `.tsmith` file association story on Linux (desktop entry / MIME) or document it as
  manual if out of scope for v1.1.

---

## Phase 4 — macOS package + CI

- **CI job on `macos-latest`** (Apple Silicon). Build `--release`.
- **Package** as a **`.dmg`** with a normal `.app` bundle (Info.plist, icon, bundle id). Audio via
  CoreAudio; MIDI via CoreMIDI (both through `cpal` / `midir`).
- **Sign + notarize** if a Developer ID certificate is available (gate on a secret, like the Windows
  `TONESMITHY_CERT` path); otherwise ship unsigned with documented Gatekeeper-bypass instructions.
- **Test** on a clean macOS machine: download, open, play a preset; note any Gatekeeper prompts.

---

## Phase 5 — `xtask dist`, release workflow, docs

- **`cargo xtask dist`:** accept a `--target` flag (or branch per host OS) so each platform produces
  its package, **without breaking the existing Windows path** (Inno Setup installer remains).
- **`release.yml`:** add the Linux and macOS jobs so a `v1.1.0` tag publishes Windows, Linux, and
  macOS artefacts to the same GitHub Release.
- **Docs:** update `README.md` (download/build/run for three platforms, new system deps), the
  getting-started doc, and any platform notes. README triggers fire here (build/run commands + new
  system dependency).

---

## Sequencing & risks

- Phases 1 and 2 are pure engine/UI and have no dependency on the installer work — do them first so
  they are merged and testable, and so M20 can rely on them.
- Phases 3–5 are CI-heavy and iterate through the GitHub Actions runners; expect several CI round
  trips to get system deps and packaging right. Keep the Windows path green at every step.
- Both bundled engine changes are **append-only** to `ModDest` / `FilterRouting` (no renumbering), so
  the preset format stays backward-compatible — but the **filter-routing default change** alters how
  default-omitting presets render and must be paired with a factory-preset audit
  ([[feedback-preset-default-coupling]]).
- Close-out follows the standard flow (merge `milestone/m19-installers` → `development` → `main`
  `--no-ff`, tag `m19`) after the user has tested the installers on at least one non-Windows
  platform ([[feedback-milestone-user-testing]]).

## Done when (milestone)

A clean Linux and macOS machine can download the respective package, launch Tone Smithy, and play a
preset with no extra setup; filter 2 defaults to off (v1.0 presets restored); `Osc2Pan` / `Osc3Pan`
modulate independently; Windows packaging still works; `fmt` / `clippy -D warnings` / full test
suite clean.
