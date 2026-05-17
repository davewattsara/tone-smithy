# Unit testing

How we write automated unit tests in this repo. Prescriptive — these are decisions, not suggestions.

This doc covers **unit tests only** — tests that exercise a single unit (function, type, small module) in isolation. Other test types each get their own doc as we approach the milestone that needs them:

- **Integration tests** — multi-module, cross-thread, engine-plus-host behaviour. (Future doc.)
- **Snapshot tests** — render fixed MIDI through the engine, compare audio output to a baseline. (Future doc, around M2/M5.)
- **Property tests** — round-trip presets through serialise/deserialise/migrate. (Future doc, around M10.)
- **Real-time safety tests** — `assert_no_alloc`-wrapped audio path. (Future doc, around M3.)
- **Benchmarks** — `criterion` micro-benches for hot DSP loops. (Future doc, alongside performance work.)

For now, this is unit tests.

---

## Principles

1. **Test behaviour, not implementation.** A test that fails when code is refactored without behaviour change is a bad test. Write tests against the public observable behaviour of the unit.
2. **Pragmatic coverage.** No coverage floor. Test what's worth testing: DSP correctness, edge cases, invariants, public APIs. Don't chase coverage percentages by testing trivial code.
3. **Tests are documentation.** A test name should read like a sentence describing what the unit does. A maintainer reading test names should learn the unit's contract.
4. **Determinism is mandatory.** Tests that pass sometimes are worse than no test. Seed randomness, avoid time/IO/threads at the unit-test level.

---

## Where unit tests live

- **Inline `#[cfg(test)] mod tests`** at the bottom of the source file they test.
- One `tests` module per source file.
- **Do not** put unit tests in the `tests/` directory at the crate root — that directory is reserved for integration tests (different process, no access to private items).

```rust
// crates/synth-engine/src/envelope.rs

pub struct Adsr { ... }

impl Adsr {
    pub fn next(&mut self) -> f32 { ... }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_reaches_one_at_end_of_attack_time() { ... }
}
```

---

## What to unit test

### Always

- **Pure functions with non-trivial logic** — DSP math, parameter mapping, serialisation/validation helpers, voice-stealing decisions.
- **Public APIs** of each crate's exported types — at minimum, the happy-path call site.
- **Invariants enforced by types** — parameter clamping, range checks, state-machine transitions.
- **Error paths** — does the right error get returned in the right situation?
- **Edge cases** — zero, negative, NaN, infinity for floats; empty slices; max/min buffer sizes.
- **State-machine transitions** — envelope ADSR stages, voice lifecycle, filter mode changes.

### Selectively (only when worth it)

- **Methods that just wire other tested units together.** If the test would essentially restate the implementation, it's likely integration territory.
- **Helpers in `synth-ui` that compute display strings or format numbers.**

### Don't unit test

- **Trivial getters/setters.** If the test is `assert_eq!(x.foo(), x.foo)`, skip it.
- **UI rendering.** That's visual, not unit.
- **Audio output** (waveform shape, spectral content). That's snapshot territory.
- **Cross-thread or cross-module behaviour.** That's integration.
- **External library behaviour.** Trust `serde`, `crossbeam`, `cpal` — test our use of them via integration, not by re-testing them.

---

## Test naming

Format: `<verb_phrase_describing_behaviour>`.

Read each test name as a sentence: *"test that the envelope releases to silence"*, *"test that the voice manager steals the oldest released voice first"*.

```rust
#[test] fn releases_to_silence() { ... }
#[test] fn clamps_negative_pitch_to_minimum() { ... }
#[test] fn returns_error_on_unknown_param_id() { ... }
#[test] fn steals_oldest_released_voice_first() { ... }
#[test] fn rejects_preset_with_future_schema_version() { ... }
```

- No `test_` prefix — the `#[test]` attribute makes that redundant.
- No camelCase — snake_case per Rust convention.
- No "should" — implied by the `#[test]` context.

---

## Test structure (Arrange / Act / Assert)

Three logical phases, separated by blank lines for visibility:

```rust
#[test]
fn attack_reaches_one_at_end_of_attack_time() {
    // Arrange
    let sample_rate = 48_000.0;
    let mut env = Adsr::new(sample_rate);
    env.set_attack_ms(10.0);
    env.note_on();
    let attack_samples = (sample_rate * 0.010) as usize;

    // Act
    let mut value = 0.0;
    for _ in 0..attack_samples {
        value = env.next();
    }

    // Assert
    assert!(approx_eq(value, 1.0, 1e-3), "envelope should reach 1.0 at end of attack, got {value}");
}
```

The Arrange / Act / Assert comments are optional once the structure is obvious; the blank lines aren't.

