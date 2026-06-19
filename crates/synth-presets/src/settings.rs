//! App-level settings persisted between sessions.
//!
//! Stored as a RON file at `<data_dir>/settings.ron`. All fields are optional
//! so unknown keys from a future version are ignored on load and a missing
//! file silently returns defaults.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persistent application settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// Preferred audio output device name. `None` means "use the OS default".
    pub audio_output_device: Option<String>,

    /// Preferred MIDI input port name. `None` means "use the first available".
    pub midi_input_port: Option<String>,

    /// Path to the last successfully loaded preset file.
    pub last_preset_path: Option<PathBuf>,

    /// Set to `true` once the first-run wizard has been completed.
    #[serde(default)]
    pub first_run_complete: bool,

    /// The release tag the user last dismissed from the update notice (e.g.
    /// `"v1.3.0"`). `None` means no update has been dismissed. The notice is
    /// suppressed while the latest available tag equals this value, and
    /// reappears once a still-newer tag is published.
    #[serde(default)]
    pub dismissed_update_version: Option<String>,
}

/// Returns the path to the settings file, creating the parent directory if
/// needed. Returns `None` if the platform has no data directory.
#[must_use]
pub fn settings_path() -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("", "Tone Smithy", "Tone Smithy")?;
    let dir = base.data_dir();
    let _ = std::fs::create_dir_all(dir);
    Some(dir.join("settings.ron"))
}

/// Loads settings from disk. Returns defaults if the file does not exist or
/// fails to parse — the caller should never hard-fail on a settings load.
#[must_use]
pub fn load_settings() -> AppSettings {
    let Some(path) = settings_path() else {
        return AppSettings::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return AppSettings::default();
    };
    ron::from_str(&text).unwrap_or_default()
}

/// Saves `settings` to disk. Errors are logged but not propagated — settings
/// failures are non-fatal.
pub fn save_settings(settings: &AppSettings) {
    let Some(path) = settings_path() else {
        tracing::warn!("no data directory available — settings not saved");
        return;
    };
    let config = ron::ser::PrettyConfig::new().depth_limit(3);
    match ron::ser::to_string_pretty(settings, config) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                tracing::warn!("could not write settings to {path:?}: {e}");
            }
        }
        Err(e) => tracing::warn!("could not serialise settings: {e}"),
    }
}
