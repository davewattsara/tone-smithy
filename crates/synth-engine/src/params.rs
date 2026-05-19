//! The engine's parameter tree and outward-facing snapshot.
//!
//! The tree is the single source of truth for sound-affecting state, per
//! `docs/planning/03-architecture/design-patterns.md` §1.3. The UI never
//! mutates parameters directly — every change arrives as an
//! [`EngineEvent`] and is applied here. Each block the engine reads the
//! tree to produce an immutable [`ParamSnapshot`] for the UI.
//!
//! Continuous parameters fall into two flavours:
//!
//! - **Smoothed** — read every sample by the audio path, so a UI step
//!   would cause a click. The tree owns a [`SmoothedParam`] and the
//!   audio thread advances it per sample (§2.6).
//! - **Stepped** — read only on edge transitions (e.g. amp release time
//!   is sampled when the envelope enters its release phase), so an
//!   instantaneous change has no audible artefact. Stored as a plain
//!   `f32` and pushed to the consuming DSP component on the event.
//!
//! Discrete parameters (waveform, filter mode, FM algorithm) have their
//! own typed event variants and live as enum fields on the tree;
//! changes take effect at the next block boundary per §2.7.
//!
//! [`EngineEvent`]: crate::EngineEvent

use crate::oscillator::Waveform;
use crate::smoothing::SmoothedParam;

/// Default amp envelope release time, in seconds.
const DEFAULT_AMP_RELEASE_SECS: f32 = 0.200;

/// Identifies a continuous parameter for [`EngineEvent::ParameterChange`].
///
/// Discrete parameters (e.g. waveform) have their own typed
/// `EngineEvent` variants so the value type is checked at compile time
/// rather than reinterpreted from `f32`.
///
/// Ids are stable: once shipped in a preset, a variant's discriminant
/// and meaning do not change. New parameters get new variants appended.
///
/// [`EngineEvent::ParameterChange`]: crate::EngineEvent::ParameterChange
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamId {
    /// Pitch offset applied on top of held MIDI note, in semitones.
    /// Range -24..=24 by convention; the engine does not clamp.
    PitchOffsetSemis,

    /// Amp envelope release time, in seconds. Range 0.001..=10.0 by
    /// convention; the envelope clamps below one sample period.
    AmpReleaseSecs,
}

/// An immutable snapshot of the engine's outward-facing parameter
/// state, published once per audio block.
///
/// Built by [`ParameterTree::snapshot`] without allocating. The audio
/// callback is responsible for wrapping it in an `Arc` and storing in
/// the snapshot slot — the recycled-pool optimisation from
/// `docs/planning/03-architecture/design-patterns.md` §2.5 is a later
/// milestone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSnapshot {
    /// Current pitch offset, in semitones.
    pub pitch_offset_semis: f32,

    /// Current amp release time, in seconds.
    pub amp_release_secs: f32,

    /// Current oscillator waveform.
    pub waveform: Waveform,

    /// True if a voice is currently producing audio (not idle).
    pub voice_active: bool,
}

impl Default for ParamSnapshot {
    fn default() -> Self {
        Self {
            pitch_offset_semis: 0.0,
            amp_release_secs: DEFAULT_AMP_RELEASE_SECS,
            waveform: Waveform::Sine,
            voice_active: false,
        }
    }
}

/// The engine's typed parameter tree.
///
/// Owns every sound-affecting parameter the engine exposes. Mutation
/// happens through [`set_continuous`](Self::set_continuous),
/// [`set_waveform`](Self::set_waveform), and similar typed setters; the
/// crate-private `set_voice_active` lets the engine reflect runtime
/// voice state into the next snapshot.
///
/// The tree itself does no I/O and holds no audio DSP. It is the place
/// where parameter validation, smoothing, and snapshot building live so
/// that adding a new parameter is one match arm rather than a new
/// engine field.
pub struct ParameterTree {
    pitch_offset_semis: SmoothedParam,
    amp_release_secs: f32,
    waveform: Waveform,
    voice_active: bool,
}

