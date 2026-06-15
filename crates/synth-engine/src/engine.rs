//! The top-level DSP engine.
//!
//! Holds the [`VoiceManager`] and the [`ParameterTree`] that owns all
//! sound-affecting state. [`Engine::prepare`] is the once-per-stream
//! setup point where all buffer sizing and pool allocation must happen
//! — see `docs/planning/03-architecture/design-patterns.md` §2.5.
//! [`Engine::process_stereo`] is allowed zero heap allocations.
//!
//! [`ParameterTree`]: crate::params::ParameterTree
//! [`VoiceManager`]: crate::voice_manager::VoiceManager

use crate::arp::{ArpEngine, ArpEvent, ArpMode, ArpRate};
use crate::events::EngineEvent;
use crate::fx::FxChain;
use crate::lfo::LfoShape;
use crate::mod_matrix::{ModDest, ModSource};
use crate::params::{ParamId, ParamSnapshot, ParameterTree};
use crate::seq::{SeqEngine, SeqMode};
use crate::voice_manager::VoiceManager;

/// Maximum block size the engine promises to handle, in frames.
///
/// `process_stereo` is given the actual block size each call; this
/// constant is a soft upper bound used by future internal scratch
/// buffers. M2.2 has none yet, so the value is informative until later
/// M2 work needs it.
pub const MAX_BLOCK_SIZE: usize = 4096;

/// Default pitch-bend range in semitones (±2 = GM default). Applied
/// symmetrically: a fully-deflected wheel shifts pitch by this many
/// semitones up or down. Configurable via Settings in M13.
pub const PITCH_BEND_RANGE_SEMIS: f32 = 2.0;

/// The DSP engine. Owns the parameter tree and the polyphonic voice
/// pool.
///
/// Construct with [`Engine::new`], wire to the audio thread, and call
/// [`Engine::handle`] for each input event before each block.
pub struct Engine {
    /// Sample rate the audio device opened with, captured at `new()`.
    sample_rate_hz: f32,

    /// Single source of truth for sound-affecting parameters per
    /// design-patterns.md §1.3.
    params: ParameterTree,

    /// Polyphonic voice pool sized to [`crate::POLYPHONY`].
    voices: VoiceManager,

    /// Post-mix effects chain: EQ → Drive → Chorus → Delay → Reverb.
    fx: FxChain,

    /// Arpeggiator — clocks NoteOn/NoteOff into the voice pool each block.
    arp: ArpEngine,

    /// Step sequencer — sibling of the arp; mutually exclusive with it.
    seq: SeqEngine,
}

impl Engine {
    /// Creates an engine ready to process at the given sample rate.
    ///
    /// The sample rate is fixed for the engine's lifetime; if the audio
    /// device changes rate, build a new engine.
    #[must_use]
    pub fn new(sample_rate_hz: f32) -> Self {
        let params = ParameterTree::new(sample_rate_hz);
        let mut voices = VoiceManager::new(sample_rate_hz);
        // Seed every voice with the parameter defaults so the first
        // note played sees the same values the tree publishes in the
        // first snapshot.
        voices.set_attack_secs(params.amp_attack_secs());
        voices.set_decay_secs(params.amp_decay_secs());
        voices.set_sustain_level(params.amp_sustain_level());
        voices.set_release_secs(params.amp_release_secs());
        voices.set_main_waveform(params.waveform());
        voices.set_filter_mode(params.filter_mode());
        voices.set_filter2_mode(params.filter2_mode());
        voices.set_filter_routing(params.filter_routing());
        let slopes = params.filter_slope();
        voices.set_filter_slope(0, slopes[0]);
        voices.set_filter_slope(1, slopes[1]);
        voices.set_lfo1_rate_hz(params.lfo1_effective_rate_hz());
        voices.set_lfo1_shape(LfoShape::from_index(params.lfo1_shape_index()));
        voices.set_lfo1_reset_on_note_on(params.lfo1_reset_on_note_on());
        voices.set_lfo2_rate_hz(params.lfo2_effective_rate_hz());
        voices.set_lfo2_shape(LfoShape::from_index(params.lfo2_shape_index()));
        voices.set_lfo2_reset_on_note_on(params.lfo2_reset_on_note_on());
        voices.set_env2_attack_secs(params.env2_attack_secs());
        voices.set_env2_decay_secs(params.env2_decay_secs());
        voices.set_env2_sustain_level(params.env2_sustain_level());
        voices.set_env2_release_secs(params.env2_release_secs());
        voices.set_env2_attack_curve(params.env2_attack_curve());
        voices.set_env2_decay_curve(params.env2_decay_curve());
        voices.set_env2_release_curve(params.env2_release_curve());
        voices.set_env3_attack_secs(params.env3_attack_secs());
        voices.set_env3_decay_secs(params.env3_decay_secs());
        voices.set_env3_sustain_level(params.env3_sustain_level());
        voices.set_env3_release_secs(params.env3_release_secs());
        voices.set_env3_attack_curve(params.env3_attack_curve());
        voices.set_env3_decay_curve(params.env3_decay_curve());
        voices.set_env3_release_curve(params.env3_release_curve());
        Self {
            sample_rate_hz,
            params,
            voices,
            fx: FxChain::new(sample_rate_hz),
            arp: ArpEngine::new(sample_rate_hz),
            seq: SeqEngine::new(sample_rate_hz),
        }
    }