---

## Assertions

- **Order:** `assert_eq!(actual, expected)` — actual first. Failure messages read naturally: *"left: 0.42, right: 1.0"* — "we got 0.42, expected 1.0".
- **Context:** add a message when the failure isn't self-explanatory: `assert_eq!(actual, expected, "after {n} samples, env value was wrong")`.
- **Boolean checks:** `assert!(condition, "explanation")`.
- **Floats:** never `==`. Always tolerance — see below.

---

## Floating-point comparison

Almost all DSP code is `f32`. Direct equality is wrong almost everywhere.

Helper, defined once per crate that needs it (e.g. `synth-engine/src/test_utils.rs` behind `#[cfg(test)]`):

```rust
#[cfg(test)]
pub(crate) fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
    (a - b).abs() < epsilon
}

#[cfg(test)]
macro_rules! assert_approx_eq {
    ($a:expr, $b:expr, $eps:expr) => {
        let (a, b, eps) = ($a, $b, $eps);
        assert!(approx_eq(a, b, eps), "expected {a} ≈ {b} (ε={eps})");
    };
}
```

Tolerances:

| Test category | Typical ε | Example |
|---|---|---|
| Bit-exact data shuffling | `0.0` | Re-serialised preset matches original |
| Single-operation math | `1e-6` | A pan law calculation |
| Composed DSP (few ops) | `1e-3` | Envelope value at a sample index |
| Filter response over a block | `1e-2` | RMS of filtered signal |
| Audio-level | `1e-1` | (Reserved for snapshot tests — not unit) |

Pick the loosest tolerance that still catches real bugs. Tighter than necessary makes tests fragile under harmless implementation tweaks.

---

## Setup, teardown, shared state

- **Prefer no setup.** A test that fits in one screen with no helpers is easiest to maintain.
- **If setup is needed, use a builder function.** Don't use `#[test]` `setup`/`teardown` patterns from other languages — there isn't one in Rust.
  ```rust
  fn make_engine_with_default_patch() -> Engine {
      let mut engine = Engine::new(48_000.0, 256);
      engine.load_default();
      engine
  }
  ```
- **Never share mutable state between tests.** Cargo runs tests in parallel by default; shared `static mut` will silently corrupt under parallelism.
- **Each test is independent.** Order must not matter.

---

## Determinism

Tests **must** be deterministic.

- **Seed every PRNG explicitly.** Never use `thread_rng()` or other entropy sources in tests.
  ```rust
  use rand::{SeedableRng, rngs::StdRng};
  let mut rng = StdRng::seed_from_u64(0x517);
  ```
- **No wall-clock time.** Don't measure elapsed time; don't use `Instant::now()` to compare values.
- **No file system or network.** Period.
- **No spawned threads.** Threading lives in integration tests with explicit synchronisation.
- **No floating-point summation that depends on iteration order.** SIMD-vectorised vs scalar summation can produce different results at the last bit; if a test depends on the exact bit-pattern, narrow the test or widen the tolerance.

---

## Test data

- **Small data: inline in the test.** A handful of literals is fine and keeps the test readable.
- **Larger fixtures: `tests/fixtures/` under the crate root.** (Rare for unit tests; common for integration / snapshot tests.)
- **Generated data with a seed.** If you need a varied input set, generate it with a seeded PRNG inside the test.

---

## DSP-specific test patterns

These are templates for the most common unit tests in `synth-engine`.

### Oscillator: periodicity

```rust
#[test]
fn sine_completes_one_full_cycle_per_period() {
    let sample_rate = 48_000.0;
    let frequency = 1.0;
    let mut osc = SineOscillator::new(sample_rate);
    osc.set_frequency_hz(frequency);

    let samples_per_period = (sample_rate / frequency) as usize;
    let mut last = 0.0;
    for _ in 0..samples_per_period {
        last = osc.next();
    }

    assert_approx_eq!(last, 0.0, 1e-3);
}
```

### Oscillator: zero-crossings

```rust
#[test]
fn saw_has_one_zero_crossing_per_period() {
    let sample_rate = 48_000.0;
    let mut osc = SawOscillator::new(sample_rate);
    osc.set_frequency_hz(440.0);

    let mut crossings = 0;
    let mut previous = osc.next();
    for _ in 0..(sample_rate as usize) {
        let current = osc.next();
        if (previous < 0.0) != (current < 0.0) {
            crossings += 1;
        }
        previous = current;
    }

    // 440Hz × 1 second = ~440 cycles; allow ±2 for rounding
    assert!((crossings as i32 - 440).abs() <= 2, "got {crossings} crossings");
}
```

### Filter: cutoff attenuation

