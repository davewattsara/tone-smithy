# M7 manual test checklist

Run these before closing out M7 and merging to main.

Operator numbering in this document matches the UI: **Op 1** through
**Op 4** (1-indexed). Internally the code uses 0-indexed indices, so
UI "Op 4" = code op index 3, etc.

---

## Setup: load a clean state

Before starting, restart the synth (or kill and relaunch) so all
parameters are at their defaults. Default state: slot 0 = Subtractive
(saw, unison), slot 1 = Subtractive at level 0 (silent). Filter cutoff
8000 Hz, amp envelope with short attack, no mod slots active.

---

## 1. Slot mode switching

- [ ] Open the FM panel. Both slots show **Subtractive** selected. Slot 0
  level knob reads ~1.0; slot 1 level reads 0.0.
- [ ] Hold a note — you hear the standard saw. Release.
- [ ] On **Slot 1**, click **FM**. Hold a note — still hear only slot 0
  saw (slot 1 level is 0). No crash.
- [ ] Raise **Slot 1 level** to ~0.5. Hold a note — you hear a mix of
  saw (slot 0) and the default FM tone (slot 1, algorithm 1, all
  operators at ratio 1). The FM tone sounds like a bright sine. Release.
- [ ] Click **Subtractive** again on slot 1. Hold a note — the FM
  contribution disappears; only the saw remains. Release.
- [ ] Switch **Slot 0** to **FM** and raise its level if needed.
  The saw goes silent; you hear FM from slot 0. Switch back to
  Subtractive. Saw returns. Release.

---

## 2. Slot level and pan

- [ ] Set slot 1 to FM, level 0.5. Hold a note. Move the **Slot 1 Pan**
  knob left — FM sound pans to the left channel. Right — pans right.
  Centre — balanced. Release.
- [ ] Move **Slot 0 Pan** right. Hold a note. Saw comes from the right,
  FM from the left (or wherever you set each). A hybrid stereo image.
  Return both pans to centre. Release.
- [ ] Set slot 1 level to 0 — FM goes silent mid-note. Set back to 0.5.
  FM returns. (No clicks expected here since amp envelope gates the
  output — just a volume change.)

---

## 3. All 8 FM algorithms on Slot 1

Set: Slot 0 level = 0 (silent), Slot 1 = FM, level = 1.0.
Set all operator levels to 1.0, ratio integer = 1 for all, fine = 0,
Attack = 0.05 s, Decay = 0.5 s, Sustain = 0.8, Release = 0.3 s for
all operators.

For each algorithm, hold a note for 1–2 seconds, then release.

- [ ] **Algorithm 1** (Op 4→Op 3→Op 2→Op 1 stack, Op 1 is the carrier):
  Classic FM stack. Should sound like a bright, bell-like or brass-like
  tone. Only Op 1 contributes to the audio output.
- [ ] **Algorithm 2** (same stack + Op 4 self-feedback): Same stack but
  Op 4 feeds back on itself. Should sound richer and rougher than Alg 1
  — more overtones, especially with feedback cranked up. Set the
  **Op 4 Feedback** knob to +0.7 to hear the difference clearly.
- [ ] **Algorithm 3** (two stacks: Op 4→Op 3 and Op 2→Op 1, mixed):
  Op 1 and Op 3 are both carriers. Should sound like two separate FM
  stacks layered, giving a fuller timbre.
- [ ] **Algorithm 4** (Op 4 modulates Op 1, Op 2, Op 3 in parallel):
  Three carriers (Op 1, Op 2, Op 3) all FM'd by the same modulator
  (Op 4). Should sound dense and organ-like.
- [ ] **Algorithm 5** (Op 4 modulates Op 2+Op 3; Op 3 modulates Op 1):
  Branching modulator. Should produce a complex, evolving timbre.
- [ ] **Algorithm 6** (Op 3+Op 2 modulate Op 1; Op 4 separate carrier):
  Op 1 and Op 4 are both carriers. Should sound like a carrier with
  frequency-modulated harmonic content plus Op 4's direct sine tone.
- [ ] **Algorithm 7** (all four parallel, additive): All four operators
  are carriers with no modulation. Should sound like four sine waves
  added together — a clean, slightly rich tone.
- [ ] **Algorithm 8** (Op 4→Op 1; Op 3→Op 2; Op 1 and Op 2 are carriers):
  Two modulator–carrier pairs in parallel. Should sound similar to two
  separate FM tones mixed.

**Pass criterion**: every algorithm produces a recognisably different
timbre; none produce silence, DC offset, or distorted/broken audio.

---

## 4. Operator ratio

Set: Slot 1 = FM, Algorithm 1 (Op 4→Op 3→Op 2→Op 1), all levels 1.0.

- [ ] Set **Op 4 Ratio Integer** to 1. Hold a note — baseline timbre.
  Increase to 2, 4, 8. The timbre gets progressively brighter and more
  complex as the modulation index ratio increases.
- [ ] Set all operators to **Ratio Integer = 1**, then set **Op 1**
  (carrier) to 2. Hold a note. The output pitch should be one octave
  higher than the note played (carrier runs at 2× the note frequency).
