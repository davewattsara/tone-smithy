# M24 plan — Auto-update check + v1.2 release

Auto-update notification followed by the v1.2.0 release. The update check is non-blocking,
opt-out, and does not download anything automatically.

**Target version:** v1.2
**Estimate:** 1 week
**Branch:** `milestone/m24-release` (or commit directly to `development` — single-commit
changes; no multi-session risk)
**Prerequisite:** M21 + M22 + M23 complete.

---

## Phase 1 — Auto-update check

### Design

On app start, a background thread queries the GitHub Releases API for the latest release
tag. If the tag is newer than the running version (from `CARGO_PKG_VERSION`), a dismissible
notice appears in the UI. The notice disappears if dismissed and does not reappear until a
newer version is available. No download, no installer, no telemetry.

### Dependency

Add `ureq` to `crates/synth-app/Cargo.toml`:

```toml
[dependencies]
ureq = { version = "2", features = ["tls"] }
```

`ureq` is a small synchronous HTTP client — no async runtime, no dependency on tokio. It
blocks on the calling thread, which is why it must run on a dedicated background thread
rather than on the UI thread.

### Background thread

In `crates/synth-app/src/main.rs` (or an `update_check` module), spawn a thread at startup:

```rust
let update_tx = /* channel sender shared with the UI */;

std::thread::Builder::new()
    .name("update-check".into())
    .spawn(move || {
        // 2-second startup delay so the audio engine initialises first
        std::thread::sleep(std::time::Duration::from_secs(2));

        let url = "https://api.github.com/repos/<owner>/tone-smithy/releases/latest";
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(5))
            .build();

        if let Ok(response) = agent.get(url)
            .set("User-Agent", &format!("tone-smithy/{}", env!("CARGO_PKG_VERSION")))
            .call()
        {
            if let Ok(body) = response.into_string() {
                // Parse the "tag_name" field from the JSON
                // Avoid a full JSON dependency: the tag_name is always the first
                // occurrence of "tag_name" in the response body
                if let Some(tag) = parse_tag_name(&body) {
                    let _ = update_tx.send(tag);
                }
            }
        }
    })
    .expect("failed to spawn update-check thread");
```

`parse_tag_name` extracts the value of the `"tag_name"` field from a JSON string without
a full JSON parser dependency. A simple approach:

```rust
fn parse_tag_name(body: &str) -> Option<String> {
    // Looks for: "tag_name":"v1.2.0"
    let key = r#""tag_name":""#;
    let start = body.find(key)? + key.len();
    let end = body[start..].find('"')? + start;
    Some(body[start..end].to_owned())
}
```

If the shape of GitHub's API response ever changes, this fails silently (returns `None`)
rather than crashing. Acceptable — update checks are best-effort.

### Version comparison

Compare `tag` (e.g. `"v1.3.0"`) to `CARGO_PKG_VERSION` (e.g. `"1.2.0"`):

```rust
fn is_newer(tag: &str, current: &str) -> bool {
    // Strip leading "v" if present
    let tag = tag.trim_start_matches('v');
    // Naive numeric comparison: split on "." and compare each component
    let parse = |s: &str| -> [u32; 3] {
        let mut parts = s.splitn(3, '.').map(|p| p.parse().unwrap_or(0));
        [parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0)]
    };
    parse(tag) > parse(current)
}
```

### Persistence — suppress repeated notices

Store the dismissed version in the settings file (`settings.toml` or wherever app settings
are persisted). Add a field:

```toml
dismissed_update_version = "v1.3.0"   # empty string = never dismissed
```

When the update check returns a newer version, compare it to `dismissed_update_version`.
If they match, don't show the notice. If they differ (a newer version arrived), show it.

When the user dismisses the notice, write the current `tag` to `dismissed_update_version`
in the settings file.

### UI — notice

In the header bar or status strip, when an update is available and not dismissed:

```rust
if let Some(ref tag) = app.state.available_update {
    ui.label(format!("Update available: {}", tag));
    if ui.small_button("Get it").clicked() {
        open::that("https://github.com/<owner>/tone-smithy/releases/latest").ok();
    }
    if ui.small_button("x").clicked() {
        app.dismiss_update(tag.clone());
    }
}
```

The "Get it" button opens the releases page (not a download URL) in the system browser.
The "x" button dismisses and persists.

### Done when

- A mock newer version returned by a local test confirms the notice appears.
- Already on the latest version: no notice.
- Dismissed notice: no notice on restart.
- New version after dismissal: notice reappears with the new tag.
- Network unavailable (no internet, firewall): app starts normally, no crash or hang.
- Timeout (5 s) is respected; app is responsive during the check.

---

## Phase 2 — v1.2.0 release

Same flow as M15, M19, M20.

### Checklist

- [ ] `cargo test --workspace` — all tests pass.
- [ ] `cargo fmt --all --check` — no diffs.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
- [ ] `cargo deny check` — licences clean.
- [ ] Smoke-test on Windows, Linux, macOS (open app, load presets, play notes, check update notice).
- [ ] `CHANGELOG.md` — add v1.2 section:
  - List all M21–M24 features.
  - Note the OSC2/3 default change and its impact on user presets (from M23 plan).
- [ ] Version bump: `version = "1.2.0"` in all `Cargo.toml` workspace member files.
  Check that `CARGO_PKG_VERSION` in the auto-update check matches.
- [ ] Commit with message: `Prep v1.2.0 release`
- [ ] Merge `development` → `main` with `--no-ff` and message `Milestone M24: v1.2.0 release`
- [ ] Tag `v1.2.0` on `main` (annotated: `git tag -a v1.2.0 -m "Tone Smithy v1.2.0"`)
- [ ] Push tag: `git push origin v1.2.0` — CI publishes Windows, Linux, macOS artefacts.
- [ ] Verify the GitHub Release page shows three installers.
- [ ] Update `CLAUDE.md` project state line.

### Done when

GitHub Release for `v1.2.0` is live with three-platform installers and the correct
CHANGELOG text.

---

## Milestone done when

1. Update notice appears for a newer version; no notice for current version.
2. Dismissal persists; reappears for a still-newer version.
3. All M21–M23 features ship in the same binary.
4. GitHub Release publishes three-platform installers tagged `v1.2.0`.
5. `CLAUDE.md` project state updated to v1.2.

---

## Progress

- [ ] Phase 1 — Auto-update check
- [ ] Phase 2 — v1.2.0 release
