use synth_engine::EngineEvent;
use synth_engine::param_bus::load_snapshot;
use synth_presets::{Preset, map_to_events, map_to_snapshot, snapshot_to_map};

use super::state::ToneSmithyApp;

impl ToneSmithyApp {
    pub(crate) fn save_preset(&mut self) {
        let snapshot = load_snapshot(&self.snapshot_slot);
        let mut preset = Preset::new(self.patch_name.clone());
        preset.parameters = snapshot_to_map(&snapshot);
        preset.midi_learn = self.midi_learn_mappings.clone();

        let default_filename = format!("{}.tsmith", self.patch_name);
        let start_dir = synth_presets::user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

        if let Some(path) = rfd::FileDialog::new()
            .set_title("Save Preset")
            .set_file_name(&default_filename)
            .add_filter("Tone Smithy Preset", &["tsmith"])
            .set_directory(&start_dir)
            .save_file()
        {
            if let Err(e) = synth_presets::save(&path, &preset) {
                self.preset_error = Some(format!("Save failed: {e}"));
            } else {
                self.preset_error = None;
            }
        }
    }

    pub(crate) fn load_preset(&mut self) {
        let start_dir = synth_presets::user_presets_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

        if let Some(path) = rfd::FileDialog::new()
            .set_title("Load Preset")
            .add_filter("Tone Smithy Preset", &["tsmith"])
            .set_directory(&start_dir)
            .pick_file()
        {
            match synth_presets::load(&path) {
                Ok(preset) => {
                    self.apply_preset_params(&preset.parameters);
                    self.patch_name = preset.metadata.name.clone();
                    self.midi_learn_mappings = preset.midi_learn.clone();
                    self.preset_error = None;
                }
                Err(e) => {
                    self.preset_error = Some(format!("Load failed: {e}"));
                }
            }
        }
    }

    /// Applies a loaded preset's parameters to the engine and the UI.
    ///
    /// Presets are sparse — they store only the parameters that matter to
    /// the patch and rely on engine defaults for the rest. Driving the
    /// engine from that sparse map alone would leave every omitted
    /// parameter at the *previously loaded* preset's value, so switching
    /// patches smears state from one into the next (the first load from a
    /// fresh default engine looks fine; every load after it inherits
    /// stale values). We rebuild the full parameter set — defaults
    /// overlaid with the preset's values — and emit an event for every
    /// parameter, guaranteeing a clean, complete load. A preceding
    /// AllNotesOff stops any held note so it can't hang under the new
    /// patch's envelope.
    pub(crate) fn apply_preset_params(&mut self, params: &std::collections::BTreeMap<String, f32>) {
        self.events.send(EngineEvent::AllNotesOff);
        let snap = map_to_snapshot(params);
        for event in map_to_events(&snapshot_to_map(&snap)) {
            self.events.send(event);
        }
        self.sync_from_snapshot(&snap);
    }
}
