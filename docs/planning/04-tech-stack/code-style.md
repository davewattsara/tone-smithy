# Code style

How code is written in this repo. Prescriptive, on purpose — a single consistent style makes it much easier for newcomers (and future-you) to read, modify, and trust the codebase.

Mechanical formatting (line width, spaces, trailing commas) is handled by `rustfmt` per [`tooling.md`](tooling.md). This doc covers the things `rustfmt` and `clippy` can't enforce: structure, naming, documentation, and judgement.

---

## Principles

1. **Optimise for the reader, not the writer.** Code is read many more times than it is written. A few extra characters in a clear name save minutes of head-scratching.
2. **Document the *why*; let the code show the *what*.** Public APIs need doc comments because callers can't see the implementation. Inline comments belong where the code would otherwise be surprising.
3. **Newcomers are first-class citizens.** Someone with general Rust experience but no audio background should be able to open any file in this repo, read it top-to-bottom, and understand what's going on.
4. **Boring code wins.** Clever one-liners are debt. Reach for the simplest construct that does the job; reach for generics, traits, and macros only when they materially reduce duplication.
5. **Naming is a feature.** A well-named symbol replaces a comment.

---

## Formatting

`rustfmt` with the project `rustfmt.toml` is authoritative. Don't argue with it — fix the formatter config if a rule consistently produces bad output, don't hand-format individual files.

- `edition = "2024"` (or the latest stable at project start)
- `max_width = 120`
- Defaults for everything else.

If a piece of code looks ugly after formatting, that's a signal to restructure — extract a function, split an expression, introduce intermediate variables — not to override the formatter.

---

## Naming

### General

- **Types** (structs, enums, traits, type aliases): `PascalCase`.
- **Values** (functions, methods, variables, fields, modules): `snake_case`.
- **Constants and statics**: `SCREAMING_SNAKE_CASE`.
- **Generic parameters**: short and `PascalCase` (`T`, `E`, `S` for source, `Out` if `T` is ambiguous).
- **Lifetimes**: short and lowercase (`'a`, `'src`, `'engine`).

### Acronyms count as one word

```rust
// good
struct LfoConfig { ... }
fn parse_fm_algorithm(...) { ... }
const MAX_ADSR_TIME_MS: f32 = 30_000.0;

// avoid
struct LFOConfig { ... }
fn parse_FM_algorithm(...) { ... }
```

`clippy::upper_case_acronyms` enforces this.

### Domain-aware names

Audio code has many quantities that look like the same primitive (`f32` everywhere). Make them legible:

| Quantity | Convention | Example |
|---|---|---|
| Frequency in Hz | suffix `_hz` | `cutoff_hz: f32` |
| Time in seconds | suffix `_secs` or `_s` | `release_secs: f32` |
| Time in milliseconds | suffix `_ms` | `attack_ms: f32` |
| Pitch in cents | suffix `_cents` | `detune_cents: f32` |
| Pitch in semitones | suffix `_semis` | `transpose_semis: i32` |
| Sample index | suffix `_samples` | `delay_samples: usize` |
| Buffer length | `block_size`, `buffer_size` | `block_size: usize` |
| Voice index | `voice_id` or `voice_idx` | `voice_id: u8` |
| Linear amplitude (0..1) | `amp` or `_amp` | `amp: f32` |
| Decibels | suffix `_db` | `gain_db: f32` |
| MIDI note number | suffix `_midi` or type | `note_midi: u8` |
| Sample rate | `sample_rate` (never `sr`) | `sample_rate: f32` |
| Phase (0..1 or 0..TAU) | `phase` (document range) | `phase: f32 /* 0..TAU */` |

When the type system can do it better than a suffix, use a newtype (`Hz(f32)`, `Cents(f32)`) — but newtypes have ergonomics cost, so reserve them for values that cross many module boundaries.

### Avoid abbreviations

