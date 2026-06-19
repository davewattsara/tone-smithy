# M22 — Engine additions: manual test checklist

Hands-on test plan for M22. Plan: [`m22-plan.md`](m22-plan.md).

This is **audio testing on real hardware** — it can't be covered by the automated suite.
Tick each box as you verify it.

> Scope: this file currently covers **Phase 2 — editable FM operator routing**. Phase 1
> (second sequencer mod lane, `Seq2`) still needs its own section added before close-out.

## Orientation

- UI operator labels are **1-based**: grid "Op 1 / 2 / 3 / 4" map to engine ops **0 / 1 / 2 / 3**.
- Routing rule: **higher-numbered ops modulate lower-numbered ops.** Op 4 is a pure modulator
  (top of the chain), Op 1 only receives. Op 4's self-feedback is the **FB knob**, not a grid
  checkbox.
- All FM lives in the **FM slot** — the slot showing the operator grid and the `Alg` dropdown.
- Suggested starting patch: **Keys -> "DX Piano"** — FM-only, on factory algorithm **8 Paired**,
  with all four operator levels up, so every connection is audible.

## Build & run

- [ ] `cargo run --bin tonesmithy` launches the app (debug is fine for testing).
- [ ] For representative listening / lowest CPU: `cargo run --release --bin tonesmithy`.
- [ ] Load **Keys -> DX Piano** and confirm it plays.

## Phase 2 — editable FM operator routing

### 1. Regression — factory algorithms 0-7 unaffected

- [ ] With DX Piano playing, cycle `Alg` through **1 Stack -> 8 Paired**; each changes the
      timbre, none is silent, crackles, or hangs.
- [ ] Return to **8 Paired** -> sounds like the original DX Piano.

### 2. Seed continuity — factory -> Custom is seamless

- [ ] With DX Piano on **8 Paired**, switch `Alg` to **9 Custom** -> the sound **does not
      change** at the moment of switching.
- [ ] The "Custom routing" grid appears below the selector.

### 3. Grid matches the seeded algorithm

With **8 Paired** seeded, the grid reads exactly:

- [ ] **Op 1**: Carrier **on**; "mod by" **Op4** checked (Op2, Op3 off).
- [ ] **Op 2**: Carrier **on**; "mod by" **Op3** checked (Op4 off).
- [ ] **Op 3**: Carrier off; "mod by" Op4 off.
- [ ] **Op 4**: Carrier off; shows "(feedback via FB knob)".

### 4. Live editing is audible

While holding a note in Custom:

- [ ] Uncheck **Op 2 -> mod by Op3** -> the glassy/bell layer thins out; re-check -> it returns.
- [ ] Uncheck **Op 1 Carrier** -> that operator drops out of the mix; re-check -> it returns.

### 5. Minimal one-carrier one-modulator patch

In the grid, uncheck **all** carriers and **all** "mod by" boxes (silence), then:

- [ ] Check **Op 1 Carrier** only -> a clean sine plays. (Ensure Op 1's **Level** is up.)
- [ ] Check **Op 1 -> mod by Op2** (ensure Op 2's **Level** is up) -> the tone gets
      brighter/buzzier.

### 6. Equivalence to factory "1 Stack"

In Custom, build the full stack by hand: **only Op 1 Carrier on**, then chain
**Op 1 mod-by Op2**, **Op 2 mod-by Op3**, **Op 3 mod-by Op4** (all other boxes off).

- [ ] A/B against factory by toggling `Alg` between **1 Stack** and **9 Custom** -> **identical
      sound**.

### 7. No-carrier = silence

- [ ] In Custom, uncheck every Carrier box -> playing a note is **silent** (confirms the carrier
      flag is honoured).

### 8. Preset round-trip

- [ ] Build a recognisable Custom patch and **save** it as a new preset.
- [ ] Load a different preset, then reload yours -> grid checkboxes restored exactly and it
      sounds the same.

### 9. Old-preset safety

- [ ] Load any pre-M22 preset (e.g. a v1.0/v1.1 non-FM patch) -> unchanged behaviour, no Custom
      artifacts, `Alg` shows its original factory algorithm (never "9 Custom").

## Automated / mechanical checks (run by agent)

- [ ] `cargo fmt --all --check` clean.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo test --workspace` passes.
