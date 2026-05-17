# Design patterns

The patterns this codebase commits to. These are decisions, not options — code that deviates from them should justify why in a code comment or PR description.

The doc is in two parts: **architectural patterns** (how the major pieces fit together) and **real-time safety patterns** (the hard rules for code that runs on the audio thread). Rust-specific idioms and DSP-internal patterns are *not* covered here — they belong elsewhere if they need codifying at all.

---

## Part 1 — Architectural patterns

### 1.1 Hexagonal architecture (ports and adapters)

**The pattern:** the audio engine is the centre. Everything else — audio I/O, MIDI, UI, presets, settings — is an adapter that surrounds it. The engine knows nothing about cpal, egui, midir, the file system, or the operating system.

**How we apply it:** the workspace's crate boundaries are the architectural boundaries.

```
                    synth-app
                  /     |     \
           synth-ui  synth-host  synth-presets
                  \     |     /
                     synth-engine          ← the hexagon's core
```

- `synth-engine` has **no I/O dependencies**. It declares an API and processes events. It can be unit-tested with zero external setup.
- `synth-host`, `synth-ui`, `synth-presets` are **adapters**. They translate from the outside world (audio drivers, mouse clicks, files on disk) into engine events.
- `synth-app` is the **composition root**. It is the only place that wires adapters to the engine.

**Why:** the engine survives every later change of plans — adding a CLAP plugin in v2 is a new adapter, not a rewrite. The UI can be swapped, the audio driver can be swapped, the preset format can be swapped; the engine doesn't notice.

**Test of the rule:** if `synth-engine/Cargo.toml` ever depends on `cpal`, `midir`, `eframe`, or `rfd`, the boundary has been broken. CI fails this case.

---

### 1.2 Command pattern via `EngineEvent`

**The pattern:** every input to the engine is an immutable event value. The engine has one entry point for events; it never calls back out into adapters synchronously.

**How we apply it:** one enum, `EngineEvent`, captures every possible input.

```rust
pub enum EngineEvent {
    NoteOn  { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8, velocity: u8 },
    PitchBend         { channel: u8, value: i16 },
    ChannelAftertouch { channel: u8, value: u8 },
    ModWheel          { channel: u8, value: u8 },
    Sustain           { channel: u8, on: bool },
    Cc                { channel: u8, controller: u8, value: u8 },
    ParameterChange   { id: ParamId, value: f32 },
    PresetChange      { snapshot: Box<ParamSnapshot> },
    TempoChange       { bpm: f32 },
}
```

Every adapter produces `EngineEvent`s and pushes them into a queue the engine drains. The engine never calls back into the UI or the MIDI thread; if the engine has something to publish, it does so via a separate snapshot/telemetry channel.

**Why:** events are auditable, testable, and serialisable. We can record a session as a stream of events and replay it deterministically. The engine has one place that needs reviewing when behaviour changes.

**Test of the rule:** every input to the engine flows through `EngineEvent`. If a subsystem needs a new input type, it adds a variant — it does not add a side-channel method on the engine.

---

### 1.3 Single source of truth for parameters

**The pattern:** all sound-affecting state lives in one parameter tree, owned by the engine. The UI does not own state; it reads snapshots and emits change events.

**How we apply it:**

- The engine owns a `ParameterTree` — a typed, flat collection of parameters with stable `ParamId`s.
- Each block, the engine publishes an **immutable `ParamSnapshot`** for the UI to consume.
- The UI **never mutates** parameter state directly. To change a parameter, the UI sends `EngineEvent::ParameterChange { id, value }`. The engine validates, clamps, smooths, and updates the tree.
- Presets are just serialised parameter trees; `EngineEvent::PresetChange` replaces the tree wholesale.

**Why:** one writer eliminates an entire class of synchronisation bugs. Presets can be saved by serialising the snapshot. Undo/redo becomes trivial (store snapshots).

**Test of the rule:** there is no setter on the parameter tree accessible from outside the engine crate. All mutation goes through `EngineEvent`.

---

### 1.4 Snapshot publishing (engine → UI)

**The pattern:** the engine publishes snapshots of its outward-facing state to a single atomic slot. Readers (UI, telemetry) load the latest snapshot whenever they want; they never block the engine.

