# M4 — Minimal playable UI

Branch: `milestone/m04-minimal-ui`  
Status: **In progress**

## Done-when

A musician with no source-code access can play the synth from MIDI, sweep the
filter, adjust ADSR, and find it usable. Visual polish not required yet.

---

## Sub-milestones

### M4.0 — Engine: full ADSR + master volume

**Status:** Pending

Expose the remaining ADSR params (`AmpAttackSecs`, `AmpDecaySecs`,
`AmpSustainLevel`) and `MasterVolume` through the parameter bus. Currently only
`AmpReleaseSecs` is wired; attack, decay, and sustain are hardcoded defaults in
`Adsr::new`.

**Files:**
- `crates/synth-engine/src/params.rs` — 4 new `ParamId` variants; 4 new fields
  in `ParamSnapshot` and `ParameterTree`; `MasterVolume` smoothed, others
  stepped; `master_volume` added to `SampleParams`; `set_continuous` arms; new
  getters `amp_attack_secs`, `amp_decay_secs`, `amp_sustain_level`
- `crates/synth-engine/src/voice.rs` — `set_attack_secs`, `set_decay_secs`,
  `set_sustain_level` setters delegating to `Adsr`; `default_sample_params`
  test helper gains `master_volume: 1.0`
- `crates/synth-engine/src/voice_manager.rs` — `set_attack_secs`,
  `set_decay_secs`, `set_sustain_level` fan-out
- `crates/synth-engine/src/engine.rs` — seed voices with A/D/S defaults in
  `new`; fan-out in `ParameterChange` handler; apply smoothed `master_volume`
  to stereo output in `process_stereo`

---

### M4.1 — CPU meter

**Status:** Pending

Measure the fraction of each audio block's available time actually spent in
`render_block`. Report via an `Arc<AtomicU32>` (f32 bits) readable by the UI.

**Files:**
- `crates/synth-host/src/audio.rs` — `AudioStream` gains `cpu_load:
  Arc<AtomicU32>`; `render_block` takes a `block_cpu_load: &AtomicU32`
  parameter and writes the fraction; `start_with_engine` creates the arc and
  passes it into the closure

---

### M4.2 — Custom knob widget

**Status:** Pending

Minimal but functional circular knob for continuous parameters. Drag up to
increase, drag down to decrease, tooltip shows value.

**Files:**
- `crates/synth-ui/src/knob.rs` — `Knob` struct implementing `egui::Widget`
- `crates/synth-ui/src/lib.rs` — `pub mod knob`

---

### M4.3 — Panel layout

**Status:** Pending

Restructure `ToneSmithyApp::update` around three side-by-side panels (Osc 1,
Filter, Amp Envelope) plus a footer. All continuous parameters use the new
`Knob` widget.

**Files:**
- `crates/synth-ui/src/app.rs` — new `ToneSmithyApp` fields for A/D/S/volume
  and `cpu_load` arc; `osc1_panel`, `filter_panel`, `amp_env_panel` helpers;
  `footer_bar` helper; top-level `update` wired to panels + keyboard
- `crates/synth-app/src/main.rs` — pass `audio.cpu_load.clone()` into
  `ToneSmithyApp::new`

---

### M4.4 — Architecture review + close-out

**Status:** Pending

- Review: parameter tree growth, knob widget API, CPU meter design
- Update `milestones.md`, `README.md`, `CLAUDE.md`
- Merge `milestone/m04-minimal-ui` → `development` → `main`
- Tag `m04`
