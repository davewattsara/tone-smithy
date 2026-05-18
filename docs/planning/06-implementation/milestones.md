# Milestones

An ordered sequence of milestones from empty repo to v1.0 release. Each milestone has a clear "done when" criterion.

## Sizing assumptions

These estimates assume a developer who is **learning Rust, audio DSP, real-time programming, and egui in parallel with the build**, working **10–20 hrs/week**. Sizes are roughly double the equivalent "experienced developer" estimate — that's the honest accounting, made up front rather than discovered mid-project.

**Total: ~50–60 weeks of part-time work ≈ 12–15 months elapsed.**

Sizing is revisited at every milestone boundary. Faster than estimated is fine; slower triggers a conversation about scope or pace.

The milestones are ordered to **get something playable as early as possible** — M-1 produces a toy synth that runs; M3 makes the project's own synth playable via MIDI; M4 makes it tweakable with knobs. Deep DSP work (FM, full effects chain) comes after the synth is already a usable instrument, so motivation stays high through the long middle.

---

## M-1 — Learning ramp (3–4 weeks)

Before touching the project repo, build the foundational understanding the rest of the plan depends on.

- Work through the `cpal` examples (sine wave generator, audio passthrough).
- Build a **toy single-file synth** outside this repo (e.g. `~/projects/synth-toy/`) — one oscillator, one ADSR, virtual or computer-keyboard input. The point is to feel out Rust + `cpal` end-to-end without the workspace overhead.
- Skim the relevant chapters of:
  - Will Pirkle, *Designing Audio Effect Plug-Ins in C++* (concepts translate fine; ignore the C++ syntax).
  - Vadim Zavalishin, *The Art of VA Filter Design* (the standard reference for TPT/ZDF filters).
- Skim the `egui` demo source and one or two showcase apps.
- Re-read [`../03-architecture/design-patterns.md`](../03-architecture/design-patterns.md) and [`../04-tech-stack/code-style.md`](../04-tech-stack/code-style.md) now that the patterns will make more sense.

**Done when:** The toy synth runs locally, plays a clean sine wave with ADSR, responds to MIDI from a controller or computer keyboard. No deliverable in the main repo — the goal is competence, not output.

---

## M0 — Scaffold (2–3 weeks) — **complete (2026-05-17, tag `m00`)**

Set up the workspace and basic plumbing for the real project.

- Cargo workspace with all crates (`synth-engine`, `synth-host`, `synth-presets`, `synth-ui`, `synth-app`, `xtask`).
- `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`.
- GitHub Actions CI (build + test + clippy + `cargo deny`).
- Pre-commit hook.
- `synth-app` opens an empty egui window via `eframe`.
- `cpal` audio passthrough: open the default output device, write silence, verify no crackle, latency reported.
- `cargo xtask check-deps` enforces the hexagonal layering rules from [`../03-architecture/design-patterns.md`](../03-architecture/design-patterns.md) (added during M0; wired into CI and the pre-commit hook).

**Done when:** CI is green; app launches; an empty window is visible; audio device is open and producing silence.

---

## M1 — First sound (2–3 weeks) — **complete (2026-05-18, tag `m01`)**

Get the project's own engine producing sound.

- Single subtractive oscillator (saw + sine) in `synth-engine`.
- Amp ADSR envelope.
- On-screen virtual keyboard plays notes.
- Parameter bus prototype (UI changes pitch and amp release; engine receives via lock-free queue).
- First `assert_no_alloc` test wrapping the audio path.

**Done when:** Clicking a virtual key plays a sustained, click-free note that decays via amp release. No allocations on the audio thread.

---

## M2 — Subtractive voice (4–5 weeks)

Build out the full single subtractive voice.

- All 3 oscillators + sub oscillator per slot (PolyBLEP for saw/square).
- Single TPT state-variable filter (LP/HP/BP/Notch, 12 dB/oct).
- Per-oscillator detune, level, pan; per-slot mixer.
- Unison (up to 7 voices per oscillator).
- **Architecture lock-in:** the parameter tree, event model, and engine API are reviewed and frozen at the end of this milestone. Changing them after this point requires explicit justification.