**How we apply it:** an `ArcSwap<ParamSnapshot>` (from the `arc-swap` crate) is shared between the engine and UI.

```rust
// In the engine, at the top of each audio block:
let snapshot = self.params.snapshot();   // build immutable copy
self.snapshot_slot.store(Arc::new(snapshot));

// In the UI, every frame:
let snapshot = self.snapshot_slot.load_full();
self.render(&snapshot);
```

Telemetry (CPU%, level meters, active voice count) follows the same pattern with its own `ArcSwap<Telemetry>` slot.

**Why:** the audio thread never blocks waiting for the UI; the UI never blocks waiting for the audio thread. The snapshot is immutable, so multiple readers are safe.

**Cost:** one `Arc` allocation per block on the audio thread. That sounds like it violates §2.1, and it would — except we use a small `ParamSnapshot` (≤8 KB) and a recycled pool of pre-allocated `Arc<ParamSnapshot>` slots to keep allocation off the hot path. See §2.5.

---

### 1.5 Acyclic crate dependency graph (enforced)

**The pattern:** crates depend on each other in one direction only. The graph is checked in CI.

**How we apply it:** see §1.1 for the graph. `cargo deny check` (via `bans`) plus a simple `cargo metadata`-based check in the `xtask` crate verifies no cycles and no forbidden cross-dependencies (e.g. `synth-engine` must not depend on `cpal`).

**Why:** layering is only real if it's enforced. Once a cycle exists, untangling it is expensive.

---

### 1.6 One composition root

**The pattern:** wiring of adapters to the engine happens in exactly one place — `synth-app/src/main.rs`. No other crate constructs the full system.

**How we apply it:**

```rust
fn main() -> anyhow::Result<()> {
    let (ui_to_engine_tx, ui_to_engine_rx) = bounded::<EngineEvent>(4096);
    let (midi_to_engine_tx, midi_to_engine_rx) = bounded::<EngineEvent>(4096);
    let snapshot_slot = Arc::new(ArcSwap::from_pointee(ParamSnapshot::default()));
    let telemetry_slot = Arc::new(ArcSwap::from_pointee(Telemetry::default()));

    let engine = Engine::new(/* ... */);
    let audio = synth_host::audio::start(engine, ui_to_engine_rx, midi_to_engine_rx,
                                         snapshot_slot.clone(), telemetry_slot.clone())?;
    let _midi = synth_host::midi::start(midi_to_engine_tx)?;
    let _presets = synth_presets::Browser::new(/* ... */)?;

    synth_ui::run(ui_to_engine_tx, snapshot_slot, telemetry_slot)
}
```

**Why:** "where does this thing get started?" has one answer. Configuration, lifetimes, and error handling for the whole app live in one file.

---

## Part 2 — Real-time safety patterns

The audio thread is the only place these rules apply. Code in other threads can use mutexes, allocate, log, and call the file system normally.

### 2.1 The three forbidden things on the audio thread

On the audio callback thread, in steady state:

1. **No allocation.** No `Vec::push`, no `Box::new`, no `String::from`, no `format!`, no `Arc::new` (with exceptions handled by recycled pools).
2. **No locks.** No `Mutex::lock`, no `RwLock::read/write`. Atomic operations are fine.
3. **No syscalls or blocking I/O.** No file I/O, no logging in release builds, no `println!`, no network, no sleeping.

A panic on the audio thread aborts the process. This is intentional — silent corruption is worse than a clear crash.

These are not guidelines. They are checked in CI by tests running under `assert_no_alloc` (or an equivalent custom allocator) wrapping the audio-path entry point. A test that allocates in audio scope fails the build.

---

### 2.2 Lock-free SPSC ring for UI → engine

**The pattern:** the UI is a single producer; the engine is a single consumer. Use a bounded SPSC queue. The UI never blocks; if the queue is full, the change is dropped and a warning is logged on the UI thread.

**How we apply it:**

```rust
use crossbeam_channel::{bounded, Sender, Receiver, TrySendError};

// at startup:
let (tx, rx) = bounded::<EngineEvent>(4096);

// UI thread:
match tx.try_send(EngineEvent::ParameterChange { id, value }) {
    Ok(_) => {}
    Err(TrySendError::Full(_)) => tracing::warn!("UI->engine queue full; dropping event"),
    Err(TrySendError::Disconnected(_)) => { /* engine gone, app shutting down */ }
}

// Audio thread (at the top of each callback):
while let Ok(event) = rx.try_recv() {
    self.engine.handle(event);
}
```

