//! Per-frame MIDI Learn tick: detect CC movement, bind it, apply mappings.

use eframe::egui;
use synth_engine::ParamSnapshot;
use synth_presets::MidiLearnEntry;

use crate::app::state::ToneSmithyApp;
use crate::app::utils::param_range_for_key;

impl ToneSmithyApp {
    /// Called once per frame from `update()`:
    /// 1. Reads any pending "MIDI Learn" intent set by the Knob widget via egui
    ///    memory (key `"ts_midi_learn_pending"`).
    /// 2. If in learn mode, watches for the first CC that moves; binds it.
    /// 3. Applies every active mapping as a ParameterChange event.
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
                let delta = (snapshot.cc_values[cc] - self.prev_cc_values[cc]).abs();
                if delta > 0.02 {
                    found_cc = Some(cc as u8);
                    break;
                }
            }
            if let Some(cc) = found_cc {
                let key = self.midi_learn_target.take().unwrap();
                // Remove any previous binding for this CC or this param.
                self.midi_learn_mappings.retain(|e| e.cc != cc && e.parameter != key);
                self.midi_learn_mappings.push(MidiLearnEntry { cc, parameter: key });
            }
        }

        // Step 3 — apply mappings: CC value → ParameterChange.
        for entry in &self.midi_learn_mappings {
            let cc_val = snapshot.cc_values[entry.cc as usize]; // 0..=1
            if let Some((param_id, range_start, range_end)) = param_range_for_key(&entry.parameter) {
                let target = range_start + cc_val * (range_end - range_start);
                self.events.send(synth_engine::EngineEvent::ParameterChange {
                    id: param_id,
                    value: target,
                });
            }
        }

        // Update prev snapshot for next frame's delta.
        self.prev_cc_values = snapshot.cc_values;
    }
}
