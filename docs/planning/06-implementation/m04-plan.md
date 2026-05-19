# M4 — Minimal playable UI

Branch: `milestone/m04-minimal-ui`  
Status: **Done (3f94922)**

## Done-when

A musician with no source-code access can play the synth from MIDI, sweep the
filter, adjust ADSR, and find it usable. Visual polish not required yet.

---

## Sub-milestones

### M4.0 — Engine: full ADSR + master volume

**Status:** Done (8cfac33)

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

**Status:** Done (a8635de)

Measure the fraction of each audio block's available time actually spent in
`render_block`. Report via an `Arc<AtomicU32>` (f32 bits) readable by the UI.

**Files:**
- `crates/synth-host/src/audio.rs` — `AudioStream` gains `cpu_load:
  Arc<AtomicU32>`; `render_block` takes a `block_cpu_load: &AtomicU32`
  parameter and writes the fraction; `start_with_engine` creates the arc and
  passes it into the closure

---

### M4.2 — Custom knob widget

**Status:** Done (3f94922)

Minimal but functional circular knob for continuous parameters. Drag up to
increase, drag down to decrease, tooltip shows value.

**Files:**
- `crates/synth-ui/src/knob.rs` — `Knob` struct implementing `egui::Widget`
- `crates/synth-ui/src/lib.rs` — `pub mod knob`

---

### M4.3 — Panel layout

**Status:** Done (3f94922)

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

**Status:** Done

**Review notes:**

- **Parameter tree growth** — adding 4 new `ParamId` variants, 4 snapshot
  fields, and 4 tree fields remained mechanical and friction-free. The
  "stepped vs smoothed" distinction held cleanly: `MasterVolume` is smoothed
  (per-sample consumer in `process_stereo`); A/D/S are stepped (sampled on
  envelope phase transitions). No concerns about tree complexity at current
  size.

- **Knob widget API** — `Knob::new(value, range, label)` with builder methods
  `.default_value()` and `.format()` reads naturally at the call site. The
  `impl egui::Widget` approach means `ui.add(Knob::new(...))` fits egui's
  standard idiom. The 270-degree arc convention (7 o'clock to 5 o'clock) is
  standard DAW practice. Drag sensitivity (200 px = full range) is reasonable
  for a 1080p screen; M11 can expose this as a configurable preference.

- **CPU meter design** — `Arc<AtomicU32>` with f32 bits is the lightest
  thread-safe option. Using `Instant::now()` in the audio callback is
  technically not hard-real-time (syscall), but is universally accepted for
  display-only metering and does not affect DSP correctness. The measurement
  is taken outside `catch_unwind`, which means a panic in `render_block` skips
  the CPU update for that block — acceptable since the process aborts anyway.

- **`sample_rate_f32` truncation** — `f32::from(sample_rate as u16)` in the
  audio callback loses precision above 65535 Hz, which no consumer audio
  device uses; safe in practice. Would use `sample_rate as f32` directly if
  there were any risk.

- **No architectural violations** — hexagonal layering intact. `synth-ui`
  still depends only on `synth-engine`; `synth-host` unchanged in its
  dependencies; `synth-app` is the only composition root.

**Merge and tag:** `milestone/m04-minimal-ui` → `development` → `main` at tag `m04`.