**Why:** lock-free queues are the standard way to cross the audio/non-audio boundary without violating §2.1. `crossbeam-channel`'s bounded queues are battle-tested and allocation-free on `try_send` / `try_recv` once constructed.

**Sizing:** 4096 is a starting size; revisited at M3 with measurements. A queue should be large enough that bursty UI activity (rapid knob drags, preset loads) doesn't fill it.

---

### 2.3 MPSC bounded queue for multi-source input

**The pattern:** when more than one producer pushes to the engine (MIDI thread, UI thread, internal clock), use an MPSC queue, still bounded, still lock-free.

**How we apply it:** `crossbeam-channel` MPSC queues are the same API as SPSC; we use one MPSC queue (`engine_inbox`) that takes input from MIDI and tempo-source threads. The UI gets its own SPSC queue (§2.2) for clarity, but architecturally it could share the MPSC.

**Why:** consolidating inputs at the boundary means the engine has one place to drain events per block.

---

### 2.4 Atomic pointer swap for engine → UI snapshot

**The pattern:** §1.4 spelled out. Repeated here as the real-time rule: the engine publishes via `ArcSwap::store`, which is lock-free and non-blocking. The UI reads via `ArcSwap::load_full`, also lock-free.

**How we apply it:**

```rust
use arc_swap::ArcSwap;

// Shared:
let snapshot: Arc<ArcSwap<ParamSnapshot>> = Arc::new(ArcSwap::from_pointee(ParamSnapshot::default()));

// Engine (audio thread):
self.snapshot_slot.store(self.next_snapshot.take().unwrap());

// UI (any thread):
let s = self.snapshot_slot.load_full();
```

The engine **does not allocate** the snapshot during `store` — see §2.5.

---

### 2.5 Pre-allocated object pools (no alloc on the audio path)

**The pattern:** anything the audio thread needs to "create" is in fact taken from a pre-allocated pool. The pool is sized at `prepare()` time, before audio starts.

**How we apply it:**

- **Voice pool:** 32 voices, fixed array, allocated once. Voice "creation" is allocation of an index into the array.
- **Snapshot pool:** the engine keeps `N` (typically 4) pre-allocated `Arc<ParamSnapshot>` slots. Each block, it picks the next slot, writes into it, and stores its `Arc` clone in the `ArcSwap`. Because `ArcSwap` holds an `Arc`, the previous snapshot's `Arc` is dropped only when no reader holds it — but the *storage* for the snapshot is reused on the next cycle.
- **Event scratch buffers:** any per-block working memory (mod accumulators, voice mix buffers) is part of the engine struct, sized in `prepare()`.

**Why:** allocation is variable-time and may take a lock inside the allocator. Pre-allocating is the standard real-time pattern across audio software.

**The hard rule:** `Engine::prepare(sample_rate, max_block_size)` is called once when audio starts. Everything that might need memory allocates here. `Engine::process(buffer)` is allowed zero heap allocations.

---

### 2.6 Parameter smoothing on the audio thread

**The pattern:** continuous parameters do not jump instantaneously when the UI changes them. They smooth toward their target via a one-pole filter on the audio thread.

**How we apply it:**

```rust
struct SmoothedParam {
    target: f32,
    current: f32,
    coeff: f32,   // computed from time constant + sample rate at prepare()
}

impl SmoothedParam {
    fn set_target(&mut self, v: f32) { self.target = v; }

    fn next(&mut self) -> f32 {
        self.current += self.coeff * (self.target - self.current);
        self.current
    }
}
```

UI events set the target. The audio thread advances `current` toward `target` each sample (or each block, for cheaper params).

**Why:** parameter jumps cause clicks. Smoothing on the audio thread (not the UI thread) keeps the value coherent with the audio rate even if UI updates arrive at irregular times.

**Time constants:** typical 5–20 ms. Some params (filter cutoff under heavy modulation) want shorter; some (level fades) want longer.

---

### 2.7 Block-boundary latching for discrete parameters

