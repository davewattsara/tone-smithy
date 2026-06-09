# M14 — Factory Bank

**Status:** In progress (branch `milestone/m14-factory-bank`)

## Goal

Author and ship ~60 factory presets that give every category a strong, distinct
character and demonstrate the hybrid engine's range — from clean subtractive
basses to FM bells to layered hybrid pads.

---

## Done when

- 60 factory presets are embedded in the binary (all 7 existing plus 53 new).
- Every preset has a clear, unique identity — no two patches are minor variations
  of each other.
- Each category contains at least three "demo" presets showcasing the engine's
  best behaviour for that role.
- All presets pass a QA listen: no clicks, no denormals, no amplitude runaway on
  long holds.
- Existing 7 presets are corrected for the `osc_1_level` → `osc1_level` key-name
  bug introduced before the M13 param-key fix.

---

## Parameter encoding reference

| Symbol | Key | Values |
|---|---|---|
| Waveform | `waveform` | 0=Sine, 1=Saw, 2=Square, 3=Triangle |
| Filter mode | `filter_mode` | 0=LP, 1=HP, 2=BP, 3=Notch |
| Slot mode | `slot_mode_0`, `slot_mode_1` | 0=Subtractive, 1=FM |
| LFO shape | `lfo1_shape_index`, `lfo2_shape_index` | 0=Sine, 1=Tri, 2=SawUp, 3=SawDown, 4=Square, 5=S&H, 6=SmoothRandom |
| Mod source | `mod_slot_source_N` | 0=Off, 1=LFO1, 2=LFO2, 3=Env2, 4=AmpEnv, 5=Vel, 6=Key, 7=ModWhl, 8=AfterT, 9=Bend |
| Mod dest | `mod_slot_dest_N` | 0=Cutoff, 1=Reso, 2=Pitch, 3=Vol, 4=Osc1Det, 5=Osc1Pan |
| FM algorithm | `fm_algorithm_0`, `fm_algorithm_1` | 0–7 (see below) |
| Arp mode | `arp_mode` | 0=Up, 1=Down, 2=UpDown, 3=Random, 4=Played |

### FM algorithms (0-indexed)

| # | Description |
|---|---|
| 0 | 4→3→2→1 stack — single carrier, deep modulation |
| 1 | 4→3→2→1 with op3 self-feedback — richer mod content |
| 2 | Two parallel stacks (4→3 and 2→1) — two carriers |
| 3 | Op3 modulates ops 0+1+2 in parallel — three carriers |
| 4 | Branching: 3→2, 3→1, 2→0 — single carrier |
| 5 | 2+1 modulate op0; op3 separate carrier |
| 6 | All four additive (no modulation) |
| 7 | Paired stacks: 3→1, 2→0 — two carriers |

For all preset authors: feedback is op3 (`fm_op_feedback_S_3`). Ops are 0-indexed.

---

## Bug fix: osc key names

The 7 existing presets use `osc_1_level` (with underscore-number). The engine
parameter map uses `osc1_level` (no underscore). On load the old keys are silently
ignored, so osc levels fall back to defaults. All 7 presets must be corrected as
part of M14.

---

## Preset list (60 total)

Each row gives: **File** · identity sentence · techniques that make it work.

### Bass (15)

| # | File | Identity | Key techniques |
|---|---|---|---|
| 1 | `sub_bass` | Deep sine fundamental with a tightly filtered saw bite | Existing — fix keys |
| 2 | `bass_wool_stack` | Thick, furry Minimoog-style bass | 3 saws + sub, LP 300 Hz, slow Env2 filter sweep, drive |
| 3 | `bass_acid_line` | Sharp 303-style resonant bass | Square, LP, high reso, Env2 fast decay to cutoff, slightly open |
| 4 | `bass_reese` | Classic DnB Reese (two saws beating against each other) | Osc1+Osc2 at ±8 cents, chorus, LP ~800 Hz |
| 5 | `bass_fm_bell_bass` | FM with a bell-like transient and a low fundamental | FM slot, alg 0, fast op envelope, ratio 1:2:3:4 |
| 6 | `bass_upright` | Acoustic upright feel — woody, quick | Triangle, LP 1.2 kHz, attack 5ms, decay 250ms, sustain 0.1 |
| 7 | `bass_drive_stack` | Driven, midrange-heavy bass that cuts through a mix | Saw, drive stage at 12, EQ mid boost, LP 2 kHz |
| 8 | `bass_stab` | One-shot punchy stab, dead in 300ms | Saw, near-zero sustain, fast filter close, no release |
| 9 | `bass_tape_warmth` | Lo-fi warm bass with slight even-harmonic softness | Sine+Saw mix, drive asymmetry 0.4, LP 900 Hz |
| 10 | `bass_growl` | Aggressive growl — detuned and saturated | 3 saws at ±12/0 cents, filter 600 Hz, drive, slight chorus |
| 11 | `bass_plonk` | FM-plucked feel at low pitch | FM slot, alg 7 (additive), very fast attack+decay, long release tail |
| 12 | `bass_fm_sub_layer` | FM shimmer sitting on a subtractive sub | Slot0=Sub subtractive (sine, sub_level 1.0), Slot1=FM (alg 3, high partials) |
| 13 | `bass_mono_tight` | Clean, tight, no-nonsense monosynth bass | Single saw, no detune, LP 800 Hz, fast envelope |
| 14 | `bass_rubber` | Rubbery slow-attack bass with pronounced pitch bend feel | Saw, attack 80ms, Env2 to cutoff slow sweep |
| 15 | `bass_sine_sub_duo` | Two-note sub harmony — sine + octave sub blended | Sine waveform, sub_level 0.7, very low cutoff, long release |

