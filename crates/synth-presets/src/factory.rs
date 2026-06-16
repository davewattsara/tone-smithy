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

/// All factory preset RON strings, compiled into the binary. Grouped by
/// category in display order (Bass, Lead, Pad, Pluck, Keys, FX) with the
/// `Init` patch first; within each group the original M0-era preset leads.
static FACTORY_RAWS: &[&str] = &[
    include_str!("../factory/init.tsmith"),
    // ── Bass ──────────────────────────────────────────────────────────────
    include_str!("../factory/sub_bass.tsmith"),
    include_str!("../factory/bass_wool_stack.tsmith"),
    include_str!("../factory/bass_acid_line.tsmith"),
    include_str!("../factory/bass_reese.tsmith"),
    include_str!("../factory/bass_fm_bell_bass.tsmith"),
    include_str!("../factory/bass_upright.tsmith"),
    include_str!("../factory/bass_drive_stack.tsmith"),
    include_str!("../factory/bass_stab.tsmith"),
    include_str!("../factory/bass_tape_warmth.tsmith"),
    include_str!("../factory/bass_growl.tsmith"),
    include_str!("../factory/bass_plonk.tsmith"),
    include_str!("../factory/bass_fm_sub_layer.tsmith"),
    include_str!("../factory/bass_mono_tight.tsmith"),
    include_str!("../factory/bass_rubber.tsmith"),
    include_str!("../factory/bass_sine_sub_duo.tsmith"),
    // v1.1 additions (M20)
    include_str!("../factory/bass_reese_mk2.tsmith"),
    include_str!("../factory/bass_neuro_growl.tsmith"),
    include_str!("../factory/bass_wobble.tsmith"),
    include_str!("../factory/bass_dist_reese.tsmith"),
    include_str!("../factory/bass_amen_stab.tsmith"),
    include_str!("../factory/bass_ladder_24.tsmith"),
    include_str!("../factory/bass_split_filter.tsmith"),
    include_str!("../factory/bass_formant_growl.tsmith"),
    include_str!("../factory/bass_pluck.tsmith"),
    include_str!("../factory/bass_house.tsmith"),
    include_str!("../factory/bass_808.tsmith"),
    include_str!("../factory/bass_fm_bite.tsmith"),
    include_str!("../factory/bass_dub_sub.tsmith"),
    include_str!("../factory/bass_hoover.tsmith"),
    // ── Lead ──────────────────────────────────────────────────────────────
    include_str!("../factory/saw_lead.tsmith"),
    include_str!("../factory/lead_screamer.tsmith"),
    include_str!("../factory/lead_fm_bell.tsmith"),
    include_str!("../factory/lead_square_mono.tsmith"),
    include_str!("../factory/lead_supersaw.tsmith"),
    include_str!("../factory/lead_glass.tsmith"),
    include_str!("../factory/lead_acid.tsmith"),
    include_str!("../factory/lead_whistle.tsmith"),
    include_str!("../factory/lead_biting_pwm.tsmith"),
    include_str!("../factory/lead_unison_blade.tsmith"),
    include_str!("../factory/lead_formant.tsmith"),
    include_str!("../factory/lead_fm_feedback.tsmith"),
    include_str!("../factory/lead_vintage_solo.tsmith"),
    include_str!("../factory/lead_triangle_soft.tsmith"),
    include_str!("../factory/lead_dual_slot.tsmith"),
    // ── Pad ───────────────────────────────────────────────────────────────
    include_str!("../factory/analog_pad.tsmith"),
    include_str!("../factory/pad_fm_shimmer.tsmith"),
    include_str!("../factory/pad_string_section.tsmith"),
    include_str!("../factory/pad_dark_void.tsmith"),
    include_str!("../factory/pad_glass_choir.tsmith"),
    include_str!("../factory/pad_warm_blanket.tsmith"),
    include_str!("../factory/pad_arctic_air.tsmith"),
    include_str!("../factory/pad_hybrid_sweep.tsmith"),
    include_str!("../factory/pad_velvet.tsmith"),
    include_str!("../factory/pad_drone.tsmith"),
    include_str!("../factory/pad_brass_spread.tsmith"),
    include_str!("../factory/pad_cosmic.tsmith"),
    // ── Pluck ─────────────────────────────────────────────────────────────
    include_str!("../factory/pluck.tsmith"),
    include_str!("../factory/pluck_koto.tsmith"),
    include_str!("../factory/pluck_fm_steel.tsmith"),
    include_str!("../factory/pluck_nylon.tsmith"),
    include_str!("../factory/pluck_marimba.tsmith"),
    include_str!("../factory/pluck_pizzicato.tsmith"),
    include_str!("../factory/pluck_dulcimer.tsmith"),
    include_str!("../factory/pluck_harpsi.tsmith"),
    // ── Keys ──────────────────────────────────────────────────────────────
    include_str!("../factory/keys.tsmith"),
    include_str!("../factory/keys_rhodes_warm.tsmith"),
    include_str!("../factory/keys_wurli_edge.tsmith"),
    include_str!("../factory/keys_fm_organ.tsmith"),
    include_str!("../factory/keys_vibes.tsmith"),
    include_str!("../factory/keys_bell_chime.tsmith"),
    // ── FX ────────────────────────────────────────────────────────────────
    include_str!("../factory/fx_pad.tsmith"),
    include_str!("../factory/fx_sweep_rise.tsmith"),
    include_str!("../factory/fx_glitch_bell.tsmith"),
    include_str!("../factory/fx_alien_texture.tsmith"),
];

