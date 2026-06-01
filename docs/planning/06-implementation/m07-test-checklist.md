# M7 manual test checklist

Run these before closing out M7 and merging to main.

Operator numbering in this document matches the UI: **Op 1** through
**Op 4** (1-indexed). Internally the code uses 0-indexed indices, so
UI "Op 4" = code op index 3, etc.

Each section is self-contained. Restart the synth (or kill and relaunch)
before any section you want to run in isolation.

---

## Default state reference

After a restart, the synth is in this state:

- Slot 0: Subtractive, level 1.00, pan C
- Slot 1: Subtractive, level 0.00 (silent)
- Waveform: Saw; Filter: LP, cutoff 8.0 kHz, resonance 0.00
- Amp env: A=10 ms, D=200 ms, S=0.80, R=200 ms
- LFOs: rate 1.00 Hz, Sine shape, sync off
- Env2: A=10 ms, D=200 ms, S=0.80, R=200 ms
- Mod matrix: all 8 slots disabled
- FM operators (both slots): all ratio=1, fine=0 ct, level=1.00,
  A=10 ms, D=200 ms, S=0.80, R=200 ms, feedback=0.00

---

## 1. Slot mode switching

**Starting state:** defaults.

- [ ] Open the FM panel. Both slots show **Subtractive** selected. Slot 0
  level knob reads ~1.00; slot 1 level reads 0.00.
- [ ] Hold a note — you hear the standard saw. Release.
- [ ] On **Slot 1**, click **FM**. Hold a note — still hear only slot 0
  saw (slot 1 level is 0.00). No crash.
- [ ] Raise **Slot 1 level** to ~0.50. Hold a note — you hear a mix of
  saw (slot 0) and the default FM tone (slot 1, algorithm 1, all
  operators at ratio 1). The FM tone sounds like a bright sine. Release.
- [ ] Click **Subtractive** again on slot 1. Hold a note — the FM
  contribution disappears; only the saw remains. Release.
- [ ] Switch **Slot 0** to **FM** and raise its level if needed.
  The saw goes silent; you hear FM from slot 0. Switch back to
  Subtractive. Saw returns. Release.

---

## 2. Slot level and pan

**Starting state:** defaults, then set Slot 1 to FM, level 0.50.

- [ ] Hold a note. Move the **Slot 1 Pan** knob left — FM sound pans to
  the left channel. Right — pans right. C — balanced. Release.
- [ ] Move **Slot 0 Pan** right. Hold a note. Saw comes from the right,
  FM from the left (or wherever you set each). A hybrid stereo image.
  Return both pans to C. Release.
- [ ] Set slot 1 level to 0.00 — FM goes silent mid-note. Set back to 0.50.
  FM returns. (No clicks expected here since amp envelope gates the
  output — just a volume change.)

---

## 3. All 8 FM algorithms on Slot 1

**Starting state:** defaults, then:
- Slot 0 level = 0.00 (silent)
- Slot 1 = FM, level = 1.00
- All operator ADSR: A=50 ms, D=500 ms, S=0.80, R=300 ms
- All operator levels = 1.00

**Note:** with every operator at ratio 1, algorithms that combine
multiple carriers modulated by the same source will sound identical
in timbre (just different volume). Each test below uses operator
ratios that reveal the algorithm's unique character.

For each algorithm, hold a note for 1–2 seconds, then release.

- [ ] **Algorithm 1** (Op 4→Op 3→Op 2→Op 1 stack, Op 1 is the carrier):
  Ratios: Op 1=1, Op 2=1, Op 3=1, Op 4=4.
  Classic FM stack with a 4× modulator. Should sound bright and
  bell-like. Only Op 1 contributes to the audio output.

- [ ] **Algorithm 2** (same stack + Op 4 self-feedback):
  Ratios: Op 1=1, Op 2=1, Op 3=1, Op 4=4. Op 4 Feedback = 0.70.
  Should sound richer and rougher than Alg 1 — set the **Op 4
  Feedback** knob to 0.70 and notice the extra harmonic content
  compared to Alg 1 at the same feedback setting (which has none).

- [ ] **Algorithm 3** (two stacks: Op 4→Op 3 and Op 2→Op 1, mixed):
  Ratios: Op 1=1, Op 2=3, Op 3=1, Op 4=5.
  The two stacks use different modulator ratios (3× and 5×), so
  they produce two distinct FM timbres layered together. You should
  hear a richer, more complex sound than either stack alone.

- [ ] **Algorithm 4** (Op 4 modulates Op 1, Op 2, Op 3 in parallel):
  Ratios: Op 1=1, Op 2=2, Op 3=3, Op 4=2.
  Three carriers at different harmonics (1×, 2×, 3×), all driven
  by the same modulator. Should sound organ-like or additive, with
  three distinct pitch layers all responding to the same modulation.

