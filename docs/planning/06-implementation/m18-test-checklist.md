# M18 — Step sequencer (+ bundled additions): manual test checklist

Hands-on test plan for M18: the 16-step sequencer, the unified transport BPM, the global
(mono) LFO mode, and the OSC2/OSC3 detune mod destinations. Plan: [`m18-plan.md`](m18-plan.md).

Most of this is **audio testing on real hardware** — it can't be covered by the automated
suite. Tick each box as you verify it.

## Build & run

- [x] `cargo run` launches the app (debug is fine for testing).
- [ ] For representative listening / lowest CPU: `cargo run --release`.

## 1. Unified transport BPM

- [x] The **Master** tab still has a single **BPM** knob; the **Arp** tab no longer has its own
      BPM knob.
- [x] Enable the **arp**, hold a chord, and change the Master **BPM** — the arp speed follows.
- [x] With an **LFO sync**-enabled patch, changing Master **BPM** retempos the LFO too (same one
      knob drives both).
- [ ] After Phase 2 is wired, the **sequencer** also follows the same Master BPM (verified in §2).

## 2. Step sequencer — core + UI

- [x] A new **Seq** tab sits between **Arp** and **FX** in the tab bar.
- [x] **Enabled** toggle starts the sequencer; hold a key and a 16-step line plays.
- [x] **Step note offsets** transpose correctly: the line follows the **lowest held note**
      (hold a different key → the whole sequence transposes).
- [x] **Per-step velocity** is audible (set some steps loud, some quiet).
- [ ] **Per-step gate** changes note length (short vs sustained steps).
- [x] **Rest** steps are silent and still consume their time slot.
- [ ] **Tie** (the **T** toggle): a tie step holds the previous note across its slot instead of
      retriggering — set step 1 to a note and step 2 to a tie and the note rings through both with
      no re-attack. Chained ties ring continuously; the last tie's gate sets where it ends. A
      leading tie (nothing held) is silent.
- [ ] **Length** (1–16) shortens the active pattern; steps beyond the length are dimmed/disabled.
- [ ] **Rate** (1/32…1/2) changes step speed; **Swing** shuffles the timing.
- [ ] **Playback modes** all walk correctly: **Fwd**, **Rev**, **Ping** (no doubled endpoints),
      **Rand**.
- [ ] The **playhead** highlight sweeps the active step in time with playback.
- [ ] **Mutual exclusion:** enabling the **Seq** turns the **Arp** off (its toggle clears), and
      enabling the **Arp** turns the Seq off — no stuck notes on either switch.
- [ ] Releasing all keys silences the sequencer; **panic / all-notes-off** stops it cleanly.

## 3. Sequencer mod lane

- [ ] The matrix source dropdown (**Modulation** tab) now lists **Seq** (shown last).
- [ ] Set each step's **mod** slider to different values, route a matrix slot **source = Seq →
      dest = `F1 Cut`** (non-zero amount), and confirm the cutoff **steps through the lane values**
      in time with the sequence.
- [ ] Routing **Seq** to a different dest (e.g. `Pitch` or `Vol`) also tracks the per-step lane.

## 4. Global (mono) LFO mode

- [ ] Each LFO panel (**Envelopes** tab) has a **Global** toggle next to **Reset**.
- [ ] Route an LFO to an audible dest (e.g. `F1 Cut`), set a moderate rate, and **hold a chord**:
  - [ ] **Per-voice (default):** with **Reset** on, re-pressing notes at different times makes the
        voices' LFOs drift out of phase (shimmer).
  - [ ] **Global on:** all held voices lock to **one shared phase** (chord moves as a block).
- [ ] **Reset** greys out (disabled) while **Global** is active.
- [ ] Toggling Global off returns to per-voice behaviour seamlessly.

## 5. OSC2 / OSC3 detune mod destinations

- [ ] The matrix dest dropdown lists **Osc2Det** and **Osc3Det** (appended after `F2 Res`).
- [ ] With OSC1/OSC2/OSC3 all active, route an LFO → **`Osc2Det`**: only **OSC2** detunes
      (audible beating/width), OSC1 and OSC3 stay put.
- [ ] Route a second slot → **`Osc3Det`** and confirm OSC3 moves independently of OSC2 — an
      evolving chorus/width the global `Pitch` dest can't produce.

## 6. Combined acceptance test (the M18 "done when")

Build one patch that exercises everything at once:

- [ ] A **16-step melodic line** plays with independent velocity and gate per step.
- [ ] The **Seq mod lane** drives a destination audibly.
- [ ] One **LFO in Global mode** phase-locks a held chord.
- [ ] An **LFO → `Osc2Det`** detunes only OSC2.
- [ ] The single **Master BPM** retempos the sequencer and the LFO sync together.
- [ ] Patch plays, sounds sane, and nothing crashes or runs away.

## 7. Preset round-trip & back-compat

- [ ] Save the combined patch (`.tsmith`).
- [ ] Load a factory/default patch to clear state.
- [ ] Reload the saved patch; confirm these all come back **identical**:
  - [ ] Sequencer enabled, length, mode, rate, swing
  - [ ] All 16 steps (note offset, velocity, gate, rest, tie, mod value)
  - [ ] Both LFO **Global** flags
  - [ ] The `Osc2Det` / `Osc3Det` matrix routings
- [ ] An **old v1.0 / v1.1 preset** still opens cleanly and **sounds identical** — sequencer
      defaults to **off**, LFOs default to **per-voice**, and existing matrix dest indices are
      unchanged (no migration).

## 8. Automated checks

- [ ] `cargo test --workspace` passes (includes the seq engine, global-LFO phase-lock,
      mod-table guard, and preset round-trip tests).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- [ ] `cargo fmt --all --check` is clean.
