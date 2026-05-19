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

use crate::MAIN_OSCILLATOR_COUNT;
use crate::filter::FilterMode;
use crate::oscillator::Waveform;
use crate::smoothing::SmoothedParam;

/// Default amp envelope release time, in seconds.
const DEFAULT_AMP_RELEASE_SECS: f32 = 0.200;

/// Default filter cutoff frequency, in Hz. Sits well above the
/// fundamental of every playable MIDI note, so a fresh patch is
/// effectively wide open until the user turns the knob down.
const DEFAULT_FILTER_CUTOFF_HZ: f32 = 8_000.0;

/// Default filter resonance, on the 0..=1 user-facing scale. Zero is
/// the maximally damped end of the range — no peak at all.
const DEFAULT_FILTER_RESONANCE: f32 = 0.0;

/// Default per-oscillator level. All four oscillators arrive at unity;
/// the slot mixer's headroom scale handles the worst-case in-phase
/// sum without clipping.
const DEFAULT_OSC_LEVEL: f32 = 1.0;

/// Default per-oscillator detune, in cents. Zero detune = exactly on
/// pitch with the held note.
const DEFAULT_OSC_DETUNE_CENTS: f32 = 0.0;

/// Default per-oscillator pan position. Centered.
const DEFAULT_OSC_PAN: f32 = 0.0;

/// Default unison voice count per main oscillator. `1` means unison
/// is effectively off — the bank behaves like a single oscillator
/// and unison detune / spread are inert.
const DEFAULT_UNISON_VOICES: f32 = 1.0;

/// Default unison detune in cents. Subtle enough not to be obvious if
/// the user enables unison without touching the detune knob, but
/// large enough to actually beat audibly between voices.
const DEFAULT_UNISON_DETUNE_CENTS: f32 = 10.0;

/// Default unison stereo spread (0..=1). Half spread is musical when
/// the user first enables unison without dialling in the width
/// explicitly.
const DEFAULT_UNISON_SPREAD: f32 = 0.5;

/// Identifies a continuous parameter for [`EngineEvent::ParameterChange`].
///
/// Discrete parameters (e.g. waveform, filter mode) have their own
/// typed `EngineEvent` variants so the value type is checked at
/// compile time rather than reinterpreted from `f32`.
///
/// Ids are stable: once shipped in a preset, a variant's discriminant
/// and meaning do not change. New parameters get new variants
/// appended.
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

    /// Filter cutoff frequency, in Hz. Range 20..=~Nyquist; the SVF
    /// clamps internally.
    FilterCutoffHz,

    /// Filter resonance on a 0..=1 scale, mapped internally to a
    /// musically useful Q range. Values outside 0..=1 are clamped.
    FilterResonance,

    /// Main oscillator 1 level (0..=1).
    Osc1Level,
    /// Main oscillator 2 level (0..=1).
    Osc2Level,
    /// Main oscillator 3 level (0..=1).
    Osc3Level,
    /// Sub oscillator level (0..=1).
    SubLevel,

    /// Main oscillator 1 detune, in cents (±100 = one semitone).
    Osc1DetuneCents,
    /// Main oscillator 2 detune, in cents.
    Osc2DetuneCents,
    /// Main oscillator 3 detune, in cents.
    Osc3DetuneCents,

    /// Main oscillator 1 pan position (-1 = full left, +1 = full
    /// right). Equal-power.
    Osc1Pan,
    /// Main oscillator 2 pan position.
    Osc2Pan,
    /// Main oscillator 3 pan position.
    Osc3Pan,
    /// Sub oscillator pan position.
    SubPan,

    /// Main oscillator 1 unison voice count, treated as an integer
    /// 1..=MAX_UNISON_VOICES (rounded and clamped when consumed).
    Osc1UnisonVoices,
    /// Main oscillator 2 unison voice count.
    Osc2UnisonVoices,
    /// Main oscillator 3 unison voice count.
    Osc3UnisonVoices,

    /// Main oscillator 1 unison detune width, in cents. Voices spread
    /// across `[-detune, +detune]`.
    Osc1UnisonDetuneCents,
    /// Main oscillator 2 unison detune width, in cents.
    Osc2UnisonDetuneCents,
    /// Main oscillator 3 unison detune width, in cents.
    Osc3UnisonDetuneCents,

    /// Main oscillator 1 unison stereo spread (0..=1). Voices spread
    /// across the stereo field around the per-osc pan.
    Osc1UnisonSpread,
    /// Main oscillator 2 unison stereo spread (0..=1).
    Osc2UnisonSpread,
    /// Main oscillator 3 unison stereo spread (0..=1).
    Osc3UnisonSpread,
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

    /// Current oscillator waveform (applied to all three main
    /// oscillators; the sub is always sine).
    pub waveform: Waveform,

    /// Current filter cutoff frequency, in Hz.
    pub filter_cutoff_hz: f32,

    /// Current filter resonance on the 0..=1 user scale.
    pub filter_resonance: f32,

    /// Current filter output mode.
    pub filter_mode: FilterMode,

    /// Per-main-oscillator levels (0..=1), indexed as the voice
    /// indexes its main oscillators.
    pub osc_main_levels: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator level (0..=1).
    pub sub_level: f32,

    /// Per-main-oscillator detune in cents.
    pub osc_main_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator pan position (-1..=1).
    pub osc_main_pans: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator pan position (-1..=1).
    pub sub_pan: f32,

    /// Per-main-oscillator unison voice count (1..=MAX_UNISON_VOICES,
    /// stored as f32 because the parameter bus carries f32 values;
    /// rounded and clamped at the consumer).
    pub osc_main_unison_voices: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison detune width, in cents.
    pub osc_main_unison_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison stereo spread (0..=1).
    pub osc_main_unison_spreads: [f32; MAIN_OSCILLATOR_COUNT],

    /// Number of voices currently producing audio (not idle). Zero means
    /// the engine is silent. At M2 this is 0 or 1; the voice manager
    /// at M3 raises the ceiling to 32.
    pub active_voice_count: u8,
}

