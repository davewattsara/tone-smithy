# M3 implementation plan — MIDI input + polyphony

Companion to the M3 entry in [`milestones.md`](milestones.md). This doc is the **resumable** plan: an agent picking up the milestone at any point should be able to read this, see where things stand, and continue. Update the **Status** field on each sub-milestone as work lands.

## Scope recap

From [`milestones.md`](milestones.md):

- `midir` integration in `synth-host`.
- Device enumeration, selection, hot-plug detection.
- Notes (with velocity), pitch bend, mod wheel, sustain, channel aftertouch, arbitrary CC.
- Computer keyboard input (AWSEDFTGYHUJ layout, Z/X for octave).
- Voice manager with fixed-size voice array (32).
- Voice stealing (oldest released, then quietest).
- Note-off and amp-envelope-driven voice release.

**Done when:** a connected MIDI controller plays polyphonically; chords sustain; pitch bend and mod wheel work; computer keyboard works when focused. CPU well below 50% with 32 simple voices.

## Branch

Milestone branch: **`milestone/m03-midi-polyphony`** (branched off `development`). Sub-milestones commit directly on this branch; final merge follows the standard close-out flow in [`../../working-conventions.md`](../../working-conventions.md#milestone-completion-‐-merge-development-to-main).

## Order and rationale

Strict sequence — each sub-milestone depends on the previous one's behaviour being correct:

1. **M3.0 — VoiceManager** first. Everything else routes notes through it; without it, M3.1 onwards would all stub against a single voice and need rewiring.
2. **M3.1 — Computer keyboard** before MIDI hardware. Pure egui input → existing SPSC event queue, no platform-specific `midir` thread to debug at the same time as the polyphony refactor. Gives an end-to-end test of M3.0 from inside the app.
3. **M3.2 — MIDI hardware (notes)**. Once the keyboard proves the polyphony plumbing, swap in `midir` as a second input source.
4. **M3.3 — MIDI controllers**. CC routing, pitch bend, sustain pedal. Sustain reaches back into the voice manager (defer note-off while held), which is why it lands after M3.0 is settled.
5. **M3.4 — Architecture review / lock-in** mirroring M2's pattern. Freeze the MIDI event surface and `VoiceManager` API before M4 (the panel UI) builds on them.

Alternative considered: MIDI before keyboard. Rejected because it bundles two unfamiliar areas (32-voice polyphony correctness *and* cross-platform MIDI thread) into the same debug session.

## Sub-milestones

### M3.0 — VoiceManager

**Status:** Done (`3e4a018`).

**Scope.** Replace `Engine::voice: Voice` with a fixed-size 32-voice array behind a `VoiceManager` that owns allocation, stealing, and per-block summing.

**Files touched.**
- `crates/synth-engine/src/voice_manager.rs` — new file. The `VoiceManager` type.
- `crates/synth-engine/src/lib.rs` — re-export `VoiceManager`, add `POLYPHONY: usize = 32` top-level constant.
- `crates/synth-engine/src/engine.rs` — switch `voice: Voice` → `voices: VoiceManager`. Update `Engine::handle` so `NoteOn`/`NoteOff` go through the manager. `Engine::process_stereo` sums all voices each frame.
- `crates/synth-engine/src/params.rs` — `ParameterTree::set_active_voice_count` already exists from M2 lock-in; the engine now passes the real count.
- `crates/synth-engine/tests/no_alloc.rs` — extend to render with 32 simultaneous notes in scope.

**Allocation rules.** All 32 `Voice` instances pre-allocated in `VoiceManager::new`. No `Vec::push`, no `Box::new`, no `Arc::new` on the audio path. Run under the existing `assert_no_alloc` test.

**Allocation/stealing policy.**

1. On NoteOn:
   - First pass: find any voice where `is_idle() == true`. Take the first one.
   - Second pass (steal): if none are idle, find the voice in its **release phase** with the **oldest note-off timestamp** (i.e. been releasing the longest). A monotonic note-on counter incremented per NoteOn and stored per voice is the simplest "age" surrogate.
   - Third pass (steal): if none are in release, find the voice with the **lowest current envelope level** and steal it. Ties broken by oldest note-on counter.
   - The chosen voice gets `note_on(note_midi)` called on it; its tracked note id is updated.
2. On NoteOff: find the voice whose `held_note_midi == Some(note_midi)` (already tracked at M2) and call `note_off(note_midi)`. If multiple voices play the same note (rare; same MIDI note retriggered while still releasing), release the oldest one.

**Engine integration.**
- `Engine::process_stereo` loops `for v in &mut self.voices` and sums `(l, r)` per frame. With 32 simple voices the inner loop is `32 × per-voice next_sample` per output frame.
- `Engine::handle` for `ParameterChange { id: AmpReleaseSecs, .. }` fans out to every voice's `set_release_secs` (every voice owns its envelope). Same fan-out pattern for `SetOscillatorWaveform` and `SetFilterMode`.
- After processing, `set_active_voice_count(u8)` gets `voices.iter().filter(|v| !v.is_idle()).count()`.

**Tests (engine-side, deterministic).**
- 32 distinct notes can all sound simultaneously; mute one and the rest keep going.
- 33rd note steals: the released-longest voice's note disappears, new note takes its slot.
- 33rd note with all 32 still attacking: lowest-envelope voice gets stolen.
- NoteOff for note that no voice is playing is a no-op (no panic, no state change).
- Param changes fan out: setting release to 3.0 s changes the release of all 32 voices.
- `assert_no_alloc` passes with 32 active voices through a 2048-frame block.

**Exit criteria.** Tests above all green. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean. The existing virtual on-screen keyboard still plays correctly (now polyphonic since each tap allocates a fresh voice).

---

### M3.1 — Computer keyboard input

**Status:** Done (`e9e9c68`).

**Scope.** Add a computer-keyboard input layer in `synth-ui` that emits `EngineEvent::NoteOn` / `NoteOff` via the existing `EngineEventSender`. Validates M3.0's polyphony end-to-end without depending on MIDI hardware.

**Layout.**

```
   W   E       T   Y   U
 A   S   D   F   G   H   J
```

- A=C, W=C#, S=D, E=D#, D=E, F=F, T=F#, G=G, Y=G#, H=A, U=A#, J=B (chromatic, one row).
- Z = octave down, X = octave up. Default base = C3 (MIDI 48), so A plays MIDI 48 by default; matches Vital/Serum convention.
- Octave range clamp: keep base in `[0, 96]` (so the highest J = MIDI 107 stays inside the MIDI 0..127 range).

**Files touched.**
- `crates/synth-ui/src/keyboard.rs` — extend with a `ComputerKeyboard` struct (or split into a new `synth-ui/src/computer_keyboard.rs` if `keyboard.rs` is virtual-only — choose by file size; the rule in CLAUDE.md says no implementation in `mod.rs`, but `keyboard.rs` is a real file so either approach is allowed).
- `crates/synth-ui/src/app.rs` — call the computer keyboard's `handle_input` once per frame from `update`.

**Behaviour.**
- On key **press** (transition unheld → held): send `NoteOn { note_midi, velocity: 100 }` (velocity is fixed for the computer keyboard; MIDI hardware has the real velocity).
- On key **release**: send `NoteOff { note_midi }`.
- Held keys do **not** auto-repeat — only the press transition triggers a note.
- A press while focus moves away from the window leaves the note hanging; on focus loss, send NoteOff for every currently-held key (egui exposes a focus-loss event).
- Velocity: hardcoded 100 for now. Future: shift-key modifier for low/high velocity.

**Tests.** Mostly manual (it's UI input). Add a unit test for the layout map: `key_to_midi_note(Key::A, octave=0) == 48`, octave=1 gives 60, etc.

**Exit criteria.** Holding A on the keyboard sustains a note; pressing A and S together produces two voices simultaneously (visible in the footer voice count, audible as a dyad); Z and X shift the octave; alt-tabbing away kills all held notes.

---

### M3.2 — MIDI hardware input (notes)

**Status:** Done (`be38d0a`). Hot-plug intentionally deferred — if no MIDI device is present at startup the app runs without MIDI; if one is added later the user restarts. Same shape as the audio device hot-plug limitation noted at M4.

**Scope.** `midir` integration in `synth-host`. Enumerate input ports, open one, route note-on/note-off (with velocity) into the existing `EngineEventSender` SPSC.

**Files touched.**
- `crates/synth-host/Cargo.toml` — add `midir`.
- `crates/synth-host/src/midi.rs` — new file. `MidiInput` (the cpal-equivalent: opens a port, holds the connection alive for its lifetime).
- `crates/synth-host/src/lib.rs` — re-export `MidiInput` and any error types.
- `crates/synth-app/src/main.rs` (composition root) — instantiate `MidiInput` alongside `AudioStream`, keep it alive for app lifetime.

**Behaviour.**
- Enumerate `MidiInput::ports()`; expose as `Vec<MidiPortName>` for later UI use.
- For M3.2, open the **first available input port** automatically. The device picker is M4 scope; for now, if a controller is plugged in, it just works. If no port exists, the app runs without MIDI (not an error).
- `midir`'s callback runs on its own thread. The callback parses raw MIDI bytes, builds `EngineEvent`s, and pushes them into the engine's SPSC via the cloned `EngineEventSender`. Lock-free, allocation-free (the sender is just an atomic ring write).
- Parse:
  - `0x90 nn vv` (Note On, velocity > 0): `EngineEvent::NoteOn { note_midi: nn, velocity: vv }`.
  - `0x90 nn 00` (Note On with velocity 0): treat as Note Off (running-status convention).
  - `0x80 nn vv` (Note Off): `EngineEvent::NoteOff { note_midi: nn }`. Release velocity is ignored for v1 (very few synths surface it).
- Channel filtering: omnidirectional in M3.2 (accept any channel). Per-channel filtering is an M13 settings feature.
- Hot-plug: poll `MidiInput::ports()` every ~1 s from a host-side thread; on connection loss or new device, reopen. Simpler than a platform-native notification API and good enough for a desktop synth.

**Tests.** Unit-test the MIDI byte parser with hand-crafted byte arrays (note on, note off, running status, velocity=0 note off). The connection layer is hard to test without hardware — exercise it manually with a real controller before declaring done.

**Exit criteria.** A connected MIDI controller plays the synth. Velocity affects amp envelope peak (already shaped by the M2 TODO comment on `NoteOn { velocity: _ }`). Unplugging and replugging the controller restores playability without an app restart.

---

### M3.3 — MIDI controllers

**Status:** Done (`aa7b9d6`).

**Scope.** Pitch bend, mod wheel, sustain pedal, channel aftertouch, arbitrary CC routing.

**Files touched.**
- `crates/synth-engine/src/events.rs` — new variants on `EngineEvent`: `PitchBend { value_normalised: f32 }`, `Sustain { held: bool }`, `ChannelAftertouch { value_normalised: f32 }`, `ControlChange { cc: u8, value_normalised: f32 }`. Mod wheel is just CC #1.
- `crates/synth-engine/src/params.rs` — add `ParamId::PitchBendSemis`, `ParamId::ModWheel`, `ParamId::ChannelAftertouch`. Map controller events to these in the engine.
- `crates/synth-engine/src/voice_manager.rs` — sustain handling: when sustain is on, intercept `NoteOff` and stash the note in a "deferred releases" set. When sustain goes off, fire all deferred note-offs at once.
- `crates/synth-engine/src/voice.rs` — pitch bend influences pitch alongside `pitch_offset_semis`. New `SampleParams` field: `pitch_bend_semis`. Default bend range: ±2 semitones.
- `crates/synth-host/src/midi.rs` — extend the parser to recognise CC, pitch bend (`0xE0 lsb msb`), aftertouch (`0xD0 vv`).

**Sustain semantics.**
- While sustain CC (#64) is ≥ 64, every incoming NoteOff is deferred.
- When sustain falls to < 64, every deferred NoteOff is fired in order.
- A NoteOn for a note that already has a deferred NoteOff cancels the deferral (the new note "re-takes" the held voice).
- This logic lives in the **VoiceManager**, not in individual voices, because it's a cross-voice policy.

**Tests.** Unit-test the CC parser. Unit-test sustain behaviour: NoteOn → SustainOn → NoteOff → voice still sounds; SustainOff → voice releases. Pitch-bend semis range: ±1 normalised → ±2 semitones at the oscillator pitch.

**Exit criteria.** Pitch wheel bends; mod wheel moves a visible param value in the snapshot; sustain pedal extends notes through release; aftertouch and arbitrary CCs are reachable via the snapshot for future mod-matrix wiring (M6).

---

### M3.4 — Architecture review / lock-in

**Status:** Done (`aa7b9d6` + review pass below).

**Scope.** Mirror the M2.5 pattern. Read every file touched in M3.0–M3.3 against `design-patterns.md` and `audio-engine.md`. Look for:

- Allocation, locks, or syscalls that snuck onto the audio thread.
- Event variants that will need renaming once M5/M6 reach the modulation matrix.
- API choices in `VoiceManager` that M4's panel UI will want to read but can't (e.g. per-voice state for the polyphony indicator).
- Anything marked `// TODO: M3` from M2 — verify it's been addressed.

**Exit criteria.** Either everything is clean, or two-to-four small follow-up commits land before the M3 close-out. M3 done-when ("CPU well below 50% with 32 simple voices") is verified by running the binary, holding 32 notes via the computer keyboard, and watching the footer's CPU% (the indicator is M4 scope, but a manual `tracing::info!` of the audio callback duration is fine in M3.4 if M4 hasn't surfaced it yet).

**Review notes (2026-05-19):**
- No allocation / lock / syscall found on the audio path. Sustain uses `[bool;128]`, CC values use `[f32;128]` — both fixed-size stack arrays.
- `EngineEvent` surface is stable for M4–M6: existing variants are immutable; new controller variants (`PitchBend`, `Sustain`, `ChannelAftertouch`, `ControlChange`) match the M6 mod-matrix source list.
- `VoiceManager` API exposes `active_count()` for the M4 polyphony indicator — already being read by `synth-ui`.
- `mod_wheel` and `channel_aftertouch` are stepped (not smoothed) in the parameter tree because they have no DSP consumer until M6. Smoothing to be added there.
- Queue capacity (4096) comfortably handles worst-case burst (32×2 note events + 128 CC values per block).
- Open question resolved: default pitch-bend range is ±2 semitones (`PITCH_BEND_RANGE_SEMIS`), configurable in M13.
- Velocity curve: linear for v1 (implemented M3.3). Log in open-questions if a non-linear curve is desired before M6.

## Cross-cutting concerns

**Real-time safety.** The audio thread already meets the no-alloc / no-lock / no-syscall rule. M3 adds 32× the inner-loop cost; profile with `tracing::info!` of callback duration in the M3.4 review. The audio callback is currently called every ~10 ms (256-frame block at 48 kHz); 32 simple voices through one filter should comfortably fit in 5 ms.

**No-alloc tests.** `crates/synth-engine/tests/no_alloc.rs` must be extended in M3.0 to cover the 32-voice case. Don't accept passing single-voice no-alloc as proof — voice fan-out can hide allocation bugs that only fire when stealing kicks in.

**MIDI thread → audio thread bridge.** Resolved at M3.2: the bus is already MPMC (`crossbeam_channel::bounded`), so the on-screen keyboard, the computer keyboard, and the MIDI thread each clone `EngineEventSender` and push directly. The `param_bus.rs` doc comment that called it "SPSC" was wrong and was fixed in the M3.2 commit.

**Naming.** `crossbeam-channel`'s "queue capacity" already exists from M1. Confirm the capacity is sized for the worst case: 32 voices × (note-on + note-off) + ~16 CCs/frame = comfortably under 256. Current capacity is set in `param_bus.rs`; verify.

## Open questions to log if they come up

- **Velocity curve.** M3.0 turns `velocity: u8` into envelope-peak scaling. Linear? `sqrt`? `^2`? Defer to M2's done dsp-and-sound.md if it has a recommendation; otherwise log a new entry in `open-questions.md`.
- **Note-priority on same-note retrigger while still releasing.** Re-use the same voice (continuity), or take a fresh voice (clean retrigger)? Plausible defaults differ between hardware synths.
- **Default pitch-bend range.** ±2 semis is the GM default; ±12 (whole octave) is common on lead synths. Stick to ±2 for v1 and let the user change it via Settings (M13).

## References

- [`milestones.md`](milestones.md) — M3 entry.
- [`../03-architecture/audio-engine.md`](../03-architecture/audio-engine.md) — Voice management, block-based processing.
- [`../03-architecture/design-patterns.md`](../03-architecture/design-patterns.md) — Real-time safety rules; event command pattern.
- [`../03-architecture/midi-and-input.md`](../03-architecture/midi-and-input.md) — MIDI architecture.
- [`../04-tech-stack/libraries.md`](../04-tech-stack/libraries.md) — `midir` rationale.
- [`../../conversations/`](../../conversations/) — running log; check the most recent dated file for the latest decisions if this plan is out of date.

## Status legend (update as you go)

- `Not started` — sub-milestone untouched.
- `In progress` — at least one commit landed but exit criteria not yet met.
- `Done (<commit-hash>)` — exit criteria met, hash of the closing commit.

When a sub-milestone moves to **Done**, also note the commit in the running conversation log so future agents can find the work without grepping git.
