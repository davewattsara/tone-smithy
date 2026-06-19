//! Best-effort "a newer version is available" check.
//!
//! On startup a dedicated background thread queries the GitHub Releases API for
//! the latest release tag and, if that tag is strictly newer than the running
//! build, sends it to the UI over a channel. The UI shows a dismissible notice.
//!
//! Everything here is best-effort: no internet, a firewall block, a timeout, or
//! an unexpected response shape all just mean "no notice" — never a crash or a
//! hang. The HTTP call blocks, which is why it lives on its own thread and not
//! on the audio or UI threads. Nothing is downloaded and no telemetry is sent;
//! the only outbound request is the single GET below.

use std::sync::mpsc::Sender;
use std::time::Duration;

/// GitHub Releases "latest" endpoint for the project repository.
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/davewattsara/tone-smithy/releases/latest";

/// Spawns the background update-check thread.
///
/// The thread sleeps briefly so the audio engine and window finish initialising
/// first, performs one short-timeout GET, and sends the latest release tag on
/// `tx` only if it is newer than [`CARGO_PKG_VERSION`]. Any failure is logged at
/// debug level and otherwise ignored.
pub fn spawn(tx: Sender<String>) {
    let builder = std::thread::Builder::new().name("update-check".into());
    if let Err(e) = builder.spawn(move || run(&tx)) {
        // Failing to spawn the optional update check must never be fatal.
        tracing::debug!("update-check thread not started: {e}");
    }
}

/// Body of the update-check thread.
fn run(tx: &Sender<String>) {
    // Let the audio engine initialise before we do any blocking network I/O.
    std::thread::sleep(Duration::from_secs(2));

    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(5)).build();
    let user_agent = format!("tone-smithy/{}", env!("CARGO_PKG_VERSION"));

    let body = match agent.get(LATEST_RELEASE_URL).set("User-Agent", &user_agent).call() {
        Ok(response) => match response.into_string() {
            Ok(body) => body,
            Err(e) => {
                tracing::debug!("update check: could not read response body: {e}");
                return;
            }
        },
        Err(e) => {
            tracing::debug!("update check: request failed (offline or blocked?): {e}");
            return;
        }
    };

    let Some(tag) = parse_tag_name(&body) else {
        tracing::debug!("update check: no tag_name in response");
        return;
    };

    if is_newer(&tag, env!("CARGO_PKG_VERSION")) {
        tracing::info!("update available: {tag} (running {})", env!("CARGO_PKG_VERSION"));
        let _ = tx.send(tag);
    }
}

/// Extracts the value of the `"tag_name"` field from a GitHub release JSON body
/// without pulling in a full JSON parser. Returns `None` if the field is absent
/// or malformed (a silently-tolerated change in the API shape).
fn parse_tag_name(body: &str) -> Option<String> {
    let key = r#""tag_name""#;
    let after_key = &body[body.find(key)? + key.len()..];
    // Skip the colon and any whitespace, then require an opening quote.
    let after_colon = after_key.trim_start().strip_prefix(':')?.trim_start();
    let inner = after_colon.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_owned())
}

/// Returns `true` if release tag `tag` is a strictly newer SemVer than the
/// running `current` version. A leading `v` on either side is ignored, and any
/// non-numeric or missing component is treated as `0`, so a malformed tag can
/// only ever compare as "not newer" rather than spuriously triggering a notice.
fn is_newer(tag: &str, current: &str) -> bool {
    parse_semver(tag) > parse_semver(current)
}

/// Parses `major.minor.patch` into a comparable triple, tolerating a leading
/// `v` and missing/garbage components (which become `0`).
fn parse_semver(s: &str) -> [u32; 3] {
    let s = s.trim().trim_start_matches('v');
    let mut parts = s.splitn(3, '.').map(|p| p.trim().parse::<u32>().unwrap_or(0));
    [
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_name_from_realistic_body() {
        let body = r#"{"url":"https://...","tag_name":"v1.3.0","name":"Tone Smithy v1.3.0"}"#;
        assert_eq!(parse_tag_name(body).as_deref(), Some("v1.3.0"));
    }

    #[test]
    fn parses_tag_name_with_whitespace() {
        let body = r#"{ "tag_name" : "v2.0.1" }"#;
        assert_eq!(parse_tag_name(body).as_deref(), Some("v2.0.1"));
    }

    #[test]
    fn missing_tag_name_returns_none() {
        assert_eq!(parse_tag_name(r#"{"name":"no tag here"}"#), None);
    }

    #[test]
    fn newer_versions_are_detected() {
        assert!(is_newer("v1.3.0", "1.2.0"));
        assert!(is_newer("1.2.1", "1.2.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("v1.2.10", "1.2.9"));
    }

    #[test]
    fn same_or_older_versions_are_not_newer() {
        assert!(!is_newer("v1.2.0", "1.2.0"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(!is_newer("v1.1.9", "1.2.0"));
        assert!(!is_newer("v1.0.0", "1.2.0"));
    }

    #[test]
    fn malformed_tag_never_reads_as_newer() {
        assert!(!is_newer("garbage", "1.2.0"));
        assert!(!is_newer("", "1.2.0"));
        assert!(!is_newer("v", "1.2.0"));
    }
}
