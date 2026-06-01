# M8 Test Checklist — Effects Chain

Manual testing steps. All items must be ticked before M8 is merged to `main`.

## Smoke test — app launches

- [x] App starts without crash
- [x] FX panel visible below mod matrix
- [x] All 5 effect columns present (EQ | Drive | Chorus | Delay | Reverb)
- [x] All enabled checkboxes unchecked by default
- [x] Knobs in each column are greyed out when checkbox is unchecked

## Enable / disable behaviour

- [x] Checking an effect's checkbox enables its knobs immediately
- [x] Unchecking disables knobs (greyed out) without audio click
- [x] With no effect enabled, audio passes through unchanged (compare with all disabled vs. a sine/pad preset)

## 3-Band EQ

- [x] Enable EQ, play a note — audio continues without change at 0 dB gain (flat response)
- [x] Low shelf boost (+12 dB, 200 Hz) — audible bass increase on sustained note
- [x] Low shelf cut (−12 dB, 200 Hz) — audible bass reduction
- [x] Mid peak boost (+12 dB, 1 kHz) — audible nasal mid bump
- [x] Mid peak cut (−12 dB, 1 kHz) — scooped mid sound
- [x] High shelf boost (+12 dB, 6 kHz) — added brightness/air
- [x] High shelf cut (−12 dB, 6 kHz) — darker/duller sound
- [x] Disabling EQ while knobs are at non-zero values — audio bypasses immediately

## Drive

- [x] Enable Drive, drive at minimum (~1) — effectively transparent
- [x] Increase drive to maximum (~20) — clear harmonic saturation / clipping effect
- [x] Asymmetry at 0 — symmetric clipping (even harmonics suppressed)
- [x] Asymmetry at +1 or −1 — asymmetric clipping (audible even harmonic character)
- [x] Output level stays roughly consistent as drive increases (compensation gain working)

## Chorus

- [x] Enable Chorus — audible widening/shimmer on sustained note
- [x] Rate knob: slow rate (0.1 Hz) — slow gentle sweep; fast rate (3+ Hz) — vibrato-like
- [x] Depth knob at minimum — subtle; at maximum — obvious pitch variation
- [x] Mix at 0 — dry signal (no chorus); mix at 1 — full wet
- [x] Stereo spread: with spread at 0 — narrower; at 1 — wider stereo image
- [x] Chorus introduces no obvious DC offset or runaway on long notes

## Delay

- [x] Enable Delay — audible echo after note played
- [x] Time knob: short (50 ms) — tight slapback; long (500 ms+) — clear distinct echo
- [x] Feedback at 0 — single echo; feedback at ~0.8 — many repeats fading over time
- [x] Mix at 0 — dry; mix at 1 — fully wet (only echoes, no dry signal)
- [x] Low-cut knob: set low (50 Hz) — bass builds up in repeats; set high (1 kHz) — repeats sound thin
- [x] Ping-pong toggle: off — echoes centred; on — echoes bounce left/right
- [x] Feedback does not run away at maximum setting (0.95) — echoes gradually fade

## Reverb

- [x] Enable Reverb — audible room/hall tail on note release
- [x] Predelay knob: 0 ms — reverb starts immediately; 50 ms — small gap before tail
- [x] Decay knob: 0.5 s — tight room; 5+ s — long hall
- [x] Size knob: small — brighter, faster build; large — more diffuse, denser tail
- [x] Damping knob: 0 — bright reverb tail; 1 — dark/muffled tail (HF absorbed)
- [x] Mix at 0 — dry; mix at 1 — full wet
- [x] Long decay (10 s+) — tail fades to silence, no infinite feedback or blow-up

## Effect chain order

- [x] Enable EQ + Drive: drive comes after EQ (drive the EQ'd signal)
- [x] Enable Delay + Reverb: delay echoes are fed into the reverb tail
- [x] Enable all 5 simultaneously — no crash, no silent output, sound is audibly processed

## Regression checks

- [x] Subtractive synth core (oscillators, filter, envelopes) still works with FX enabled
- [x] FM operators still work with FX enabled
- [x] Mod matrix routing (e.g. LFO → filter cutoff) still functions with FX enabled
- [x] No audio dropout or xrun on default buffer size during FX stress (all 5 enabled + polyphonic chord)
