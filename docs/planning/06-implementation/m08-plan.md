# M8 — Effects chain

Branch: `milestone/m08-effects-chain`
Status: **In progress**

## Done-when

All five effects (EQ, Drive, Chorus, Delay, Reverb) run in series on the
post-mix stereo bus. Each has an enabled/bypassed switch. Key parameters
are reachable from the mod matrix. Reverb tail is free of denormals over
a 30-second decay. CPU overhead is acceptable at 32 voices + full chain.

---

## Effect order

```
Voice mix → EQ → Drive → Chorus → Delay → Reverb → Master volume → Output
```

All effects are global (post-mix), not per-voice. The `FxChain` struct
lives in `Engine` alongside `VoiceManager`, and is processed once per
sample inside `process_stereo`.

---

## EQ — 3-band biquad

Three second-order biquad sections in series. Direct Form II transposed
(memory-efficient, numerically stable).

| Band | Type | Default freq | Gain range | Extra |
|---|---|---|---|---|
| Low | Low shelf | 200 Hz | ±15 dB | — |
| Mid | Peak EQ | 1 000 Hz | ±15 dB | Q 0.1–10, default 0.7 |
| High | High shelf | 6 000 Hz | ±15 dB | — |

Coefficients computed from the Audio EQ Cookbook (Zölzer / Reiss &
McPherson). Recomputed on parameter change, not per sample.

## Drive — soft clip with asymmetry

Pre-gain (1×–20×), tanh soft clip, optional asymmetry (bias before
clip, remove after), output level compensation to prevent loudness
jumps.

```
y = tanh((x + bias) * drive) / tanh(drive) - bias_compensate
```

## Chorus — 3-tap, two-LFO

Three delay-line taps; each tap position is modulated independently.
LFO1 drives taps 1 and 3 in-phase; LFO2 (90° offset) drives tap 2.
Left and right use the same depth but opposite LFO phase to create
stereo width without comb filtering on mono sources.

| Param | Range | Default |
|---|---|---|
| Rate | 0.1–8 Hz | 0.5 Hz |
| Depth | 0–15 ms | 3 ms |
| Mix | 0–1 | 0.5 |
| Spread | 0–1 | 0.5 |

## Delay — stereo, sync, ping-pong

Stereo delay with a low-cut filter (one-pole) in the feedback path.
Ping-pong mode routes L output to R input and vice versa.

| Param | Range | Default |
|---|---|---|
| Time | 1 ms–2 s (or BPM divisions) | 375 ms |
| Feedback | 0–0.95 | 0.35 |
| Mix | 0–1 | 0.30 |
| Low-cut | 20–2 000 Hz | 200 Hz |
| Ping-pong | bool | false |

Maximum delay buffer: 2 s × sample_rate per channel. Allocated
once at `Engine::new` — zero allocations on the audio thread.

## Reverb — FDN-8

8-channel feedback delay network (Jot 1992 / Schroeder heritage).

| Param | Range | Default |
|---|---|---|
| Predelay | 0–50 ms | 10 ms |
| Decay | 0.1–30 s | 2 s |
| Size | 0.1–1.0 | 0.7 |
| Damping | 0–1 | 0.5 |
| Mix | 0–1 | 0.25 |

Eight prime-length delay lines (scaled by `size`). Hadamard feedback
matrix. Per-line one-pole absorption filters (controlled by `damping`).
Denormal guard: add `1e-20` to each delay line input each block.

---

## Implementation notes

### Coefficient stability (EQ / Chorus)

EQ biquad coefficients must not be recomputed at audio rate. Compute
on parameter change only (stepped update, same pattern as ADSR times
in M2/M6).

### Memory (Delay / Reverb)

Both effects use circular buffers allocated at init time. No allocations
on the audio thread (R2 compliance). Maximum delay time is fixed at
init; if BPM sync is added later, the buffer is large enough already.

### Denormals (Reverb)

FDN feedback loops can produce sub-normal floating-point values in the
tail. Inject a tiny DC offset (`1e-20`) at the input of each delay line
to keep values in normal range without audible effect.

### Mod matrix wiring

Targets added in M8 (mod matrix expanded in M11):
- `ReverbMix` — allow LFO or Env2 to swell the reverb
- `ChorusDepth` — allow Env2 to widen chorus on attack
- `DelayFeedback` — allow mod wheel to push into wash territory

---

## File layout

```
crates/synth-engine/src/fx/
├── mod.rs      -- FxChain struct, chains all 5 effects
├── biquad.rs   -- reusable Biquad (used by EQ and Delay's LP filter)
├── eq.rs       -- Eq3Band
├── drive.rs    -- Drive
├── chorus.rs   -- Chorus
├── delay.rs    -- StereoDelay
└── reverb.rs   -- Fdn8Reverb
```