**Done when:** A single held note plays through the full subtractive signal flow with audible filter sweeps and unison detune.

---

## M3 — MIDI input + polyphony (3 weeks)

Make the synth playable from real hardware.

- `midir` integration in `synth-host`.
- Device enumeration, selection, hot-plug detection.
- Notes (with velocity), pitch bend, mod wheel, sustain, channel aftertouch, arbitrary CC.
- Computer keyboard input (AWSEDFTGYHUJ layout, Z/X for octave).
- Voice manager with fixed-size voice array (32).
- Voice stealing (oldest released, then quietest).
- Note-off and amp-envelope-driven voice release.

**Done when:** A connected MIDI controller plays the synth polyphonically; chords sustain; pitch bend and mod wheel work; computer keyboard works when focused. CPU well below 50% with 32 simple voices.

---

## M4 — Minimal playable UI (3–4 weeks)

Make the synth tweakable from the window, not just from MIDI.

- First version of the custom widget library: knob, slider, toggle, dropdown.
- Panels for: Oscillators (slot 1 only at this stage), Filter, Amp envelope.
- Master volume.
- Footer with CPU%, voice count, audio device indicator.
- Window resizing with the enforced 1280×720 minimum.
- The two later UI milestones (M11 polish, M12 browser) will build on these widgets — get them functional now, beautiful later.

**Done when:** A musician with no source-code access can play the synth from MIDI, sweep the filter, adjust ADSR, and find it usable. Visual polish not required yet.

---

## M5 — LFOs and Env2 (2 weeks)

Add the modulation sources that the matrix will route in M6.

- 2 LFOs per voice with all 7 shapes (sine, tri, saw up, saw down, square, S&H, smooth random).
- Free or BPM-synced; phase reset on note-on optional; per-voice or global scope.
- Env2 (the one mod envelope in v1 scope) with curve shaping.
- UI surfaces for both — basic, will be polished in M11.

**Done when:** LFOs and Env2 produce expected waveforms / shapes; they're visible and tweakable from the UI; they respond correctly to note-on and tempo changes.

---

## M6 — Modulation matrix (2 weeks)

Wire the sources to anything.

- 8-slot matrix.
- Source / destination / amount / "via" source handling per slot.
- All sources (Amp env, Env2, LFO1, LFO2, MIDI velocity, key tracking, mod wheel, channel aftertouch, pitch bend, MIDI CC) and all continuous destinations wired up.
- Basic mod-matrix table UI (polished in M11).

**Done when:** Mod wheel can scale a key-tracked LFO mod of filter cutoff — the canonical "source → destination via" test. All sources route to at least one tested destination.

---

## M7 — FM engine (4–5 weeks)

The harder of the two synthesis methods. Conceptually new ground for someone learning DSP.

- 4-operator FM slot.
- 8 starter algorithms (DX7-family routings).
- Per-operator envelope, ratio (integer + fine), level, feedback on op 4.
- 2× internal oversampling when modulation index is high.
- Hybrid voice working end-to-end: slot 1 = subtractive, slot 2 = FM (or any other combination) in a single patch.

**Done when:** All 8 algorithms produce expected sounds; combining subtractive slot 1 with FM slot 2 in one patch sounds coherent; no audible aliasing at high mod indices.

---

## M8 — Effects chain (6–8 weeks)

Five effects, each a small DSP project in its own right. Tackle in order, listening between each.

- **EQ** (biquad-based, 3-band): 1 week.
- **Drive** (soft tanh, asymmetry): 0.5 week.
- **Chorus** (3-tap, two-LFO modulated): 1 week.
- **Delay** (stereo, sync, ping-pong, feedback filtering): 1–2 weeks.
- **Reverb** (FDN-8): 2–3 weeks — the biggest of the five.
- Integration into the post-mix chain; mod-matrix wiring for key parameters.

**Done when:** All 5 effects work in series; mod matrix can address their key parameters; reverb tail is clean and free of denormals over a 30-second decay.

---

## M9 — Arpeggiator (1–2 weeks)

