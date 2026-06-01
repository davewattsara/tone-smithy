# M9 — Arpeggiator

Branch: `milestone/m09-arpeggiator`
Status: **In progress**

## Goal

A reliable, glitch-free arpeggiator with five modes, 1–4 octave range, BPM or free-running rate,
gate length, and swing. No step sequencer (v1.1).

## Parameters

| Parameter      | Range / options                            | Default  |
|----------------|--------------------------------------------|----------|
| Enabled        | bool                                       | false    |
| Mode           | Up / Down / UpDown / Random / Played       | Up       |
| Octaves        | 1 – 4                                      | 1        |
| Rate           | 1/32 / 1/16 / 1/8 / 1/4 / 1/2 (per beat)  | 1/8      |
| Sync           | Internal BPM / External MIDI clock         | Internal |
| BPM            | 20.0 – 300.0                               | 120.0    |
| Gate           | 0.01 – 1.0 (fraction of step duration)    | 0.5      |
| Swing          | 0.5 – 0.75 (fraction of beat pair)        | 0.5      |

Swing of 0.5 = straight; 0.75 = maximum dotted-eighth feel. Applied to every other step
(odd steps lengthened, even steps shortened by the same amount).

## Architecture

The arpeggiator lives on the **audio thread** — it is a pure clock + note scheduler, no
allocation. It owns no voice state; it sends `NoteOn` / `NoteOff` events into the engine's
existing event queue each time a step fires.

```
UI / MIDI ──► ArpEngine (audio thread)
                 │
                 ├── holds: sorted note list, step index, phase accumulator
                 │
                 └── emits: NoteOn / NoteOff → VoicePool
```

### Key types

```rust
// crates/synth-engine/src/arp.rs
pub enum ArpMode { Up, Down, UpDown, Random, Played }
pub enum ArpRate { R32, R16, R8, R4, R2 }  // note values

pub struct ArpEngine {
    // Config (set from UI, read on audio thread)
    enabled: bool,
    mode: ArpMode,
    octaves: u8,           // 1–4
    rate: ArpRate,
    bpm: f32,
    gate: f32,
    swing: f32,

    // Runtime state
    held_notes: [u8; 32],  // MIDI note numbers, sorted/ordered
    held_count: usize,
    step_index: usize,
    phase: f32,            // 0.0–1.0 within current step
    gate_open: bool,
    current_note: u8,
    going_up: bool,        // for UpDown mode

    sample_rate_hz: f32,
    rng_state: u32,        // xorshift32 for Random mode, no std::rand on audio thread
}
```

### Note list management

- `note_on(midi_note)` — insert into `held_notes`, maintaining sort order for Up/Down/UpDown,
  or append for Played mode.
- `note_off(midi_note)` — remove from `held_notes`; if this was the active step note, do not
  immediately re-trigger (wait for next step boundary).
- When `held_count` drops to zero: send NoteOff for the currently sounding note, reset `phase`
  and `step_index`, stop until next note arrives.

### Clock / phase accumulator

Each call to `process(n_samples)`:

```
step_samples = (60.0 / bpm) * beats_per_step * sample_rate_hz
             with swing applied to alternate steps
phase += n_samples / step_samples
```

When `phase` crosses 1.0: advance step, send NoteOff for current note, compute next note,
send NoteOn. Gate-off is scheduled at `phase = gate` within the same step.

Swing: steps are paired (0,1), (2,3), … Step 0 of the pair gets `swing` fraction of the
pair's total duration; step 1 gets `(1 - swing)`.

### Rate table

| ArpRate | Beats per step |
|---------|---------------|
| R32     | 0.125         |
| R16     | 0.25          |
| R8      | 0.5           |
| R4      | 1.0           |
| R2      | 2.0           |

### Mode step logic

- **Up** — step through `held_notes` low→high, wrapping across octaves 0…octaves-1.
- **Down** — high→low, wrapping across octaves.
- **UpDown** — alternates direction at each end; the top and bottom notes are not repeated at
  the turn (standard behaviour).
- **Random** — xorshift32 picks a random index into the full expanded note list (notes × octaves).
- **Played** — steps through notes in the order they were pressed (insertion order), expanding
  across octaves high→low.

### Integration with Engine

`ArpEngine` is owned by `Engine`. On each `Engine::process_stereo` call:

1. Feed `n_frames` into `arp.process(n_frames, &mut event_queue)` before voice processing.
2. `ArpEngine::process` pushes `EngineEvent::NoteOn` / `NoteOff` into the queue.
3. The existing event-dispatch loop handles them exactly like external MIDI.

`Engine::handle` handles `ArpNoteOn` / `ArpNoteOff` (new variants) to update `ArpEngine`'s
held note list from incoming MIDI, and parameter-change events for all arp params.

## ParamId additions

```
ArpEnabled, ArpMode, ArpOctaves, ArpRate, ArpBpm, ArpGate, ArpSwing  (7 total)
```

Mode and Rate are stepped (integer discriminant stored as f32).

## File layout

```
crates/synth-engine/src/arp.rs        ArpEngine + ArpMode + ArpRate
crates/synth-engine/src/engine.rs     integrate ArpEngine, new ParamId arms
crates/synth-engine/src/params.rs     7 new ParamId variants + snapshot fields
crates/synth-ui/src/app.rs            arp_panel() — enable toggle + 6 controls
```

## Implementation order

1. `arp.rs` — `ArpEngine` with full mode/clock/gate logic, unit tested
2. `params.rs` — 7 new ParamIds
3. `engine.rs` — integrate: owned field, `process_stereo` hook, `handle` arms
4. `app.rs` — UI panel

## Test plan (unit)

- Up mode steps through notes in correct pitch order across octaves.
- Down mode steps in reverse.
- UpDown reverses at endpoints without repeating boundary notes.
- Random never picks an out-of-range index.
- Gate-off fires at the correct fractional phase.
- With held_count=0, no NoteOn events are emitted.
- Swing at 0.5 produces equal step durations; at 0.75 the odd steps are 1.5× longer.
- BPM change takes effect at the next step boundary (no mid-step tempo jump).
