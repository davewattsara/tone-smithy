//! Modulation matrix — 8-slot source → destination routing.
//!
//! Each [`ModSlot`] connects one [`ModSource`] to one [`ModDest`] with a
//! signed `amount`. An optional `via` source scales the amount so that, for
//! example, a mod-wheel can control LFO depth.
//!
//! [`ModMatrix::compute_offsets`] is called once per block per voice (after
//! the LFO / Env2 advance step) and returns a [`DestOffsets`] struct of
//! additive corrections that the caller applies to [`crate::SampleParams`]
//! before the per-sample inner loop.
//!
//! All types are `Copy`; no heap allocation occurs on the hot path.

/// A modulation source value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ModSource {
    /// No source — evaluates to 0.0. Used as the default and as a sentinel
    /// meaning "via scaling disabled" when placed in the `via` field.
    #[default]
    Off,
    /// LFO 1 output, -1..=1.
    Lfo1,
    /// LFO 2 output, -1..=1.
    Lfo2,
    /// Mod envelope (Env2) output, 0..=1.
    Env2,
    /// Amp envelope current level, 0..=1. Sampled once per block.
    AmpEnv,
    /// MIDI velocity captured at note-on, 0..=1.
    Velocity,
    /// Key tracking: `(midi_note - 60) / 60`, giving 0 at middle C,
    /// roughly -1..=1 across the playable range.
    KeyTracking,
    /// MIDI mod wheel, 0..=1.
    ModWheel,
    /// MIDI channel aftertouch, 0..=1.
    Aftertouch,
    /// MIDI pitch bend, -1..=1.
    PitchBend,
}

impl ModSource {
    /// Total number of variants; used to validate parameter bus values.
    pub const COUNT: u8 = 10;

    /// Converts a `u8` index (as stored in the parameter bus) to a variant.
    /// Returns `None` if the index is out of range.
    pub fn from_index(i: u8) -> Option<Self> {
        match i {
            0 => Some(Self::Off),
            1 => Some(Self::Lfo1),
            2 => Some(Self::Lfo2),
            3 => Some(Self::Env2),
            4 => Some(Self::AmpEnv),
            5 => Some(Self::Velocity),
            6 => Some(Self::KeyTracking),
            7 => Some(Self::ModWheel),
            8 => Some(Self::Aftertouch),
            9 => Some(Self::PitchBend),
            _ => None,
        }
    }

    /// Returns the index used on the parameter bus.
    pub fn to_index(self) -> u8 {
        self as u8
    }
}

/// A modulation destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ModDest {
    /// Additive offset to filter cutoff, in Hz.
    #[default]
    FilterCutoffHz,
    /// Additive offset to filter resonance, 0..=1 units.
    FilterResonance,
    /// Additive offset to pitch, in semitones.
    PitchSemis,
    /// Additive scale applied to amp envelope × velocity.
    /// Effective amplitude = `(env * vel * (1 + offset)).clamp(0, 1)`.
    Volume,
    /// Additive offset to oscillator 1 detune, in cents.
    Osc1DetuneCents,
    /// Additive offset to oscillator 1 pan, -1..=1 units.
    Osc1Pan,
}

impl ModDest {
    /// Total number of variants.
    pub const COUNT: u8 = 6;

    /// Converts a `u8` index to a variant.
    pub fn from_index(i: u8) -> Option<Self> {
        match i {
            0 => Some(Self::FilterCutoffHz),
            1 => Some(Self::FilterResonance),
            2 => Some(Self::PitchSemis),
            3 => Some(Self::Volume),
            4 => Some(Self::Osc1DetuneCents),
            5 => Some(Self::Osc1Pan),
            _ => None,
        }
    }

    /// Returns the index used on the parameter bus.
    pub fn to_index(self) -> u8 {
        self as u8
    }
}

/// Per-voice modulation source values, built once per block by the voice
/// manager and passed to [`ModMatrix::compute_offsets`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ModSources {
    pub lfo1: f32,
    pub lfo2: f32,
    pub env2: f32,
    pub amp_env: f32,
    pub velocity: f32,
    pub key_tracking: f32,
    pub mod_wheel: f32,
    pub aftertouch: f32,
    pub pitch_bend: f32,
}

impl ModSources {
    fn get(&self, source: ModSource) -> f32 {
        match source {
            ModSource::Off => 0.0,
            ModSource::Lfo1 => self.lfo1,
            ModSource::Lfo2 => self.lfo2,
            ModSource::Env2 => self.env2,
            ModSource::AmpEnv => self.amp_env,
            ModSource::Velocity => self.velocity,
            ModSource::KeyTracking => self.key_tracking,
            ModSource::ModWheel => self.mod_wheel,
            ModSource::Aftertouch => self.aftertouch,
            ModSource::PitchBend => self.pitch_bend,
        }
    }
}

/// Accumulated destination offsets returned by [`ModMatrix::compute_offsets`].
/// All fields are additive corrections; the caller applies clamping.
#[derive(Clone, Copy, Debug, Default)]
pub struct DestOffsets {
    pub filter_cutoff_hz: f32,
    pub filter_resonance: f32,
    pub pitch_semis: f32,
    pub volume: f32,
    pub osc1_detune_cents: f32,
    pub osc1_pan: f32,
}

