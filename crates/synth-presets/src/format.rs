//! Preset file format types.
//!
//! A `.tsmith` preset is a RON file containing a top-level [`Preset`]
//! struct. Parameters are stored as a `BTreeMap<String, f32>` so the
//! file is human-editable and forward-compatible: unknown keys from a
//! newer version are silently ignored on load.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The current preset schema version. Bumped when the migration path
/// adds a new step.
pub const CURRENT_VERSION: u32 = 1;

/// A complete Tone Smithy preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    /// Schema version — used by [`crate::migrate::migrate`] to apply
    /// any outstanding migrations on load.
    pub version: u32,

    /// Human-facing metadata.
    pub metadata: PresetMetadata,

    /// Parameter snapshot: every saveable param keyed by a stable
    /// string name. See [`crate::preset_params::snapshot_to_map`] for
    /// the key convention.
    pub parameters: BTreeMap<String, f32>,

    /// MIDI learn assignments. Empty for most presets.
    pub midi_learn: Vec<MidiLearnEntry>,
}

impl Preset {
    /// Creates a new preset with default metadata and an empty parameter
    /// map.  Callers typically fill `parameters` via
    /// [`crate::preset_params::snapshot_to_map`].
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: CURRENT_VERSION,
            metadata: PresetMetadata {
                name: name.into(),
                author: String::new(),
                category: String::new(),
                tags: Vec::new(),
                description: String::new(),
            },
            parameters: BTreeMap::new(),
            midi_learn: Vec::new(),
        }
    }
}

/// Human-facing preset metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetMetadata {
    /// Display name shown in the browser and header bar.
    pub name: String,

    /// Author string, e.g. `"Factory"` or a sound designer's name.
    pub author: String,

    /// Broad category, e.g. `"Pad"`, `"Lead"`, `"Bass"`.
    pub category: String,

    /// Free-form tags for filtering, e.g. `["analog", "warm"]`.
    pub tags: Vec<String>,

    /// Optional longer description.
    pub description: String,
}

/// One MIDI-learn assignment: a CC number mapped to a parameter.
///
/// `range_start` and `range_end` are stored so that routing does not need
/// any external lookup table — any parameter can be learned regardless of
/// whether it appears in a whitelist.  Both fields default to `0.0` so
/// preset files written before M13 load without error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiLearnEntry {
    /// MIDI CC number, 0..=127.
    pub cc: u8,
    /// Parameter key matching a key in [`Preset::parameters`].
    pub parameter: String,
    /// Minimum value of the parameter range (CC 0 → this value).
    #[serde(default)]
    pub range_start: f32,
    /// Maximum value of the parameter range (CC 1 → this value).
    #[serde(default)]
    pub range_end: f32,
}