```rust
#[test]
fn lowpass_attenuates_above_cutoff() {
    let sample_rate = 48_000.0;
    let mut filter = SvfLowpass::new(sample_rate);
    filter.set_cutoff_hz(1_000.0);

    let input_rms = sine_rms(5_000.0, sample_rate, 4096);
    let output_rms = run_filter_rms(&mut filter, 5_000.0, sample_rate, 4096);

    // 5kHz is well above 1kHz cutoff; expect significant attenuation
    assert!(output_rms < input_rms * 0.5, "input rms {input_rms}, output rms {output_rms}");
}
```

### Envelope: ADSR stage transitions

```rust
#[test]
fn release_decays_to_zero_after_release_time() {
    let sample_rate = 48_000.0;
    let mut env = Adsr::new(sample_rate);
    env.set_attack_ms(0.0);
    env.set_decay_ms(0.0);
    env.set_sustain(1.0);
    env.set_release_ms(10.0);

    env.note_on();
    let _ = env.next();           // jump past attack/decay
    env.note_off();
    let release_samples = (sample_rate * 0.010) as usize + 8;
    let mut value = 0.0;
    for _ in 0..release_samples {
        value = env.next();
    }

    assert_approx_eq!(value, 0.0, 1e-3);
}
```

### Voice manager: voice-stealing policy

```rust
#[test]
fn steals_oldest_released_voice_first() {
    let mut vm = VoiceManager::new(2);
    vm.note_on(60, 100);          // voice 0
    vm.note_on(62, 100);          // voice 1
    vm.note_off(60);              // voice 0 released
    vm.note_on(64, 100);          // should steal voice 0

    assert_eq!(vm.note_of_voice(0), Some(64));
    assert_eq!(vm.note_of_voice(1), Some(62));
}
```

### Parameter clamping

```rust
#[test]
fn cutoff_clamps_to_nyquist() {
    let sample_rate = 48_000.0;
    let mut filter = SvfLowpass::new(sample_rate);
    filter.set_cutoff_hz(40_000.0);   // above Nyquist (24kHz)

    assert_approx_eq!(filter.cutoff_hz(), 24_000.0, 1.0);
}
```

---

## Per-crate strategy

### `synth-engine`
Heaviest test surface. Every DSP block has tests for: correctness at typical parameter values, edge-case behaviour at extremes, and state transitions. Anti-aliasing is verified by spectrum properties at high frequencies (sweep tests live in snapshot territory; the *parameter-handling* parts can be unit-tested).

### `synth-host`
- Parameter event handling (received `EngineEvent::ParameterChange` produces correct internal updates).
- MIDI message → `EngineEvent` conversion (note on/off, pitch bend, CC mapping).
- Settings parsing.

Cross-thread queue behaviour and `cpal` integration are integration tests.

### `synth-presets`
- Parameter ID → default value lookup.
- Validation/clamping on load.
- Schema-version comparison logic.

Round-trip (serialise → deserialise → equal) and migration correctness are property-test territory.

### `synth-ui`
- Pure helpers only — display-string formatting, value-to-knob-angle mapping, colour palette lookups.
- Widget rendering isn't unit-tested.

### `synth-app`
Minimal direct unit tests; mostly composition that's tested via integration.

---

## Running tests

- **All:** `cargo test --workspace`
- **One crate:** `cargo test -p synth-engine`
- **One module:** `cargo test -p synth-engine envelope::`
- **One test:** `cargo test -p synth-engine releases_to_silence`
- **Release mode:** `cargo test --workspace --release` — runs once in CI to catch optimisation-only bugs (denormals, fast-math behaviours).

---

## Doctests

`///` doc-comment code blocks are real tests. They count as unit tests for our purposes.

- Include them for any public function whose behaviour benefits from a worked example (per [`code-style.md`](code-style.md#documentation-comments)).
- A failing doctest fails CI.
- Use `# fn main()` to hide setup that isn't part of the example.

```rust
/// Computes constant-power pan gains.
///
/// # Examples
///
/// ```
/// use synth_engine::amplifier::pan_gains;
/// let (l, r) = pan_gains(0.0);
/// assert!((l - r).abs() < 1e-6, "centre pan should be equal L/R");
/// ```
pub fn pan_gains(pan: f32) -> (f32, f32) { ... }
```

---

## What this doc deliberately doesn't cover

Listed at the top, repeated here for the index:

- Integration testing (separate doc).
- Snapshot / audio output testing (separate doc).
- Property-based testing (separate doc).
- Real-time safety / `assert_no_alloc` (separate doc).
- Benchmarks with `criterion` (separate doc).
- UI / visual testing.

Each gets its own doc when we approach the milestone that needs it. Until then, this is the test-writing baseline.
