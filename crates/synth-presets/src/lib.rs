//! Tone Smithy preset format and I/O.
//!
//! Presets are RON files (`.tsmith`) containing a parameter snapshot,
//! metadata, and MIDI Learn mappings. See
//! `docs/planning/03-architecture/persistence.md` for the format.

pub mod format;
pub mod io;
pub mod migrate;
pub mod preset_params;

pub use format::{MidiLearnEntry, Preset, PresetMetadata};
pub use io::{load, save, user_presets_dir};
pub use preset_params::{map_to_events, map_to_snapshot, snapshot_to_map};

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
