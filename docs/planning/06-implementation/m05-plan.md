# M5 — LFOs and Env2

Branch: `milestone/m05-lfo-env2`
Status: **In progress**

## Done-when

LFOs and Env2 produce expected waveforms / shapes; they are visible and
tweakable from the UI; they respond correctly to note-on; BPM sync changes
the LFO period correctly.

---

## Design decisions

### LFO output range
Bipolar: **-1.0 to +1.0**. The mod matrix (M6) will scale by an amount that
can be positive or negative. Destinations receive a summed offset from the
matrix.

### Env2 output range
Unipolar: **0.0 to 1.0**, matching the amp envelope shape. The mod matrix
amount can invert it if needed.

### Block-rate updates
LFOs and Env2 are modulation sources, not audio signals. They are advanced
**once per inner block** (64 samples default). At typical LFO rates (<20 Hz),
this gives >750 Hz update resolution — far above the Nyquist of any audible
LFO. Env2 is also block-rate; fast attacks are served by the amp envelope
(per-sample), not the mod envelope.

### Per-voice scope only at M5
Each `Voice` owns its own LFO phase state. Global-scope LFOs (one phase shared
across all voices, useful for slow filter sweeps) are deferred to a later
milestone or M11 polish. The difference is inaudible for slow LFOs and
architecturally straightforward to add once the mod matrix (M6) is in place.

### BPM sync
A global `bpm` parameter (default 120.0) lives in `ParameterTree`. Each LFO
has a sync-enable flag and a `SyncDivision` enum. When sync is on, the engine
converts `bpm / 60 / division_multiplier` to Hz and stores it as the LFO's
rate. The LFO itself only knows Hz — the conversion happens at parameter
update time.

### Curve shaping on Env2
Each of the three timed stages (Attack, Decay, Release) has a curve parameter
in **[-1, +1]**:
- 0 → linear
- positive → logarithmic (fast initial change, slow tail — sounds "snappy")
- negative → exponential (slow initial change, fast tail — sounds "smooth")

Implemented as `y = x^(2^(-curve))` evaluated on the normalised stage fraction.
Note: the exponent is `2^(-curve)`, not `2^curve`. Positive curve → exponent < 1 (e.g. √x at curve=1) → fast initial rise; negative curve → exponent > 1 (e.g. x² at curve=-1) → slow initial rise.
The amp envelope keeps linear stages (no curve parameters) — the change is
Env2-specific per v1 scope.

### No destinations at M5
LFO and Env2 values are computed but **not routed to any parameter**. They are
exposed via the snapshot (first-voice values) so the UI can display a live
readout. M6 wires them into the mod matrix.

---

## Sub-milestones

### M5.0 — LFO engine

Implement `Lfo` in `crates/synth-engine/src/lfo.rs`.

**Shapes** (`LfoShape` enum):
- `Sine` — `sin(2π·phase)`
- `Triangle` — `1 - 4·|phase - 0.5|` (bipolar triangle)
- `SawUp` — `2·phase - 1`
- `SawDown` — `1 - 2·phase`
- `Square` — `if phase < 0.5 { 1.0 } else { -1.0 }`
- `SampleAndHold` — random value held until phase wraps
- `SmoothRandom` — linear interpolation between two S&H values across each period

**State:**
```rust
pub struct Lfo {
    phase: f32,            // 0.0 .. 1.0
    rate_hz: f32,
    shape: LfoShape,
    reset_on_note_on: bool,
    // S&H / smooth-random state
    held_value: f32,
    next_value: f32,       // target for smooth random
}
```

**Methods:**
- `advance(block_size: usize) -> f32` — advances phase, returns current output.
  (`sample_rate_hz` is stored in the struct at construction, not passed each call.)
- `note_on(&mut self)` — resets phase to 0 if `reset_on_note_on`
- `set_rate_hz`, `set_shape`, `set_reset_on_note_on`

**Files:**
- `crates/synth-engine/src/lfo.rs` — new

---

### M5.1 — Env2 (ModEnv) engine

Implement `ModEnv` in `crates/synth-engine/src/mod_env.rs`. Structurally
similar to `Adsr` but with per-stage curve parameters and block-rate stepping.

**State:**
```rust
pub struct ModEnv {
    sample_rate_hz: f32,
    stage: ModEnvStage,      // Idle, Attack, Decay, Sustain, Release
    progress: f32,           // normalised 0..1 through the current stage
    stage_start_level: f32,  // output level at which the current stage began
    attack_secs: f32,
    decay_secs: f32,
    sustain_level: f32,
    release_secs: f32,
    attack_curve: f32,       // -1..1
    decay_curve: f32,
    release_curve: f32,
}
```

`progress` (not a single `level`) is required so the curve function can be
applied to the normalised stage fraction rather than the output. `stage_start_level`
enables legato-safe retrigger: `note_on` captures the current output and
restarts attack from there instead of from 0.

**Methods:**
- `advance(block_size: usize) -> f32` — `sample_rate_hz` stored in struct
- `note_on(&mut self)` — restarts from current level (legato-safe)
- `note_off(&mut self)` — enters release from current level
- `is_idle() -> bool`
- Setters for all parameters