- [ ] **Algorithm 5** (Op 4 modulates Op 2+Op 3; Op 3 modulates Op 1):
  Ratios: Op 1=1, Op 2=1, Op 3=2, Op 4=3.
  Branching modulator. Op 3 acts as both a carrier-modulator (shaping
  Op 1) and is itself modulated by Op 4. Should produce a complex,
  evolving timbre that changes markedly as Op 4's level varies.

- [ ] **Algorithm 6** (Op 3+Op 2 modulate Op 1; Op 4 separate carrier):
  Ratios: Op 1=1, Op 2=3, Op 3=5, Op 4=2.
  Op 1 and Op 4 are both carriers at different pitches (1× and 2×).
  Should sound like two layers: a harmonically rich FM tone (Op 1
  modulated by Op 3 and Op 2) plus a cleaner tone from Op 4.

- [ ] **Algorithm 7** (all four parallel, additive):
  Ratios: Op 1=1, Op 2=2, Op 3=3, Op 4=4.
  No modulation — pure additive synthesis of four sine harmonics.
  Should sound clear and flute-like, noticeably cleaner than any
  FM algorithm.

- [ ] **Algorithm 8** (Op 4→Op 1; Op 3→Op 2; Op 1 and Op 2 are carriers):
  Ratios: Op 1=1, Op 2=1, Op 3=2, Op 4=5.
  Two independent modulator–carrier pairs at different modulator ratios
  (5× and 2×). Should sound like two distinct FM tones mixed.

**Pass criterion**: each algorithm produces a recognisably different
timbre; none produce silence, DC offset, or distorted/broken audio.

---

## 4. Operator ratio

**Starting state:** defaults, then:
- Slot 0 level = 0.00 (silent)
- Slot 1 = FM, Algorithm 1 (Op 4→Op 3→Op 2→Op 1), level = 1.00
- All operator levels = 1.00, all ratios = 1

- [ ] Set **Op 4 Ratio Integer** to 1. Hold a note — baseline timbre.
  Increase to 2, 4, 8. The timbre gets progressively brighter and more
  complex as the modulation index ratio increases.
- [ ] Set all operators to **Ratio Integer = 1**, then set **Op 1**
  (carrier) to 2. Hold a note. The output pitch should be one octave
  higher than the note played (carrier runs at 2× the note frequency).
- [ ] Set **Op 4 Ratio Fine** to +100 ct. Hold a note — the timbre
  becomes "detuned" or slightly inharmonic compared to fine = 0 ct.
  Set to -100 ct for the opposite detuning. Return to 0 ct.

---

## 5. Operator level

**Starting state:** defaults, then:
- Slot 0 level = 0.00 (silent)
- Slot 1 = FM, Algorithm 1, level = 1.00
- All operator ratios = 1, all operator levels = 1.00

- [ ] Set **Op 4 Level** (top modulator in the stack) to 0.00. Hold a note.
  With Op 4 silent, its modulation contribution to Op 3 is zero, so the
  stack simplifies to Op 3→Op 2→Op 1 with the top link broken. Timbre
  becomes simpler/cleaner.
- [ ] Set Op 4 Level back to 1.00. Set **Op 3 Level** to 0.00. Now Op 3
  contributes no modulation to Op 2, so the remaining chain is just
  Op 2 → Op 1 — effectively a simple FM pair.
- [ ] Set all levels to 0.00 except **Op 1 Level** = 1.00. Algorithm 1 only
  has Op 1 as carrier. With no modulators active, you should hear a pure
  sine at the note frequency.

---

## 6. Operator ADSR

**Starting state:** defaults, then:
- Slot 0 level = 0.00 (silent)
- Slot 1 = FM, Algorithm 1, level = 1.00
- Op 1 level = 1.00, Op 4 level = 1.00, Op 2 and Op 3 levels = 0.50
- All operator ratios = 1, all ADSR at defaults (A=10 ms, D=200 ms, S=0.80, R=200 ms)

- [ ] Set **Op 4 Attack** to 2.00 s. Hold a note. The timbre should start
  clean (little modulation) and gradually become brighter over 2 seconds
  as Op 4's envelope ramps up. Release — the modulation dies.
- [ ] Set **Op 4 Decay** to 100 ms, **Op 4 Sustain** = 0.00. Hold a note.
  Op 4 fires and decays quickly — a sharp transient "click" or bright
  attack followed by a simpler sustained tone as the modulator envelope
  falls to 0.00. Classic DX7-style bell.
