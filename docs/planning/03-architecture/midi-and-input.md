# MIDI & input

How notes and control events reach the synth. Lives in `synth-host` with a small UI surface in `synth-ui`.

## Sources of input

1. **External MIDI hardware** — via `midir`.
2. **On-screen virtual keyboard** — egui-rendered keyboard at the bottom of the window.
3. **Computer (QWERTY) keyboard** — keys mapped to MIDI notes when the synth has keyboard focus.

All three sources feed the same internal event queue that the audio thread consumes.

## Event model

A single `EngineEvent` enum is consumed by the engine:

```
enum EngineEvent {
    NoteOn  { channel, note, velocity },
    NoteOff { channel, note, velocity },
    PitchBend { channel, value },
    ChannelAftertouch { channel, value },
    ModWheel { channel, value },
    Sustain { channel, on },
    Cc { channel, controller, value },
    ParameterChange { id, value },
    PresetChange { snapshot },
    TempoChange { bpm },
}
```

The MIDI thread, the GUI thread, and the timing/clock subsystem all push into a single multi-producer / single-consumer queue drained by the audio thread at the start of each callback.

## MIDI device handling

- **Enumeration** at startup and on user request. The settings UI lists all available input devices.
- **Hot-plug** — a background thread periodically polls `midir` for the device list and updates the UI; new connections are not auto-selected (avoids surprise input).
- **Selection** — user picks one or more inputs. Multiple inputs are merged into the same event queue.
- **Channel filter** — per input: Omni (all 16 channels) or a specific channel. Default Omni.
- **Running status, SysEx** — running status handled by `midir`. SysEx ignored in v1.
- **Clock / sync** — MIDI clock input is consumed for the sequencer/arp BPM source when "External" sync is selected. Defer MIDI Start/Stop/Continue handling to v1.1 if it introduces complexity.

## MIDI Learn

- Right-click any parameter → "MIDI Learn" → next incoming CC binds to that parameter.
- Bindings are stored per preset (most common) **and** as a global override layer (for users who keep the same controller across presets). UI exposes both.
- Bindings can be unassigned from the same context menu.

## Virtual keyboard

- Always visible at the bottom of the window, occupying a configurable octave range (default C2 – C6).
- Click triggers note-on at default velocity. Vertical click position can optionally vary velocity (off by default for simplicity).
- Sustains while mouse is held; releases on mouse up.
- Visually shows currently-playing notes from any source.

## Computer keyboard

- Standard layout:
  - White keys: A S D F G H J K = C D E F G A B C (one octave).
  - Black keys: W E   T Y U      = C# D#   F# G# A#.
- `Z` / `X` shift the keyboard octave down/up.
- `C` / `V` cycle through a small set of velocity presets (40 / 80 / 110 / 127).
- Active only when the synth window has focus **and** the user has not disabled it in settings (some users prefer the keys for shortcuts).
- Indicator in the footer shows whether QWERTY input is active.

## Note priority and re-trigger

- The engine is fully polyphonic in v1; there is no monophonic mode. (Mono / legato modes are a v1.x consideration.)
- Voice allocation: described in [`audio-engine.md`](audio-engine.md).

## Latency budget

Total target latency from key press to audio is **under 10 ms at a 256-sample buffer / 48 kHz**:

- MIDI driver: ~1–2 ms.
- Event queueing: <0.1 ms.
- Audio callback wait: up to a buffer length (~5.3 ms at 256/48k).
- Engine processing: <1 ms typical.
- Driver output: ~1–2 ms.

Lower buffer sizes are supported but not required for the target.

## Testing

- Mock MIDI input source for integration tests (no hardware needed).
- Automated test that injects 128 note-on/off events per second for 60 seconds and verifies no events are dropped and no allocations occur on the audio thread.
