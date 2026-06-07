//! Embedded factory presets and the browser data model.
//!
//! Factory presets are compiled into the binary as static RON strings. They
//! are always available, read-only, and appear in the browser under the
//! "Factory" heading.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::Receiver;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::format::{Preset, PresetMetadata};
use crate::io::PRESET_EXT;

// ── Category constants ────────────────────────────────────────────────────────

pub const CAT_ALL: &str = "";
pub const CAT_BASS: &str = "Bass";
pub const CAT_LEAD: &str = "Lead";
pub const CAT_PAD: &str = "Pad";
pub const CAT_PLUCK: &str = "Pluck";
pub const CAT_KEYS: &str = "Keys";
pub const CAT_FX: &str = "FX";

/// All category labels in display order.
pub const CATEGORIES: &[&str] = &[CAT_BASS, CAT_LEAD, CAT_PAD, CAT_PLUCK, CAT_KEYS, CAT_FX];

// ── Preset entry ──────────────────────────────────────────────────────────────

/// A lightweight reference to a preset in the browser — metadata only, no
/// parameters. Parameters are loaded on demand when the preset is selected.
#[derive(Debug, Clone)]
pub struct PresetEntry {
    /// Absolute path to the `.tsmith` file. `None` for embedded factory presets.
    pub path: Option<PathBuf>,
    /// Display metadata.
    pub metadata: PresetMetadata,
    /// `true` for factory presets (read-only); `false` for user presets.
    pub is_factory: bool,
}

impl PresetEntry {
    /// Display name shown in the browser list.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Returns `true` if the entry matches `query` (case-insensitive name,
    /// author, or tag substring match).
    #[must_use]
    pub fn matches_search(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_ascii_lowercase();
        self.metadata.name.to_ascii_lowercase().contains(&q)
            || self.metadata.author.to_ascii_lowercase().contains(&q)
            || self.metadata.tags.iter().any(|t| t.to_ascii_lowercase().contains(&q))
    }

    /// Returns `true` if the category filter matches (empty = all categories).
    #[must_use]
    pub fn matches_category(&self, category: &str) -> bool {
        category.is_empty() || self.metadata.category == category
    }
}

// ── Directory scan ────────────────────────────────────────────────────────────

/// Scans `dir` for `.tsmith` files and returns one [`PresetEntry`] per file.
///
/// Files that fail to parse are silently skipped. Entries are sorted by name.
/// `is_factory` is forwarded to each entry.
#[must_use]
pub fn scan_dir(dir: &Path, is_factory: bool) -> Vec<PresetEntry> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<PresetEntry> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some(PRESET_EXT))
        .filter_map(|e| {
            let path = e.path();
            let text = std::fs::read_to_string(&path).ok()?;
            let preset: Preset = ron::from_str(&text).ok()?;
            Some(PresetEntry {
                path: Some(path),
                metadata: preset.metadata,
                is_factory,
            })
        })
        .collect();
    entries.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));
    entries
}

// ── Factory presets ───────────────────────────────────────────────────────────

/// Returns all embedded factory presets as [`PresetEntry`] values.
///
/// Parameters are **not** parsed here; call [`crate::io::load`] with the
/// factory preset's path, or use [`factory_preset_ron`] to get the raw RON.
#[must_use]
pub fn factory_entries() -> Vec<PresetEntry> {
    FACTORY_RAWS
        .iter()
        .filter_map(|raw| {
            let preset: Preset = ron::from_str(raw).ok()?;
            Some(PresetEntry {
                path: None,
                metadata: preset.metadata,
                is_factory: true,
            })
        })
        .collect()
}

/// Returns the raw RON string for a factory preset by name, or `None` if no
/// factory preset with that name exists.
#[must_use]
pub fn factory_preset_ron(name: &str) -> Option<&'static str> {
    FACTORY_RAWS
        .iter()
        .copied()
        .find(|raw| ron::from_str::<Preset>(raw).is_ok_and(|p| p.metadata.name == name))
}

/// Parses and returns the factory [`Preset`] with the given name, or `None`.
#[must_use]
pub fn load_factory_preset(name: &str) -> Option<Preset> {
    let raw = factory_preset_ron(name)?;
    ron::from_str::<Preset>(raw).ok()
}

// ── File watcher ──────────────────────────────────────────────────────────────

/// Starts a file-system watcher on `dir`, returning the watcher (keep alive)
/// and a receiver that fires `()` on any create/modify/remove event.
///
/// If the watcher cannot be started (e.g. `dir` does not exist), returns
/// `None` for both — the browser still works without auto-refresh.
#[must_use]
pub fn start_watcher(dir: &Path) -> (Option<RecommendedWatcher>, Receiver<()>) {
    let (tx, rx) = mpsc::channel::<()>();
    let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            ) {
                let _ = tx.send(());
            }
        }
    })
    .ok()
    .and_then(|mut w| {
        w.watch(dir, RecursiveMode::NonRecursive).ok()?;
        Some(w)
    });
    (watcher, rx)
}

// ── Embedded factory preset RON ───────────────────────────────────────────────

/// All factory preset RON strings, compiled into the binary.
static FACTORY_RAWS: &[&str] = &[
    include_str!("../factory/init.tsmith"),
    include_str!("../factory/saw_lead.tsmith"),
    include_str!("../factory/analog_pad.tsmith"),
    include_str!("../factory/sub_bass.tsmith"),
    include_str!("../factory/pluck.tsmith"),
    include_str!("../factory/keys.tsmith"),
    include_str!("../factory/fx_pad.tsmith"),
];