### Lead (15)

| # | File | Identity | Key techniques |
|---|---|---|---|
| 1 | `lead_saw` | Classic bright saw lead | Existing — fix keys |
| 2 | `lead_screamer` | Aggressive unison screamer | 3 saws ±15 cents, chorus, BP filter at 2 kHz, drive |
| 3 | `lead_fm_bell` | Glassy FM bell lead | FM slot alg 0, op ratios 1:2:4:7, fast mod decay |
| 4 | `lead_square_mono` | Crisp square-wave mono lead | Square, LP 5 kHz, slight resonance, tight envelope |
| 5 | `lead_supersaw` | Dense supersaw with wide stereo spread | 3 saws at 0/+7/-7 cents, unison 5 voices, chorus, LP 8 kHz |
| 6 | `lead_glass` | FM glass-harmonic shimmer | FM slot alg 4, high-ratio ops, long release |
| 7 | `lead_acid` | Fast-decay acid lead | Square, very high reso, Env2 decay 60ms to cutoff |
| 8 | `lead_whistle` | Thin, piercing whistle | High reso BP, sine waveform, HP EQ shelf |
| 9 | `lead_biting_pwm` | Chorus-widened PWM simulation | Square, chorus rate 0.3Hz depth 6ms, warm LP |
| 10 | `lead_unison_blade` | Razor unison — no FX, just edge | 7-voice unison, osc1 only, LP 9 kHz, reso 0.35 |
| 11 | `lead_formant` | Filter-comb formant sweep character | BP filter, key tracking mod (Dest:Cutoff), LFO slow vibrato |
| 12 | `lead_fm_feedback` | Harsh FM with feedback distortion on op3 | FM alg 1, fm_op_feedback_0_3 = 0.8, high mod index |
| 13 | `lead_vintage_solo` | Warm vintage analog solo | Single saw, mild detune, LP 6 kHz, gentle reverb |
| 14 | `lead_triangle_soft` | Soft triangle — almost flute-like | Triangle, LP 4 kHz, low reso, chorus off, reverb |
| 15 | `lead_dual_slot` | Hybrid: subtractive warmth + FM sparkle | Slot0 saw low level, Slot1 FM alg 7 (additive), balanced mix |

### Pad (12)

| # | File | Identity | Key techniques |
|---|---|---|---|
| 1 | `pad_analog` | Warm analog-style slow pad | Existing — fix keys |
| 2 | `pad_fm_shimmer` | Subtractive body + FM shimmer layer | Slot0 saw, Slot1 FM alg 3 (parallel), mix 60/40 |
| 3 | `pad_string_section` | Orchestral-style string section | 3 saws ±9 cents, unison 5, chorus, reverb large |
| 4 | `pad_dark_void` | Dark, slow, heavy | Saw, LP 400 Hz, reso 0.05, attack 2s, heavy reverb |
| 5 | `pad_glass_choir` | Shimmery glass choir | FM slot alg 2 (two stacks), slow attack, chorus |
| 6 | `pad_warm_blanket` | Ultra-warm, enveloping | Triangle, LP 1.8 kHz, unison 3, chorus, long reverb decay |
| 7 | `pad_arctic_air` | Cold, ethereal, HP-bright | Saw, HP 200 Hz, long attack, long reverb pre-delay |
| 8 | `pad_hybrid_sweep` | Slow filter sweep across both slots | Env2 to Cutoff, amount large, decay 4s, slow motion |
| 9 | `pad_velvet` | Lush velvet — dense midrange warmth | Saw+sub, chorus rate 0.2 Hz, decay 3s reverb |
| 10 | `pad_drone` | Long sustaining drone, subtle LFO drift | Saw, LFO1 (SmoothRandom) to Cutoff, slow rate 0.08 Hz |
| 11 | `pad_brass_spread` | Wide brass ensemble spread | 3 saws ±5 cents, BP filter 1.2 kHz, reverb |
| 12 | `pad_cosmic` | Cinematic outer-space texture | FM alg 0 (stack), slot0 subtractive low, long reverb 8s |

