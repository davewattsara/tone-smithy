# M9 Test Checklist — Arpeggiator

Manual testing steps. All items must be ticked before M9 is merged to `main`.

## Smoke test

- [ ] App starts without crash
- [ ] Arpeggiator panel visible between mod matrix and FX chain
- [ ] All controls (mode, octaves, rate, BPM, gate, swing) greyed out when On is unchecked
- [ ] Controls become active when On is checked

## Basic operation

- [ ] Hold a single note with arp off — note sustains normally
- [ ] Enable arp, hold same note — note repeats at the set rate
- [ ] Release note — arp stops, no stuck notes
- [ ] Hold chord (3+ notes), arp enabled — steps through all held notes

## Modes

- [ ] **Up** — steps from lowest to highest note, then wraps back to lowest
- [ ] **Down** — steps from highest to lowest note, then wraps back to highest
- [ ] **Up/Down** — goes up to highest then back down, endpoints not repeated
- [ ] **Random** — notes play in unpredictable order (not always the same sequence)
- [ ] **Played** — notes play in the order the keys were pressed

## Octave range

- [ ] 1 oct — only the held notes, no transposition
- [ ] 2 oct — steps through held notes, then again one octave up
- [ ] 4 oct — full four-octave expansion audible on a held chord

## Rate

- [ ] 1/32 — very fast, machine-gun feel
- [ ] 1/8 — default, moderate speed
- [ ] 1/2 — slow, half-notes

## BPM knob

- [ ] Increasing BPM makes steps faster
- [ ] Decreasing BPM makes steps slower
- [ ] Change is smooth (no glitch or stuck note)

## Gate

- [ ] Gate at 0.01 — very short staccato notes
- [ ] Gate at 0.5 — notes half the step duration
- [ ] Gate at 1.0 — notes held for the full step (legato feel)

## Swing

- [ ] Swing at 0.5 — even, straight rhythm
- [ ] Swing at 0.75 — obvious shuffle/swing feel (odd steps noticeably shorter)

## Enable / disable mid-hold

- [ ] Hold chord, enable arp mid-hold — arp picks up the held notes and starts stepping
- [ ] Hold chord, arp running, disable arp — sounding arp note released immediately, no stuck note

## Mode switching mid-hold

- [ ] Switch mode while arp is running — changes take effect without crash or stuck note

## Regression checks

- [ ] Subtractive synth (no arp) still plays notes normally when arp is off
- [ ] FM synthesis works with arp enabled
- [ ] Mod matrix (e.g. LFO → filter cutoff) still functions while arp is running
- [ ] No audio dropout with arp running and all FX enabled simultaneously
