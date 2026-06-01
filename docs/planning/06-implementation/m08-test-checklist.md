# M8 Test Checklist — Effects Chain

Manual testing steps. All items must be ticked before M8 is merged to `main`.

## Smoke test — app launches

- [ ] App starts without crash
- [ ] FX panel visible below mod matrix
- [ ] All 5 effect columns present (EQ | Drive | Chorus | Delay | Reverb)
- [ ] All enabled checkboxes unchecked by default
- [ ] Knobs in each column are greyed out when checkbox is unchecked

## Enable / disable behaviour

- [ ] Checking an effect's checkbox enables its knobs immediately
- [ ] Unchecking disables knobs (greyed out) without audio click
- [ ] With no effect enabled, audio passes through unchanged (compare with all disabled vs. a sine/pad preset)

## 3-Band EQ

- [ ] Enable EQ, play a note — audio continues without change at 0 dB gain (flat response)
- [ ] Low shelf boost (+12 dB, 200 Hz) — audible bass increase on sustained note
- [ ] Low shelf cut (−12 dB, 200 Hz) — audible bass reduction
- [ ] Mid peak boost (+12 dB, 1 kHz) — audible nasal mid bump
- [ ] Mid peak cut (−12 dB, 1 kHz) — scooped mid sound
- [ ] High shelf boost (+12 dB, 6 kHz) — added brightness/air
- [ ] High shelf cut (−12 dB, 6 kHz) — darker/duller sound
- [ ] Disabling EQ while knobs are at non-zero values — audio bypasses immediately

## Drive

- [ ] Enable Drive, drive at minimum (~1) — effectively transparent
- [ ] Increase drive to maximum (~20) — clear harmonic saturation / clipping effect
- [ ] Asymmetry at 0 — symmetric clipping (even harmonics suppressed)
- [ ] Asymmetry at +1 or −1 — asymmetric clipping (audible even harmonic character)
- [ ] Output level stays roughly consistent as drive increases (compensation gain working)

## Chorus

- [ ] Enable Chorus — audible widening/shimmer on sustained note
- [ ] Rate knob: slow rate (0.1 Hz) — slow gentle sweep; fast rate (3+ Hz) — vibrato-like
- [ ] Depth knob at minimum — subtle; at maximum — obvious pitch variation
- [ ] Mix at 0 — dry signal (no chorus); mix at 1 — full wet
- [ ] Stereo spread: with spread at 0 — narrower; at 1 — wider stereo image
- [ ] Chorus introduces no obvious DC offset or runaway on long notes

## Delay

- [ ] Enable Delay — audible echo after note played
- [ ] Time knob: short (50 ms) — tight slapback; long (500 ms+) — clear distinct echo
- [ ] Feedback at 0 — single echo; feedback at ~0.8 — many repeats fading over time
- [ ] Mix at 0 — dry; mix at 1 — fully wet (only echoes, no dry signal)
- [ ] Low-cut knob: set low (50 Hz) — bass builds up in repeats; set high (1 kHz) — repeats sound thin
- [ ] Ping-pong toggle: off — echoes centred; on — echoes bounce left/right
- [ ] Feedback does not run away at maximum setting (0.95) — echoes gradually fade

## Reverb

- [ ] Enable Reverb — audible room/hall tail on note release
- [ ] Predelay knob: 0 ms — reverb starts immediately; 50 ms — small gap before tail
- [ ] Decay knob: 0.5 s — tight room; 5+ s — long hall
- [ ] Size knob: small — brighter, faster build; large — more diffuse, denser tail
- [ ] Damping knob: 0 — bright reverb tail; 1 — dark/muffled tail (HF absorbed)
- [ ] Mix at 0 — dry; mix at 1 — full wet
- [ ] Long decay (10 s+) — tail fades to silence, no infinite feedback or blow-up

## Effect chain order

- [ ] Enable EQ + Drive: drive comes after EQ (drive the EQ'd signal)
- [ ] Enable Delay + Reverb: delay echoes are fed into the reverb tail
- [ ] Enable all 5 simultaneously — no crash, no silent output, sound is audibly processed

## Regression checks

- [ ] Subtractive synth core (oscillators, filter, envelopes) still works with FX enabled
- [ ] FM operators still work with FX enabled
- [ ] Mod matrix routing (e.g. LFO → filter cutoff) still functions with FX enabled
- [ ] No audio dropout or xrun on default buffer size during FX stress (all 5 enabled + polyphonic chord)
