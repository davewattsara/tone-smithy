# DSP & sound design

The sonic intent of the instrument and the design decisions that get us there. Engineering details are in [`../03-architecture/audio-engine.md`](../03-architecture/audio-engine.md).

## Sonic intent

A single sentence: **warm, precise, with two distinct voices — analog-style subtractive that breathes, and clean, bell-clear FM — that can be combined in one patch.**

The instrument should:

- Sound *good* on the very first preset, before any user changes — boring presets are a worse failure than ugly ones.
- Have a recognisable character across the factory bank. A blindfolded listener should be able to say "that's the same synth" across different categories.
- Reward exploration. Hidden depth in the modulation matrix and the FM operator routing should let advanced users do things the factory bank only hints at.
- Sit well in a mix. Most of the factory bank should not need EQ surgery to drop into a track.

## Oscillator design (subtractive slot)

- **Anti-aliased** saw and square via PolyBLEP. Triangle generated from integrated square or polynomial — pick whichever has fewer artefacts under modulation.
- **Slight analog drift** baked in: each oscillator has a tiny, slow random pitch modulation (a few cents max) that simulates analog tuning drift. Off in pure-digital patches via a global toggle.
- **Sub oscillator** as a separate operator (always a sine, always an octave down) — most subtractive patches benefit and most synths make you sacrifice an oscillator for sub. We don't.
- **Unison** up to 7 voices per oscillator, with detune (cents) and stereo spread. Phase relationships randomised on note-on so unison doesn't comb-filter.

## Operator design (FM slot)

- **4 operators**, pure sine sources, with arbitrary routing through one of **8 factory algorithms** (DX-family). User-editable routings deferred to v1.2.
- Each operator has its own envelope, ratio (integer + fine), and level — this is the part that defines FM character.
- **Feedback** on op 4 only in the factory algorithms (keeps things bright; full feedback matrix is v1.1).
- **2× internal oversampling** kicks in when an operator's instantaneous modulation index crosses a threshold, then downsamples through a steep low-pass to keep the audible band clean.

## Hybrid voice

The key creative move: in a single voice you have **slot 1** and **slot 2**, each free to be subtractive or FM. Both run through the same filter section, the same envelopes, and the same effects chain.

Patches we want to be easy:

- Warm analog pad in slot 1 + glassy FM shimmer in slot 2.
- FM bell in slot 1 + sub-saw underneath in slot 2 (the classic "Rhodes plus warmth" trick).
- Two layered FM patches at different ratios for harmonic complexity beyond what one 4-op slot can do.
- Pure subtractive Juno-style (slot 1 alone, slot 2 muted) — must sound good even when "only half" the engine is in use.

## Filter design

- **Topology-preserving transform state-variable filter (TPT SVF)**, 12 dB/oct.
- Modes available continuously crossfaded: LP, BP, HP, Notch.
- **Self-oscillation** at high resonance — the filter should sing.
- **Per-filter drive** pre-stage with soft tanh saturation — adds harmonics into the filter input, which is where a lot of "analog character" comes from.

Two filters per voice from v1.1. The second filter (with serial `F1 → F2` and parallel `F1 ∥ F2 summed` routing) defaults to Off, restoring v1.0 signal flow for old presets. The 24 dB/oct option uses a cascaded 2-pole SVF. Both delivered in v1.1 (M17).

## Envelopes

- **ADSR** with per-stage curve (linear ↔ exponential, continuously variable). Linear curves on attack feel snappy; exponential curves on decay/release feel natural.
- **Velocity sensitivity** on the amp envelope (others modulate via the matrix).
- **One-shot** mode (skip sustain) for percussive patches.

## LFOs

- 7 shapes (sine, triangle, saw up, saw down, square, S&H, smooth random).
- Free or BPM-synced. Multiple sync divisions (1/16, 1/8T, 1/4, etc.).
- **Phase reset on note-on** is a per-LFO option — some patches want stable, deterministic modulation; others want phase to drift.
- **Per-voice or global** scope. Global LFOs are useful for slow filter sweeps across all held notes.

## Modulation matrix

- **16 slots** (raised from 8 in v1.0 to 16 in v1.1).
- **Bipolar amount** (sources can subtract as well as add).
- **Via attenuator** — a second source scales each slot's depth. Critical for "mod wheel adds vibrato" patches, and a feature missing from many free synths.
- Sources include MIDI velocity, aftertouch, mod wheel, key tracking, pitch bend, the amp env, Env2, Env3 (v1.1), the two LFOs, and the step sequencer mod lane (Seq, v1.1).

## Arpeggiator

- The everyday tool — modes (up, down, up/down, random, played order), octave range, rate, gate length, swing.
- Syncs to host BPM (free-running) or to external MIDI clock when enabled.

The 16-step sequencer (with note offset, velocity, gate, tie, rest, and one assignable mod lane) was delivered in v1.1 (M18).

## Effects

A fixed insert chain. Order chosen to mirror what most engineers do in a channel strip:

`EQ → Drive → Chorus → Delay → Reverb`

- **EQ** — low shelf, parametric mid (freq, gain, Q), high shelf. Subtle, not a corrective EQ.
- **Drive** — soft tanh with pre-gain, tone (post-tilt), and asymmetry control for even-harmonic flavour.
- **Chorus** — 3-tap delay modulated by two LFOs at slightly offset rates. Stereo width control. Useful from "gentle warmth" to "lush 80s".
- **Delay** — stereo, sync or free. Per-tap low/high cut on the feedback path so delays sit in the mix without turning to mud. Ping-pong toggle.
- **Reverb** — FDN-8 starting point. Size, decay, damping (low/high), pre-delay, mix. A plate variant may be added later.

Each effect has bypass and is mod-matrix-addressable on its key parameters (mix amount on all, plus filter freq on chorus/delay, decay on reverb, drive on drive, mid gain on EQ).

## Factory bank design

- **~120 presets** total (61 shipped with v1.0 in M14; expanded to 120 in v1.1 M20).
- Categories: Bass, Lead, Pad, Pluck, Keys, FX.
- Each preset has a clear identity — never two patches that are minor variations of each other.
- Each category should include at least three "demo" presets that showcase the synth's best behaviour for that role.
- Naming convention: `<Category> - <Descriptive Name>` (e.g. "Bass - Wool Stack", "Lead - Glass Bell").
- Authoring: solo (developer + Claude), resolved at M14 and continued through M20.

## Testing for sound quality

- **Reference comparisons** — blind A/B against analog and FM synths covering the same patch goals; the test is "would you be embarrassed to switch from theirs to ours mid-session?"
- **Sweep tests** — every oscillator, filter, and FM operator scanned across its parameter range; check for clicks, NaNs, sudden amplitude jumps.
- **Long-decay tests** — every reverb / delay / pad preset rendered for 30+ seconds; check the tail for buildup, denormals, or drift.
- **Spectrum snapshots** — for a small set of canonical presets, save reference spectra; CI fails if the spectrum changes by more than a tolerance (catches accidental DSP regressions).
