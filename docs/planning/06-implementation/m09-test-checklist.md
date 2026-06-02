# M9 Test Checklist — Arpeggiator

Manual testing steps. All items must be ticked before M9 is merged to `main`.

## Smoke test

- [x] App starts without crash
- [x] Arpeggiator panel visible between mod matrix and FX chain
- [x] All controls (mode, octaves, rate, BPM, gate, swing) greyed out when On is unchecked
- [x] Controls become active when On is checked

## Basic operation

- [x] Hold a single note with arp off — note sustains normally
- [x] Enable arp, hold same note — note repeats at the set rate
- [x] Release note — arp stops, no stuck notes
- [x] Hold chord (3+ notes), arp enabled — steps through all held notes

## Modes

- [x] **Up** — steps from lowest to highest note, then wraps back to lowest
- [x] **Down** — steps from highest to lowest note, then wraps back to highest
- [x] **Up/Down** — goes up to highest then back down, endpoints not repeated
- [x] **Random** — notes play in unpredictable order (not always the same sequence)
- [x] **Played** — notes play in the order the keys were pressed

## Octave range

- [x] 1 oct — only the held notes, no transposition
- [x] 2 oct — steps through held notes, then again one octave up
- [x] 4 oct — full four-octave expansion audible on a held chord

## Rate

- [x] 1/32 — very fast, machine-gun feel
- [x] 1/8 — default, moderate speed
- [x] 1/2 — slow, half-notes

## BPM knob

- [x] Increasing BPM makes steps faster
- [x] Decreasing BPM makes steps slower
- [x] Change is smooth (no glitch or stuck note)

## Gate

- [x] Gate at 0.01 — very short staccato notes
- [x] Gate at 0.5 — notes half the step duration
- [x] Gate at 1.0 — notes held for the full step (legato feel)

## Swing

- [x] Swing at 0.5 — even, straight rhythm
- [x] Swing at 0.75 — obvious shuffle/swing feel (odd steps noticeably shorter)

## Enable / disable mid-hold

- [x] Hold chord, enable arp mid-hold — arp picks up the held notes and starts stepping
- [x] Hold chord, arp running, disable arp — sounding arp note released immediately, no stuck note

## Mode switching mid-hold

- [x] Switch mode while arp is running — changes take effect without crash or stuck note

## Regression checks

- [x] Subtractive synth (no arp) still plays notes normally when arp is off
- [x] FM synthesis works with arp enabled
- [x] Mod matrix (e.g. LFO → filter cutoff) still functions while arp is running
- [x] No audio dropout with arp running and all FX enabled simultaneously
