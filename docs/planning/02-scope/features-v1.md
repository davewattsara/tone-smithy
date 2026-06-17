# v1 features

The full v1 feature set. Anything not listed here is out of scope — see [`out-of-scope.md`](out-of-scope.md) and [`roadmap.md`](roadmap.md).

## Synth engine (per voice)

### Oscillator slots
- Two **oscillator slots** per voice with fixed roles:
  - **Slot 1 — Subtractive** — 3 oscillators (saw / square / triangle / noise) + 1 sub oscillator (sine, one octave below). Unison up to 7 voices with detune and stereo spread per oscillator.
  - **Slot 2 — FM** — 4 operators, sine fundamental, freely-routable. Ships with 8 starter algorithms (DX7-family routings); a user-editable routing matrix is on the v1.2 roadmap (see [`roadmap.md`](roadmap.md)).
- Slot levels are mixed before the filter section. Each slot has independent level and pan.

### Filter section
- **Two** multi-mode filters per voice (LP / HP / BP / Notch, 12 or 24 dB/oct). Routing: Off / Serial (F1 → F2) / Parallel (F1 ∥ F2 averaged). Filter 2 defaults to Off.
- Cutoff, resonance, drive.
- Cutoff and resonance are first-class modulation destinations on both filters.

> **v1.0 shipped:** one filter (12 dB/oct). Second filter with serial/parallel routing and 24 dB/oct option delivered in v1.1 (M17).

### Amplifier
- Per-voice amp stage with ADSR envelope, master level, velocity sensitivity.

## Modulation

### Envelopes
- **Amp envelope** (always wired to amplitude).
- **2 additional envelopes** (Env2, Env3) — ADSR with adjustable curves (lin / exp / log), freely assignable through the mod matrix.

> **v1.0 shipped:** Env2 only. Env3 delivered in v1.1 (M17).

### LFOs
- **2 LFOs** per voice.
- Shapes: sine, triangle, saw up, saw down, square, S&H, smooth random.
- Free or tempo-synced; phase reset on note-on optional; per-voice or global mode.

### Modulation matrix
- **Any source to any destination**, with **16 slots**.
- **Sources:** Amp env, Env2, Env3, LFO1, LFO2, MIDI velocity, key tracking, mod wheel, channel aftertouch, pitch bend, MIDI CC (assignable), step sequencer mod lane (Seq).
- **Destinations:** any continuous parameter (oscillator pitch/level/detune per OSC, filter cutoff/resonance for both filters, FM operator levels/ratios, FX parameters, LFO/env rates, etc.).
- Per-slot bipolar amount; per-slot via attenuator (a second source can scale the modulation depth).

> **v1.0 shipped:** 8 slots, Env3 and Seq sources absent. Expanded to 16 slots, Env3 and Seq added in v1.1 (M17/M18).

## Arpeggiator

- Modes: up, down, up/down, random, played order. Octave range 1–4. Rate sync to BPM or free. Gate length. Swing.

## Step sequencer

- 16 steps with per-step: note offset (±24 st), velocity, gate, rest, tie, and one assignable mod lane.
- Playback modes: forward, reverse, ping-pong, random. BPM shared with the Master-tab transport.
- Sequencer and arpeggiator are mutually exclusive (one note engine active at a time).

> **v1.0 shipped:** arpeggiator only. Step sequencer delivered in v1.1 (M18).

## Effects (post-mix, fixed insert chain)

Order: EQ → Drive → Chorus → Delay → Reverb.

- **EQ** — 3-band: low shelf, parametric mid, high shelf. Frequency, gain, Q for the mid.
- **Drive** — soft tanh saturator with drive amount, tone, and asymmetry.
- **Chorus** — 3-tap, two-LFO modulated, rate / depth / mix / stereo width.
- **Delay** — stereo, sync or free, feedback, ping-pong toggle, low/high cut on the feedback path.
- **Reverb** — algorithmic (FDN-8 starting point), size, decay, damping, pre-delay, mix.

Each FX has a bypass toggle and is fully mod-matrix-addressable for its key parameters.

## Preset browser

- Categories: Bass, Lead, Pad, Pluck, Keys, FX.
- Free-form tags.
- Search by name, author, tag, or category.
- Factory vs User separation. Factory is read-only.
- Save, Save As, Rename, Delete, Duplicate. Import / export single preset files via the OS file dialog.
- A small comment / description field per preset.

## Input

### MIDI hardware
- Device enumeration, hot-plug detection.
- Notes (with velocity), pitch bend, mod wheel, sustain, channel aftertouch, arbitrary CC.
- Per-input channel filter (omni by default).

### On-screen virtual keyboard
- Always-visible at the bottom of the window. Configurable octave range. Velocity from vertical click position (optional, default uniform).
- On-screen pitch bend strip (vertical slider, springs back to centre on release) and sustain toggle button alongside the keyboard, so the synth is fully playable without MIDI hardware.
- Computer-keyboard keys are highlighted on the virtual keyboard while held, and the virtual keyboard scrolls to match the computer keyboard's current octave.

### Computer keyboard
- Standard layout: A W S E D F T G Y H U J K for one octave plus accidentals (K = C one octave above A). Z / X shift octave. C / V shift velocity preset.

### MIDI Learn
- Right-click any parameter → "MIDI Learn" → move a hardware control → mapping persisted in the patch.

## Audio I/O

- **Driver:** WASAPI primary (always available on Windows 10/11). ASIO secondary, behind a feature flag (see risks doc — vendoring the ASIO SDK is non-trivial).
- **Settings:** device picker, sample rate (44.1 / 48 / 96 kHz), buffer size selector (64 / 128 / 256 / 512 / 1024).
- **Live switching** between devices/buffer sizes without restarting the app.

## Polyphony and performance

- **32 voices** of polyphony.
- Voice stealing: oldest released voice first, then quietest.
- Per-block (not per-sample) DSP processing where feasible; SIMD-friendly inner loops for hot paths.

## Application-level

- Single resizable window. Minimum size 1280×720. High-DPI aware.
- Settings persisted between sessions (audio device, MIDI input, last preset).
- Crash log written to the user data directory if the app exits abnormally.
- About dialog with version, build hash, third-party licence notices.
