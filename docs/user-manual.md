# Tone Smithy v1.0 — User Manual

Tone Smithy is a standalone software synthesizer for Windows combining
analog-style subtractive synthesis with 4-operator FM. Both synthesis modes are
available simultaneously within a single voice, so you can layer warm oscillator
textures with FM timbres in one patch.

---

## Contents

1. [Quick start](#1-quick-start)
2. [Interface overview](#2-interface-overview)
3. [Master tab](#3-master-tab)
4. [Oscillator tab](#4-oscillator-tab)
5. [Filter tab](#5-filter-tab)
6. [Envelopes and LFOs tab](#6-envelopes-and-lfos-tab)
7. [Modulation tab](#7-modulation-tab)
8. [Arpeggiator tab](#8-arpeggiator-tab)
9. [FX tab](#9-fx-tab)
10. [Preset browser](#10-preset-browser)
11. [Settings](#11-settings)
12. [MIDI Learn](#12-midi-learn)
13. [Computer keyboard](#13-computer-keyboard)
14. [Troubleshooting](#14-troubleshooting)

---

## 1. Quick start

1. Run the installer and launch Tone Smithy from the Start Menu.
2. On first launch, the setup wizard asks you to choose an **audio output** and
   a **MIDI input**. Both can be changed later under Settings.
3. Play a note — from a MIDI keyboard, the on-screen piano, or your computer
   keyboard (see [Computer keyboard](#13-computer-keyboard)).
4. Open the **Presets** tab and click any preset to load it.
5. Push your mod wheel up on presets marked "(MW)" — several respond to it with
   vibrato or tremolo.

If you see a note stuck on, click **Panic** in the header bar.

---

## 2. Interface overview

The window is divided into two areas:

- **Header bar** (top strip) — patch name, Panic button, pitch-bend and mod-wheel
  indicators, CPU meter, and audio/MIDI status.
- **Tab area** (main body) — nine tabs covering every aspect of the sound:
  Master, Osc, Filter, Envelopes, Mod, Arp, FX, Presets, and Settings.

### Controls

- **Knobs** — click and drag up/down to change value. Hold **Shift** while
  dragging for fine control. Double-click to reset to default. The current value
  shows beneath the knob while you drag. Right-click to MIDI Learn (see
  [MIDI Learn](#12-midi-learn)).
- **Toggles** — click to enable/disable a section (EQ, Drive, Chorus, etc.).
- **Selectors** — labelled buttons or drop-down menus; click to choose.

### Header bar

| Element | Description |
|---|---|
| Patch name | Shows the loaded preset name. |
| Panic | Immediately silences all notes (sends All Notes Off). Use when a note gets stuck. |
| PB | Pitch-bend indicator; moves when your MIDI controller sends pitch-bend. |
| MW | Mod-wheel indicator; moves when CC 1 is received. |
| CPU | Audio thread load percentage. |
| Status line | Sample rate, channel count, buffer hint, and active MIDI port. |

---

## 3. Master tab

Global patch controls and live status readout.

| Control | Range | Description |
|---|---|---|
| Volume | 0–100% | Master output level. Can be modulated via the mod matrix. |
| Pitch | -24 to +24 st | Global pitch offset in semitones. Useful for transposing a patch. |
| BPM | 20–300 | Tempo used by BPM-synced LFOs. The arpeggiator has its own separate BPM knob in the Arp tab. |

### Status section

Shows live values updated in real time:

- **Voices** — number of currently active voice slots.
- **LFO 1 / LFO 2** — current output value (-1 to +1).
- **Env 2** — current output value (0 to 1).
- **VU meter** — left/right peak level of the output signal.

---

## 4. Oscillator tab

The oscillator section provides the raw sound source. Each voice contains three
main oscillators, a sub-oscillator, and two slots that can run independently in
either Subtractive or FM mode.

### Waveform selector

Selects the waveform for all three main oscillators simultaneously:
**Sine**, **Saw**, **Square** (Sq), or **Triangle** (Tri).

### Oscillators 1, 2, and 3

Each oscillator has three controls:

| Control | Range | Description |
|---|---|---|
| Level | 0–1 | Output level of this oscillator. |
| Detune | -100 to +100 ct | Fine pitch offset in cents. OSC 1 Detune and Pan can be targeted by the mod matrix. |
| Pan | L100 to R100 | Stereo position. |

Each oscillator also has a **Unison** section:

| Control | Range | Description |
|---|---|---|
| Voices | 1–7 | Number of detuned unison copies. 1 = no unison. |
| Detune | 0–50 ct | Total detune spread across all unison voices. |
| Spread | 0–1 | Stereo width of the unison voices. |

### Sub oscillator (OSC 3 column)

A pure sine one octave below the fundamental, mixed with OSC 3.

| Control | Range | Description |
|---|---|---|
| Level | 0–1 | Sub oscillator level. |
| Pan | L100 to R100 | Sub oscillator stereo position. |

### Slots / FM section

Two per-voice **Slots** sit below the oscillator columns. Each slot can run
in one of two modes, switchable with the Sub / FM buttons:

**Sub mode** — the slot blends the main oscillators (OSC 1–3 + Sub) into the
voice signal. Slot 1 defaults to Sub at level 1.0; Slot 2 defaults to level 0.

**FM mode** — the slot drives a 4-operator FM engine with 8 selectable
algorithms, independently of any subtractive content in the other slot.

In either mode each slot has:

| Control | Description |
|---|---|
| Level | Output level of this slot. |
| Pan | Stereo position. |

#### FM operator controls (FM mode only)

When a slot is in FM mode, selecting an algorithm reveals the operator grid.
Each of the four operators (OP 1–4) has:

| Control | Range | Description |
|---|---|---|
| Ratio (integer) | 1–15 | Harmonic ratio: integer part. |
| Ratio (fine) | -100 to +100 | Fine ratio offset. Allows inharmonic tones. |
| Level | 0–1 | Operator output level (for carriers) or modulation depth (for modulators). |
| Feedback | -1 to +1 | Self-feedback amount (OP 4 only). |
| A / D / S / R | (time) | Per-operator ADSR envelope. |

---

## 5. Filter tab

A state-variable filter applied globally to the voice output.

### Mode

| Mode | Description |
|---|---|
| LP | Low-pass — passes frequencies below the cutoff. |
| HP | High-pass — passes frequencies above the cutoff. |
| BP | Band-pass — passes a band around the cutoff. |
| Notch | Notch filter — cuts a narrow band around the cutoff. |

### Controls

| Control | Range | Description |
|---|---|---|
| Cutoff | 20 Hz–20 kHz | Filter cutoff frequency. Modulatable via the mod matrix. |
| Res | 0–1 | Resonance. Higher values add a peak at the cutoff. Approaching 1 nears self-oscillation. |

Typical use: assign Env 2 to Cutoff in the mod matrix for a classic filter sweep.

---

## 6. Envelopes and LFOs tab

Four generators: the Amp envelope, Env 2, and two LFOs.

### Amp Env

Controls the amplitude shape of every note.

| Control | Range | Description |
|---|---|---|
| A (Attack) | 0–10 s | Time to rise from silence to full level. |
| D (Decay) | 0–10 s | Time to fall from peak to the sustain level. |
| S (Sustain) | 0–1 | Level held while the note is held. |
| R (Release) | 0–10 s | Time to fall to silence after the note is released. |

### Env 2

A second ADSR envelope available as a modulation source in the mod matrix.
In addition to the standard ADSR knobs, Env 2 has **Curve** controls:

| Control | Range | Description |
|---|---|---|
| A curve | -1 to +1 | Shape of the attack stage. 0 = linear; negative = convex; positive = concave. |
| D curve | -1 to +1 | Shape of the decay stage. |
| R curve | -1 to +1 | Shape of the release stage. |

The current Env 2 output value is shown in real time below the knobs.

### LFO 1 and LFO 2

Two identical low-frequency oscillators available as mod matrix sources.

**Shape** — click one of the seven shape buttons:

| Shape | Description |
|---|---|
| Sin | Smooth sine wave. |
| Tri | Triangle wave (linear ramp up/down). |
| Saw+ | Rising sawtooth. |
| Saw- | Falling sawtooth. |
| Sq | Square wave. |
| S&H | Sample-and-hold: new random value on each cycle. |
| Rnd | Smooth random (interpolated between random values). |

**Rate** — 0.01–20 Hz (only active when Sync is off).

**Reset** — when enabled, the LFO phase resets to zero on every new note-on.

**Sync** — locks the LFO to the BPM set in the Master tab, using one of the
following divisions: 1/32, 1/16, 1/8, 1/4, 1/2, 1, 2, or 4 bars.

The current LFO output value is shown in real time below the controls.

---

## 7. Modulation tab

An 8-slot modulation matrix: each slot routes one source to one destination,
scaled by an amount, with an optional via source.

### Columns

| Column | Description |
|---|---|
| (number toggle) | Enable / disable this slot without clearing its settings. |
| Source | What drives the modulation. |
| Dest | What parameter is modulated. |
| Amount | How much modulation is applied (positive or negative). |
| Via | An optional secondary source that scales the amount. When set, the actual depth = Amount x Via value. Useful for mod-wheel control over vibrato depth. |

### Sources

| Label | Source |
|---|---|
| Off | No modulation (slot inactive at the source level). |
| LFO1 | LFO 1 output (-1 to +1). |
| LFO2 | LFO 2 output (-1 to +1). |
| Env2 | Env 2 output (0 to 1). |
| AmpEnv | Amp envelope output (0 to 1). |
| Vel | MIDI velocity of the current note (0 to 1). |
| Key | Key tracking — linear from 0 (C-1) to 1 (G9). |
| ModWhl | MIDI mod wheel / CC 1 (0 to 1). |
| AfterT | MIDI channel aftertouch (0 to 1). |
| Bend | MIDI pitch bend (-1 to +1). |

### Destinations

| Label | Destination | Amount range |
|---|---|---|
| Cutoff | Filter cutoff frequency | +/-10,000 Hz |
| Reso | Filter resonance | +/-1 |
| Pitch | Global pitch offset | +/-24 semitones |
| Vol | Master volume | +/-1 |
| Osc1Det | OSC 1 detune | +/-2,400 cents |
| Osc1Pan | OSC 1 pan | +/-1 |

### Example: mod wheel controls vibrato depth

1. Enable slot 1.
2. Source: **LFO1** (set LFO 1 shape to Sin, rate ~5 Hz in the Env/LFO tab).
3. Dest: **Pitch**.
4. Amount: **0.5** semitones (or however deep you want full vibrato).
5. Via: **ModWhl**.

With the mod wheel down there is no vibrato; pushing it up smoothly introduces
the LFO pitch modulation.

---

## 8. Arpeggiator tab

Automatically sequences held notes at a rhythmic rate.

| Control | Options | Description |
|---|---|---|
| Enabled | On/Off | Engage or bypass the arpeggiator. |
| Mode | Up, Down, Up/Dn, Rand, Played | Order in which held notes are played. |
| Octaves | 1–4 oct | How many octaves the pattern spans. |
| Rate | 1/32, 1/16, 1/8, 1/4, 1/2 | Note duration, relative to the BPM knob below. |
| BPM | 20–300 | Arpeggiator tempo. Independent from the Master tab BPM (which drives LFO sync). |
| Gate | 1–100% | How much of each step the note is held before releasing. Lower values give a more staccato feel. |
| Swing | 0–100% | Delays every other step, creating a shuffle feel. 0% = straight. |

Hold multiple notes simultaneously for chords — all held keys are included in
the arp pattern.

---

## 9. FX tab

A five-stage effects chain: EQ -> Drive -> Chorus -> Delay -> Reverb.
Each stage has an enable toggle; disabled stages consume no CPU and are
fully bypassed. Controls within a disabled section are greyed out.

### EQ

A three-band equalizer (low shelf, peaking mid, high shelf).

| Control | Range | Description |
|---|---|---|
| Low Gain | -15 to +15 dB | Low shelf gain. |
| Low Freq | 20–2,000 Hz | Low shelf frequency. |
| Mid Gain | -15 to +15 dB | Mid peak gain. |
| Mid Freq | 200–8,000 Hz | Mid peak centre frequency. |
| Mid Q | 0.1–10 | Mid peak width. Higher = narrower. |
| High Gain | -15 to +15 dB | High shelf gain. |
| High Freq | 2–20 kHz | High shelf frequency. |

### Drive

Soft-clipping waveshaper for harmonic saturation or heavier distortion.

| Control | Range | Description |
|---|---|---|
| Drive | 1–20x | Input gain before clipping. Higher = more distortion. |
| Asym | -1 to +1 | Asymmetry of the clipping curve. Adds even harmonics. |

### Chorus

Stereo chorus using modulated short delays.

| Control | Range | Description |
|---|---|---|
| Rate | 0.1–8 Hz | Modulation rate of the internal delay. |
| Depth | 0–15 ms | Modulation depth (delay time variation). |
| Mix | 0–100% | Wet/dry blend. |
| Spread | 0–100% | Stereo width of the effect. |

### Delay

Stereo echo effect with feedback.

| Control | Range | Description |
|---|---|---|
| Time | 1 ms–2 s | Delay time. |
| Fdbk (Feedback) | 0–95% | Amount of output fed back into the delay line. |
| Mix | 0–100% | Wet/dry blend. |
| LoCut | 20–2,000 Hz | High-pass filter on the feedback path — prevents low-end buildup. |
| Ping-pong | On/Off | Alternates the repeats between left and right channels. |

### Reverb

FDN-8 algorithmic reverb.

| Control | Range | Description |
|---|---|---|
| Pre | 0–50 ms | Pre-delay — time before the reverb tail begins. |
| Decay | 0.1–30 s | Reverb tail length (RT60). |
| Size | 0.1–1 | Virtual room size. Affects the density of early reflections. |
| Damp | 0–100% | High-frequency damping. Higher = darker, more natural tail. |
| Mix | 0–100% | Wet/dry blend. |

---

## 10. Preset browser

### Loading a preset

Open the **Presets** tab. The browser shows two sections:

- **FACTORY** — 61 built-in presets across six categories.
- **USER** — presets you have saved to your user folder.

Use the **category chips** (Bass, Lead, Pad, Pluck, Keys, FX) to filter the
list, or type in the **Search** box to filter by name, author, or tag. Click a
preset name to load it.

### Saving a preset

Right-click any preset in the USER section and choose **Save current as this
preset** to overwrite it with the current patch. To create a new preset file,
place a `.tsmith` file in your user preset folder and it will appear in the
browser automatically.

**User preset folder:** `%APPDATA%\Tone Smithy\presets\`

### .tsmith file association

If you enabled the file association during installation, double-clicking a
`.tsmith` file opens it directly in Tone Smithy.

---

## 11. Settings

Access via the **Settings** tab.

### Audio output

Select your audio output device from the drop-down list. Changes take effect
immediately without restarting the app. The current sample rate, channel count,
and buffer size are shown in the status line.

### MIDI input

Select your MIDI input device. If a device was connected after Tone Smithy
launched, unplug and reconnect it so the operating system re-registers it, then
re-select it here.

Settings (selected devices) are saved automatically and restored on the next
launch.

---

## 12. MIDI Learn

Any knob can be mapped to a MIDI CC controller:

1. **Right-click** the knob you want to control.
2. Choose **MIDI Learn** from the context menu.
3. Move the physical knob or fader on your controller — the assignment is made
   automatically on the first CC received.

Mappings are saved with the preset. To remove a mapping, right-click the knob
and choose **Clear MIDI Learn**.

---

## 13. Computer keyboard

Tone Smithy is playable from your computer keyboard when no MIDI device is
available.

### Note layout (one chromatic octave)

```
 W  E     T  Y  U
A  S  D  F  G  H  J
```

This maps to: A=C, W=C#, S=D, E=D#, D=E, F=F, T=F#, G=G, Y=G#, H=A, U=A#, J=B.

### Octave shift

- **Z** — shift one octave down.
- **X** — shift one octave up.

The default starting octave is C3 (MIDI note 48). The range is C-1 to G#7.

---

## 14. Troubleshooting

**No sound**
- Check that the correct audio output is selected in Settings.
- Confirm the system volume and the selected device are not muted.
- Check the Master Volume knob is not at zero.

**Note stuck on**
- Click **Panic** in the header bar. This sends an All Notes Off message to the
  engine and clears the arpeggiator.
- Panic can also be triggered by MIDI CC 120 (All Sound Off) or CC 123 (All
  Notes Off) from your controller.

**MIDI keyboard not responding**
- Confirm the device is selected in Settings.
- If the device was plugged in after launch, unplug and reconnect it, then
  re-select it in Settings.
- Check the status line at the top of the window — it shows the active MIDI port
  name when connected.

**Crackle or audio dropouts**
- Increase the buffer size in your audio driver's control panel (ASIO or
  Windows Audio Session).
- Close other audio applications that may be competing for the device.
- Check the CPU meter in the header; heavy FX chains (large reverb decay, high
  unison voice counts) increase CPU load.

**SmartScreen warning on first launch**
- Tone Smithy v1.0 is unsigned. Windows SmartScreen may show "Windows protected
  your PC". Click **More info** then **Run anyway** to continue. This prompt
  only appears on the first launch of a new build.

---

*Tone Smithy v1.0 — dual-licensed MIT OR Apache-2.0.*
*Source code and issue tracker: https://github.com/davewattsara/tone-smithy*