/// One routing slot: source → destination with a signed amount and optional
/// via scaling.
#[derive(Clone, Copy, Debug)]
pub struct ModSlot {
    pub enabled: bool,
    pub source: ModSource,
    pub dest: ModDest,
    /// Signed amount in destination-natural units (Hz, semitones, 0..1, etc.).
    pub amount: f32,
    /// When not `Off`, scales the amount by this source's value.
    pub via: ModSource,
}

impl Default for ModSlot {
    fn default() -> Self {
        Self {
            enabled: false,
            source: ModSource::Off,
            dest: ModDest::FilterCutoffHz,
            amount: 0.0,
            via: ModSource::Off,
        }
    }
}

/// 8-slot modulation matrix.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModMatrix {
    pub slots: [ModSlot; 8],
}

impl ModMatrix {
    /// Evaluates all enabled slots against `sources` and returns the summed
    /// destination offsets. No heap allocation; all arithmetic is on the stack.
    pub fn compute_offsets(&self, sources: &ModSources) -> DestOffsets {
        let mut out = DestOffsets::default();
        for slot in &self.slots {
            if !slot.enabled {
                continue;
            }
            let src_val = sources.get(slot.source);
            let via_val = match slot.via {
                ModSource::Off => 1.0,
                v => sources.get(v),
            };
            let contribution = src_val * slot.amount * via_val;
            match slot.dest {
                ModDest::FilterCutoffHz => out.filter_cutoff_hz += contribution,
                ModDest::FilterResonance => out.filter_resonance += contribution,
                ModDest::PitchSemis => out.pitch_semis += contribution,
                ModDest::Volume => out.volume += contribution,
                ModDest::Osc1DetuneCents => out.osc1_detune_cents += contribution,
                ModDest::Osc1Pan => out.osc1_pan += contribution,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources_with(f: impl Fn(&mut ModSources)) -> ModSources {
        let mut s = ModSources::default();
        f(&mut s);
        s
    }

    fn single_slot(source: ModSource, dest: ModDest, amount: f32, via: ModSource) -> ModMatrix {
        let mut m = ModMatrix::default();
        m.slots[0] = ModSlot {
            enabled: true,
            source,
            dest,
            amount,
            via,
        };
        m
    }

    // ── Source routing ───────────────────────────────────────────────────────

    #[test]
    fn lfo1_to_filter_cutoff() {
        let m = single_slot(ModSource::Lfo1, ModDest::FilterCutoffHz, 1000.0, ModSource::Off);
        let s = sources_with(|s| s.lfo1 = 0.5);
        assert!((m.compute_offsets(&s).filter_cutoff_hz - 500.0).abs() < 1e-4);
    }

    #[test]
    fn lfo2_to_pitch() {
        let m = single_slot(ModSource::Lfo2, ModDest::PitchSemis, 12.0, ModSource::Off);
        let s = sources_with(|s| s.lfo2 = -1.0);
        assert!((m.compute_offsets(&s).pitch_semis - -12.0).abs() < 1e-4);
    }

    #[test]
    fn env2_to_filter_cutoff() {
        let m = single_slot(ModSource::Env2, ModDest::FilterCutoffHz, 5000.0, ModSource::Off);
        let s = sources_with(|s| s.env2 = 0.8);
        assert!((m.compute_offsets(&s).filter_cutoff_hz - 4000.0).abs() < 1e-4);
    }

    #[test]
    fn amp_env_to_volume() {
        let m = single_slot(ModSource::AmpEnv, ModDest::Volume, 1.0, ModSource::Off);
        let s = sources_with(|s| s.amp_env = 0.5);
        assert!((m.compute_offsets(&s).volume - 0.5).abs() < 1e-4);
    }

    #[test]
    fn velocity_to_volume() {
        let m = single_slot(ModSource::Velocity, ModDest::Volume, 1.0, ModSource::Off);
        let s = sources_with(|s| s.velocity = 0.75);
        assert!((m.compute_offsets(&s).volume - 0.75).abs() < 1e-4);
    }

    #[test]
    fn key_tracking_to_filter_cutoff() {
        let m = single_slot(ModSource::KeyTracking, ModDest::FilterCutoffHz, 6000.0, ModSource::Off);
        // Middle C (note 60) → key_tracking = 0 → no offset
        let s = sources_with(|s| s.key_tracking = 0.0);
        assert!((m.compute_offsets(&s).filter_cutoff_hz).abs() < 1e-4);
        // One octave up (note 72) → key_tracking = 12/60 = 0.2
        let s2 = sources_with(|s| s.key_tracking = 0.2);
        assert!((m.compute_offsets(&s2).filter_cutoff_hz - 1200.0).abs() < 1e-4);
    }

    #[test]
    fn mod_wheel_to_filter_cutoff() {
        let m = single_slot(ModSource::ModWheel, ModDest::FilterCutoffHz, 2000.0, ModSource::Off);
        let s = sources_with(|s| s.mod_wheel = 1.0);
        assert!((m.compute_offsets(&s).filter_cutoff_hz - 2000.0).abs() < 1e-4);
    }

    #[test]
    fn aftertouch_to_filter_resonance() {
        let m = single_slot(ModSource::Aftertouch, ModDest::FilterResonance, 0.5, ModSource::Off);
        let s = sources_with(|s| s.aftertouch = 0.4);
        assert!((m.compute_offsets(&s).filter_resonance - 0.2).abs() < 1e-4);
    }

    #[test]
    fn pitch_bend_to_pitch() {
        let m = single_slot(ModSource::PitchBend, ModDest::PitchSemis, 2.0, ModSource::Off);
        let s = sources_with(|s| s.pitch_bend = -1.0);
        assert!((m.compute_offsets(&s).pitch_semis - -2.0).abs() < 1e-4);
    }

    // ── Destinations ─────────────────────────────────────────────────────────

    #[test]
    fn osc1_detune_cents() {
        let m = single_slot(ModSource::Lfo1, ModDest::Osc1DetuneCents, 100.0, ModSource::Off);
        let s = sources_with(|s| s.lfo1 = 0.5);
        assert!((m.compute_offsets(&s).osc1_detune_cents - 50.0).abs() < 1e-4);
    }

    #[test]
    fn osc1_pan() {
        let m = single_slot(ModSource::Lfo2, ModDest::Osc1Pan, 1.0, ModSource::Off);
        let s = sources_with(|s| s.lfo2 = 0.3);
        assert!((m.compute_offsets(&s).osc1_pan - 0.3).abs() < 1e-4);
    }

    // ── Via scaling ──────────────────────────────────────────────────────────

    #[test]
    fn via_mod_wheel_scales_amount() {
        // Canonical done-when test: LFO1 → cutoff via mod wheel
        let m = single_slot(ModSource::Lfo1, ModDest::FilterCutoffHz, 4000.0, ModSource::ModWheel);
        // Wheel at zero: no modulation
        let s0 = sources_with(|s| {
            s.lfo1 = 1.0;
            s.mod_wheel = 0.0;
        });
        assert!((m.compute_offsets(&s0).filter_cutoff_hz).abs() < 1e-4);
        // Wheel at half: half depth
        let s1 = sources_with(|s| {
            s.lfo1 = 1.0;
            s.mod_wheel = 0.5;
        });
        assert!((m.compute_offsets(&s1).filter_cutoff_hz - 2000.0).abs() < 1e-4);
        // Wheel at full: full depth
        let s2 = sources_with(|s| {
            s.lfo1 = 1.0;
            s.mod_wheel = 1.0;
        });
        assert!((m.compute_offsets(&s2).filter_cutoff_hz - 4000.0).abs() < 1e-4);
    }

    // ── Slot enable / disable ────────────────────────────────────────────────

    #[test]
    fn disabled_slot_produces_no_offset() {
        let mut m = single_slot(ModSource::Lfo1, ModDest::FilterCutoffHz, 5000.0, ModSource::Off);
        m.slots[0].enabled = false;
        let s = sources_with(|s| s.lfo1 = 1.0);
        assert_eq!(m.compute_offsets(&s).filter_cutoff_hz, 0.0);
    }

    // ── Multi-slot summing ───────────────────────────────────────────────────

    #[test]
    fn two_slots_sum_to_same_dest() {
        let mut m = ModMatrix::default();
        m.slots[0] = ModSlot {
            enabled: true,
            source: ModSource::Lfo1,
            dest: ModDest::FilterCutoffHz,
            amount: 1000.0,
            via: ModSource::Off,
        };
        m.slots[1] = ModSlot {
            enabled: true,
            source: ModSource::Env2,
            dest: ModDest::FilterCutoffHz,
            amount: 2000.0,
            via: ModSource::Off,
        };
        let s = sources_with(|s| {
            s.lfo1 = 0.5;
            s.env2 = 1.0;
        });
        // 0.5*1000 + 1.0*2000 = 2500
        assert!((m.compute_offsets(&s).filter_cutoff_hz - 2500.0).abs() < 1e-4);
    }

    // ── Off source ───────────────────────────────────────────────────────────

    #[test]
    fn off_source_is_zero() {
        let m = single_slot(ModSource::Off, ModDest::FilterCutoffHz, 9999.0, ModSource::Off);
        let s = ModSources::default();
        assert_eq!(m.compute_offsets(&s).filter_cutoff_hz, 0.0);
    }

    // ── Index round-trips ────────────────────────────────────────────────────

    #[test]
    fn mod_source_index_round_trip() {
        for i in 0..ModSource::COUNT {
            let s = ModSource::from_index(i).unwrap();
            assert_eq!(s.to_index(), i);
        }
        assert!(ModSource::from_index(ModSource::COUNT).is_none());
    }

    #[test]
    fn mod_dest_index_round_trip() {
        for i in 0..ModDest::COUNT {
            let d = ModDest::from_index(i).unwrap();
            assert_eq!(d.to_index(), i);
        }
        assert!(ModDest::from_index(ModDest::COUNT).is_none());
    }
}
