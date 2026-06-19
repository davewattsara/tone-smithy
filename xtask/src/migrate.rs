//! One-shot factory-preset migration for M23 Phase 2.
//!
//! Background: preset `.tsmith` files are *sparse* — a parameter key is only
//! written when its value differs from the engine default at save time. Before
//! M23 Phase 3, `osc2_level` / `osc3_level` default to `1.0`, so a subtractive
//! patch that wants those oscillators at full level simply omits the keys.
//!
//! Phase 3 changes that default to `0.0` (init becomes a single-oscillator
//! voice). Without intervention, every factory preset that *omits*
//! `osc2_level` / `osc3_level` would suddenly load with those oscillators
//! muted. This subcommand writes the keys in explicitly at `1.0` wherever the
//! omission would actually change the sound, so the bank survives the default
//! change unchanged.
//!
//! What "would change the sound" means concretely: the main-oscillator levels
//! feed **slot 0 only** (always the subtractive bank); the FM bank ignores
//! them. Slot 0's contribution is scaled by `slot_level_0` (default `1.0`).
//! So a preset needs the write-in only when `slot_level_0 > 0` — a "pure FM"
//! preset (`slot_level_0 == 0`) is immune and is left untouched.
//!
//! `init.tsmith` is deliberately excluded: it carries an empty parameter map
//! and is the one patch Phase 3 *intends* to change (three coherent
//! oscillators → a single oscillator). Writing the keys in would defeat that.
//!
//! The rewrite is a targeted textual insertion that preserves the rest of each
//! file's hand-authored formatting — it does not parse and re-serialise the
//! RON, which would reformat every value.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use cargo_metadata::MetadataCommand;

/// The init patch is the deliberate exception — Phase 3 turns it into a
/// single-oscillator voice, so it must keep its empty parameter map.
const SKIP_FILES: &[&str] = &["init.tsmith"];

/// Keys this migration ensures are present (in insertion order), each with the
/// pre-Phase-3 default value that preserves the current sound.
const OSC2_KEY: &str = "osc2_level";
const OSC3_KEY: &str = "osc3_level";

/// Entry point for `cargo xtask migrate-osc-defaults [--dry-run]`.
pub fn migrate_osc_defaults(dry_run: bool) -> Result<()> {
    let meta = MetadataCommand::new().no_deps().exec()?;
    let root = meta.workspace_root.clone().into_std_path_buf();
    let factory_dir = root.join("crates").join("synth-presets").join("factory");
    if !factory_dir.is_dir() {
        bail!("factory preset directory not found at {}", factory_dir.display());
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&factory_dir)
        .with_context(|| format!("reading {}", factory_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "tsmith"))
        .collect();
    files.sort();

    let mut checked = 0usize;
    let mut modified = 0usize;

    for path in &files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if SKIP_FILES.contains(&name) {
            continue;
        }
        checked += 1;

        let content = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

        // Only the subtractive slot reads the osc levels; if it is silent the
        // default change is inaudible and the file is left alone.
        if !subtractive_slot_audible(&content) {
            continue;
        }

        let mut updated = content.clone();
        let mut added: Vec<&str> = Vec::new();

        // osc2 first so osc3 can anchor after it for a tidy osc1/2/3 grouping.
        if !has_key(&updated, OSC2_KEY) {
            updated = insert_key(&updated, OSC2_KEY, &["osc1_unison_spread", "osc1_level"])?;
            added.push(OSC2_KEY);
        }
        if !has_key(&updated, OSC3_KEY) {
            updated = insert_key(&updated, OSC3_KEY, &[OSC2_KEY, "osc1_unison_spread", "osc1_level"])?;
            added.push(OSC3_KEY);
        }

        if added.is_empty() {
            continue;
        }
        modified += 1;
        println!("{}  +{}", name, added.join(" +"));
        if !dry_run {
            fs::write(path, &updated).with_context(|| format!("writing {}", path.display()))?;
        }
    }

    println!();
    if dry_run {
        println!("dry-run: {checked} presets checked, {modified} would be modified (no files written)");
    } else {
        println!("migrate-osc-defaults: {checked} presets checked, {modified} modified");
    }
    Ok(())
}

/// True when slot 0 (the subtractive bank, the only consumer of the main
/// oscillator levels) is audible. `slot_level_0` defaults to `1.0` when the
/// key is absent, matching the engine default `slot_level = [1.0, 0.0]`.
fn subtractive_slot_audible(content: &str) -> bool {
    match read_value(content, "slot_level_0") {
        Some(v) => v > 0.0,
        None => true,
    }
}

/// Returns whether a `"key"` entry is present in the parameters map.
fn has_key(content: &str, key: &str) -> bool {
    content.contains(&format!("\"{key}\""))
}

/// Reads a numeric parameter value by key, if present.
fn read_value(content: &str, key: &str) -> Option<f32> {
    let needle = format!("\"{key}\"");
    let start = content.find(&needle)?;
    let after = &content[start + needle.len()..];
    let colon = after.find(':')?;
    let rest = &after[colon + 1..];
    let end = rest.find([',', '\n', '}']).unwrap_or(rest.len());
    rest[..end].trim().parse::<f32>().ok()
}

/// Inserts `"key": 1.0,` on its own line immediately after the first line
/// matching one of `anchors`, copying that line's indentation. The anchors are
/// tried in order; the first present in the file wins.
fn insert_key(content: &str, key: &str, anchors: &[&str]) -> Result<String> {
    for anchor in anchors {
        let needle = format!("\"{anchor}\"");
        if let Some(pos) = content.find(&needle) {
            // Find the start of the anchor's line for its indentation, and the
            // end of that line for the insertion point.
            let line_start = content[..pos].rfind('\n').map_or(0, |i| i + 1);
            let indent: String = content[line_start..pos]
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            let line_end = content[pos..].find('\n').map_or(content.len(), |i| pos + i);
            let insertion = format!("\n{indent}\"{key}\": 1.0,");
            let mut out = String::with_capacity(content.len() + insertion.len());
            out.push_str(&content[..line_end]);
            out.push_str(&insertion);
            out.push_str(&content[line_end..]);
            return Ok(out);
        }
    }
    bail!("no anchor key {anchors:?} found to insert {key} after");
}
