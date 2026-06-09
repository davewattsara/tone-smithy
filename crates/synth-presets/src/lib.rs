//! Tone Smithy preset format and I/O.
//!
//! Presets are RON files (`.tsmith`) containing a parameter snapshot,
//! metadata, and MIDI Learn mappings. See
//! `docs/planning/03-architecture/persistence.md` for the format.

pub mod factory;
pub mod format;
pub mod io;
pub mod migrate;
pub mod preset_params;
pub mod settings;

pub use factory::{
    CAT_ALL, CAT_BASS, CAT_FX, CAT_KEYS, CAT_LEAD, CAT_PAD, CAT_PLUCK, CATEGORIES, PresetEntry, factory_entries,
    factory_preset_ron, load_factory_preset, scan_dir, start_watcher,
};
pub use format::{MidiLearnEntry, Preset, PresetMetadata};
pub use io::{load, save, user_presets_dir};
pub use preset_params::{map_to_events, map_to_snapshot, snapshot_to_map};
pub use settings::{AppSettings, load_settings, save_settings};

use thiserror::Error;

/// Errors that can occur during preset save or load.
#[derive(Debug, Error)]
pub enum PresetError {
    /// File system I/O error.
    #[error("preset I/O error for {1:?}: {0}")]
    Io(std::io::Error, std::path::PathBuf),

    /// RON serialisation error.
    #[error("preset serialise error: {0}")]
    Serialise(ron::Error),

    /// RON deserialisation error.
    #[error("preset deserialise error: {0}")]
    Deserialise(ron::error::SpannedError),

    /// The preset was written by a newer version of Tone Smithy.
    #[error("preset version {0} is newer than the current schema version")]
    UnsupportedVersion(u32),
}
