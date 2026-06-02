//! Preset file I/O — save, load, and user presets directory.

use std::fs;
use std::path::{Path, PathBuf};

use crate::PresetError;
use crate::format::Preset;
use crate::migrate;

/// File extension for Tone Smithy preset files.
pub const PRESET_EXT: &str = "tsmith";

/// Returns the user preset directory (`%APPDATA%\Tone Smithy\presets\user\`
/// on Windows; XDG equivalent on Linux/macOS). Creates the directory if it
/// does not yet exist.
///
/// Returns `None` if the platform does not expose a data directory.
#[must_use]
pub fn user_presets_dir() -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("", "Tone Smithy", "Tone Smithy")?;
    let dir = base.data_dir().join("presets").join("user");
    // Create silently; errors are suppressed because the caller can
    // fall back to any writable path.
    let _ = fs::create_dir_all(&dir);
    Some(dir)
}

/// Serialises `preset` to a RON file at `path`.
///
/// Creates parent directories if they do not exist. Overwrites any
/// existing file at `path`.
pub fn save(path: &Path, preset: &Preset) -> Result<(), PresetError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| PresetError::Io(e, path.to_path_buf()))?;
    }

    let ron_config = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .separate_tuple_members(true)
        .enumerate_arrays(false);

    let text = ron::ser::to_string_pretty(preset, ron_config).map_err(PresetError::Serialise)?;

    fs::write(path, text).map_err(|e| PresetError::Io(e, path.to_path_buf()))
}

/// Loads and validates a preset from `path`.
///
/// Applies any outstanding schema migrations before returning.
pub fn load(path: &Path) -> Result<Preset, PresetError> {
    let text = fs::read_to_string(path).map_err(|e| PresetError::Io(e, path.to_path_buf()))?;
    let mut preset: Preset = ron::from_str(&text).map_err(PresetError::Deserialise)?;
    migrate::migrate(&mut preset.parameters, preset.version)?;
    preset.version = crate::format::CURRENT_VERSION;
    Ok(preset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset_params::{map_to_snapshot, snapshot_to_map};
    use synth_engine::ParamSnapshot;

    /// Full disk round-trip: snapshot → map → Preset → RON → Preset → map → snapshot.
    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn disk_round_trip() {
        let mut snap = ParamSnapshot::default();
        snap.filter_cutoff_hz = 1_234.5;
        snap.amp_attack_secs = 0.42;
        snap.arp_enabled = true;
        snap.arp_bpm = 137.0;

        let mut preset = Preset::new("test");
        preset.parameters = snapshot_to_map(&snap);

        let dir = std::env::temp_dir();
        let path = dir.join("tonesmithy_test_round_trip.tsmith");
        save(&path, &preset).expect("save failed");
        let loaded = load(&path).expect("load failed");
        let _ = fs::remove_file(&path); // clean up, ignore errors

        let got = map_to_snapshot(&loaded.parameters);
        assert_eq!(snap.filter_cutoff_hz, got.filter_cutoff_hz);
        assert_eq!(snap.amp_attack_secs, got.amp_attack_secs);
        assert_eq!(snap.arp_enabled, got.arp_enabled);
        assert_eq!(snap.arp_bpm, got.arp_bpm);
    }
}
