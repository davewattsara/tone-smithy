# Milestones

An ordered sequence of milestones from empty repo to v1.0 release. Each milestone has a clear "done when" criterion. Sizes are rough — they assume part-time work and a single developer; multiple milestones can overlap when they touch different parts of the codebase.

The total estimate is **~6 months of part-time effort**. Compressing this requires more contributors, especially for factory content (M14).

## M0 — Scaffold (1 week)

Set up the workspace and basic plumbing.

- Cargo workspace with all crates (`synth-engine`, `synth-host`, `synth-presets`, `synth-ui`, `synth-app`, `xtask`).
- `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`.
- GitHub Actions CI (build + test + clippy + cargo deny).
- Pre-commit hook.
- `synth-app` opens an empty egui window.
- `cpal` audio passthrough: open the default output device and write silence; verify no crackle, latency reported in the footer.

**Done when:** CI is green; app launches; an empty window is visible; audio device is open and producing silence.

## M1 — First sound (1 week)

- Single-oscillator subtractive engine (saw + sine).
- Amp envelope (ADSR).
- Virtual keyboard plays notes.
- Parameter bus prototype (UI changes oscillator pitch and amp release).

**Done when:** Clicking a key plays a sustained, click-free note that decays via amp release.

## M2 — Subtractive voice (2 weeks)

- All 3 oscillators + sub oscillator per slot.
- TPT state-variable filter (LP/HP/BP/Notch), 12 dB/oct first.
- Per-oscillator detune, level, pan; per-slot mixer.
- Unison (up to 7 voices per oscillator).
- **Architecture lock-in**: the parameter tree, event model, and engine API are reviewed and frozen at the end of this milestone. Changing them after this point requires explicit justification.

**Done when:** A held note plays through full subtractive signal flow with audible filter sweeps and unison detune.

## M3 — Polyphony (1 week)

- Voice manager with fixed-size voice array (32).
- Voice stealing (oldest released, then quietest).
- Note-off and amp-envelope-driven voice release.
- `assert_no_alloc` test in CI verifying no audio-thread allocations.

**Done when:** Holding a chord works; releasing one note doesn't cut others; CPU stays well below the 50% target with 32 simple voices.

## M4 — MIDI input (1 week)

- `midir` integration in `synth-host`.
- Device enumeration; selection persisted.
- Notes (with velocity), pitch bend, mod wheel, sustain, channel aftertouch, arbitrary CC.
- Computer keyboard input.
- Hot-plug detection.

**Done when:** A connected MIDI controller plays the synth; pitch bend and mod wheel work; computer keyboard works when focused.

## M5 — FM engine (2 weeks)

- 4-operator FM slot.
- 8 starter algorithms.
- Per-operator envelope, ratio, level, feedback on op 4.
- 2× internal oversampling when modulation index is high.
- Hybrid voice: each of slot 1 and slot 2 can be subtractive or FM.

**Done when:** All 8 algorithms produce expected sounds; combining a subtractive slot 1 with an FM slot 2 in one patch works.

## M6 — Modulation matrix (1 week)

- 16-slot matrix.
- Source / destination / amount / "via" handling per slot.
- All sources and destinations from the features doc wired up.

**Done when:** Mod wheel can scale a key-tracked LFO mod of filter cutoff (real-world test of source → dest with via).

## M7 — Envelopes and LFOs (1 week)

- 2 additional envelopes (Env2, Env3) with curve shaping.
- 2 LFOs with all 7 shapes, free/sync modes, per-voice/global scope, phase reset toggle.

**Done when:** Mod matrix sources panel is fully populated; envelopes and LFOs respond correctly to note-on and tempo changes.

## M8 — Effects chain (3 weeks)

In order, one at a time:
- EQ (biquad)
- Drive (tanh)
- Chorus (3-tap)
- Delay (stereo, sync)
- Reverb (FDN-8)

Each effect is unit-tested, benchmarked, and listened to in isolation before integration.

**Done when:** All 5 effects work in series; mod matrix can address their key parameters; reverb tail is clean over a 30-second decay.

## M9 — Arpeggiator and step sequencer (2 weeks)

- Arp with all modes, octave range, gate, swing.
- 16-step sequencer with notes, velocities, gates, one mod lane.
- BPM source: internal or external MIDI clock.

**Done when:** Both run reliably for hours; sync switches cleanly; sequencer mod lane modulates a real destination.

## M10 — Preset save / load (1 week)

- RON format with metadata + parameter snapshot + mod matrix + MIDI Learn map.
- Save, save-as, load, delete, duplicate.
- Schema versioning with a single (no-op) migration in place.
- Round-trip property test in CI.

**Done when:** Any patch can be saved and reloaded with bit-identical parameter state.

## M11 — UI v1 (3 weeks)

- Final panel layout for all sections.
- Custom widget library (knob, slider, toggle, dropdown, step grid, mod row, meter).
- Theme tokens locked.
- Tooltips, right-click context menus, MIDI Learn, modulation visualisation.
- High-DPI handling; window resizing; minimum size enforcement.

**Done when:** Every parameter is reachable, labelled, modulated visually, and usable without a manual.

## M12 — Preset browser (1 week)

- Category and tag filtering, search, sort.
- Factory / user separation.
- Folder tree for user presets.
- Import / export via OS dialog.
- File-watcher refresh.

**Done when:** A user can drop a preset pack into their user folder, see it appear, browse by tag, and load any preset.

## M13 — Settings and MIDI Learn polish (1 week)

- Audio device picker with live switching.
- MIDI input picker (multi-select), channel filter per input.
- Per-preset and global MIDI Learn layers.
- Settings persisted; first-run wizard.

**Done when:** A fresh install walks the user through device pickers and a learned CC survives preset changes.

## M14 — Factory bank (3 weeks, can overlap with M11–M13)

- Authoring of ~120–150 presets across categories.
- A small QA pass: every preset listened to in context, levels normalised within a target loudness range, descriptions written.
- Authoring is a parallelisable, distributable effort — decide at the start of the milestone whether to solo, recruit, or open-call.

**Done when:** Bank meets the success criteria in `01-vision/success-criteria.md`.

## M15 — Installer and release (1 week)

- Inno Setup script producing a signed (if cert available) or unsigned installer.
- `xtask dist` produces all release artefacts.
- README, changelog, getting-started doc.
- Licence chosen (open question must be resolved before this point).
- v1.0 tag and GitHub Release.

**Done when:** A clean Windows machine can download the installer, install, launch, and play a preset within 60 seconds.

## Critical-path dependencies

- M0 → M1 → M2 → M3 → M4 are strictly sequential.
- M5, M6, M7 can be parallel after M4.
- M8 is large and runs partly in parallel with M5–M7 (different files).
- M9 needs M3 and M4.
- M10 needs the parameter tree from M2.
- M11 needs at least M2; M12, M13 need M11.
- M14 needs the engine stable (M5–M9 complete) before serious patching.
- M15 needs M11–M14.

## Out of scope for the milestone list

Anything in [`../02-scope/out-of-scope.md`](../02-scope/out-of-scope.md) or [`../02-scope/roadmap.md`](../02-scope/roadmap.md).
