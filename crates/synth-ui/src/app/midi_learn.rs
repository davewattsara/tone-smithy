//! Per-frame MIDI Learn tick: detect CC movement, bind it, apply mappings.

use eframe::egui;
use synth_engine::ParamSnapshot;
use synth_presets::MidiLearnEntry;

use crate::app::state::ToneSmithyApp;
use crate::app::utils::param_range_for_key;

impl ToneSmithyApp {
    /// Called once per frame from `update()`:
    /// 1. Reads any pending "MIDI Learn" intent deposited by a knob context menu.
    /// 2. If in learn mode, waits for the first CC that moves and binds it.
    /// 3. Routes each active mapping: fires `ParameterChange` only when the
    ///    CC actually moved, and syncs the corresponding UI field so the knob
    ///    display stays in agreement with the engine.
    pub(crate) fn tick_midi_learn(&mut self, ctx: &egui::Context, snapshot: &ParamSnapshot) {
        // Step 1 — consume any learn intent deposited by a knob this frame.
        let intent: Option<String> = ctx.memory_mut(|m| m.data.remove_temp(egui::Id::new("ts_midi_learn_pending")));
        if let Some(key) = intent {
            self.midi_learn_target = Some(key);
        }

        // Step 2 — detect CC movement while in learn mode.
        if self.midi_learn_target.is_some() {
            let mut found_cc: Option<u8> = None;
            for cc in 0..128usize {
                if (snapshot.cc_values[cc] - self.prev_cc_values[cc]).abs() > 0.02 {
                    found_cc = Some(cc as u8);
                    break;
                }
            }
            if let Some(cc) = found_cc {
                let key = self.midi_learn_target.take().unwrap();
                self.midi_learn_mappings.retain(|e| e.cc != cc && e.parameter != key);
                self.midi_learn_mappings.push(MidiLearnEntry { cc, parameter: key });
            }
        }

        // Step 3 — apply mappings, but only when the CC actually moved.
        // Firing every frame would continuously override the knob with
        // `range_start` while the CC sits at 0, making the knob unusable.
        let mappings: Vec<MidiLearnEntry> = self.midi_learn_mappings.clone();
        for entry in &mappings {
            let cc_idx = entry.cc as usize;
            let cc_val = snapshot.cc_values[cc_idx];
            let prev_val = self.prev_cc_values[cc_idx];
            if (cc_val - prev_val).abs() > 1e-4 {
                if let Some((param_id, range_start, range_end)) = param_range_for_key(&entry.parameter) {
                    let value = range_start + cc_val * (range_end - range_start);
                    self.events
                        .send(synth_engine::EngineEvent::ParameterChange { id: param_id, value });
                    // Keep the UI field in sync so the knob shows the CC position.
                    self.sync_cc_to_ui(&entry.parameter, value);
                }
            }
        }

        self.prev_cc_values = snapshot.cc_values;
    }

    /// Updates the UI-local field that mirrors `key` so the knob display
    /// stays in agreement after a CC routing event.
    fn sync_cc_to_ui(&mut self, key: &str, value: f32) {
        match key {
            "filter_cutoff_hz" => self.filter_cutoff_hz = value,
            "filter_resonance" => self.filter_resonance = value,
            "master_volume" => self.master_volume = value,
            "pitch_offset_semis" => self.pitch_offset_semis = value,
            "bpm" => self.bpm = value,
            "amp_attack_secs" => self.amp_attack_secs = value,
            "amp_decay_secs" => self.amp_decay_secs = value,
            "amp_sustain_level" => self.amp_sustain_level = value,
            "amp_release_secs" => self.amp_release_secs = value,
            "env2_attack_secs" => self.env2_attack_secs = value,
            "env2_decay_secs" => self.env2_decay_secs = value,
            "env2_sustain_level" => self.env2_sustain_level = value,
            "env2_release_secs" => self.env2_release_secs = value,
            "osc_1_level" => self.osc_level[0] = value,
            "osc_2_level" => self.osc_level[1] = value,
            "osc_3_level" => self.osc_level[2] = value,
            "sub_level" => self.sub_level = value,
            "osc_1_detune_cents" => self.osc_detune_cents[0] = value,
            "osc_2_detune_cents" => self.osc_detune_cents[1] = value,
            "osc_3_detune_cents" => self.osc_detune_cents[2] = value,
            "osc_1_pan" => self.osc_pan[0] = value,
            "osc_2_pan" => self.osc_pan[1] = value,
            "osc_3_pan" => self.osc_pan[2] = value,
            "lfo1_rate_hz" => self.lfo1_rate_hz = value,
            "lfo2_rate_hz" => self.lfo2_rate_hz = value,
            "fx_chorus_mix" => self.fx_chorus_mix = value,
            "fx_chorus_rate_hz" => self.fx_chorus_rate_hz = value,
            "fx_chorus_depth_ms" => self.fx_chorus_depth_ms = value,
            "fx_delay_mix" => self.fx_delay_mix = value,
            "fx_delay_time_secs" => self.fx_delay_time_secs = value,
            "fx_delay_feedback" => self.fx_delay_feedback = value,
            "fx_reverb_mix" => self.fx_reverb_mix = value,
            "fx_reverb_size" => self.fx_reverb_size = value,
            "fx_reverb_decay_secs" => self.fx_reverb_decay_secs = value,
            "fx_reverb_damping" => self.fx_reverb_damping = value,
            "fx_drive_drive" => self.fx_drive_drive = value,
            "arp_bpm" => self.arp_bpm = value,
            "arp_gate" => self.arp_gate = value,
            "arp_swing" => self.arp_swing = value,
            _ => {}
        }
    }
}