- `sample_rate`, not `sr`.
- `frequency`, not `freq`. (`*_hz` is fine because it names the unit, not abbreviates the word.)
- `parameter`, not `param` (unless it's so universal in audio code — `ParamId`, `params: &[Parameter]` — that the long form would look odd). Be consistent within a module.
- `index`, not `idx` in public APIs. In tight loops, `idx` or `i` is fine.

### Booleans

Prefer affirmative names; avoid negatives.

```rust
// good
let is_enabled: bool;
let has_modulation: bool;

// avoid
let is_disabled: bool;   // reader has to invert
let no_modulation: bool;
```

---

## File and module organisation

### Within a file

Order top-to-bottom:

1. **Module documentation** — `//!` block describing what this module is for.
2. **`use` statements** — grouped into three blocks separated by a blank line:
   1. `std::...`
   2. external crates
   3. `crate::...`, `super::...`, `self::...`
3. **Public re-exports** (`pub use`) — only if this is a module root that aggregates submodules.
4. **Constants and statics** — public first, then private.
5. **Types** — structs, enums, type aliases — each immediately followed by its `impl` blocks.
6. **Free functions** — public first, then private helpers.
7. **`#[cfg(test)] mod tests`** — at the bottom.

A file should rarely exceed 500 lines. If it grows past that, split by responsibility.

### Within a crate

- One top-level concept per module.
- Use `<name>.rs` for leaf modules; use `<name>/mod.rs` (or `<name>.rs` + `<name>/` directory) for modules with submodules.
- The crate root (`lib.rs`) re-exports the public API. External callers should never need to know the internal module structure.

```rust
// crates/synth-engine/src/lib.rs
//! Tone Smithy DSP engine — voice management, oscillators, filters,
//! modulation, and effects. No I/O dependencies.

pub use crate::engine::Engine;
pub use crate::events::EngineEvent;
pub use crate::params::{ParamId, ParamSnapshot};
// ...

mod engine;
mod events;
mod params;
mod voice;
mod voice_manager;
mod oscillator;
mod filter;
mod envelope;
mod lfo;
mod modulation;
mod effects;
```

---

## Documentation comments

Rust has two doc-comment forms:

- `//!` — **inner doc comment**, documents the enclosing item (typically a module or crate, written at the very top).
- `///` — **outer doc comment**, documents the next item (a type, function, field).

### Every module gets a `//!` header

At the top of every `.rs` file:

```rust
//! Subtractive oscillator slot. Hosts up to three primary oscillators
//! (saw / square / triangle / noise) plus a sub oscillator, each with
//! per-oscillator detune, level, pan, and unison spread.
//!
//! Anti-aliasing uses PolyBLEP for saw and square; see [`polyblep`]
//! for the implementation and the references it draws from.
```

Three uses, in order:

1. One sentence: what is this module *for*?
2. Optional: what does it contain, at a glance?
3. Optional: cross-references to related modules (`[`name`]` becomes a link in `cargo doc`).

### Every public item gets a `///`

If a function, type, enum variant, struct field, or constant is `pub`, it has a doc comment. No exceptions for "obvious" items — what's obvious to the author rarely is to the reader six months later.

Structure for non-trivial public items:

```rust
/// Brief one-line summary of what this does.
///
/// Longer description, if useful — invariants, intended use, gotchas.
/// Don't restate the signature; explain meaning, not mechanics.
///
/// # Examples
///
/// ```
/// use synth_engine::Engine;
/// let mut engine = Engine::new(48_000.0, 256);
/// engine.process(&mut buffer);
/// ```
///
/// # Panics
///
/// Panics if `sample_rate <= 0.0` or `max_block_size == 0`.
///
/// # Errors
///
/// Returns [`EngineError::DeviceMismatch`] if the buffer length
/// exceeds `max_block_size`.
fn ...
```

Section guide:

- **Summary line**: complete sentence, period at the end, under ~80 chars.
- **Body paragraph(s)**: separated by blank lines.
- **`# Examples`**: include for any API a newcomer will reasonably want to call. Doctests are real tests — they run in CI and keep examples honest.
- **`# Panics`**: list every condition under which the function panics. If the function never panics, omit the section.
- **`# Errors`**: for `Result`-returning functions, name the error variants and when each is produced.
- **`# Safety`**: required for `unsafe fn`. Explain the invariants the caller must uphold.

### Field-level docs

Public fields also get `///`:

```rust
pub struct OscillatorConfig {
    /// Linear amplitude, 0.0 = silent, 1.0 = full scale. No upper clamp;
    /// values above 1.0 are allowed and will be summed by the mixer.
    pub level: f32,

    /// Detune in cents. Conventional range -1200..=1200; the engine
    /// allows wider but extreme values may alias.
    pub detune_cents: f32,
}
```

Crate-private items (`pub(crate)` or no `pub`) do **not** need doc comments — but a one-line `//` is welcome if the purpose isn't obvious.

### What to write in a doc comment

- The *meaning* of the parameter or return value, beyond what the type already says.
- The *contract*: what the caller must guarantee, what the function guarantees in return.
- Units, ranges, defaults — especially for floats.
- Cross-references to related items with `` `[`OtherType`]` `` (linkified by `cargo doc`).

What *not* to write:

- A retelling of the signature: `/// Returns the cutoff` adds nothing.
- The current implementation strategy (it'll go stale).
- The PR or issue that introduced the function (that belongs in `git log`).

---

## Inline comments

Inline comments (`//`) explain what isn't already clear from the code itself.

### Use a comment when

- **The math is non-obvious.** DSP formulas should cite their source.
  ```rust
  // Topology-preserving transform SVF coefficient (Vadim Zavalishin,
  // "The Art of VA Filter Design", section 5.2).
  let g = (PI * cutoff_hz / sample_rate).tan();
  ```

- **There's a hidden constraint.**
  ```rust
  // Must be called before `prepare()` — voice count cannot change
  // once the audio thread is running.
  pub fn set_polyphony(&mut self, voices: usize) { ... }
  ```

- **A workaround needs context.**
  ```rust
  // `tan()` is undefined near pi/2; clamp to keep the filter stable
  // at extreme cutoffs. See issue tracker for the exact failure mode.
  let g = ((PI * cutoff_hz / sample_rate).min(PI * 0.49)).tan();
  ```

- **A subtle invariant matters.**
  ```rust
  // `current_voices` is updated only on the audio thread; the UI
  // reads it via the atomic telemetry struct.
  ```

### Don't write a comment when

- The comment restates the code.
  ```rust
  // BAD — adds nothing
  // Set the cutoff
  filter.set_cutoff(value);
  ```

- You're commenting out code. Delete it; `git` remembers.

- You're using `//` as a section header inside a function. Extract a function instead.

- The comment is just a thank-you, mood, or noise.

### TODO comments

Use `// TODO:` sparingly, and always with a short note explaining what and why.

```rust
// TODO: replace with SIMD path once `std::simd` stabilises on our MSRV.
```

Pure `// TODO` with no context is forbidden — if it's worth marking, it's worth a sentence.

---

## Error handling

### Library crates (`synth-engine`, `synth-host`, `synth-presets`, `synth-ui`)

Use `thiserror` to define typed error enums per crate.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PresetError {
    /// The preset file could not be read from disk.
    #[error("could not read preset file at {path}: {source}")]
    Read { path: PathBuf, #[source] source: std::io::Error },

    /// The preset format is older than this build can migrate.
    #[error("preset version {version} is older than the minimum supported version {minimum}")]
    UnsupportedVersion { version: u32, minimum: u32 },
}
```

- Variant names describe the *condition*, not the *action*.
- `#[error("...")]` message: lowercase, no trailing period, explains what went wrong from a developer's perspective.
- Use `#[source]` (or `#[from]` for transparent wrapping) for underlying causes — preserve the chain.

### Application (`synth-app`)

Use `anyhow::Result<T>` at the top level. Add context generously:

```rust
use anyhow::{Context, Result};

fn load_factory_presets(dir: &Path) -> Result<PresetBank> {
    let bank = PresetBank::load_dir(dir)
        .with_context(|| format!("failed to load factory presets from {}", dir.display()))?;
    Ok(bank)
}
```

### `unwrap()` and `expect()`

- `unwrap()` is **not allowed** in non-test code. If a value cannot be `None` / `Err`, prove it with `expect("explanation")`.
- `expect()` message: a complete sentence stating the invariant.
  ```rust
  let snapshot = self.snapshot.take()
      .expect("snapshot must be present at the start of each audio block — see Engine::prepare");
  ```

### Panics

Panic only when continuing would corrupt state. On the audio thread, a panic aborts the process (see [`../03-architecture/design-patterns.md`](../03-architecture/design-patterns.md), §2.8) — so write audio-thread code defensively to make panics impossible.

---

## Imports

- Group `use` statements into three blocks (std / external / crate), separated by blank lines.
- Sort within each block.
- Prefer nested imports when they reduce noise:
  ```rust
  use std::sync::{atomic::AtomicU32, Arc};
  ```
- Avoid `use foo::*` at module scope (acceptable in `tests` modules and `prelude` patterns).

---

## Tests

- Unit tests live in `#[cfg(test)] mod tests { ... }` at the bottom of the file they test.
- Integration tests live in `tests/` next to `src/`.
- Test names describe behaviour:
  ```rust
  #[test]
  fn release_phase_runs_to_silence() { ... }
  #[test]
  fn voice_stealing_picks_oldest_released_first() { ... }
  ```
- Use `assert_eq!(actual, expected)` (not the other way around) so failure messages read naturally.
- Doctests in `///` blocks are real tests; keep them honest. Use `///` `# fn main()` blocks to hide setup that distracts from the example.

---

## What to avoid

- **Premature traits.** A trait with one implementor is just an interface gestured at. Wait until the second impl exists.
- **Premature generics.** Concrete types are easier to read. Generics earn their keep when there are multiple instantiations.
- **Macros when a function works.** Macros are harder to read and debug. Use them when the alternative is significant boilerplate.
- **Deep nesting.** If a function has more than three levels of indentation, extract.
- **One-letter variable names outside tight scopes.** `i`, `j`, `k` in a loop are fine; `a`, `b` as function parameters are not.
- **`mod.rs` files containing real code.** Use `mod.rs` only to declare submodules and re-export their contents. Put implementation in named files.
- **Unused `_underscore` prefixes left in committed code.** If a value is truly unused, remove it. Prefix only when transitional.
- **Long match arms with side effects.** If an arm is more than a few lines, extract a function.

---

## A worked example

A short illustration of the above, end to end:

```rust
//! Per-voice amplifier stage. Applies the amp envelope and master
//! level to a mono signal, with constant-power pan to stereo.

use crate::envelope::Adsr;

/// Drives a mono input through the amp envelope and pans it to stereo.
///
/// The amplifier owns its [`Adsr`]; the voice manager calls
/// [`Amplifier::note_on`] / [`Amplifier::note_off`] to drive envelope
/// state transitions.
pub struct Amplifier {
    /// The amplitude envelope. Always present; controls voice lifetime.
    envelope: Adsr,

    /// Constant linear gain applied after the envelope. 1.0 = unity.
    level: f32,

    /// Pan, -1.0 = full left, 0.0 = centre, 1.0 = full right.
    pan: f32,
}

impl Amplifier {
    /// Creates a new amplifier with default level (1.0) and centre pan.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            envelope: Adsr::new(sample_rate),
            level: 1.0,
            pan: 0.0,
        }
    }

    /// Processes one block in place, writing stereo output.
    ///
    /// `input` is mono, length `n`. `out_left` and `out_right` must also
    /// be length `n`. Existing contents of the output buffers are
    /// overwritten.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if buffer lengths disagree. In release
    /// builds, processes `min(input.len(), out_left.len(), out_right.len())`.
    pub fn process(&mut self, input: &[f32], out_left: &mut [f32], out_right: &mut [f32]) {
        debug_assert_eq!(input.len(), out_left.len());
        debug_assert_eq!(input.len(), out_right.len());

        // Constant-power pan: equal perceived loudness across the stereo
        // field. See e.g. Pirkle, "Designing Audio Effect Plug-Ins in C++",
        // section on panning laws.
        let pan_rad = (self.pan + 1.0) * std::f32::consts::FRAC_PI_4;
        let left_gain  = pan_rad.cos() * self.level;
        let right_gain = pan_rad.sin() * self.level;

        for i in 0..input.len() {
            let env = self.envelope.next();
            let sample = input[i] * env;
            out_left[i]  = sample * left_gain;
            out_right[i] = sample * right_gain;
        }
    }
}
```

Note the things it does:

- Module-level `//!`.
- `///` on the struct, every field, and every public method.
- A doc comment that explains *meaning* (units, ranges) rather than restating types.
- A `# Panics` section listing exactly when the function panics.
- An inline comment that cites a reference for non-obvious DSP math.
- A `debug_assert!` to make the panic precondition explicit and free in release.
- Domain naming (`pan_rad`, `left_gain`, `sample_rate`) without abbreviations.
