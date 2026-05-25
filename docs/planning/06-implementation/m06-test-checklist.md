# M6 manual test checklist

Run these before closing out M6 and merging to main.

## Mod matrix

- [ ] Enable slot 1, Source=Env2, Dest=Cutoff, Amount=+8000, Via=Off. Hold a note — filter sweeps open with the Env2 shape.
- [ ] Enable slot 2, Source=LFO1, Dest=Cutoff, Via=ModWheel, Amount=+5000. Mod wheel at 0: no sweep. Mod wheel at max: full LFO sweep. (Canonical "source via" test.)
- [ ] Source=Velocity, Dest=Vol, Amount=+1.0. Soft MIDI notes quieter than hard ones.
- [ ] Source=Key, Dest=Cutoff, Amount=+5000. High notes brighter than low notes.
- [ ] Disable a slot mid-note — contribution stops immediately, no click or glitch.
- [ ] Change Dest on an active slot — amount knob resets to 0 and re-ranges correctly for the new destination.
- [ ] Enable all 8 slots simultaneously — no crash or audio dropout.

## Keyboard and input

- [ ] Virtual keyboard plays notes on click; releasing stops them (unless sustain on).
- [ ] Computer keyboard: A S D F G H J plays white keys, W E T Y U plays black keys.
- [ ] Z / X shifts octave down / up; hint label in UI updates.
- [ ] Pitch bend slider moves to dragged position; springs back to centre on mouse release.
- [ ] Sustain toggle: ON holds notes after key release; OFF drops them.

## Regression — features present before M6

- [ ] Filter cutoff and resonance knobs respond with no mod slots enabled.
- [ ] Amp envelope A/D/S/R knobs change the envelope shape audibly.
- [ ] LFO shape buttons and rate knob work; live "Out:" readout updates.
- [ ] Env2 ADSR and curve knobs work; live "Out:" readout updates.
- [ ] Scrolling down in the window reaches the keyboard.
- [ ] Footer shows CPU% and voice count updating in real time.
