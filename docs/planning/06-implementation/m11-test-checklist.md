# M11 Test Checklist — UI v1 Polish

## Automated (CI)

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all --check` clean

## Layout

- [ ] Header bar: "Tone Smithy" title, Patch name field, Save, Load buttons, audio status
- [ ] Seven tabs visible: Osc | Filter | Envelopes | Modulation | Arp | FX | Master
- [ ] Virtual keyboard strip always visible, full height, not clipped
- [ ] Footer always visible: CPU%, voice count, audio device info
- [ ] Central panel scrolls vertically — FM operator grid reachable without being hidden

## Theme

- [ ] Dark background throughout (no egui default grey panels)
- [ ] Section labels in FG1, secondary labels in FG2
- [ ] Knob arcs in accent cyan
- [ ] Toggle pills use accent colour when on, muted when off
- [ ] Mod rings visible in green (positive) / magenta (negative) when modulation is active

## Knob behaviour

- [ ] Drag changes value
- [ ] Shift+drag gives fine (1/10th) control
- [ ] Double-click resets to default
- [ ] Right-click opens context menu with Reset, Copy value, MIDI Learn (greyed)
- [ ] Hover shows tooltip with label and formatted value

## Oscillator tab

- [ ] Osc 1, Osc 2, Osc 3 all show Level/Detune/Pan + Unison controls
- [ ] Sub oscillator controls (Level, Pan) visible below Osc 3
- [ ] Waveform selector at top applies to all main oscillators
- [ ] Slots/FM section below the osc columns at full width
- [ ] Slot mode toggle (Sub/FM) works for both slots
- [ ] FM operator grid appears when slot is in FM mode, scrollable if needed

## Filter tab

- [ ] Mode selector (LP/HP/BP/Notch) works
- [ ] Cutoff and Resonance knobs respond
- [ ] Mod ring visible on Cutoff / Resonance when a mod slot targets them

## Envelopes tab

- [ ] Amp Env ADSR knobs work
- [ ] Env2 ADSR + curve knobs work
- [ ] LFO 1 & 2: shape, rate, reset, sync, division all respond
- [ ] Live "Out" readout updates while LFOs are running

## Modulation tab

- [ ] 8 slots visible with Toggle, Source, Dest, Amount (knob), Via
- [ ] Amount knob double-click resets to 0
- [ ] Changing dest resets amount to 0

## Arp tab

- [ ] Enabled Toggle works
- [ ] Mode, Octaves, Rate dropdowns respond
- [ ] BPM, Gate, Swing knobs respond

## FX tab

- [ ] Toggle enables/disables each stage (EQ, Drive, Chorus, Delay, Reverb)
- [ ] All parameter knobs within each stage respond

## Master tab

- [ ] Volume, Pitch, BPM knobs respond
- [ ] Voice count and modulator status readout updates live
- [ ] Mod rings visible on Volume / Pitch when targeted

## Preset bar

- [ ] Patch name field editable; typing does not trigger piano notes
- [ ] Save opens native file dialog
- [ ] Load opens native file dialog; patch name updates after load
- [ ] Error message appears in its own bar when load fails; dismissable

## HiDPI

- [ ] UI renders cleanly at 100% scale
- [ ] UI renders cleanly at 125% scale (if testable)
