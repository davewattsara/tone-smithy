//! Schema migration for preset files.
//!
//! When a preset is loaded, its `version` field is compared to
//! [`crate::format::CURRENT_VERSION`]. For each step from the file's
//! version up to the current version, the corresponding migration
//! function mutates the parameter map in place. Unknown keys from a
//! future version are left untouched (forward-compat reads).

use std::collections::BTreeMap;

use crate::PresetError;

/// Applies all outstanding migrations to `parameters`, advancing
/// `from_version` up to [`crate::format::CURRENT_VERSION`].
///
/// Currently a no-op because we are at version 1 and there is
/// no previous version to migrate from.
pub fn migrate(_parameters: &mut BTreeMap<String, f32>, from_version: u32) -> Result<(), PresetError> {
    if from_version > crate::format::CURRENT_VERSION {
        return Err(PresetError::UnsupportedVersion(from_version));
    }
    // Future: match from_version { 1 => step_1_to_2(parameters), ... }
    Ok(())
}
