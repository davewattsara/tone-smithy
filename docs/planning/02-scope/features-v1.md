# v1 features

The full v1 feature set. Anything not listed here is out of scope — see [`out-of-scope.md`](out-of-scope.md) and [`roadmap.md`](roadmap.md).

## Synth engine (per voice)

### Oscillator slots
- Two **oscillator slots** per voice. Each slot can be configured independently as:
  - **Subtractive** — 3 oscillators (saw / square / triangle / noise) + 1 sub oscillator (sine, one octave below). Unison up to 7 voices with detune and stereo spread per oscillator.
  - **FM** — 4 operators, sine fundamental, freely-routable. Ships with 8 starter algorithms (DX7-family routings); a user-editable routing matrix is on the v1.2 roadmap (see [`roadmap.md`](roadmap.md)).
- Slot levels are mixed before the filter section. Each slot has independent level and pan.

### Filter section
- **One** multi-mode filter per voice (LP / HP / BP / Notch, 12 dB/oct).
- Cutoff, resonance, drive.
- Cutoff and resonance are first-class modulation destinations.

> **Deferred to v1.1:** second filter with serial/parallel routing; 24 dB/oct filter option.

### Amplifier
- Per-voice amp stage with ADSR envelope, master level, velocity sensitivity.

## Modulation

### Envelopes
- **Amp envelope** (always wired to amplitude).
- **1 additional envelope** (Env2) — ADSR with adjustable curves (lin / exp / log), freely assignable through the mod matrix.

> **Deferred to v1.1:** second mod envelope (Env3).

### LFOs
- **2 LFOs** per voice.
- Shapes: sine, triangle, saw up, saw down, square, S&H, smooth random.
- Free or tempo-synced; phase reset on note-on optional; per-voice or global mode.

### Modulation matrix
- **Any source to any destination**, with **8 slots in v1**.
- **Sources:** Amp env, Env2, LFO1, LFO2, MIDI velocity, key tracking, mod wheel, channel aftertouch, pitch bend, MIDI CC (assignable).
- **Destinations:** any continuous parameter (oscillator pitch/level/detune, filter cutoff/resonance, FM operator levels/ratios, FX parameters, LFO/env rates, etc.).
- Per-slot bipolar amount; per-slot via attenuator (a second source can scale the modulation depth).

> **Deferred to v1.1:** matrix expanded to 16 slots; Env3 added as a source.

## Arpeggiator

- Modes: up, down, up/down, random, played order. Octave range 1–4. Rate sync to BPM or free. Gate length. Swing.

> **Deferred to v1.1:** 16-step sequencer with notes, velocities, gates, and one assignable mod lane.

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

### Computer keyboard
- Standard layout: A W S E D F T G Y H U J for one octave plus accidentals. Z / X shift octave. C / V shift velocity preset.

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