### Pluck (8)

| # | File | Identity | Key techniques |
|---|---|---|---|
| 1 | `pluck` | Clean pluck | Existing — fix keys |
| 2 | `pluck_koto` | Asian koto string character | Saw, BP filter, attack 2ms, decay 400ms, sustain 0.05 |
| 3 | `pluck_fm_steel` | Metallic FM string | FM alg 7 (paired), op ratio 1:1:1:2, fast decay |
| 4 | `pluck_nylon` | Warm nylon guitar | Sine+Triangle, LP 3.5 kHz, attack 3ms, decay 500ms |
| 5 | `pluck_marimba` | Wooden mallet hit | FM alg 6 (additive), ratios 1:4:8:12, fast decay |
| 6 | `pluck_pizzicato` | Short pizzicato pop | Triangle, very fast decay, sub_level 0.3, no FX |
| 7 | `pluck_dulcimer` | Hammered dulcimer ring | FM alg 8 (paired stacks), medium decay, slight reverb |
| 8 | `pluck_harpsi` | Harpsichord-like instant attack | Square, attack 1ms, decay 200ms, LP 5 kHz, no sustain |

### Keys (6)

| # | File | Identity | Key techniques |
|---|---|---|---|
| 1 | `keys` | Simple clean keys | Existing — fix keys |
| 2 | `keys_rhodes_warm` | Warm Rhodes electric piano | FM alg 0, carrier sine with slow bell decay, drive low |
| 3 | `keys_wurli_edge` | Edgier Wurlitzer character | FM alg 1 (feedback), drive 4, LP 3.5 kHz |
| 4 | `keys_fm_organ` | Drawbar-style FM organ | FM alg 6 (additive), all op ratios 1/2/3/4, no envelope decay |
| 5 | `keys_vibes` | Vibraphone metallic sustain | FM alg 2 (two stacks), medium decay, chorus depth low |
| 6 | `keys_bell_chime` | Bell tower chime — long decay | FM alg 0, ratios 1:2:3:7, slow decay 2s, reverb |

### FX (4)

| # | File | Identity | Key techniques |
|---|---|---|---|
| 1 | `fx_pad` | Atmospheric FX texture | Existing — fix keys |
| 2 | `fx_sweep_rise` | Slow filter sweep from dark to bright | Env2 to Cutoff, long decay 6s, low start, reverb |
| 3 | `fx_glitch_bell` | Metallic FM glitch hit | FM alg 4 (branching), feedback 0.9 on op3, fast envelope |
| 4 | `fx_alien_texture` | Pitch-modulated atonal texture | LFO1 (SmoothRandom) to Pitch, BP filter, slow rate |

---

## Implementation plan

### Phase 1 — Fix existing 7 presets
Correct all `osc_1_level` → `osc1_level`, `osc_2_level` → `osc2_level`, etc. keys in the 7 existing `.tsmith` files. Also improve their parameter values now that the engine is fully built (the originals were placeholders).

### Phase 2 — Author new presets (53 files)
Write each `.tsmith` file following the identity and technique column above.
Author in category order: Bass → Lead → Pad → Pluck → Keys → FX.

### Phase 3 — Wire into factory.rs
Add `include_str!` entries for every new file in `factory.rs`. Keep them sorted by category (all Bass together, etc.) for readability.

### Phase 4 — QA listen
For each preset: hold a middle-C note for 10+ seconds, check for clicks, NaNs, runaway amplitude, or audible denormals on release. Check that the preset name matches the sonic character.

---

## Out of scope

| Item | Where |
|---|---|
| Per-preset MIDI Learn default bindings | Post-M14 |
| Preset descriptions longer than ~80 chars | M14 ships short ones; editorial pass is v1.1 |
| Expansion bank (~120 presets) | v1.1 |
| Noise waveform presets | Waveform::Noise not in the engine yet |
