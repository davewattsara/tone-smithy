# Changelog

All notable changes to Tone Smithy are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] — Unreleased

First public release. A hybrid (subtractive + FM) standalone software synthesizer
for Windows.

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

[Unreleased]: https://github.com/OWNER/REPO/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/OWNER/REPO/releases/tag/v1.0.0
