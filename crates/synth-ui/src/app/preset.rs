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
                    // Silence any held note first so it doesn't hang with
                    // the new patch's (possibly long) envelope settings.
                    self.events.send(EngineEvent::AllNotesOff);
                    for event in map_to_events(&preset.parameters) {
                        self.events.send(event);
                    }
                    let snap = map_to_snapshot(&preset.parameters);
                    self.sync_from_snapshot(&snap);
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
}