impl Default for ParamSnapshot {
    fn default() -> Self {
        Self {
            pitch_offset_semis: 0.0,
            amp_release_secs: DEFAULT_AMP_RELEASE_SECS,
            waveform: Waveform::Sine,
            filter_cutoff_hz: DEFAULT_FILTER_CUTOFF_HZ,
            filter_resonance: DEFAULT_FILTER_RESONANCE,
            filter_mode: FilterMode::LowPass,
            osc_main_levels: [DEFAULT_OSC_LEVEL; MAIN_OSCILLATOR_COUNT],
            sub_level: DEFAULT_OSC_LEVEL,
            osc_main_detune_cents: [DEFAULT_OSC_DETUNE_CENTS; MAIN_OSCILLATOR_COUNT],
            osc_main_pans: [DEFAULT_OSC_PAN; MAIN_OSCILLATOR_COUNT],
            sub_pan: DEFAULT_OSC_PAN,
            osc_main_unison_voices: [DEFAULT_UNISON_VOICES; MAIN_OSCILLATOR_COUNT],
            osc_main_unison_detune_cents: [DEFAULT_UNISON_DETUNE_CENTS; MAIN_OSCILLATOR_COUNT],
            osc_main_unison_spreads: [DEFAULT_UNISON_SPREAD; MAIN_OSCILLATOR_COUNT],
            active_voice_count: 0,
        }
    }
}

/// The engine's typed parameter tree.
///
/// Owns every sound-affecting parameter the engine exposes. Mutation
/// happens through [`set_continuous`](Self::set_continuous),
/// [`set_waveform`](Self::set_waveform), and similar typed setters; the
/// crate-private `set_active_voice_count` lets the engine reflect runtime
/// voice state into the next snapshot.
///
/// The tree itself does no I/O and holds no audio DSP. It is the place
/// where parameter validation, smoothing, and snapshot building live so
/// that adding a new parameter is one match arm rather than a new
/// engine field.
pub struct ParameterTree {
    pitch_offset_semis: SmoothedParam,
    filter_cutoff_hz: SmoothedParam,
    filter_resonance: SmoothedParam,

    osc_main_levels: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    sub_level: SmoothedParam,

    osc_main_detune_cents: [SmoothedParam; MAIN_OSCILLATOR_COUNT],

    osc_main_pans: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    sub_pan: SmoothedParam,

    osc_main_unison_detune_cents: [SmoothedParam; MAIN_OSCILLATOR_COUNT],
    osc_main_unison_spreads: [SmoothedParam; MAIN_OSCILLATOR_COUNT],

    /// Unison voice counts. Stepped (not smoothed) because the
    /// quantised integer values do not benefit from interpolation —
    /// switching from 3 to 4 voices is meant to be instant, not a
    /// glide through 3.4 voices.
    osc_main_unison_voices: [f32; MAIN_OSCILLATOR_COUNT],

    amp_release_secs: f32,