impl ParameterTree {
    /// Creates a tree initialised to [`ParamSnapshot::default`] values
    /// at the given sample rate. The sample rate is captured by the
    /// smoothers and fixed for the tree's lifetime.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        let defaults = ParamSnapshot::default();
        Self {
            pitch_offset_semis: SmoothedParam::new(defaults.pitch_offset_semis, sample_rate_hz),
            amp_release_secs: defaults.amp_release_secs,
            waveform: defaults.waveform,
            voice_active: defaults.voice_active,
        }
    }

    /// Applies a continuous parameter change. Smoothed params receive a
    /// new target (per-sample interpolation runs in the audio path);
    /// stepped params latch immediately so the next consumer-side read
    /// sees the new value.
    pub fn set_continuous(&mut self, id: ParamId, value: f32) {
        match id {
            ParamId::PitchOffsetSemis => self.pitch_offset_semis.set_target(value),
            ParamId::AmpReleaseSecs => self.amp_release_secs = value,
        }
    }

    /// Sets the oscillator waveform. Discrete: takes effect at the next
    /// block boundary per design-patterns.md §2.7.
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    /// Mirrors the engine's voice activity into the next snapshot. Not
    /// driven by an `EngineEvent` — the engine writes this each block
    /// before publishing.
    pub fn set_voice_active(&mut self, active: bool) {
        self.voice_active = active;
    }

    /// Snaps smoothed params that should jump to their target on
    /// note-on so the first sample of a new note plays exactly at the
    /// current target value rather than mid-glide.
    pub fn snap_for_note_on(&mut self) {
        self.pitch_offset_semis.snap_to_target();
    }

    /// Returns the current waveform.
    #[must_use]
    pub fn waveform(&self) -> Waveform {
        self.waveform
    }

    /// Returns the current amp release time, in seconds.
    #[must_use]
    pub fn amp_release_secs(&self) -> f32 {
        self.amp_release_secs
    }

    /// Advances all smoothed parameters by one sample and returns the
    /// values the audio path consumes per sample. Called once per frame
    /// inside `Engine::process_stereo`.
    pub fn next_sample(&mut self) -> SampleParams {
        SampleParams {
            pitch_offset_semis: self.pitch_offset_semis.next_sample(),
        }
    }

    /// Builds an outward-facing snapshot without allocating. Each field
    /// reads the current (smoothed) value, so the UI sees what the
    /// audio path is hearing right now.
    #[must_use]
    pub fn snapshot(&self) -> ParamSnapshot {
        ParamSnapshot {
            pitch_offset_semis: self.pitch_offset_semis.current(),
            amp_release_secs: self.amp_release_secs,
            waveform: self.waveform,
            voice_active: self.voice_active,
        }
    }
}

/// Per-sample smoothed parameter values consumed by the audio path.
///
/// Returned by [`ParameterTree::next_sample`] once per frame. Grown
/// field-by-field as new smoothed params land in later milestones —
/// keeping it a flat struct (vs. a map) means each consumer reads the
/// exact field it needs with no lookup cost.
#[derive(Debug, Clone, Copy)]
pub struct SampleParams {
    /// Pitch offset to apply on top of any held MIDI note, in
    /// semitones.
    pub pitch_offset_semis: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_matches_default() {
        let tree = ParameterTree::new(48_000.0);
        let snap = tree.snapshot();
        assert_eq!(snap, ParamSnapshot::default());
    }

    #[test]
    fn set_continuous_pitch_offset_smooths_toward_target() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::PitchOffsetSemis, 12.0);
        // First sample is far from target — smoothing must not jump.
        let first = tree.next_sample().pitch_offset_semis;
        assert!(first.abs() < 0.5, "expected gradual rise, got {first}");
    }

    #[test]
    fn snap_for_note_on_skips_smoothing() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::PitchOffsetSemis, 7.0);
        tree.snap_for_note_on();
        let sample = tree.next_sample().pitch_offset_semis;
        assert!((sample - 7.0).abs() < 1e-3, "expected snap to 7.0, got {sample}");
    }

    #[test]
    fn set_continuous_amp_release_latches_immediately() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::AmpReleaseSecs, 1.5);
        // Stepped — the value is visible without advancing any sample.
        assert_eq!(tree.amp_release_secs(), 1.5);
        assert_eq!(tree.snapshot().amp_release_secs, 1.5);
    }

    #[test]
    fn waveform_is_visible_in_snapshot() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_waveform(Waveform::Saw);
        assert_eq!(tree.waveform(), Waveform::Saw);
        assert_eq!(tree.snapshot().waveform, Waveform::Saw);
    }

    #[test]
    fn voice_active_mirror_is_published() {
        let mut tree = ParameterTree::new(48_000.0);
        assert!(!tree.snapshot().voice_active);
        tree.set_voice_active(true);
        assert!(tree.snapshot().voice_active);
    }
}