    /// Returns the sample rate the engine was built with, in Hz.
    #[must_use]
    pub fn sample_rate_hz(&self) -> f32 {
        self.sample_rate_hz
    }

    /// Applies a single event. Called by the audio thread at the top
    /// of each block, draining whatever the adapters have queued.
    pub fn handle(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::NoteOn { note_midi, velocity } => {
                // Feed both note engines regardless of which is active so that
                // toggling between arp and sequencer mid-hold always has a held
                // set to step through. Only the active engine drives the voices;
                // the two are mutually exclusive (see SeqEnabled/ArpEnabled).
                let arp_first = self.arp.note_on(note_midi);
                let seq_first = self.seq.note_on(note_midi);
                if self.arp.enabled {
                    if arp_first {
                        // Fire the very first arp note immediately — before
                        // process_stereo() — so there is no extra-block delay
                        // regardless of gate setting. The arp already set
                        // gate_open=true / phase=0.0 so process() handles
                        // gate-off and subsequent steps normally.
                        self.params.snap_for_note_on();
                        self.voices.note_on(self.arp.current_note, velocity);
                    }
                } else if self.seq.enabled {
                    if seq_first {
                        // Same immediate-fire path, with the step's own velocity.
                        self.params.snap_for_note_on();
                        self.voices.note_on(self.seq.current_note, self.seq.current_velocity);
                    }
                } else {
                    self.params.snap_for_note_on();
                    self.voices.note_on(note_midi, velocity);
                }
            }
            EngineEvent::NoteOff { note_midi } => {
                self.arp.note_off(note_midi);
                self.seq.note_off(note_midi);
                if !self.arp.enabled && !self.seq.enabled {
                    self.voices.note_off(note_midi);
                }
            }
            EngineEvent::AllNotesOff => {
                // Stop everything, whatever state it's in: sounding
                // voices, sustain-deferred releases, and any notes the
                // arp or sequencer is still holding. Stuck-note recovery.
                self.voices.panic();
                self.arp.clear();
                self.seq.clear();
            }
            EngineEvent::SetOscillatorWaveform { waveform } => {
                self.params.set_waveform(waveform);
                self.voices.set_main_waveform(waveform);
            }
            EngineEvent::SetFilterMode { mode } => {
                self.params.set_filter_mode(mode);
                self.voices.set_filter_mode(mode);
            }
            EngineEvent::SetFilter2Mode { mode } => {
                self.params.set_filter2_mode(mode);
                self.voices.set_filter2_mode(mode);
            }
            EngineEvent::SetFilterRouting { routing } => {
                self.params.set_filter_routing(routing);
                self.voices.set_filter_routing(routing);
            }
            EngineEvent::SetFilterSlope { filter_idx, slope } => {
                self.params.set_filter_slope(filter_idx, slope);
                self.voices.set_filter_slope(filter_idx, slope);
            }
            EngineEvent::ParameterChange { id, value } => {
                self.params.set_continuous(id, value);
                // Stepped params are read by DSP only on edge
                // transitions, so push the new value to the consumer
                // immediately; smoothed params are sampled per frame
                // from the tree and need no fan-out here.
                self.fan_out_param(id, value);
            }
            EngineEvent::PitchBend { value_normalised } => {
                let semis = value_normalised * PITCH_BEND_RANGE_SEMIS;
                self.params.set_continuous(ParamId::PitchBendSemis, semis);
                self.voices.set_global_pitch_bend(value_normalised);
            }
            EngineEvent::Sustain { held } => {
                self.voices.set_sustain(held);
            }
            EngineEvent::ChannelAftertouch { value_normalised } => {
                self.params.set_continuous(ParamId::ChannelAftertouch, value_normalised);
                self.voices.set_global_aftertouch(value_normalised);
            }
            EngineEvent::ControlChange { cc, value_normalised } => {
                if cc == 1 {
                    self.params.set_continuous(ParamId::ModWheel, value_normalised);
                    self.voices.set_global_mod_wheel(value_normalised);
                }
                // Store all CCs in the snapshot regardless of routing so
                // M6's mod matrix can address any controller.
                self.params.set_cc(cc, value_normalised);
            }
        }
    }

    /// Routes a parameter change to the appropriate DSP consumer after
    /// the tree has already been updated. Called by `handle()` for every
    /// `ParameterChange` event. Smoothed params need no fan-out here
    /// (the audio path samples them per frame from the tree); only
    /// stepped params that have live DSP consumers are dispatched.
    fn fan_out_param(&mut self, id: ParamId, value: f32) {
        match id {
            // ── Amp envelope ────────────────────────────────────────────────
            ParamId::AmpAttackSecs => self.voices.set_attack_secs(value),
            ParamId::AmpDecaySecs => self.voices.set_decay_secs(value),
            ParamId::AmpSustainLevel => self.voices.set_sustain_level(value),
            ParamId::AmpReleaseSecs => self.voices.set_release_secs(value),

            // ── LFO 1 ───────────────────────────────────────────────────────
            ParamId::Lfo1RateHz | ParamId::Lfo1SyncEnabled | ParamId::Lfo1SyncDivision | ParamId::Bpm => {
                // Re-derive LFO1 rate whenever anything that affects it changes.
                self.voices.set_lfo1_rate_hz(self.params.lfo1_effective_rate_hz());
                // Bpm is the single transport tempo: it also affects LFO2 (if
                // synced) and drives the arp clock (and, from Phase 2, the seq).
                if id == ParamId::Bpm {
                    self.voices.set_lfo2_rate_hz(self.params.lfo2_effective_rate_hz());
                    self.arp.bpm = value;
                    self.seq.bpm = value;
                }
            }
            ParamId::Lfo1Shape => {
                self.voices.set_lfo1_shape(LfoShape::from_index(value as usize));
            }
            ParamId::Lfo1ResetOnNoteOn => {
                self.voices.set_lfo1_reset_on_note_on(value >= 0.5);
            }
            ParamId::Lfo1Global => {
                self.voices.set_lfo1_global(value >= 0.5);
            }

            // ── LFO 2 ───────────────────────────────────────────────────────
            ParamId::Lfo2RateHz | ParamId::Lfo2SyncEnabled | ParamId::Lfo2SyncDivision => {
                self.voices.set_lfo2_rate_hz(self.params.lfo2_effective_rate_hz());
            }
            ParamId::Lfo2Shape => {
                self.voices.set_lfo2_shape(LfoShape::from_index(value as usize));
            }
            ParamId::Lfo2ResetOnNoteOn => {
                self.voices.set_lfo2_reset_on_note_on(value >= 0.5);
            }
            ParamId::Lfo2Global => {
                self.voices.set_lfo2_global(value >= 0.5);
            }

            // ── Env2 ────────────────────────────────────────────────────────
            ParamId::Env2AttackSecs => self.voices.set_env2_attack_secs(value),
            ParamId::Env2DecaySecs => self.voices.set_env2_decay_secs(value),
            ParamId::Env2SustainLevel => self.voices.set_env2_sustain_level(value),
            ParamId::Env2ReleaseSecs => self.voices.set_env2_release_secs(value),
            ParamId::Env2AttackCurve => self.voices.set_env2_attack_curve(value),
            ParamId::Env2DecayCurve => self.voices.set_env2_decay_curve(value),
            ParamId::Env2ReleaseCurve => self.voices.set_env2_release_curve(value),

            // ── Env3 ────────────────────────────────────────────────────────
            ParamId::Env3AttackSecs => self.voices.set_env3_attack_secs(value),
            ParamId::Env3DecaySecs => self.voices.set_env3_decay_secs(value),
            ParamId::Env3SustainLevel => self.voices.set_env3_sustain_level(value),
            ParamId::Env3ReleaseSecs => self.voices.set_env3_release_secs(value),
            ParamId::Env3AttackCurve => self.voices.set_env3_attack_curve(value),
            ParamId::Env3DecayCurve => self.voices.set_env3_decay_curve(value),
            ParamId::Env3ReleaseCurve => self.voices.set_env3_release_curve(value),

            // ── Mod matrix ──────────────────────────────────────────────────
            ParamId::ModSlotEnabled(i) => {
                self.voices.set_mod_slot_enabled(i as usize, value >= 0.5);
            }
            ParamId::ModSlotSource(i) => {
                let src = ModSource::from_index(value as u8).unwrap_or_default();
                self.voices.set_mod_slot_source(i as usize, src);
            }
            ParamId::ModSlotDest(i) => {
                if let Some(dest) = ModDest::from_index(value as u8) {
                    self.voices.set_mod_slot_dest(i as usize, dest);
                }
            }
            ParamId::ModSlotAmount(i) => {
                self.voices.set_mod_slot_amount(i as usize, value);
            }
            ParamId::ModSlotVia(i) => {
                let via = ModSource::from_index(value as u8).unwrap_or_default();
                self.voices.set_mod_slot_via(i as usize, via);
            }

            // ── FM synthesis ────────────────────────────────────────────────
            ParamId::SlotLevel(i) => self.voices.set_slot_level(i as usize, value),
            ParamId::SlotPan(i) => self.voices.set_slot_pan(i as usize, value),
            ParamId::FmAlgorithm(i) => self.voices.set_fm_algorithm(i as usize, value as u8),
            ParamId::FmOpRatioInteger(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_ratio_integer(slot, op, value as u8);
            }
            ParamId::FmOpRatioFine(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_ratio_fine(slot, op, value);
            }
            ParamId::FmOpLevel(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_level(slot, op, value);
            }
            ParamId::FmOpAttackSecs(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_attack_secs(slot, op, value);
            }
            ParamId::FmOpDecaySecs(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_decay_secs(slot, op, value);
            }
            ParamId::FmOpSustainLevel(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_sustain_level(slot, op, value);
            }
            ParamId::FmOpReleaseSecs(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_release_secs(slot, op, value);
            }
            ParamId::FmOpFeedback(packed) => {
                let slot = ((packed >> 4) & 0x0F) as usize;
                let op = (packed & 0x0F) as usize;
                self.voices.set_fm_op_feedback(slot, op, value);
            }

            // ── FX chain — push to FxChain directly; ParameterTree ──────────
            // stores the value for snapshot mirroring.
            ParamId::FxEqEnabled => self.fx.set_eq_enabled(value >= 0.5),
            ParamId::FxEqLowGainDb | ParamId::FxEqLowFreqHz => self.fx.set_eq_low(
                self.params.snapshot().fx_eq_low_freq_hz,
                self.params.snapshot().fx_eq_low_gain_db,
            ),
            ParamId::FxEqMidGainDb | ParamId::FxEqMidFreqHz | ParamId::FxEqMidQ => {
                let s = self.params.snapshot();
                self.fx
                    .set_eq_mid(s.fx_eq_mid_freq_hz, s.fx_eq_mid_gain_db, s.fx_eq_mid_q);
            }
            ParamId::FxEqHighGainDb | ParamId::FxEqHighFreqHz => self.fx.set_eq_high(
                self.params.snapshot().fx_eq_high_freq_hz,
                self.params.snapshot().fx_eq_high_gain_db,
            ),
            ParamId::FxDriveEnabled => self.fx.set_drive_enabled(value >= 0.5),
            ParamId::FxDriveDrive | ParamId::FxDriveAsymmetry => {
                let s = self.params.snapshot();
                self.fx.set_drive_params(s.fx_drive_drive, s.fx_drive_asymmetry);
            }
            ParamId::FxChorusEnabled => self.fx.set_chorus_enabled(value >= 0.5),
            ParamId::FxChorusRateHz | ParamId::FxChorusDepthMs | ParamId::FxChorusMix | ParamId::FxChorusSpread => {
                let s = self.params.snapshot();
                self.fx.set_chorus_params(
                    s.fx_chorus_rate_hz,
                    s.fx_chorus_depth_ms,
                    s.fx_chorus_mix,
                    s.fx_chorus_spread,
                );
            }
            ParamId::FxDelayEnabled => self.fx.set_delay_enabled(value >= 0.5),
            ParamId::FxDelayTimeSecs
            | ParamId::FxDelayFeedback
            | ParamId::FxDelayMix
            | ParamId::FxDelayLowcutHz
            | ParamId::FxDelayPingPong => {
                let s = self.params.snapshot();
                self.fx.set_delay_params(
                    s.fx_delay_time_secs,
                    s.fx_delay_feedback,
                    s.fx_delay_mix,
                    s.fx_delay_lowcut_hz,
                    s.fx_delay_ping_pong,
                );
            }
            ParamId::FxReverbEnabled => self.fx.set_reverb_enabled(value >= 0.5),
            ParamId::FxReverbPredelayMs
            | ParamId::FxReverbDecaySecs
            | ParamId::FxReverbSize
            | ParamId::FxReverbDamping
            | ParamId::FxReverbMix => {
                let s = self.params.snapshot();
                self.fx.set_reverb_params(
                    s.fx_reverb_predelay_ms,
                    s.fx_reverb_decay_secs,
                    s.fx_reverb_size,
                    s.fx_reverb_damping,
                    s.fx_reverb_mix,
                );
            }

            // ── Arpeggiator ─────────────────────────────────────────────────
            ParamId::ArpEnabled => {
                let new_enabled = value >= 0.5;
                if new_enabled != self.arp.enabled {
                    self.arp.enabled = new_enabled;
                    // Kill any sounding voices on both transitions:
                    //   disable → stops arp-controlled notes
                    //   enable  → stops direct notes so arp takes over
                    self.voices.all_notes_off();
                    if new_enabled {
                        // Reset clock so the first step fires on the
                        // very next process() call rather than waiting
                        // a full step to accumulate phase.
                        self.arp.reset_clock();
                        // Mutual exclusion: the sequencer and arp never clock
                        // together. Force the sequencer off and mirror that
                        // into the tree so the UI toggle clears too.
                        if self.seq.enabled {
                            self.seq.enabled = false;
                            self.params.set_continuous(ParamId::SeqEnabled, 0.0);
                        }
                    }
                }
            }
            ParamId::ArpMode => self.arp.mode = ArpMode::from_f32(value),
            ParamId::ArpOctaves => self.arp.octaves = (value as u8).clamp(1, 4),
            ParamId::ArpRate => self.arp.rate = ArpRate::from_f32(value),
            ParamId::ArpGate => self.arp.gate = value.clamp(0.01, 1.0),
            ParamId::ArpSwing => self.arp.swing = value.clamp(0.5, 0.75),

            // ── Step sequencer ──────────────────────────────────────────────
            ParamId::SeqEnabled => {
                let new_enabled = value >= 0.5;
                if new_enabled != self.seq.enabled {
                    self.seq.enabled = new_enabled;
                    self.voices.all_notes_off();
                    if new_enabled {
                        self.seq.reset_clock();
                        // Mutual exclusion with the arp (mirror into the tree).
                        if self.arp.enabled {
                            self.arp.enabled = false;
                            self.params.set_continuous(ParamId::ArpEnabled, 0.0);
                        }
                    }
                }
            }
            ParamId::SeqLength => {
                self.seq.length = (value as usize).clamp(1, crate::seq::SEQ_MAX_STEPS);
            }
            ParamId::SeqMode => self.seq.mode = SeqMode::from_f32(value),
            ParamId::SeqRate => self.seq.rate = ArpRate::from_f32(value),
            ParamId::SeqSwing => self.seq.swing = value.clamp(0.5, 0.75),
            ParamId::SeqStepNote(i) if (i as usize) < crate::seq::SEQ_MAX_STEPS => {
                self.seq.steps[i as usize].note_offset = (value.round() as i32).clamp(-24, 24) as i8;
            }
            ParamId::SeqStepVelocity(i) if (i as usize) < crate::seq::SEQ_MAX_STEPS => {
                self.seq.steps[i as usize].velocity = (value.round() as i32).clamp(0, 127) as u8;
            }
            ParamId::SeqStepGate(i) if (i as usize) < crate::seq::SEQ_MAX_STEPS => {
                self.seq.steps[i as usize].gate = value.clamp(0.0, 1.0);
            }
            ParamId::SeqStepRest(i) if (i as usize) < crate::seq::SEQ_MAX_STEPS => {
                self.seq.steps[i as usize].rest = value >= 0.5;
            }
            ParamId::SeqStepMod(i) if (i as usize) < crate::seq::SEQ_MAX_STEPS => {
                self.seq.steps[i as usize].mod_value = value.clamp(-1.0, 1.0);
            }

            _ => {}
        }
    }

    /// Fills an interleaved stereo output buffer with `frames` frames
    /// of audio. The buffer length must equal `frames * 2`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `output.len() != frames * 2`.
    pub fn process_stereo(&mut self, output: &mut [f32], frames: usize) {
        debug_assert_eq!(output.len(), frames * 2);

        // Publish the sequencer's current mod-lane value (block-rate) so the
        // `Seq` mod source picks it up when this block's modulation sources
        // are built. Reads 0.0 when the sequencer is idle.
        self.voices.set_global_seq_mod(self.seq.mod_value());

        // Advance block-rate modulators (LFOs and Env2) once per block
        // before the per-sample loop.
        self.voices.advance_modulators(frames);

        // Tick the active note engine (arp or sequencer — never both) and
        // dispatch any NoteOn/NoteOff it generates. When neither is enabled the
        // sequencer's process() returns no events.
        let note_events = if self.arp.enabled {
            self.arp.process(frames)
        } else {
            self.seq.process(frames)
        };
        for ev in note_events.iter() {
            match *ev {
                ArpEvent::NoteOn { note, velocity } => {
                    self.params.snap_for_note_on();
                    self.voices.note_on(note, velocity);
                }
                ArpEvent::NoteOff { note } => {
                    // Bypass sustain-pedal deferral: the arp controls gate
                    // timing explicitly and must not have its note-offs held.
                    self.voices.release_note_immediate(note);
                }
            }
        }

        let mut peak_l: f32 = 0.0;
        let mut peak_r: f32 = 0.0;
        for frame_index in 0..frames {
            let smoothed = self.params.next_sample();
            let (left, right) = self.voices.next_sample(&smoothed);
            let scaled_l = left * smoothed.master_volume;
            let scaled_r = right * smoothed.master_volume;
            let (out_l, out_r) = self.fx.process(scaled_l, scaled_r);
            output[frame_index * 2] = out_l;
            output[frame_index * 2 + 1] = out_r;
            peak_l = peak_l.max(out_l.abs());
            peak_r = peak_r.max(out_r.abs());
        }

        // Mirror the post-block voice count, modulator outputs, and VU peak
        // into the tree so the next snapshot reflects what just played.
        #[allow(clippy::cast_possible_truncation)]
        let count = self.voices.active_count() as u8;
        self.params.set_active_voice_count(count);
        let (lfo1, lfo2, env2, env3) = self.voices.first_active_modulator_outputs();
        self.params.set_modulator_outputs(lfo1, lfo2, env2, env3);
        self.params.set_vu_peak(peak_l, peak_r);
        // Mirror the sequencer playhead for the UI step grid (-1 when idle).
        let seq_step = self.seq.current_step().map_or(-1, |i| i as i8);
        self.params.set_seq_current_step(seq_step);
    }

    /// Returns the current parameter snapshot by value, without
    /// allocating. The caller is responsible for wrapping it in an
    /// `Arc` and publishing into the snapshot slot (see
    /// `docs/planning/03-architecture/design-patterns.md` §1.4).
    #[must_use]
    pub fn snapshot(&self) -> ParamSnapshot {
        self.params.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::FilterMode;
    use crate::oscillator::Waveform;

    #[test]
    fn engine_starts_silent() {
        let mut engine = Engine::new(48_000.0);
        let mut buffer = [0.0f32; 256 * 2];
        engine.process_stereo(&mut buffer, 256);
        assert!(buffer.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn note_on_produces_non_zero_audio() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::NoteOn {
            note_midi: 69,
            velocity: 100,
        });
        let mut buffer = [0.0f32; 1024 * 2];
        engine.process_stereo(&mut buffer, 1024);
        let peak = buffer.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(peak > 0.1, "expected audible output after NoteOn, peak was {peak}");
    }

    #[test]
    fn all_notes_off_silences_a_held_note() {
        let mut engine = Engine::new(48_000.0);
        // Hold a note (never sending NoteOff) to mimic a stuck note.
        engine.handle(EngineEvent::NoteOn {
            note_midi: 60,
            velocity: 100,
        });
        let mut buffer = [0.0f32; 1024 * 2];
        engine.process_stereo(&mut buffer, 1024);
        let held_peak = buffer.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(held_peak > 0.1, "note should be sounding, peak {held_peak}");

        // Panic, then let the default (~0.2 s) release drain to silence.
        engine.handle(EngineEvent::AllNotesOff);
        let mut tail_peak = 0.0f32;
        for _ in 0..100 {
            buffer.fill(0.0);
            engine.process_stereo(&mut buffer, 1024);
            tail_peak = buffer.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        }
        assert!(
            tail_peak < 1.0e-3,
            "note should be silent after AllNotesOff, tail peak {tail_peak}"
        );
        assert_eq!(engine.snapshot().active_voice_count, 0, "no voice should remain active");
    }

    #[test]
    fn parameter_change_updates_snapshot() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::PitchOffsetSemis,
            value: 7.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::AmpReleaseSecs,
            value: 1.5,
        });
        engine.handle(EngineEvent::SetOscillatorWaveform {
            waveform: Waveform::Saw,
        });
        engine.handle(EngineEvent::SetFilterMode {
            mode: FilterMode::BandPass,
        });

        let snap = engine.snapshot();
        // Smoothed param: snapshot reads `current`, which has not yet
        // advanced toward the new target (no block has run), so the
        // value is still the default.
        assert_eq!(snap.pitch_offset_semis, 0.0);
        // Stepped param latches immediately.
        assert_eq!(snap.amp_release_secs, 1.5);
        assert_eq!(snap.waveform, Waveform::Saw);
        assert_eq!(snap.filter_mode, FilterMode::BandPass);
        assert_eq!(snap.active_voice_count, 0);
    }

    #[test]
    fn active_voice_count_tracks_note_state() {
        let mut engine = Engine::new(48_000.0);
        assert_eq!(engine.snapshot().active_voice_count, 0);
        engine.handle(EngineEvent::NoteOn {
            note_midi: 60,
            velocity: 100,
        });
        // One sample is enough to leave idle.
        let mut buffer = [0.0f32; 2];
        engine.process_stereo(&mut buffer, 1);
        assert_eq!(engine.snapshot().active_voice_count, 1);
    }

    #[test]
    fn smoothed_param_snapshot_advances_with_processing() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::PitchOffsetSemis,
            value: 5.0,
        });
        let mut buffer = [0.0f32; 4096 * 2];
        engine.process_stereo(&mut buffer, 4096);
        let snap = engine.snapshot();
        assert!(
            (snap.pitch_offset_semis - 5.0).abs() < 0.05,
            "expected ~5.0 after smoothing, got {}",
            snap.pitch_offset_semis
        );
    }

    #[test]
    fn hard_pan_routes_audio_to_one_stereo_channel() {
        // End-to-end: pan osc1 hard left, mute oscs 2/3/sub, render a
        // note. The right channel of the interleaved output should
        // contain only essentially silent samples.
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::Osc2Level,
            value: 0.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::Osc3Level,
            value: 0.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::SubLevel,
            value: 0.0,
        });
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::Osc1Pan,
            value: -1.0,
        });
        engine.handle(EngineEvent::NoteOn {
            note_midi: 69,
            velocity: 100,
        });

        let mut buffer = vec![0.0f32; 4096 * 2];
        // Render long enough for the pan/level smoothers to settle.
        for _ in 0..6 {
            engine.process_stereo(&mut buffer, 4096);
        }
        let mut peak_l = 0.0_f32;
        let mut peak_r = 0.0_f32;
        for frame in buffer.chunks_exact(2) {
            peak_l = peak_l.max(frame[0].abs());
            peak_r = peak_r.max(frame[1].abs());
        }
        assert!(peak_l > 0.1, "expected audible left, got {peak_l}");
        assert!(peak_r < 0.01, "expected silent right, got {peak_r}");
    }

    #[test]
    fn unison_voices_change_stereo_width_end_to_end() {
        // Solo osc1 wide, render a note with 1 unison voice then with
        // 5. The second case should produce a meaningfully wider
        // L vs R difference summed across the buffer.
        fn render_diff(voice_count: f32) -> f32 {
            let mut engine = Engine::new(48_000.0);
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc2Level,
                value: 0.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc3Level,
                value: 0.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::SubLevel,
                value: 0.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc1UnisonVoices,
                value: voice_count,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc1UnisonDetuneCents,
                value: 25.0,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Osc1UnisonSpread,
                value: 1.0,
            });
            engine.handle(EngineEvent::NoteOn {
                note_midi: 69,
                velocity: 100,
            });
            let mut buffer = vec![0.0f32; 4096 * 2];
            for _ in 0..6 {
                engine.process_stereo(&mut buffer, 4096);
            }
            buffer
                .chunks_exact(2)
                .map(|frame| (frame[0] - frame[1]).abs())
                .sum::<f32>()
        }
        let one = render_diff(1.0);
        let five = render_diff(5.0);
        assert!(
            five > one * 3.0,
            "expected unison to widen stereo: 1-voice diff {one}, 5-voice diff {five}"
        );
    }

    #[test]
    fn closing_filter_silences_a_held_saw() {
        // End-to-end: a held saw note through the engine, then close
        // the LP filter — the steady-state output should drop near
        // silence. Smoothing on cutoff means we have to render long
        // enough for the new target to take effect.
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::SetOscillatorWaveform {
            waveform: Waveform::Saw,
        });
        engine.handle(EngineEvent::NoteOn {
            note_midi: 69,
            velocity: 100,
        });
        let mut buffer = vec![0.0f32; 4096 * 2];
        // Let the envelope reach sustain with the filter wide open.
        engine.process_stereo(&mut buffer, 4096);
        let open_peak = buffer.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(
            open_peak > 0.1,
            "expected audible saw before closing filter, got {open_peak}"
        );

        // Now close the filter.
        engine.handle(EngineEvent::ParameterChange {
            id: ParamId::FilterCutoffHz,
            value: 30.0,
        });
        // Let smoothing reach the new target and the filter settle.
        for _ in 0..4 {
            engine.process_stereo(&mut buffer, 4096);
        }
        let closed_peak = buffer.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(
            closed_peak < open_peak * 0.1,
            "filter did not close the voice: open {open_peak}, closed {closed_peak}"
        );
    }

    #[test]
    fn low_velocity_produces_quieter_output_than_high_velocity() {
        fn peak_for_velocity(velocity: u8) -> f32 {
            let mut engine = Engine::new(48_000.0);
            engine.handle(EngineEvent::NoteOn {
                note_midi: 69,
                velocity,
            });
            let mut buffer = vec![0.0f32; 4096 * 2];
            // Render into sustain.
            for _ in 0..3 {
                engine.process_stereo(&mut buffer, 4096);
            }
            buffer.iter().fold(0.0f32, |a, s| a.max(s.abs()))
        }
        let quiet = peak_for_velocity(32);
        let loud = peak_for_velocity(127);
        assert!(
            quiet < loud,
            "velocity 32 should be quieter than 127 (quiet={quiet}, loud={loud})"
        );
    }

    #[test]
    fn pitch_bend_full_down_shifts_frequency() {
        // Render a note with no bend and with full-down bend. The
        // zero-crossing rate (frequency proxy) should be lower with bend.
        fn count_zero_crossings(velocity: u8, bend: f32) -> u32 {
            let mut engine = Engine::new(48_000.0);
            engine.handle(EngineEvent::NoteOn {
                note_midi: 69,
                velocity,
            });
            engine.handle(EngineEvent::PitchBend { value_normalised: bend });
            let mut buffer = vec![0.0f32; 8192 * 2];
            // Settle smoothers.
            for _ in 0..3 {
                engine.process_stereo(&mut buffer, 8192);
            }
            let mut crossings = 0_u32;
            let mut prev = buffer[0];
            for chunk in buffer.chunks_exact(2) {
                let s = chunk[0];
                if (prev <= 0.0 && s > 0.0) || (prev >= 0.0 && s < 0.0) {
                    crossings += 1;
                }
                prev = s;
            }
            crossings
        }
        let no_bend = count_zero_crossings(100, 0.0);
        let full_down = count_zero_crossings(100, -1.0);
        assert!(
            full_down < no_bend,
            "full-down bend should lower frequency (no_bend={no_bend}, bent={full_down})"
        );
    }

    #[test]
    fn sustain_keeps_voice_alive_after_note_off() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::NoteOn {
            note_midi: 60,
            velocity: 100,
        });
        let mut buffer = [0.0f32; 2];
        engine.process_stereo(&mut buffer, 1);
        engine.handle(EngineEvent::Sustain { held: true });
        engine.handle(EngineEvent::NoteOff { note_midi: 60 });
        engine.process_stereo(&mut buffer, 1);
        // With sustain held the voice is still running.
        assert_eq!(engine.snapshot().active_voice_count, 1);
        // Release the pedal — voice enters release.
        engine.handle(EngineEvent::Sustain { held: false });
        engine.process_stereo(&mut buffer, 1);
        assert_eq!(engine.snapshot().active_voice_count, 1, "voice still releasing");
    }

    #[test]
    fn mod_wheel_and_aftertouch_appear_in_snapshot() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ControlChange {
            cc: 1,
            value_normalised: 0.75,
        });
        engine.handle(EngineEvent::ChannelAftertouch { value_normalised: 0.5 });
        // Stepped params — visible in the snapshot immediately.
        let snap = engine.snapshot();
        assert!(
            (snap.mod_wheel - 0.75).abs() < 1e-4,
            "mod_wheel not in snapshot: {}",
            snap.mod_wheel
        );
        assert!(
            (snap.channel_aftertouch - 0.5).abs() < 1e-4,
            "channel_aftertouch not in snapshot: {}",
            snap.channel_aftertouch
        );
    }

    #[test]
    fn arbitrary_cc_stored_in_snapshot() {
        let mut engine = Engine::new(48_000.0);
        engine.handle(EngineEvent::ControlChange {
            cc: 74,
            value_normalised: 0.8,
        });
        let snap = engine.snapshot();
        assert!(
            (snap.cc_values[74] - 0.8).abs() < 1e-4,
            "CC 74 not stored in snapshot: {}",
            snap.cc_values[74]
        );
    }

    #[test]
    fn env2_to_filter_cutoff_modulation_changes_audio() {
        // Measures RMS with mod slot enabled or disabled.
        // The smoother must settle to the low cutoff before the note plays,
        // otherwise both branches see an almost-open filter from the default
        // 8000 Hz start value.
        let measure_rms = |mod_enabled: bool| -> f32 {
            let mut engine = Engine::new(44_100.0);
            // Close the filter almost fully so modulation has maximum effect.
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::FilterCutoffHz,
                value: 80.0,
            });
            // Some resonance to make the filter boundary sharper and easier
            // to detect in RMS.
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::FilterResonance,
                value: 0.5,
            });
            // Instant Env2 attack so it's at full level by the first block.
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::Env2AttackSecs,
                value: 0.001,
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::ModSlotEnabled(0),
                value: if mod_enabled { 1.0 } else { 0.0 },
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::ModSlotSource(0),
                value: 3.0, // ModSource::Env2
            });
            engine.handle(EngineEvent::ParameterChange {
                id: ParamId::ModSlotAmount(0),
                value: 10_000.0,
            });
            // Let the cutoff smoother settle to ~80 Hz before playing. The
            // smoother has a 10ms time constant at 44100 Hz (coeff ≈ 1/441).
            // After 5 time constants (2205 samples) it's >99% settled.
            let mut settle = vec![0.0f32; 4096 * 2];
            engine.process_stereo(&mut settle, 4096);

            engine.handle(EngineEvent::NoteOn {
                note_midi: 60,
                velocity: 100,
            });
            let mut buf = vec![0.0f32; 2048 * 2];
            engine.process_stereo(&mut buf, 2048);
            let sum_sq: f32 = buf.iter().map(|s| s * s).sum();
            (sum_sq / buf.len() as f32).sqrt()
        };

        let rms_off = measure_rms(false);
        let rms_on = measure_rms(true);
        assert!(
            rms_on > rms_off * 2.0,
            "Env2→Cutoff modulation should open the filter significantly \
             (rms_off={rms_off:.6}, rms_on={rms_on:.6})"
        );
    }
}