- Modes: up, down, up/down, random, played order.
- Octave range 1–4, rate sync (BPM or free), gate length, swing.
- Sync source: internal BPM or external MIDI clock.
- UI surface in the arp panel.

**Done when:** Arp runs reliably for hours, syncs cleanly, mode switching is glitch-free. (Step sequencer is v1.1.)

---

## M10 — Preset save / load (1–2 weeks)

- RON format with metadata, parameter snapshot, mod matrix, MIDI Learn map.
- Save, save-as, load, delete, duplicate from a simple UI surface (polished browser comes in M12).
- Schema versioning with a single (no-op) migration in place.
- Property test in CI: any patch round-trips with bit-identical parameter state.

**Done when:** Any patch can be saved and reloaded with bit-identical parameter state; round-trip property test passes.

---

## M11 — UI v1 polish (4–5 weeks)

Take the functional UI from M4 and the surfaces added through M6/M9/M10 and turn them into a flagship-class interface.

- Final panel layout for all sections.
- Custom widget library extended: step grid (for arp), mod matrix row, meter, patch name editor.
- Theme tokens locked (palette, typography, spacing).
- Tooltips, right-click context menus, MIDI Learn affordances.
- Modulation visualisation (the coloured arc/ring on modulated knobs).
- High-DPI handling; window resizing breakpoints.

**Done when:** Every parameter is reachable, labelled, modulated visually, and usable without a manual. Looks like a professional plugin, not a coder UI.

---

## M12 — Preset browser (1–2 weeks)

- Category and tag filtering, search, sort.
- Factory / user separation. Folder tree for user presets.
- Import / export via OS file dialog.
- File-watcher refresh.

**Done when:** A user can drop a preset pack into their user folder, see it appear, browse by tag, and load any preset.

---

## M13 — Settings + MIDI Learn polish (1–2 weeks)

- Audio device picker with live switching.
- MIDI input picker (multi-select), channel filter per input.
- Per-preset and global MIDI Learn layers.
- Settings persisted between sessions.
- First-run wizard.

**Done when:** A fresh install walks the user through device pickers; a learned CC survives preset changes.

---

## M14 — Factory bank (4–6 weeks, can overlap with M11–M13)

- Authoring of ~60–80 presets across categories per [`../05-design/dsp-and-sound.md`](../05-design/dsp-and-sound.md).
- A QA pass: every preset listened to in context, levels normalised within a target loudness range, descriptions written.
- Authoring is parallelisable — decide at the start of the milestone whether to solo, recruit a sound designer, or open-call.

**Done when:** Bank meets the success criteria in [`../01-vision/success-criteria.md`](../01-vision/success-criteria.md) (60+ presets, distinguishable engines, recognisable character).

---

## M15 — Installer and release (1–2 weeks)

- Inno Setup script producing a signed (if cert available) or unsigned installer.
- `xtask dist` produces all release artefacts.
- README, changelog, getting-started doc.
- v1.0 tag and GitHub Release.

**Done when:** A clean Windows machine can download the installer, install, launch, and play a preset within 60 seconds.

---

## Critical-path dependencies

- **M-1 → M0 → M1 → M2 → M3** are strictly sequential. Each builds on the last.
- **M4 (UI)** can begin once M3 is stable (it needs a working engine to drive).
- **M5 (LFOs/Env2)** needs M2's parameter system; can technically start in parallel with M4 if you have the focus to context-switch.
- **M6 (mod matrix)** needs M5's sources to be testable end-to-end.
- **M7 (FM)** needs M2's voice architecture but is otherwise independent of M5/M6.
- **M8 (effects)** can be developed in parallel with M9 (arp) and M10 (presets) — they touch different files.
- **M11 (UI polish)** assumes M4's basic UI plus all the surfaces added through M9.
- **M12, M13** build on M11.
- **M14 (factory bank)** needs the engine stable (M5–M9 complete) and the preset format (M10). Can run in parallel with M11–M13.
- **M15** needs everything.

## What's out of scope

Anything in [`../02-scope/out-of-scope.md`](../02-scope/out-of-scope.md) or scheduled for v1.1+ in [`../02-scope/roadmap.md`](../02-scope/roadmap.md). If something feels missing, that's where to look first.