- [ ] Set **Op 4 Ratio Fine** to +100 ct. Hold a note — the timbre
  becomes "detuned" or slightly inharmonic compared to fine = 0.
  Set to -100 ct for the opposite detuning. Return to 0.

---

## 5. Operator level

Set: Slot 1 = FM, Algorithm 1, all ratios = 1.

- [ ] Set **Op 4 Level** (top modulator in the stack) to 0. Hold a note.
  With Op 4 silent, its modulation contribution to Op 3 is zero, so the
  stack simplifies to Op 3→Op 2→Op 1 with the top link broken. Timbre
  becomes simpler/cleaner.
- [ ] Set Op 4 Level back to 1.0. Set **Op 3 Level** to 0. Now Op 3
  contributes no modulation to Op 2, so the remaining chain is just
  Op 2 → Op 1 — effectively a simple FM pair.
- [ ] Set all levels to 0 except **Op 1 Level** = 1.0. Algorithm 1 only
  has Op 1 as carrier. With no modulators active, you should hear a pure
  sine at the note frequency.

---

## 6. Operator ADSR

Set: Slot 1 = FM, Algorithm 1. Op 1 level = 1.0, Op 4 level = 1.0 (the
others at 0.5 so there is clear modulation but the carrier is audible).

- [ ] Set **Op 4 Attack** to 2.0 s. Hold a note. The timbre should start
  clean (little modulation) and gradually become brighter over 2 seconds
  as Op 4's envelope ramps up. Release — the modulation dies.
- [ ] Set **Op 4 Decay** to 0.1 s, **Op 4 Sustain** = 0. Hold a note.
  Op 4 fires and decays quickly — a sharp transient "click" or bright
  attack followed by a simpler sustained tone as the modulator envelope
  falls to 0. Classic DX7-style bell.
- [ ] Set **Op 1 Attack** to 1.0 s (the carrier's own envelope). Hold a
  note. The output fades in slowly even though the modulator is active.
  Confirms carrier amplitude is independently enveloped.

---

## 7. Operator feedback (Op 4)

Set: Slot 1 = FM, Algorithm 2 (has self-feedback), all levels = 1.0.
Op 4 Ratio = 1.

- [ ] Set **Op 4 Feedback** to 0. Hold a note — clean FM tone.
- [ ] Slowly increase **Op 4 Feedback** toward +1.0. The timbre should
  become progressively noisier and more distorted as the feedback
  oscillates more aggressively. At +1.0 it should be very rich/noisy
  but should NOT produce clicks, infinite values, or silence.
- [ ] Set to -1.0. Should sound similar to +1.0 (the sign of feedback
  affects phase but not spectral density at high amounts).
- [ ] Set back to 0. Clean tone returns.

---

## 8. Hybrid patch (canonical M7 test)

This is the primary goal of M7: a patch that uses both a subtractive
slot and an FM slot simultaneously.

Setup:
- **Slot 0**: Subtractive, level = 1.0, pan = -0.25 (slightly left)
- **Slot 1**: FM, level = 0.7, pan = +0.25 (slightly right)
- Slot 1 algorithm: **1** (clean FM bell stack)
- Slot 1 operator settings: all ratio = 1, Op 4 level = 0.8, Op 1
  level = 1.0, Op 2 and Op 3 at 0.5.
  Op 4 envelope: A=0.01, D=0.3, S=0, R=0.1 (fast transient modulator).
  Op 1 envelope: A=0.01, D=1.0, S=0.6, R=0.5.
- Voice amp envelope: A=0.01, D=0.5, S=0.7, R=0.3.

- [ ] Hold a note. You should hear a warm saw (slot 0, slightly left)
  blended with a bell-like FM tone (slot 1, slightly right). Both
  layers should be clearly audible and stereo-imaged.
- [ ] Play a scale across 2+ octaves. Both layers track pitch correctly.
- [ ] Play several notes in quick succession (short staccato). Both
  layers start and stop cleanly with no hanging notes.
- [ ] While holding a note, move **Slot 0 Pan** to -1 (full left) and
  **Slot 1 Pan** to +1 (full right). The saw and FM bell should be
  clearly separated in the stereo field.
- [ ] Set **Slot 1 Level** to 0. Only the saw remains. Set to 1.0. FM
  bell returns. Both changes take effect immediately.

---

## 9. Anti-aliasing sanity

This tests that the 2× oversampling in the FM bank reduces artefacts
at high modulation indices.

Setup: Slot 1 = FM, Algorithm 2, Slot 0 level = 0.
**Op 4**: Ratio = 8, Level = 1.0, Feedback = 0.7.
**Op 1**: Level = 1.0 (only carrier). Op 2 and Op 3 at level 0.5.

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

This is a rough sanity check only — full FM modulation targets are M11.

- [ ] Enable mod matrix Slot 1: Source = LFO1, Dest = **Vol** (master
  volume), Amount = +0.3. Hold a note. The overall volume should
  tremolo at LFO rate. (This uses the existing Vol destination; it
  affects the whole voice, not per-slot.)
- [ ] Try Source = Env2, Dest = **Cutoff**, Amount = +5000. Hold a note.
  Filter sweeps as before — FM slot does not interfere with the filter
  modulation path. Confirms FM is upstream of the filter, not bypassing
  it.

---

## 12. Regression — features present before M7

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
