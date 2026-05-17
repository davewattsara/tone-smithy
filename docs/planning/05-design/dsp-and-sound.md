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

- **4 operators**, pure sine sources, with arbitrary routing through one of **8 factory algorithms** (DX-family). User-editable routings deferred to v1.1.
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

- **Topology-preserving transform state-variable filter (TPT SVF)** as the workhorse.
- 12 dB/oct and 24 dB/oct variants; the 24 dB option is a 4-pole ZDF ladder (selected during M2 based on listening tests).
- Modes available continuously crossfaded: LP, BP, HP, Notch.
- **Self-oscillation** at high resonance — the filter should sing.
- **Per-filter drive** pre-stage with soft tanh saturation — adds harmonics into the filter input, which is where a lot of "analog character" comes from.

Two filters per voice, with serial (`F1 → F2`) or parallel (`F1 ∥ F2 summed`) routing. Together they cover everything from clean low-pass to formant-style band-pass duals.

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

- **16 slots** is a sweet spot — enough for adventurous patches, few enough to keep the UI scannable.
- **Bipolar amount** (sources can subtract as well as add).
- **Via attenuator** — a second source scales each slot's depth. Critical for "mod wheel adds vibrato" patches, and a feature missing from many free synths.
- Sources include MIDI velocity, aftertouch, mod wheel, key tracking, pitch bend, and every internal envelope/LFO.

## Arpeggiator and step sequencer

- **Arp** is the everyday tool — modes, octave range, rate, gate length, swing.
- **Step sequencer** is the deeper tool — 16 steps with note offset (relative to held note), velocity, gate, and one assignable mod lane.
- Both sync to host BPM (free-running) or to external MIDI clock when enabled.

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

- Target **~120–150 presets** at v1.0.
- Categories: Bass (~30), Lead (~30), Pad (~25), Pluck (~20), Keys (~15), FX (~15).
- Each preset has a clear identity — never two patches that are minor variations of each other.
- Each category should include at least three "demo" presets that showcase the synth's best behaviour for that role.
- Naming convention: `<Category> - <Descriptive Name>` (e.g. "Bass - Wool Stack", "Lead - Glass Bell").
- Authoring approach decided at M14 — solo, recruit a sound designer, or run an open call.

## Testing for sound quality

- **Reference comparisons** — blind A/B against analog and FM synths covering the same patch goals; the test is "would you be embarrassed to switch from theirs to ours mid-session?"
- **Sweep tests** — every oscillator, filter, and FM operator scanned across its parameter range; check for clicks, NaNs, sudden amplitude jumps.
- **Long-decay tests** — every reverb / delay / pad preset rendered for 30+ seconds; check the tail for buildup, denormals, or drift.
- **Spectrum snapshots** — for a small set of canonical presets, save reference spectra; CI fails if the spectrum changes by more than a tolerance (catches accidental DSP regressions).
