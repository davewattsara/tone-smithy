# M12 — Preset Browser

**Status:** In progress (branch `milestone/m12-preset-browser`)

## Goal

Give the user a real browsing experience: see all available presets in one place, filter by category or tag, search by name, click to load, and see newly-dropped presets appear automatically.

---

## What already exists

- `synth-presets` crate with `Preset`, `PresetMetadata`, `save()`, `load()`, `user_presets_dir()`
- Header bar Save / Load buttons (file-dialog based)
- `PresetMetadata` already has: `name`, `author`, `category`, `tags`, `description`

---

## In scope

### 1. Preset entry model (`synth-presets/src/browser.rs`)

```rust
pub struct PresetEntry {
    pub path: PathBuf,
    pub metadata: PresetMetadata,
    pub is_factory: bool,
}

/// Scans `dir` for `.tsmith` files. Returns one entry per file, sorted by
/// name. Files that fail to parse are silently skipped.
pub fn scan_dir(dir: &Path, is_factory: bool) -> Vec<PresetEntry>;
```

### 2. Factory presets

A handful of embedded factory presets (using `include_str!`) covering the
six v1 categories: Bass, Lead, Pad, Pluck, Keys, FX. Written as RON strings
compiled into the binary. Returned by `factory_presets()` as `Vec<PresetEntry>`
with `is_factory = true`. Paths point into the binary (or a temp dir on first
run); they are read-only.

Categories available: `"Bass"`, `"Lead"`, `"Pad"`, `"Pluck"`, `"Keys"`, `"FX"`.

### 3. File watcher

`notify` v6 monitors the user presets directory. On any create/modify/remove
event the watcher sends to a channel; the UI drains the channel each frame
and calls `refresh_preset_list()` when a message arrives. This is the mechanism
that makes newly-dropped presets appear without a manual refresh.

Watcher is owned by `ToneSmithyApp` and kept alive for the app's lifetime.
If the watcher fails to start (e.g. directory does not exist yet), the browser
still works — it just does not auto-refresh.

### 4. Browser tab UI (`sections/browser.rs`)

A new `Tab::Presets` variant.

Layout (top to bottom):
```
[ Search box (full width)                          ]
[ Category chips: All | Bass | Lead | Pad | ... ]
[──────────────────────────────────────────────────]
[ FACTORY (N)                                      ]
[  preset name     category   tags         author  ] ← click loads
[  ...                                             ]
[──────────────────────────────────────────────────]
[ USER (N)                                         ]
[  ...                                             ]  ← right-click menu
```

Right-click on a user preset: Load, Save As (replaces file), Delete, Duplicate.
Right-click on a factory preset: Load only (others greyed).

The currently-loaded preset name is highlighted in the list.

### 5. ToneSmithyApp state additions

```rust
pub(crate) preset_entries: Vec<PresetEntry>,
pub(crate) preset_search: String,
pub(crate) preset_category_filter: String, // "" = All
_file_watcher: Option<notify::RecommendedWatcher>, // kept alive; field name suppresses warning
file_watch_rx: std::sync::mpsc::Receiver<()>,
```

`refresh_preset_list()` rebuilds `preset_entries` from factory + user scan.
Called once at startup and whenever the file watcher fires.

---

## Out of scope for M12

| Item | Where it lands |
|---|---|
| MIDI Learn per preset | M13 |
| Import from zip / preset pack | M13 or post-v1 |
| Sort options (by date, author) | post-v1 |
| Folder tree inside user dir | post-v1 (flat list is enough for v1) |
| Tag editor on a preset | post-v1 |

---

## Done when

- Every `.tsmith` in the user presets dir is listed in the browser
- Dropping a new file into that dir makes it appear within ~1 s
- Category chip filters the list correctly
- Search box filters by name, author, and tags (case-insensitive)
- Clicking a preset loads it and updates the header bar patch name
- Factory presets are marked read-only; user presets have a right-click menu
- All of the above works without breaking the existing Save/Load header buttons
