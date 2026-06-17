# Overview

## What we are building

**Tone Smithy** — a polished, standalone software synthesizer for Windows. The instrument is a **hybrid** — each voice can blend **subtractive (analog-style)** oscillators with **FM / phase-modulation** operators in the same patch. It is designed to be a flagship-class instrument: deep enough to make patches that are not possible on a single-engine synth, but with a UI clear enough to be approachable.

The product is the application itself: a single Windows executable distributed with an installer, a factory preset bank, and built-in audio + MIDI configuration. There is no DAW dependency for v1.

## Who it is for

- **Producers** who want one synth that covers analog warmth and FM brightness without switching plugins.
- **Sound designers** who appreciate a fully-routable modulation matrix and want to push the engine.
- **Hobbyists and musicians** who do not own (or do not want) a commercial flagship like Serum or Pigments and want something free that does not feel like a toy.
- **Live performers** who need an immediate, low-latency standalone instrument with a built-in arpeggiator/sequencer.

## Why it should exist

The free/open synth landscape is dominated by single-paradigm instruments (Surge XT is broadly capable; Vital is wavetable; Dexed is FM-only). There is room for a focused hybrid that treats **subtractive and FM as first-class peers in one voice**, with a modern flat UI that does not look like a re-skinned VST template.

The project is also a vehicle for high-quality, hand-rolled DSP — the goal is for the synth to have its own audible character, not to be a wrapper around generic building blocks.

## Shape of the v1/v1.1 product

- **Platform:** Windows 10/11, Linux, and macOS (Apple Silicon). All three platforms added by v1.1.
- **Distribution:** Free download. Freemium model is permitted (e.g. paid expansion preset packs) but no paywall on the core instrument.
- **Format:** Standalone executable. Not a plugin in v1 (CLAP/VST3 are roadmap items).
- **Engine:** Hybrid subtractive + FM, 32-voice polyphonic. Second filter, 24 dB/oct option, Env3, 16-slot matrix, and step sequencer added in v1.1.
- **Workflow:** Single window, modern flat UI, preset browser, modulation matrix, arpeggiator/sequencer, built-in effects chain.
- **Input:** External MIDI hardware, on-screen virtual keyboard, computer keyboard.

## Non-goals (v1)

- Plugin formats — deferred to v2.
- Sample/wavetable/granular synthesis — these are different products.
- Cloud / accounts / telemetry — none.

See [`../02-scope/out-of-scope.md`](../02-scope/out-of-scope.md) for the full deferred list and [`../02-scope/roadmap.md`](../02-scope/roadmap.md) for the post-v1 plan.
