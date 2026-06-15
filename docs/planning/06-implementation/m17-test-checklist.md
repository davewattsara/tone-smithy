# M17 — Engine expansion: manual test checklist

Hands-on test plan for the four M17 features (second filter, 24 dB/oct slope, Env3,
16-slot mod matrix). Plan: [`m17-plan.md`](m17-plan.md).

Most of this is **audio testing on real hardware** — it can't be covered by the automated
suite. Tick each box as you verify it.

## Build & run

- [x] `cargo run` launches the app (debug is fine for testing).
- [x] For representative listening / lowest CPU: `cargo run --release`.

## 1. Mod matrix — 16 slots (was 8)

- [x] **Modulation** tab shows **16 routing rows** (scroll if needed), not 8.
- [x] Set a routing in **slot 16** (e.g. source LFO1 → dest `F1 Cut`, non-zero amount)
      and confirm it audibly modulates — proves slots 9–16 are live, not just drawn.

## 2. Env3 — second mod envelope

- [x] **Envelopes** tab shows an **Env3** panel (A/D/S/R + curve knobs) alongside Env2.
- [x] Matrix slot with **source = Env3**, dest = `F1 Cut`, long attack → playing a note
      produces a slow cutoff sweep.
- [x] Env2 and Env3 move **independently**: changing Env3's attack does not alter Env2's sweep
      (and vice versa).
- [x] Env3 values survive a preset round-trip (see section 5).

## 3. Second filter + serial/parallel routing

- [x] **Filter** section shows **Filter 1** and **Filter 2** mode selectors and a **Routing**
      toggle (**Series** / **Parallel**).
- [x] **Series:** F1 = LP ~500 Hz, F2 = HP ~200 Hz, Routing = Series → band-limited result
      (both shape the same chain).
- [x] **Parallel:** same modes, Routing = Parallel → F1 LP and F2 HP are summed (fuller, both
      bands present). The Series-vs-Parallel difference is clearly audible.
- [x] Route a matrix slot to **`F2 Cut`** and confirm F2 responds (F2 is mod-addressable).

## 4. 24 dB/oct slope

- [x] Each filter has a **Slope** toggle: **12 dB** / **24 dB**.
- [x] Build a patch that exposes the slope: **sawtooth** osc, **F1 cutoff ~300 Hz** (low-mids,
      so most harmonics sit *above* the corner), **low resonance (~0.2-0.3)**, playing a **low
      sustained note (C2-C3)**. Hold the note and toggle F1 **12 <-> 24 dB**: 24 dB is distinctly
      **darker / tighter**, 12 dB **buzzier / more open** up top. (The difference is ~4x more
      attenuation one octave above cutoff; with the filter wide open the two slopes sound nearly
      identical, so keep the cutoff low for this test.) A slow cutoff sweep makes it clearest.
- [x] High resonance + 24 dB + cutoff sweep does **not** run away into uncontrolled
      self-oscillation — for **Filter 1**.
- [x] Same high-resonance 24 dB check passes for **Filter 2** independently.

## 5. Combined acceptance test (the M17 "done when")

Build one patch that exercises everything at once:

- [x] **Env3** routed through the matrix to a filter cutoff.
- [x] **Two filters in Series**, both set to **24 dB/oct**.
- [x] **All 16 mod slots** populated with something.
- [x] Patch plays, sounds sane, and nothing crashes or runs away.

## 6. Preset round-trip

- [x] Save the combined patch (`.tsmith`).
- [x] Load a factory/default patch to clear state.
- [x] Reload the saved patch; confirm these all come back **identical**:
  - [x] Filter 2 mode
  - [x] Filter routing (Series/Parallel)
  - [x] Both filter slopes (12/24 dB)
  - [x] Env3 A/D/S/R + curve values
  - [x] All 16 matrix slots
- [x] An **old v1.0 preset** still opens cleanly (slots 9–16 empty/disabled, no errors).

## 7. Automated checks

- [x] `cargo test --workspace` passes (includes preset round-trip serialization tests).
- [x] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [x] `cargo fmt --all --check` is clean.