    waveform: Waveform,
    filter_mode: FilterMode,

    active_voice_count: u8,
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
            filter_cutoff_hz: SmoothedParam::new(defaults.filter_cutoff_hz, sample_rate_hz),
            filter_resonance: SmoothedParam::new(defaults.filter_resonance, sample_rate_hz),
            osc_main_levels: defaults.osc_main_levels.map(|v| SmoothedParam::new(v, sample_rate_hz)),
            sub_level: SmoothedParam::new(defaults.sub_level, sample_rate_hz),
            osc_main_detune_cents: defaults
                .osc_main_detune_cents
                .map(|v| SmoothedParam::new(v, sample_rate_hz)),
            osc_main_pans: defaults.osc_main_pans.map(|v| SmoothedParam::new(v, sample_rate_hz)),
            sub_pan: SmoothedParam::new(defaults.sub_pan, sample_rate_hz),
            osc_main_unison_detune_cents: defaults
                .osc_main_unison_detune_cents
                .map(|v| SmoothedParam::new(v, sample_rate_hz)),
            osc_main_unison_spreads: defaults
                .osc_main_unison_spreads
                .map(|v| SmoothedParam::new(v, sample_rate_hz)),
            osc_main_unison_voices: defaults.osc_main_unison_voices,
            amp_release_secs: defaults.amp_release_secs,
            waveform: defaults.waveform,
            filter_mode: defaults.filter_mode,
            active_voice_count: defaults.active_voice_count,
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
            ParamId::FilterCutoffHz => self.filter_cutoff_hz.set_target(value),
            ParamId::FilterResonance => self.filter_resonance.set_target(value),
            ParamId::Osc1Level => self.osc_main_levels[0].set_target(value),
            ParamId::Osc2Level => self.osc_main_levels[1].set_target(value),
            ParamId::Osc3Level => self.osc_main_levels[2].set_target(value),
            ParamId::SubLevel => self.sub_level.set_target(value),
            ParamId::Osc1DetuneCents => self.osc_main_detune_cents[0].set_target(value),
            ParamId::Osc2DetuneCents => self.osc_main_detune_cents[1].set_target(value),
            ParamId::Osc3DetuneCents => self.osc_main_detune_cents[2].set_target(value),
            ParamId::Osc1Pan => self.osc_main_pans[0].set_target(value),
            ParamId::Osc2Pan => self.osc_main_pans[1].set_target(value),
            ParamId::Osc3Pan => self.osc_main_pans[2].set_target(value),
            ParamId::SubPan => self.sub_pan.set_target(value),
            ParamId::Osc1UnisonVoices => self.osc_main_unison_voices[0] = value,
            ParamId::Osc2UnisonVoices => self.osc_main_unison_voices[1] = value,
            ParamId::Osc3UnisonVoices => self.osc_main_unison_voices[2] = value,
            ParamId::Osc1UnisonDetuneCents => self.osc_main_unison_detune_cents[0].set_target(value),
            ParamId::Osc2UnisonDetuneCents => self.osc_main_unison_detune_cents[1].set_target(value),
            ParamId::Osc3UnisonDetuneCents => self.osc_main_unison_detune_cents[2].set_target(value),
            ParamId::Osc1UnisonSpread => self.osc_main_unison_spreads[0].set_target(value),
            ParamId::Osc2UnisonSpread => self.osc_main_unison_spreads[1].set_target(value),
            ParamId::Osc3UnisonSpread => self.osc_main_unison_spreads[2].set_target(value),
        }
    }

    /// Sets the oscillator waveform. Discrete: takes effect at the next
    /// block boundary per design-patterns.md §2.7.
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    /// Sets the filter output mode. Discrete.
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_mode = mode;
    }

    /// Mirrors the engine's active voice count into the next snapshot.
    /// Not driven by an `EngineEvent` — the engine writes this each
    /// block before publishing. At M2 the value is 0 or 1; the voice
    /// manager at M3 will pass values up to 32.
    pub fn set_active_voice_count(&mut self, count: u8) {
        self.active_voice_count = count;
    }

    /// Snaps smoothed params that should jump to their target on
    /// note-on so the first sample of a new note plays exactly at the
    /// current target value rather than mid-glide. Filter params
    /// deliberately keep smoothing so a note hit does not click the
    /// cutoff. Per-osc level / pan / detune also stay smoothed so a
    /// mid-glide adjustment continues smoothly into the new note.
    pub fn snap_for_note_on(&mut self) {
        self.pitch_offset_semis.snap_to_target();
    }

    /// Returns the current waveform.
    #[must_use]
    pub fn waveform(&self) -> Waveform {
        self.waveform
    }

    /// Returns the current filter mode.
    #[must_use]
    pub fn filter_mode(&self) -> FilterMode {
        self.filter_mode
    }

    /// Returns the current amp release time, in seconds.
    #[must_use]
    pub fn amp_release_secs(&self) -> f32 {
        self.amp_release_secs
    }

    /// Advances all smoothed parameters by one sample and returns the
    /// values the audio path consumes per sample. Called once per
    /// frame inside `Engine::process_stereo`.
    pub fn next_sample(&mut self) -> SampleParams {
        SampleParams {
            pitch_offset_semis: self.pitch_offset_semis.next_sample(),
            filter_cutoff_hz: self.filter_cutoff_hz.next_sample(),
            filter_resonance: self.filter_resonance.next_sample(),
            osc_main_levels: [
                self.osc_main_levels[0].next_sample(),
                self.osc_main_levels[1].next_sample(),
                self.osc_main_levels[2].next_sample(),
            ],
            sub_level: self.sub_level.next_sample(),
            osc_main_detune_cents: [
                self.osc_main_detune_cents[0].next_sample(),
                self.osc_main_detune_cents[1].next_sample(),
                self.osc_main_detune_cents[2].next_sample(),
            ],
            osc_main_pans: [
                self.osc_main_pans[0].next_sample(),
                self.osc_main_pans[1].next_sample(),
                self.osc_main_pans[2].next_sample(),
            ],
            sub_pan: self.sub_pan.next_sample(),
            osc_main_unison_voices: self.osc_main_unison_voices,
            osc_main_unison_detune_cents: [
                self.osc_main_unison_detune_cents[0].next_sample(),
                self.osc_main_unison_detune_cents[1].next_sample(),
                self.osc_main_unison_detune_cents[2].next_sample(),
            ],
            osc_main_unison_spreads: [
                self.osc_main_unison_spreads[0].next_sample(),
                self.osc_main_unison_spreads[1].next_sample(),
                self.osc_main_unison_spreads[2].next_sample(),
            ],
        }
    }

    /// Builds an outward-facing snapshot without allocating. Each
    /// field reads the current (smoothed) value, so the UI sees what
    /// the audio path is hearing right now.
    #[must_use]
    pub fn snapshot(&self) -> ParamSnapshot {
        ParamSnapshot {
            pitch_offset_semis: self.pitch_offset_semis.current(),
            amp_release_secs: self.amp_release_secs,
            waveform: self.waveform,
            filter_cutoff_hz: self.filter_cutoff_hz.current(),
            filter_resonance: self.filter_resonance.current(),
            filter_mode: self.filter_mode,
            osc_main_levels: [
                self.osc_main_levels[0].current(),
                self.osc_main_levels[1].current(),
                self.osc_main_levels[2].current(),
            ],
            sub_level: self.sub_level.current(),
            osc_main_detune_cents: [
                self.osc_main_detune_cents[0].current(),
                self.osc_main_detune_cents[1].current(),
                self.osc_main_detune_cents[2].current(),
            ],
            osc_main_pans: [
                self.osc_main_pans[0].current(),
                self.osc_main_pans[1].current(),
                self.osc_main_pans[2].current(),
            ],
            sub_pan: self.sub_pan.current(),
            osc_main_unison_voices: self.osc_main_unison_voices,
            osc_main_unison_detune_cents: [
                self.osc_main_unison_detune_cents[0].current(),
                self.osc_main_unison_detune_cents[1].current(),
                self.osc_main_unison_detune_cents[2].current(),
            ],
            osc_main_unison_spreads: [
                self.osc_main_unison_spreads[0].current(),
                self.osc_main_unison_spreads[1].current(),
                self.osc_main_unison_spreads[2].current(),
            ],
            active_voice_count: self.active_voice_count,
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

    /// Filter cutoff frequency for this sample, in Hz.
    pub filter_cutoff_hz: f32,

    /// Filter resonance for this sample, on the 0..=1 user scale.
    pub filter_resonance: f32,

    /// Per-main-oscillator levels for this sample.
    pub osc_main_levels: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator level for this sample.
    pub sub_level: f32,

    /// Per-main-oscillator detune in cents for this sample.
    pub osc_main_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator pan positions for this sample.
    pub osc_main_pans: [f32; MAIN_OSCILLATOR_COUNT],
    /// Sub oscillator pan position for this sample.
    pub sub_pan: f32,

    /// Per-main-oscillator unison voice counts. Carried as f32 to
    /// match the parameter bus; the voice rounds and clamps when
    /// consuming. Stepped, so this field's value is constant across
    /// the samples between two `ParameterChange` events.
    pub osc_main_unison_voices: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison detune width in cents for this
    /// sample.
    pub osc_main_unison_detune_cents: [f32; MAIN_OSCILLATOR_COUNT],

    /// Per-main-oscillator unison stereo spread (0..=1) for this
    /// sample.
    pub osc_main_unison_spreads: [f32; MAIN_OSCILLATOR_COUNT],
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
        assert_eq!(tree.amp_release_secs(), 1.5);
        assert_eq!(tree.snapshot().amp_release_secs, 1.5);
    }

    #[test]
    fn set_continuous_filter_params_smooth_toward_target() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::FilterCutoffHz, 2_000.0);
        tree.set_continuous(ParamId::FilterResonance, 0.8);
        let first = tree.next_sample();
        assert!(
            first.filter_cutoff_hz > 4_000.0,
            "cutoff jumped: {}",
            first.filter_cutoff_hz
        );
        assert!(
            first.filter_resonance < 0.5,
            "resonance jumped: {}",
            first.filter_resonance
        );
    }

    #[test]
    fn per_osc_params_route_to_the_right_slot() {
        // Set each per-osc param to a distinct value and confirm the
        // snapshot reflects the right field. Smoothing has not been
        // ticked, so we just verify the targets are dispatched
        // correctly — current() still reads the default until samples
        // advance, so we tick a long way before asserting.
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_continuous(ParamId::Osc1Level, 0.10);
        tree.set_continuous(ParamId::Osc2Level, 0.20);
        tree.set_continuous(ParamId::Osc3Level, 0.30);
        tree.set_continuous(ParamId::SubLevel, 0.40);
        tree.set_continuous(ParamId::Osc1DetuneCents, 11.0);
        tree.set_continuous(ParamId::Osc2DetuneCents, 22.0);
        tree.set_continuous(ParamId::Osc3DetuneCents, 33.0);
        tree.set_continuous(ParamId::Osc1Pan, -0.5);
        tree.set_continuous(ParamId::Osc2Pan, 0.0);
        tree.set_continuous(ParamId::Osc3Pan, 0.5);
        tree.set_continuous(ParamId::SubPan, 0.25);

        // Tick long enough that smoothers reach their targets.
        for _ in 0..8_192 {
            let _ = tree.next_sample();
        }
        let snap = tree.snapshot();
        assert!((snap.osc_main_levels[0] - 0.10).abs() < 1e-3);
        assert!((snap.osc_main_levels[1] - 0.20).abs() < 1e-3);
        assert!((snap.osc_main_levels[2] - 0.30).abs() < 1e-3);
        assert!((snap.sub_level - 0.40).abs() < 1e-3);
        assert!((snap.osc_main_detune_cents[0] - 11.0).abs() < 1e-2);
        assert!((snap.osc_main_detune_cents[1] - 22.0).abs() < 1e-2);
        assert!((snap.osc_main_detune_cents[2] - 33.0).abs() < 1e-2);
        assert!((snap.osc_main_pans[0] - -0.5).abs() < 1e-3);
        assert!((snap.osc_main_pans[1] - 0.0).abs() < 1e-3);
        assert!((snap.osc_main_pans[2] - 0.5).abs() < 1e-3);
        assert!((snap.sub_pan - 0.25).abs() < 1e-3);
    }

    #[test]
    fn filter_mode_is_visible_in_snapshot() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_filter_mode(FilterMode::BandPass);
        assert_eq!(tree.filter_mode(), FilterMode::BandPass);
        assert_eq!(tree.snapshot().filter_mode, FilterMode::BandPass);
    }

    #[test]
    fn waveform_is_visible_in_snapshot() {
        let mut tree = ParameterTree::new(48_000.0);
        tree.set_waveform(Waveform::Saw);
        assert_eq!(tree.waveform(), Waveform::Saw);
        assert_eq!(tree.snapshot().waveform, Waveform::Saw);
    }

    #[test]
    fn active_voice_count_is_published() {
        let mut tree = ParameterTree::new(48_000.0);
        assert_eq!(tree.snapshot().active_voice_count, 0);
        tree.set_active_voice_count(3);
        assert_eq!(tree.snapshot().active_voice_count, 3);
    }
}
