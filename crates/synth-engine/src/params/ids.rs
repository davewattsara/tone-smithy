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

    /// Amp envelope attack time, in seconds. Range 0.001..=10.0 by
    /// convention; the envelope clamps below one sample period.
    AmpAttackSecs,

    /// Amp envelope decay time, in seconds. Same range as attack.
    AmpDecaySecs,

    /// Amp envelope sustain level, 0..=1.
    AmpSustainLevel,

    /// Master output volume, 0..=1. Smoothed to prevent clicks when
    /// the user moves the knob. Applied after polyphony summing.
    MasterVolume,

    /// Pitch-bend wheel position converted to semitones. The engine
    /// scales the normalised -1..1 wheel value by
    /// [`crate::engine::PITCH_BEND_RANGE_SEMIS`] before writing here.
    PitchBendSemis,

    /// Mod wheel (MIDI CC #1), normalised 0..=1. Not yet wired to a
    /// destination; stored so M6 can route it without an API change.
    ModWheel,

    /// Channel aftertouch, normalised 0..=1. Same M6 rationale as
    /// `ModWheel`.
    ChannelAftertouch,

    // ── LFO 1 ──────────────────────────────────────────────────────────
    /// LFO1 rate in Hz when sync is off. Range 0.01..=20.0. Stepped.
    Lfo1RateHz,
    /// LFO1 waveform shape; value is the zero-based `LfoShape` index.
    Lfo1Shape,
    /// LFO1 phase-reset on note-on; 0.0 = off, 1.0 = on. Stepped.
    Lfo1ResetOnNoteOn,
    /// LFO1 BPM-sync enable; 0.0 = free, 1.0 = synced. Stepped.
    Lfo1SyncEnabled,
    /// LFO1 BPM-sync division; value is the zero-based `SyncDivision`
    /// index. Only used when sync is enabled.
    Lfo1SyncDivision,
    /// LFO1 global (mono) mode; 0.0 = per-voice, 1.0 = one shared instance
    /// across all voices. Stepped.
    Lfo1Global,

    // ── LFO 2 ──────────────────────────────────────────────────────────
    /// LFO2 rate in Hz when sync is off.
    Lfo2RateHz,
    /// LFO2 waveform shape index.
    Lfo2Shape,
    /// LFO2 phase-reset on note-on.
    Lfo2ResetOnNoteOn,
    /// LFO2 BPM-sync enable.
    Lfo2SyncEnabled,
    /// LFO2 BPM-sync division index.
    Lfo2SyncDivision,
    /// LFO2 global (mono) mode; 0.0 = per-voice, 1.0 = one shared instance.
    Lfo2Global,

    // ── Env2 (modulation envelope) ─────────────────────────────────────
    /// Env2 attack time, in seconds.
    Env2AttackSecs,
    /// Env2 decay time, in seconds.
    Env2DecaySecs,
    /// Env2 sustain level, 0..=1.
    Env2SustainLevel,
    /// Env2 release time, in seconds.
    Env2ReleaseSecs,
    /// Env2 Attack stage curve, -1..=1.
    Env2AttackCurve,
    /// Env2 Decay stage curve, -1..=1.
    Env2DecayCurve,
    /// Env2 Release stage curve, -1..=1.
    Env2ReleaseCurve,

    // ── Filter 2 ───────────────────────────────────────────────────────
    /// Filter 2 cutoff frequency, in Hz. Smoothed.
    Filter2CutoffHz,
    /// Filter 2 resonance, on the 0..=1 user scale. Smoothed.
    Filter2Resonance,

    // ── Env3 (second modulation envelope) ──────────────────────────────
    /// Env3 attack time, in seconds.
    Env3AttackSecs,
    /// Env3 decay time, in seconds.
    Env3DecaySecs,
    /// Env3 sustain level, 0..=1.
    Env3SustainLevel,
    /// Env3 release time, in seconds.
    Env3ReleaseSecs,
    /// Env3 Attack stage curve, -1..=1.
    Env3AttackCurve,
    /// Env3 Decay stage curve, -1..=1.
    Env3DecayCurve,
    /// Env3 Release stage curve, -1..=1.
    Env3ReleaseCurve,

    // ── Global ─────────────────────────────────────────────────────────
    /// Global tempo in BPM. Used for BPM-sync LFO rate computation.
    /// Range 20..=300. Stepped.
    Bpm,

    // ── Mod matrix (8 slots, indexed 0..=7) ────────────────────────────
    /// Enable flag for slot `i`. 0.0 = off, 1.0 = on.
    ModSlotEnabled(u8),
    /// Source index for slot `i`. Cast to [`ModSource`] via
    /// [`ModSource::from_index`].
    ModSlotSource(u8),
    /// Destination index for slot `i`. Cast to [`ModDest`] via
    /// [`ModDest::from_index`].
    ModSlotDest(u8),
    /// Signed amount for slot `i`, in destination-natural units.
    ModSlotAmount(u8),
    /// Via-source index for slot `i`. `ModSource::Off` (index 0) means
    /// no via scaling.
    ModSlotVia(u8),

    // ── FM synthesis (M7.3) ────────────────────────────────────────────────
    /// Per-slot mix level, 0..=1. Slot index 0..=1.
    SlotLevel(u8),
    /// Per-slot mix pan, -1..=1. Slot index 0..=1.
    SlotPan(u8),
    /// FM algorithm for a slot. Slot index 0..=1; value 0.0..=7.0.
    FmAlgorithm(u8),
    /// FM operator integer ratio. Packed `(slot << 4) | op`. Value 0.0..=15.0.
    FmOpRatioInteger(u8),
    /// FM operator fine ratio in cents. Packed `(slot << 4) | op`. Value -100.0..=100.0.
    FmOpRatioFine(u8),
    /// FM operator output level, 0..=1. Packed `(slot << 4) | op`.
    FmOpLevel(u8),
    /// FM operator envelope attack, seconds. Packed `(slot << 4) | op`.
    FmOpAttackSecs(u8),
    /// FM operator envelope decay, seconds. Packed `(slot << 4) | op`.
    FmOpDecaySecs(u8),
    /// FM operator envelope sustain level, 0..=1. Packed `(slot << 4) | op`.
    FmOpSustainLevel(u8),
    /// FM operator envelope release, seconds. Packed `(slot << 4) | op`.
    FmOpReleaseSecs(u8),
    /// FM operator self-feedback, -1..=1. Packed `(slot << 4) | op`.
    /// Only meaningful for op 3 in the 8 starter algorithms.
    FmOpFeedback(u8),

    // ── FX chain (M8) ─────────────────────────────────────────────────────
    /// EQ stage enabled; 0.0 = off, 1.0 = on.
    FxEqEnabled,
    /// EQ low-shelf gain, -15..=15 dB.
    FxEqLowGainDb,
    /// EQ low-shelf frequency, 20..=2000 Hz.
    FxEqLowFreqHz,
    /// EQ mid-peak gain, -15..=15 dB.
    FxEqMidGainDb,
    /// EQ mid-peak frequency, 200..=8000 Hz.
    FxEqMidFreqHz,
    /// EQ mid-peak Q, 0.1..=10.
    FxEqMidQ,
    /// EQ high-shelf gain, -15..=15 dB.
    FxEqHighGainDb,
    /// EQ high-shelf frequency, 2000..=20000 Hz.
    FxEqHighFreqHz,
    /// Drive stage enabled; 0.0 = off, 1.0 = on.
    FxDriveEnabled,
    /// Drive pre-clip gain, 1..=20.
    FxDriveDrive,
    /// Drive asymmetry, -1..=1.
    FxDriveAsymmetry,
    /// Chorus stage enabled; 0.0 = off, 1.0 = on.
    FxChorusEnabled,
    /// Chorus LFO rate, 0.1..=8 Hz.
    FxChorusRateHz,
    /// Chorus modulation depth, 0..=15 ms.
    FxChorusDepthMs,
    /// Chorus dry/wet mix, 0..=1.
    FxChorusMix,
    /// Chorus stereo spread, 0..=1.
    FxChorusSpread,
    /// Delay stage enabled; 0.0 = off, 1.0 = on.
    FxDelayEnabled,
    /// Delay time in seconds, 0.001..=2.0.
    FxDelayTimeSecs,
    /// Delay feedback, 0..=0.95.
    FxDelayFeedback,
    /// Delay dry/wet mix, 0..=1.
    FxDelayMix,
    /// Delay feedback low-cut frequency, 20..=2000 Hz.
    FxDelayLowcutHz,
    /// Delay ping-pong mode; 0.0 = off, 1.0 = on.
    FxDelayPingPong,
    /// Reverb stage enabled; 0.0 = off, 1.0 = on.
    FxReverbEnabled,
    /// Reverb pre-delay, 0..=50 ms.
    FxReverbPredelayMs,
    /// Reverb decay time, 0.1..=30 s.
    FxReverbDecaySecs,
    /// Reverb room size, 0.1..=1.0.
    FxReverbSize,
    /// Reverb HF damping, 0..=1.
    FxReverbDamping,
    /// Reverb dry/wet mix, 0..=1.
    FxReverbMix,

    // ── Arpeggiator ────────────────────────────────────────────────────────
    /// Arp on/off, 0.0 = off, 1.0 = on.
    ArpEnabled,
    /// Arp mode: 0=Up 1=Down 2=UpDown 3=Random 4=Played.
    ArpMode,
    /// Octave range, 1–4.
    ArpOctaves,
    /// Step rate: 0=1/32 1=1/16 2=1/8 3=1/4 4=1/2.
    ArpRate,
    /// Gate fraction of step duration, 0.01–1.0.
    ArpGate,
    /// Swing fraction, 0.5–0.75.
    ArpSwing,

    // ── Step sequencer ─────────────────────────────────────────────────────
    /// Sequencer master enable (mutually exclusive with the arp).
    SeqEnabled,
    /// Active step count, 1–16.
    SeqLength,
    /// Playback mode: 0=Forward 1=Reverse 2=PingPong 3=Random.
    SeqMode,
    /// Step rate: 0=1/32 1=1/16 2=1/8 3=1/4 4=1/2.
    SeqRate,
    /// Swing fraction, 0.5–0.75.
    SeqSwing,
    /// Per-step note offset from the held root, -24..=24 semitones.
    SeqStepNote(u8),
    /// Per-step velocity, 0–127.
    SeqStepVelocity(u8),
    /// Per-step gate fraction, 0.0–1.0.
    SeqStepGate(u8),
    /// Per-step rest toggle (≥0.5 = rest).
    SeqStepRest(u8),
    /// Per-step tie toggle (≥0.5 = hold the previous note).
    SeqStepTie(u8),
    /// Per-step mod-lane CV value, -1.0..=1.0.
    SeqStepMod(u8),
    /// Per-step second mod-lane CV value, -1.0..=1.0 (the `Seq2` source).
    SeqStepMod2(u8),
}
