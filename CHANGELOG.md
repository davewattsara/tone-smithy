# Changelog

All notable changes to Tone Smithy are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] — 2026-06-20

### Added

- **Second sequencer mod lane (`Seq2`)** — the step sequencer gains a second
  independent CV mod lane, routable to its own destination via the mod matrix
  alongside the existing lane.
- **Editable FM operator routing (Custom algorithm)** — a ninth "Custom" FM
  algorithm with an on-screen routing grid: toggle each operator's carrier flag
  and the legal high->low modulator connections. Switching to Custom from a
  factory algorithm seeds the grid from it; routing is saved with the preset.
- **Per-oscillator phase mode (Free / Retrig)** — each main oscillator (OSC 1-3)
  gains a "Retrig" toggle. Free (the default) keeps the v1.1 behaviour of a
  random phase on note-on; Retrig resets the phase to 0 on every note-on for a
  tight, repeatable attack. Saved with the preset.
- **Update check** — on launch a background thread checks GitHub Releases for a
  newer version and, if one exists, shows a dismissible "Update available" notice
  in the header with a link to the releases page. Best-effort and non-blocking:
  no automatic download, no telemetry, and it fails silently when offline.
  Dismissing a version is remembered until a still-newer release appears.

### Changed

- **Init patch now starts with one main oscillator.** OSC 2 and OSC 3 default to
  level 0, matching the universal convention. Existing factory presets are
  unaffected (their OSC 2/3 levels were written in explicitly where needed).
  User-created presets that relied on OSC 2/3 being at full level by default may
  sound different — open them, set OSC 2/3 levels explicitly, and re-save.

## [1.1.1] — 2026-06-17

Packaging-only patch. No changes to the synth itself.

### Fixed

- **macOS third-party licences** — the macOS `.dmg` now ships the full
  `THIRD-PARTY-LICENSES.txt` instead of a placeholder. `xtask` resolves
  `cargo-about` via Cargo's bin directory when it isn't on `PATH` (as happened
  on the macOS CI runner).

### Changed

- **Release workflow** — publishing is now resilient to a single platform's
  build failing; the platforms that succeeded are still released, instead of the
  whole release being skipped.

## [1.1.0] — 2026-06-16

The second release. Expands the engine, adds a step sequencer, ships on Linux and
macOS alongside Windows, and roughly doubles the factory bank to ~120 presets.

### Added

- **Second filter** — an independent state-variable filter with **off / serial /
  parallel** routing (default off, so existing patches are unchanged) and a
  **12 or 24 dB/oct** slope selectable per filter.
- **Third envelope (Env3)** — a second assignable modulation envelope with
  per-stage curve control.
- **16-step sequencer** — per-step note, velocity, gate, rest, tie, and a CV mod
  lane, with forward/reverse/ping-pong/random modes, rate, and swing, driven by a
  unified transport BPM shared with the arpeggiator.
- **Larger modulation matrix** — expanded from 8 to **16 slots**, with new
  sources (Env3, the sequencer's mod lane) and new destinations (filter 2 cutoff
  and resonance, OSC2/OSC3 detune, and OSC2/OSC3 pan).
- **Global / mono LFO mode** — run an LFO as one shared phase across all voices.
- **Cross-platform installers** — Linux (`.tar.gz`) and macOS (`.dmg`, with a
  drag-installable app bundle and gated code-signing/notarization) join the
  Windows installer; a single `v1.1.0` tag publishes all three via CI.
- **~120-preset factory bank** — roughly doubled, including a Hammond
  tonewheel/Leslie organ family, grimy modern DnB basses (Reese, neuro, wobble),
  multi-filter and deep-modulation showcase patches, and a step-sequencer riff.

### Changed

- **Computer-keyboard note** — the keyboard's reference key now maps to C.
- **Preset browser** — presets are sorted alphabetically within each category.
- **Oscillator panel** — the OSC/Sub controls are shown conditionally to reduce
  clutter when a slot is in FM mode.
- **Original factory presets** — the M0-era patches were revised to use the new
  engine features (second filter, Env3, wider matrix) while keeping their sound.

## [1.0.0] — 2026-06-09

First public release. A hybrid (subtractive + FM) standalone software synthesizer
for Windows. Ships unsigned (see the README SmartScreen note); code signing and a
custom application icon are deferred to a later version.

### Added

- **Hybrid synth engine** — two per-voice slots, each independently subtractive
  (three unison main oscillators + sub) or 4-operator FM with eight algorithms.
- **Filter** — state-variable filter with low-pass, high-pass, band-pass, and
  notch modes.
- **Envelopes** — amp ADSR plus a second assignable modulation envelope (Env2)
  with per-stage curve control.
- **Two LFOs** — multiple shapes (sine, triangle, saw, square, S&H, smooth
  random) with free-run or BPM-sync.
- **8-slot modulation matrix** — sources (LFOs, Env2, amp env, velocity, key
  tracking, mod wheel, aftertouch, pitch bend) to destinations (cutoff,
  resonance, pitch, volume, osc detune/pan), with optional via-source scaling.
- **Effects chain** — EQ, drive, chorus, delay, and reverb.
- **Arpeggiator** — up/down/up-down/random/played modes, octave range, rate,
  gate, and swing, with BPM sync.
- **Preset browser** — categories, tags, search, and a user preset folder with
  live file-watching.
- **60-preset factory bank** — across Bass, Lead, Pad, Pluck, Keys, and FX, plus
  an Init patch; several respond to the mod wheel.
- **Settings + MIDI Learn** — audio device and MIDI port pickers with live
  switching, persisted settings, a first-run wizard, and right-click MIDI Learn
  on any knob.
- **Panic / all-notes-off** — header button plus MIDI CC 120/123 to clear stuck
  notes.
- **Standalone app** — low-latency `cpal` audio, `midir` MIDI input, and a
  computer-keyboard fallback.

[Unreleased]: https://github.com/davewattsara/tone-smithy/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/davewattsara/tone-smithy/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/davewattsara/tone-smithy/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/davewattsara/tone-smithy/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/davewattsara/tone-smithy/releases/tag/v1.0.0
