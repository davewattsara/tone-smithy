# Audio engine

The DSP heart of the synth. Lives in the `synth-engine` crate. No I/O, no allocations on the audio path.

## Signal flow (per voice)

```
              ┌─────────────┐    ┌─────────────┐
   note on ─▶ │ OSC Slot 1  │    │ OSC Slot 2  │
              │ (subtr/FM)  │    │ (subtr/FM)  │
              └──────┬──────┘    └──────┬──────┘
                     ▼                  ▼
                   ┌────────────────────────┐
                   │       Mixer            │  per-slot level + pan
                   └───────────┬────────────┘
                               ▼
                   ┌────────────────────────┐
                   │   Filter Section       │  F1 → F2 (serial)
                   │   (LP/HP/BP/Notch)     │  or F1 ∥ F2 (parallel)
                   └───────────┬────────────┘
                               ▼
                   ┌────────────────────────┐
                   │   Amp + Amp Envelope   │
                   └───────────┬────────────┘
                               ▼
                          to global mix
```

Modulation sources (Env2, Env3, LFO1, LFO2, MIDI sources) run in parallel per voice and feed the **modulation matrix**, which sums into the various destinations above each block.

## Voice management

- **32 voices**, allocated up front in a fixed array.
- Each voice owns its DSP state; voices do not share heap data while running.
- **Voice stealing** policy: oldest released voice first; if none are in release, steal the quietest active voice.
- **Note-off** triggers the release phase of the amp envelope; the voice is freed when amp goes below a small epsilon for one full block.
- A `VoiceManager` struct holds the array, dispatches note-on/off events, and orchestrates the per-block processing loop.

## Block-based processing

The audio callback receives a buffer of N samples. Internally the engine processes in **fixed inner blocks** (default 64 samples), which:

- Amortises parameter smoothing and modulation updates.
- Maps well to SIMD vectorisation.
- Makes it easy to interleave per-voice processing without re-doing per-block setup every sample.

If `N` is not a multiple of 64, the final block is processed at its true size.

Parameter changes from the UI are drained at the top of each callback (not each inner block), so a single block sees a coherent parameter state.

## DSP building blocks

### Oscillators (subtractive)
- **PolyBLEP** anti-aliasing for saw and square.
- Triangle via integrated square or a polynomial approximation.
- Noise from a fast xorshift PRNG.
- Sub oscillator: pure sine, one octave below the slot's primary frequency.
- Per-oscillator: detune in cents, fine tune, level, hard sync (defer to v1.x).
- Unison: 1–7 stacked voices per oscillator with detune spread and stereo pan spread.

### Operators (FM)
- 4 sine operators per slot.
- Each operator has: frequency ratio, fine tune (cents), level, ADSR envelope (own per-op envelope, separate from the per-voice envelopes).
- 8 starter algorithms (DX7 family routings: linear stack, parallel pairs, feedback on op 4, etc.).
- Per-operator feedback (single tap), supported on op 4 in factory algorithms.
- Internal 2× oversampling on operator output when modulation index is high; tunable threshold.

### Filters
- **Topology-preserving transform (TPT) state-variable filter**, 12 dB/oct base.
- 24 dB/oct via cascaded TPT-SVF or a 4-pole ZDF ladder (decision during M2).
- Modes: LP, HP, BP, Notch, with crossfade for routing.
- Self-oscillation at maximum resonance.
- Per-filter input drive (soft saturation pre-filter).

### Envelopes
- ADSR with adjustable curve per stage (linear ↔ exponential).
- Sample-and-hold during sustain; one-shot mode (skips sustain) optional.

### LFOs
- Shapes: sine, triangle, saw up, saw down, square, sample-and-hold, smooth random.
- Free or tempo-synced. Phase reset on note-on optional. Per-voice or global mode.

### Modulation matrix
- 16 slots. Each slot: `(source, destination, amount, via_source_optional)`.
- Per-block summing: each block, sources are sampled once; destination accumulators receive the weighted contributions; modulated values are computed at block start.
- The "via" source allows depth modulation (e.g. mod wheel scales an LFO's effect on cutoff).

## Effects chain

Post-mix, fixed insert order: **EQ → Drive → Chorus → Delay → Reverb**. Each FX has a bypass and parameter set; selected parameters are mod-matrix-addressable.

- **EQ** — biquad-based, low shelf + parametric mid + high shelf.
- **Drive** — soft tanh, with pre-gain, tone, and asymmetry.
- **Chorus** — 3-tap modulated delay, two LFOs at slightly different rates for stereo movement.
- **Delay** — stereo delay line, sync to BPM or free; ping-pong mode swaps L/R on the feedback path; per-tap low/high cut.
- **Reverb** — **FDN8** (feedback delay network with 8 lines) starting point. Size, decay, damping (low/high), pre-delay, mix. Plate variant a possible later addition.

## Real-time safety

The audio thread:

- Never allocates. All buffers are pre-sized in `prepare(sample_rate, max_block_size)`.
- Never locks. Cross-thread communication is exclusively via lock-free queues and atomics.
- Never blocks. No file I/O, no network, no logging beyond a lock-free tracing layer (or none at all in release).
- Avoids panics. Panics on the audio thread abort the process; we audit with clippy and tests.

A CI test runs a synthetic workload through the engine under `assert_no_alloc` (or a custom global allocator that asserts in audio scope) to catch regressions.

## Parameter changes & smoothing

- UI sends `(param_id, value)` events through a lock-free SPSC ring sized for thousands of events.
- Continuous parameters use a one-pole smoother (~5–20 ms time constant) to avoid clicks.
- Discrete parameters (osc waveform, filter mode, FM algorithm) change at block boundaries to avoid mid-buffer glitches.

## Performance

- Tight inner loops are written to enable auto-vectorisation; hot loops will use explicit SIMD (`std::simd` when stable enough, otherwise the `wide` crate).
- Voices are processed sequentially in v1 (single-threaded engine); multi-threading voices is a v1.x consideration once profiling shows it would help.
- Profiling targets are in [`../01-vision/success-criteria.md`](../01-vision/success-criteria.md).

## Testing

- **Unit tests** for each DSP block (envelope shape, filter frequency response, oscillator zero-crossings).
- **Snapshot tests** for full-engine output (render a fixed MIDI sequence; compare audio fingerprint or spectrum to a baseline).
- **Benchmarks** with `criterion` for hot blocks (oscillator, filter, FM op, reverb).
- **Soak test** harness — long-running render with random parameter automation, checked for NaNs, infinities, and excessive amplitude.