**The pattern:** discrete (enum) parameter changes — oscillator waveform, filter mode, FM algorithm — take effect at the **start of a block**, never mid-block.

**How we apply it:** the engine drains the event queue once at the top of each callback, applies the changes, then runs the block. The block sees a single coherent set of discrete-parameter values.

**Why:** mid-block changes to discrete state cause discontinuities the smoother can't hide (e.g. waveform change from saw to square produces an instant amplitude jump). Latching at the boundary is cheaper than crossfading and is what users expect.

---

### 2.8 Panic on the audio thread = abort

**The pattern:** if the audio thread panics, the process aborts. We do **not** unwind, catch, log, and continue. Silent corruption of audio state is worse than a clear crash.

**How we apply it:** the audio callback is wrapped at the top:

```rust
fn audio_callback(buffer: &mut [f32]) {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        engine.process(buffer);
    }));
    if result.is_err() {
        // We log only because we are already aborting; no recovery is attempted.
        eprintln!("FATAL: audio thread panicked");
        std::process::abort();
    }
}
```

**Why:** a panicked audio thread leaves voices, filters, and effect tails in undefined states. Continuing is unsafe (in the correctness sense, not the Rust sense).

---

### 2.9 Telemetry from the audio thread

**The pattern:** the audio thread publishes telemetry (CPU%, level peaks, active voice count) via atomic stores into a small fixed struct. The UI polls it. No allocation, no locks.

**How we apply it:** a `Telemetry` struct made of `AtomicU32` / `AtomicF32` (via `atomic_float`) shared between threads:

```rust
struct Telemetry {
    cpu_load_pct:  AtomicF32,
    peak_left:     AtomicF32,
    peak_right:    AtomicF32,
    active_voices: AtomicU8,
}

// Engine (audio thread):
telemetry.cpu_load_pct.store(load, Ordering::Relaxed);
telemetry.peak_left.store(peak_l, Ordering::Relaxed);

// UI:
let load = telemetry.cpu_load_pct.load(Ordering::Relaxed);
```

`Relaxed` ordering is fine — telemetry doesn't need to be consistent with other state.

**Why:** atomic field updates are wait-free. No allocation, no locks, no chance of priority inversion.

---

### 2.10 Logging policy on the audio thread

**The pattern:** **no logging on the audio thread in release builds.** Tracing macros expand to no-ops via a feature flag.

**How we apply it:**

- In `synth-engine`, audio-path logging uses a project-specific `audio_trace!` macro that compiles to nothing unless the `audio-debug` feature is on.
- When `audio-debug` is on (developer builds only), `audio_trace!` writes into a lock-free ring buffer; another thread drains it and feeds the standard `tracing` subscriber.
- Release builds: zero logging cost on the audio path.

**Why:** standard `tracing` macros allocate (string formatting) and lock (subscriber state). Both are forbidden by §2.1.

---

### 2.11 No `async/await` on the audio thread

**The pattern:** the audio thread is synchronous. It never `.await`s anything.

**How we apply it:** there is no async runtime in the engine or host crates' audio paths. If a feature needs background async (e.g. a network update check), it lives in a separate thread in `synth-app`.

**Why:** futures hide allocations, locks, and unpredictable execution timing. None of those are acceptable on the audio path.

---

## CI enforcement

The patterns above are not aspirations. Each has a specific CI check:

| Rule | Check |
|---|---|
| §1.1 layering | `cargo metadata` script in `xtask` rejects forbidden dependencies. |
| §1.2 command pattern | Code review (no automated check; convention). |
| §1.3 single SoT | The parameter tree's mutators are crate-private; the compiler enforces this. |
| §2.1 no alloc | `assert_no_alloc`-wrapped tests around `Engine::process`. |
| §2.1 no lock | Audited via clippy + manual review (no general-purpose static checker; tests below catch regressions). |
| §2.5 pre-allocation | Test renders 10 seconds of audio, asserts allocation count over `Engine::process` is zero. |
| §2.8 panic policy | A test installs a custom panic handler and verifies the wrapper aborts. |
| §2.10 logging policy | A release-build test confirms `audio_trace!` expands to no code. |

If you find yourself wanting to break one of these rules to "just get something working", stop and write a plan-doc note instead — the rule is almost always right, and the few legitimate exceptions are worth designing rather than discovering.
