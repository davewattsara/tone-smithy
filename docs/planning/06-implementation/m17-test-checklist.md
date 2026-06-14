# M17 — Engine expansion: manual test checklist

Hands-on test plan for the four M17 features (second filter, 24 dB/oct slope, Env3,
16-slot mod matrix). Plan: [`m17-plan.md`](m17-plan.md).

Most of this is **audio testing on real hardware** — it can't be covered by the automated
suite. Tick each box as you verify it.

## Build & run

- [ ] `cargo run` launches the app (debug is fine for testing).
- [ ] For representative listening / lowest CPU: `cargo run --release`.

## 1. Mod matrix — 16 slots (was 8)

- [ ] **Modulation** tab shows **16 routing rows** (scroll if needed), not 8.
- [ ] Set a routing in **slot 16** (e.g. source LFO1 → dest Filter 1 cutoff, non-zero amount)
      and confirm it audibly modulates — proves slots 9–16 are live, not just drawn.

## 2. Env3 — second mod envelope

- [ ] **Envelopes** tab shows an **Env3** panel (A/D/S/R + curve knobs) alongside Env2.
- [ ] Matrix slot with **source = Env3**, dest = Filter 1 cutoff, long attack → playing a note
      produces a slow cutoff sweep.
- [ ] Env2 and Env3 move **independently**: changing Env3's attack does not alter Env2's sweep
      (and vice versa).
- [ ] Env3 values survive a preset round-trip (see section 5).

## 3. Second filter + serial/parallel routing

- [ ] **Filter** section shows **Filter 1** and **Filter 2** mode selectors and a **Routing**
      toggle (**Series** / **Parallel**).
- [ ] **Series:** F1 = LP ~500 Hz, F2 = HP ~200 Hz, Routing = Series → band-limited result
      (both shape the same chain).
- [ ] **Parallel:** same modes, Routing = Parallel → F1 LP and F2 HP are summed (fuller, both
      bands present). The Series-vs-Parallel difference is clearly audible.
- [ ] Route a matrix slot to **Filter 2 cutoff** and confirm F2 responds (F2 is mod-addressable).

## 4. 24 dB/oct slope

- [ ] Each filter has a **Slope** toggle: **12 dB** / **24 dB**.
- [ ] On a bright sawtooth at a moderate cutoff, switching F1 to **24 dB** sounds noticeably
      darker / steeper than 12 dB.
- [ ] High resonance + 24 dB + cutoff sweep does **not** run away into uncontrolled
      self-oscillation — for **Filter 1**.
- [ ] Same high-resonance 24 dB check passes for **Filter 2** independently.

## 5. Combined acceptance test (the M17 "done when")

Build one patch that exercises everything at once:

- [ ] **Env3** routed through the matrix to a filter cutoff.
- [ ] **Two filters in Series**, both set to **24 dB/oct**.
- [ ] **All 16 mod slots** populated with something.
- [ ] Patch plays, sounds sane, and nothing crashes or runs away.

## 6. Preset round-trip

- [ ] Save the combined patch (`.tsmith`).
- [ ] Load a factory/default patch to clear state.
- [ ] Reload the saved patch; confirm these all come back **identical**:
  - [ ] Filter 2 mode
  - [ ] Filter routing (Series/Parallel)
  - [ ] Both filter slopes (12/24 dB)
  - [ ] Env3 A/D/S/R + curve values
  - [ ] All 16 matrix slots
- [ ] An **old v1.0 preset** still opens cleanly (slots 9–16 empty/disabled, no errors).

## 7. Automated checks

- [ ] `cargo test --workspace` passes (includes preset round-trip serialization tests).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `cargo fmt --all --check` is clean.
