# Success criteria

A v1 release is "ready" when these objective criteria are met. They are intentionally measurable so that "done" is not a matter of opinion.

## Sound

- The factory bank contains at least **120 presets** spanning Bass, Lead, Pad, Pluck, Keys, and FX categories.
- A blind listening session with at least three musicians can identify which presets are subtractive, which are FM, and which are hybrid — i.e. each engine has its own audible voice.
- No oscillator aliases audibly across the MIDI playable range (C-2 to G8) at 48 kHz with default settings.
- FM operators sound clean at extreme modulation indices (no zipper noise, no obvious quantisation artefacts).

## Performance

- **32 voices** of moderate-complexity patches play simultaneously below **50% CPU** on a mid-range Windows laptop (reference target: a 2022-era 8-core machine, single thread of the audio engine).
- **Input-to-output latency under 10 ms** at a 256-sample buffer / 48 kHz, including UI overhead.
- **No allocations on the audio thread** in steady state, verified by a CI test using `assert_no_alloc` or equivalent.
- **No audio dropouts** during a one-hour stress test (random preset changes every 5 s, sustained 32-voice playback).

## Stability

- The app survives an 8-hour soak test (sequencer running, preset switching every 30 s) without crashing or leaking memory.
- All preset loads round-trip: saving and reloading a preset yields an identical parameter state.
- A corrupted preset file produces a clear error message and is skipped — it does not crash the browser.

## UX

- New install to first-sound under **60 seconds** including the audio device picker.
- Every parameter has a tooltip, a default value, a "reset to default" right-click action, and a MIDI Learn option.
- Window is resizable; UI remains usable from 1280×720 up to 4K.
- Keyboard focus and shortcuts allow navigating major sections without a mouse.

## Distribution

- Single Windows installer (Inno Setup or MSI), under **100 MB**.
- Installer adds Start Menu entry, file association for the preset format, and clean uninstall.
- A first-run wizard offers to pick an audio device and MIDI input.

## Definition of "v1.0"

All of the above, plus:
- No known crash bugs.
- No P1 or P2 issues open in the tracker.
- Release notes published, factory bank shipped, installer signed (or, if signing is deferred, a clear "unsigned build" message in the readme).