#[cfg(test)]
mod qa_tests {
    //! Automated QA for the factory bank. Stands in for the manual
    //! "listen to every preset" pass: each preset is parsed, driven
    //! through a real [`Engine`], and checked for the failure modes a
    //! human would otherwise catch by ear — NaNs, amplitude runaway,
    //! a dead (silent) patch, or a voice that never releases.

    use std::collections::BTreeSet;

    use synth_engine::{Engine, EngineEvent};

    use super::{CATEGORIES, FACTORY_RAWS};
    use crate::format::Preset;
    use crate::preset_params::map_to_events;

    const SAMPLE_RATE_HZ: f32 = 48_000.0;
    const BLOCK: usize = 512;
    /// Generous finite-output ceiling. There is no master limiter, so
    /// dense unison or FX-heavy patches can legitimately exceed unity;
    /// genuine runaway diverges far past this or goes non-finite.
    const RUNAWAY_CEILING: f32 = 16.0;

    /// Parses one factory raw, panicking with a helpful label on failure.
    fn parse(raw: &str) -> Preset {
        ron::from_str::<Preset>(raw).expect("factory preset must be valid RON")
    }

    /// Runs `engine` for `seconds`, returning the peak absolute sample.
    /// Asserts every sample is finite and bounded as it goes.
    fn run_for(engine: &mut Engine, seconds: f32, label: &str) -> f32 {
        let blocks = (seconds * SAMPLE_RATE_HZ / BLOCK as f32).ceil() as usize;
        let mut buffer = [0.0f32; BLOCK * 2];
        let mut peak = 0.0f32;
        for _ in 0..blocks {
            buffer.fill(0.0);
            engine.process_stereo(&mut buffer, BLOCK);
            for &s in &buffer {
                assert!(s.is_finite(), "{label}: non-finite sample {s}");
                assert!(s.abs() <= RUNAWAY_CEILING, "{label}: amplitude runaway, sample {s}");
                peak = peak.max(s.abs());
            }
        }
        peak
    }

    /// Runs `engine` for `seconds` and returns the peak over only the
    /// final ~0.25 s — used to confirm a released note has died away.
    fn run_and_tail_peak(engine: &mut Engine, seconds: f32, label: &str) -> f32 {
        let total_blocks = (seconds * SAMPLE_RATE_HZ / BLOCK as f32).ceil() as usize;
        let tail_blocks = (0.25 * SAMPLE_RATE_HZ / BLOCK as f32).ceil() as usize;
        let mut buffer = [0.0f32; BLOCK * 2];
        let mut tail_peak = 0.0f32;
        for b in 0..total_blocks {
            buffer.fill(0.0);
            engine.process_stereo(&mut buffer, BLOCK);
            for &s in &buffer {
                assert!(s.is_finite(), "{label}: non-finite sample in tail {s}");
                assert!(
                    s.abs() <= RUNAWAY_CEILING,
                    "{label}: amplitude runaway in tail, sample {s}"
                );
                if b >= total_blocks - tail_blocks {
                    tail_peak = tail_peak.max(s.abs());
                }
            }
        }
        tail_peak
    }

    #[test]
    fn every_factory_preset_parses_with_unique_name_and_valid_category() {
        let mut names = BTreeSet::new();
        for raw in FACTORY_RAWS {
            let preset = parse(raw);
            let name = preset.metadata.name.clone();
            assert!(names.insert(name.clone()), "duplicate factory preset name: {name}");
            // Init has no category; every other preset must use a known one.
            if name != "Init" {
                assert!(
                    CATEGORIES.contains(&preset.metadata.category.as_str()),
                    "{name}: unknown category {:?}",
                    preset.metadata.category
                );
            }
        }
    }

    #[test]
    fn factory_bank_has_expected_preset_count() {
        // v1.1 (M20) factory expansion, in progress. Current categorised total
        // plus the Init patch. The per-category distribution and feature-coverage
        // guards live in the tests below.
        // 29 Bass + 15 Lead + 12 Pad + 8 Pluck + 6 Keys + 4 FX = 74, plus Init.
        assert_eq!(FACTORY_RAWS.len(), 75);
    }

    #[test]
    fn every_preset_is_audible_finite_and_releases() {
        for raw in FACTORY_RAWS {
            let preset = parse(raw);
            let label = preset.metadata.name.as_str();
            // The Init patch is a silent starting point with no note
            // shaping worth exercising here; the parse test covers it.
            if label == "Init" {
                continue;
            }

            let mut engine = Engine::new(SAMPLE_RATE_HZ);
            for ev in map_to_events(&preset.parameters) {
                engine.handle(ev);
            }

            engine.handle(EngineEvent::NoteOn {
                note_midi: 60,
                velocity: 100,
            });
            // Hold long enough to clear slow attacks and Env2 sweeps.
            let hold_peak = run_for(&mut engine, 6.0, label);
            assert!(
                hold_peak > 0.01,
                "{label}: produced effectively no sound (peak {hold_peak}) — likely a dead patch"
            );

            engine.handle(EngineEvent::NoteOff { note_midi: 60 });
            // Allow the longest releases and reverb tails to drain.
            let tail_peak = run_and_tail_peak(&mut engine, 10.0, label);
            assert!(
                tail_peak < 0.25,
                "{label}: still ringing after release (tail peak {tail_peak}) — stuck voice or self-oscillation"
            );
        }
    }
}