- [ ] Set **Op 1 Attack** to 1.00 s (the carrier's own envelope). Hold a
  note. The output fades in slowly even though the modulator is active.
  Confirms carrier amplitude is independently enveloped.

---

## 7. Operator feedback (Op 4)

**Starting state:** defaults, then:
- Slot 0 level = 0.00 (silent)
- Slot 1 = FM, Algorithm 2, level = 1.00
- All operator levels = 1.00, all ratios = 1, Op 4 Feedback = 0.00

- [x] Hold a note — clean FM tone with no feedback.
- [x] Slowly increase **Op 4 Feedback** toward 1.00. The timbre should
  become progressively noisier and more distorted as the feedback
  oscillates more aggressively. At 1.00 it should be very rich/noisy
  (near-static with a faint tonal residue) but should NOT produce
  clicks, infinite values, or silence.
- [x] Set to -1.00. Should sound similar to 1.00 (the sign of feedback
  affects phase but not spectral density at high amounts).
- [x] Set back to 0.00. Clean tone returns.

---

## 8. Hybrid patch (canonical M7 test)

**Starting state:** defaults, then apply:
- Slot 0: Subtractive, level = 1.00, pan = L25
- Slot 1: FM, level = 0.70, pan = R25
- Slot 1 algorithm: **1** (clean FM bell stack)
- Slot 1 operator settings: all ratio = 1, Op 4 level = 0.80, Op 1
  level = 1.00, Op 2 and Op 3 at 0.50.
  Op 4 envelope: A=10 ms, D=300 ms, S=0.00, R=100 ms (fast transient modulator).
  Op 1 envelope: A=10 ms, D=1.00 s, S=0.60, R=500 ms.
- Voice amp envelope: A=10 ms, D=500 ms, S=0.70, R=300 ms.

- [ ] Hold a note. You should hear a warm saw (slot 0, slightly left)
  blended with a bell-like FM tone (slot 1, slightly right). Both
  layers should be clearly audible and stereo-imaged.
- [ ] Play a scale across 2+ octaves. Both layers track pitch correctly.
- [ ] Play several notes in quick succession (short staccato). Both
  layers start and stop cleanly with no hanging notes.
- [ ] While holding a note, move **Slot 0 Pan** to L100 (full left) and
  **Slot 1 Pan** to R100 (full right). The saw and FM bell should be
  clearly separated in the stereo field.
- [ ] Set **Slot 1 Level** to 0.00. Only the saw remains. Set to 1.00. FM
  bell returns. Both changes take effect immediately.

---

## 9. Anti-aliasing sanity

**Starting state:** defaults, then:
- Slot 0 level = 0.00 (silent)
- Slot 1 = FM, Algorithm 2, level = 1.00
- Op 4: Ratio Integer = 8, Level = 1.00, Feedback = 0.70
- Op 1: Level = 1.00 (only carrier). Op 2 and Op 3 at level 0.50.

- [ ] Hold a high note (C5 or above). The output should sound dense and
  harmonically complex but should NOT include obvious "digital" noise
  or screeching tonal artefacts unrelated to the note pitch.
- [ ] Play the same patch at C2 (low) then C5 (high). Timbre should get
  brighter as pitch rises (expected FM behaviour), not produce random
  hash or aliasing noise that changes character with pitch in an
  unmusical way.

**Pass criterion**: subjectively clean FM tone even at high modulation
index and high pitch. Perfect attenuation is not required — the
half-band filter provides ~-44 dB which should be inaudible in a mix.

---

## 10. 32-voice stress test

**Starting state:** defaults, then set Slot 1 to FM, level = 1.00
(so the stress test exercises the FM engine, not just the subtractive path).

- [ ] Set sustain ON. Rapidly play 32 different notes (use the virtual
  keyboard to tap across its full range). All 32 should sound. Check
  the footer **voice count** reads 32.
- [ ] Watch the **CPU %** in the footer. With 32 voices and FM active
  on slot 1, it should stay well below 50%. If it exceeds 50%, note
  the value here: `______%`.
- [ ] Release sustain. All voices fade and the count drops to 0.
- [ ] No audio dropout, no click, no crash during the above.

---

## 11. Mod matrix with FM slot levels

**Starting state:** defaults, then set Slot 1 to FM, level = 1.00
(FM must be active for this test to be meaningful).

This is a rough sanity check only — full FM modulation targets are M11.

- [ ] Enable mod matrix Slot 1: Source = LFO1, Dest = **Vol** (master
  volume), Amount = 0.30. Hold a note. The overall volume should
  tremolo at LFO rate. (This uses the existing Vol destination; it
  affects the whole voice, not per-slot.)
- [ ] Change to Source = Env2, Dest = **Cutoff**, Amount = 5000. Hold a
  note. Filter sweeps as before — FM slot does not interfere with the
  filter modulation path. Confirms FM is upstream of the filter, not
  bypassing it.

---

## 12. Regression — features present before M7

**Starting state:** defaults (both slots Subtractive).

- [ ] With **both slots set to Subtractive**, the synth sounds identical
  to pre-M7 (warm saw, filter, mod matrix all work as before).
- [ ] Filter Cutoff and Resonance knobs respond. LFO and Env2 panels
  show live readouts.
- [ ] Virtual keyboard, computer keyboard, pitch bend, mod wheel, and
  sustain all work normally.
- [ ] Scrolling the main window still reaches the virtual keyboard at
  the bottom.
- [ ] Footer CPU% and voice count update in real time.
- [ ] No audio dropout or crash during normal play after several minutes.