**Files:**
- `crates/synth-engine/src/mod_env.rs` — new

---

### M5.2 — Voice integration

Add `lfo1`, `lfo2`, `mod_env` fields to `Voice`. Hook into the note lifecycle
and block processing.

**Changes to `voice.rs`:**
- `Voice` gains `lfo1: Lfo`, `lfo2: Lfo`, `mod_env: ModEnv`
- `note_on()` calls `lfo1.note_on()`, `lfo2.note_on()`, `mod_env.note_on()`
- `note_off()` calls `mod_env.note_off()`
- `is_idle()` AND-gate: `amp_envelope.is_idle() && mod_env.is_idle()`
  (voice stays alive while Env2 is still releasing even if amp is silent)
- New method `advance_modulators(block_size, sample_rate)` called by the voice
  manager before the per-sample loop; stores LFO/Env2 outputs in fields
  `lfo1_out`, `lfo2_out`, `env2_out: f32`
- Getters `lfo1_out()`, `lfo2_out()`, `env2_out()`

**Files:**
- `crates/synth-engine/src/voice.rs`
- `crates/synth-engine/src/voice_manager.rs` — add fan-out setters for all
  new LFO / Env2 params; call `advance_modulators` in the inner block loop

---

### M5.3 — Parameter bus

New `ParamId` variants and snapshot fields.

**LFO1 params** (mirror for LFO2 with `Lfo2` prefix):
- `Lfo1RateHz` — continuous, smoothed? No — LFO rate can be stepped
  (audible click from a stepped LFO rate change is negligible); use
  stepped to keep it simple. Range 0.01..=20.0 Hz.
- `Lfo1Shape` — discrete (stepped), maps to `LfoShape`
- `Lfo1ResetOnNoteOn` — discrete bool (0.0 = off, 1.0 = on)
- `Lfo1SyncEnabled` — discrete bool
- `Lfo1SyncDivision` — discrete, maps to `SyncDivision` enum index

**Env2 params:**
- `Env2AttackSecs`, `Env2DecaySecs`, `Env2SustainLevel`, `Env2ReleaseSecs`
  — stepped continuous (same rationale as amp A/D/S)
- `Env2AttackCurve`, `Env2DecayCurve`, `Env2ReleaseCurve` — stepped, -1..=1

**Global:**
- `Bpm` — continuous, smoothed? No — BPM changes are musical cue-points, not
  interpolated. Stepped, range 20..=300.

**Snapshot additions:**
- All parameter mirror values (for UI knob init)
- `lfo1_out: f32`, `lfo2_out: f32`, `env2_out: f32` — live output of the
  first active voice (or 0 if no voices active), for the UI readout

**Files:**
- `crates/synth-engine/src/params.rs`
- `crates/synth-engine/src/engine.rs` — fan-out new params to `voices`; seed
  defaults in `Engine::new`

---

### M5.4 — UI surfaces

Add LFO and Env2 panels. Live readouts let the user verify output without
needing to route anything.

**LFO panel** (show for LFO1; LFO2 is a copy):
- Shape selector: 7-button row. Implemented labels: Sin / Tri / Saw+ / Saw- / Sq / S&H / Rnd.
- Rate knob (0.01–20 Hz), hidden when sync is active. Currently linear; logarithmic feel deferred to M11.
- Phase-reset toggle (labelled "Reset")
- BPM sync toggle + 8 division buttons (1/32 1/16 1/8 1/4 1/2 1 2 4), shown only when synced
- Live readout: e.g. "Out: 0.734"

**Env2 panel:**
- A / D / S / R knobs (same ranges as amp env)
- Curve knobs for A / D / R, range -1..+1, format e.g. "+0.50"
- Live readout: e.g. "Out: 0.000"

**Layout implemented:** LFO1, LFO2, Env2 as a second row of three columns
directly below the Osc1/Filter/AmpEnv row. Exact layout is flexible — usability
over polish, M11 handles the final layout.

**Files:**
- `crates/synth-ui/src/app.rs` — new `lfo_panel`, `env2_panel` helpers; new
  fields for all LFO/Env2 UI mirrors

---

### M5.5 — Architecture review + close-out

- Verify LFO shapes against expected waveforms (unit tests for each shape at
  known phase values)
- Verify Env2 curve shaping (test that attack at curve +1 reaches 0.5 faster
  than linear)
- Verify phase reset on note-on
- Verify BPM sync: at 120 BPM / 1-bar, LFO period = 2 seconds
- Verify `lfo1_out` / `lfo2_out` / `env2_out` in snapshot track the first voice
- Confirm voice `is_idle()` correctly waits for Env2 release
- Review readiness for M6 mod matrix: the `advance_modulators` output fields
  are the inputs M6 will consume

---

## What M5 does NOT include

- Routing LFOs or Env2 to any parameter (that is M6)
- Global-scope LFOs (all voices share one phase)
- Velocity sensitivity on Env2 depth (M6 via-source)
- Retrigger modes for Env2 (legato vs. always-retrigger) — simple always-restart for now
