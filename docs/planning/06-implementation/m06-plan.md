# M6 — Modulation matrix

Branch: `milestone/m06-mod-matrix`
Status: **In progress** (M6.0–M6.3 complete; M6.4 close-out pending user test)

## Done-when

Mod wheel can scale a key-tracked LFO mod of filter cutoff — the canonical
"source → destination via" test. All sources route to at least one tested
destination. Env2 → filter cutoff works end-to-end (user's primary use case).

---

## Design decisions

### Architecture: where the matrix lives

`ModMatrix` is owned by `VoiceManager`. The engine receives parameter changes
and calls `voices.update_mod_slot(...)`. The matrix is evaluated per-voice
inside VoiceManager after each block's modulator advance, before the
per-sample inner loop.

### Per-voice evaluation

After `advance_modulators`, VoiceManager builds a `ModSources` struct for each
active voice (LFO outputs, Env2, amp env level, velocity, MIDI note) combined
with global values (mod wheel, aftertouch, pitch bend) and calls
`matrix.compute_offsets(&sources)`. The resulting `DestOffsets` are stored on
the voice.

In the per-sample inner loop, a local copy of `SampleParams` is made for each
voice and the filter/pitch offsets are applied before calling
`voice.next_sample`. Volume offset is applied inside `next_sample` to the amp
envelope output.

No heap allocation in the hot path — all structs are `Copy` and live on the stack.

### Sources (10)

| Variant | Value range | Notes |
|---|---|---|
| `Lfo1` | -1..1 | Block-rate |
| `Lfo2` | -1..1 | Block-rate |
| `Env2` | 0..1 | Block-rate |
| `AmpEnv` | 0..1 | Via `amp_envelope.current_level()` at start of block |
| `Velocity` | 0..1 | Captured at note-on |
| `KeyTracking` | -1..1 | `(note as f32 - 60.0) / 60.0` — 0 at middle C |
| `ModWheel` | 0..1 | Global |
| `Aftertouch` | 0..1 | Global |
| `PitchBend` | -1..1 | Global |
| `Off` | 0.0 | Sentinel for "no source / no via" |

### Destinations (6)

| Variant | Applied to | Amount units | Typical range |
|---|---|---|---|
| `FilterCutoffHz` | `SampleParams::filter_cutoff_hz` (additive) | Hz | ±10 000 Hz |
| `FilterResonance` | `SampleParams::filter_resonance` (additive) | 0..1 | ±1.0 |
| `PitchSemis` | `SampleParams::pitch_offset_semis` (additive) | semitones | ±24 |
| `Volume` | Amp env × velocity (multiplicative scale: `1 + offset`) | 0..1 scale | ±1.0 |
| `Osc1DetuneCents` | `SampleParams::osc_main_detune_cents[0]` (additive) | cents | ±2400 |
| `Osc1Pan` | `SampleParams::osc_main_pans[0]` (additive) | -1..1 | ±1.0 |

Additional oscillator destinations (Osc2/3 detune, pan, level) and
per-oscillator level follow the same pattern and can be added during
implementation without new design decisions.

### Via source

Each slot has an optional `via` source. When set, the effective modulation
is `src_val * amount * via_val`. When via is `Off`, it is treated as 1.0
(full amount always applies).

Via is a raw multiply — no absolute-value or unipolar coercion. Standard
usage is unipolar via sources (ModWheel, Velocity, Env2).

### Destination clamping

Applied after summing all slot contributions:
- `filter_cutoff_hz`: clamped to 20.0..=20 000.0 Hz
- `filter_resonance`: clamped to 0.0..=1.0
- `pitch_offset_semis`: unclamped (oscillator frequency calculation already
  handles extreme values)
- volume offset: `(env * velocity * (1.0 + volume_offset)).clamp(0.0, 1.0)`
- detune/pan: clamped to their natural ranges at application point

### Parameter representation

Indexed tuple variants in `ParamId` to avoid 40 flat names:

```rust
ModSlotEnabled(u8),   // index 0..7; 0.0 = off, 1.0 = on
ModSlotSource(u8),    // index into ModSource as u8
ModSlotDest(u8),      // index into ModDest as u8
ModSlotAmount(u8),    // f32 amount
ModSlotVia(u8),       // index into ModSource as u8; ModSource::Off = no via
```

Defaults: all slots disabled, source Off, dest FilterCutoffHz, amount 0.0,
via Off.

---

## Sub-milestones

### M6.0 — ModMatrix data types

New file `crates/synth-engine/src/mod_matrix.rs`.

```rust
pub enum ModSource { Off, Lfo1, Lfo2, Env2, AmpEnv, Velocity,
                     KeyTracking, ModWheel, Aftertouch, PitchBend }

pub enum ModDest { FilterCutoffHz, FilterResonance, PitchSemis,
                   Volume, Osc1DetuneCents, Osc1Pan }

pub struct ModSources {
    pub lfo1: f32, pub lfo2: f32, pub env2: f32, pub amp_env: f32,
    pub velocity: f32, pub key_tracking: f32,
    pub mod_wheel: f32, pub aftertouch: f32, pub pitch_bend: f32,
}

#[derive(Default, Copy, Clone)]
pub struct DestOffsets {
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
    pub pitch_semis: f32,
    pub volume: f32,
    pub osc1_detune_cents: f32,
    pub osc1_pan: f32,
}

pub struct ModSlot {
    pub enabled: bool,
    pub source: ModSource,
    pub dest: ModDest,
    pub amount: f32,
    pub via: ModSource,   // Off = always-on
}

pub struct ModMatrix { pub slots: [ModSlot; 8] }
impl ModMatrix {
    pub fn compute_offsets(&self, sources: &ModSources) -> DestOffsets { … }
}
```

Unit tests for each source/dest combination and for via scaling.

**Files:** `crates/synth-engine/src/mod_matrix.rs` (new)

---

### M6.1 — Voice + VoiceManager integration

**Voice changes (`voice.rs`):**
- Add `mod_offsets: DestOffsets` field (default zeros)
- Add `pub fn set_mod_offsets(&mut self, offsets: DestOffsets)`
- In `next_sample`: apply volume offset:
  `let env = (self.amp_envelope.next_sample() * self.velocity_scale * (1.0 + self.mod_offsets.volume)).clamp(0.0, 1.0);`
- Add `pub fn amp_env_level(&self) -> f32` calling `self.amp_envelope.current_level()`
  (used by VoiceManager to build `ModSources`)

**VoiceManager changes (`voice_manager.rs`):**
- Add `matrix: ModMatrix` field
- After `advance_modulators` loop, for each active voice:
  1. Build `ModSources` from voice's lfo1/lfo2/env2/amp_env outputs, velocity,
     held note, and VoiceManager's global mod-wheel / aftertouch / pitch-bend values
  2. Call `self.matrix.compute_offsets(&sources)` → `DestOffsets`
  3. Call `voice.set_mod_offsets(offsets)`
- In the per-sample loop (`next_sample`):
  ```rust
  let mut vp = *params;  // copy SampleParams
  vp.filter_cutoff_hz = (vp.filter_cutoff_hz + voice.mod_offsets.filter_cutoff_hz).clamp(20.0, 20_000.0);
  vp.filter_resonance = (vp.filter_resonance + voice.mod_offsets.filter_resonance).clamp(0.0, 1.0);
  vp.pitch_offset_semis += voice.mod_offsets.pitch_semis;
  vp.osc_main_detune_cents[0] += voice.mod_offsets.osc1_detune_cents;
  vp.osc_main_pans[0] = (vp.osc_main_pans[0] + voice.mod_offsets.osc1_pan).clamp(-1.0, 1.0);
  voice.next_sample(&vp)
  ```

**Files:** `crates/synth-engine/src/voice.rs`,
           `crates/synth-engine/src/voice_manager.rs`

---

### M6.2 — Parameter bus

**`params.rs`:**
- New `ParamId` tuple variants:
  `ModSlotEnabled(u8)`, `ModSlotSource(u8)`, `ModSlotDest(u8)`,
  `ModSlotAmount(u8)`, `ModSlotVia(u8)`
- `ModSource` and `ModDest` each implement `TryFrom<u8>` / `Into<u8>` for
  encoding as f32 in the parameter bus
- Snapshot fields: `mod_slot_enabled: [bool; 8]`, `mod_slot_source: [u8; 8]`,
  `mod_slot_dest: [u8; 8]`, `mod_slot_amount: [f32; 8]`, `mod_slot_via: [u8; 8]`
- Default snapshot: all slots disabled, amount 0

**`engine.rs`:**
- Fan-out new ParamId variants to `self.voices.update_mod_slot_*(index, value)`
- Seed defaults in `Engine::new`

**`voice_manager.rs`:**
- `update_mod_slot_enabled(i, v)`, `update_mod_slot_source(i, v)`, etc.
  mutate `self.matrix.slots[i]`

**Files:** `crates/synth-engine/src/params.rs`,
           `crates/synth-engine/src/engine.rs`,
           `crates/synth-engine/src/voice_manager.rs`

---

### M6.3 — UI (mod matrix table)

8-row table in `app.rs`. Each row:
- Enable toggle
- Source combo box (10 choices)
- Destination combo box (6+ choices)
- Amount knob (range depends on dest: ±10000 Hz for cutoff, ±24 for pitch, ±1.0 for rest)
- Via combo box (10 choices, "Off" = no scaling)

Layout: scrollable table below the existing LFO / Env2 row, or in a separate
collapsible section. Polish deferred to M11.

Amount knob range is determined by the currently selected destination — the UI
re-ranges the knob when the dest changes.

**Files:** `crates/synth-ui/src/app.rs`

---

### M6.4 — Close-out / verification

- **Env2 → FilterCutoff:** hold a note, watch cutoff sweep with Env2 shape
- **LFO1 → FilterCutoff via ModWheel:** at wheel=0 no effect, at wheel=1 full sweep
- **Velocity → Volume:** soft notes quieter than hard notes via matrix
- **KeyTracking → FilterCutoff:** high notes brighter than low notes
- **Disable slot:** toggling a slot off silences its contribution immediately
- Confirm no audible discontinuities when changing slot params at runtime
- Confirm `cargo clippy` and `cargo fmt` clean

---

## What M6 does NOT include

- MIDI CC sources (added in M11 polish)
- Per-oscillator-2/3 level and detune as distinct destinations (same pattern, add if time permits)
- Preset save/load of matrix config (M10)
- Polished matrix UI (M11 — drag-to-connect, visual signal flow)
- Global-scope LFOs (deferred beyond M5)
