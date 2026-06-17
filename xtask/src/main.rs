//! Build / packaging tasks for Tone Smithy.
//!
//! Runs from the repo root with `cargo xtask <subcommand>`. Subcommands grow
//! milestone-by-milestone:
//!
//! - `help` — show usage.
//! - `check-deps` — enforce the architectural layering rule from
//!   `docs/planning/03-architecture/design-patterns.md` §1.1: `synth-engine`
//!   must not depend on any I/O or UI crate. CI runs this on every push.
//! - `dist` — assemble the release artefact set under `target/dist/<version>/`
//!   and build the host-appropriate package: on Windows (with Inno Setup) the
//!   installer; on Linux a `.tar.gz`; on macOS a `.dmg` containing the `.app`
//!   bundle. Each runner produces its own package.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use cargo_metadata::MetadataCommand;
use sha2::{Digest, Sha256};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = args.first().map(String::as_str).unwrap_or("help");

    match sub {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "check-deps" => check_deps(),
        "dist" => dist(),
        other => bail!("unknown xtask subcommand: {other}\n\n{}", help_text()),
    }
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    "\
xtask — build and packaging tasks for Tone Smithy.

USAGE:
    cargo xtask <subcommand>

SUBCOMMANDS:
    help          Show this message.
    check-deps    Enforce architectural layering rules on the workspace's
                  Cargo manifests (see docs/planning/03-architecture/
                  design-patterns.md §1.1).
    dist          Build the release binary and assemble the distribution
                  artefacts under target/dist/<version>/, then build the
                  host-appropriate package: on Windows with Inno Setup (iscc)
                  on PATH, the installer (installer/installer.iss); on Linux,
                  a tonesmithy-<version>-linux-x64.tar.gz; on macOS, a
                  tonesmithy-<version>-macos.dmg holding the Tone Smithy.app
                  bundle. Set TONESMITHY_CERT (+ optional
                  TONESMITHY_CERT_PASSWORD) to sign the exe and installer via
                  signtool on Windows. Set APPLE_SIGNING_IDENTITY to codesign
                  the macOS bundle, and APPLE_NOTARY_APPLE_ID /
                  APPLE_NOTARY_PASSWORD / APPLE_NOTARY_TEAM_ID to notarize the
                  dmg."
}

/// Layering rule: a workspace crate that may not depend on any of the named
/// crates. Direct manifest dependencies only — transitive deps through
/// permitted crates are allowed (e.g. `synth-engine -> thiserror -> proc-macro2`
/// is fine).
struct LayeringRule {
    /// Name of the workspace crate the rule applies to.
    crate_name: &'static str,

    /// Crate names this crate must not depend on directly.
    forbidden: &'static [&'static str],

    /// Human-readable rationale, printed when a violation is found.
    reason: &'static str,
}

const LAYERING_RULES: &[LayeringRule] = &[LayeringRule {
    crate_name: "synth-engine",
    forbidden: &["cpal", "midir", "eframe", "egui", "egui_extras", "rfd", "notify"],
    reason: "synth-engine is the hexagon core; I/O and UI must live in adapter crates \
             (docs/planning/03-architecture/design-patterns.md §1.1)",
}];

fn check_deps() -> Result<()> {
    let meta = MetadataCommand::new().no_deps().exec()?;

    let mut violations: Vec<String> = Vec::new();
    let mut rules_checked = 0usize;

    for rule in LAYERING_RULES {
        let Some(pkg) = meta
            .workspace_packages()
            .into_iter()
            .find(|p| p.name.as_str() == rule.crate_name)
        else {
            bail!(
                "layering rule references unknown workspace crate `{}`; update LAYERING_RULES",
                rule.crate_name,
            );
        };
        rules_checked += 1;

        for dep in &pkg.dependencies {
            if rule.forbidden.contains(&dep.name.as_str()) {
                violations.push(format!("  {} → {}  ({})", rule.crate_name, dep.name, rule.reason,));
            }
        }
    }

    if violations.is_empty() {
        println!("check-deps: OK ({rules_checked} rule(s) checked, no forbidden dependencies)");
        Ok(())
    } else {
        eprintln!("check-deps: {} forbidden dependency violation(s):", violations.len());
        for v in &violations {
            eprintln!("{v}");
        }
        bail!("layering check failed")
    }
}

/// Assemble the release artefact set.
///
/// Cross-platform: the build and assembly steps run on any host, so the bundle
/// can be inspected from the Linux dev sandbox. The Windows-only steps —
/// compiling the Inno Setup installer and `signtool` signing — are skipped with
/// a clear message when their tools are unavailable, so the same command works
/// unchanged in CI on `windows-latest`.
fn dist() -> Result<()> {
    let meta = MetadataCommand::new().no_deps().exec()?;
    let version = meta
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == "synth-app")
        .map(|p| p.version.to_string())
        .context("workspace does not contain a `synth-app` package to read the version from")?;
    let root = meta.workspace_root.clone().into_std_path_buf();

    println!("dist: building Tone Smithy v{version}");

    // 1. Release build of the standalone app.
    run(Command::new("cargo").args(["build", "--release", "-p", "synth-app"]))
        .context("release build of synth-app failed")?;

    // The binary is named `tonesmithy` (see synth-app/Cargo.toml [[bin]]).
    let exe_name = if cfg!(windows) { "tonesmithy.exe" } else { "tonesmithy" };
    let built_exe = meta.target_directory.join("release").join(exe_name);
    if !built_exe.as_std_path().exists() {
        bail!("expected release binary at {built_exe} but it is missing");
    }

    // 2. Assemble target/dist/<version>/.
    let dist_dir = meta.target_directory.join("dist").join(&version).into_std_path_buf();
    if dist_dir.exists() {
        fs::remove_dir_all(&dist_dir).with_context(|| format!("clearing stale dist dir {}", dist_dir.display()))?;
    }
    fs::create_dir_all(&dist_dir).with_context(|| format!("creating dist dir {}", dist_dir.display()))?;

    copy_into(built_exe.as_std_path(), &dist_dir)?;
    copy_into(&root.join("LICENSE-MIT"), &dist_dir)?;
    copy_into(&root.join("LICENSE-APACHE"), &dist_dir)?;
    copy_into(&root.join("CHANGELOG.md"), &dist_dir)?;

    fs::write(dist_dir.join("README.txt"), readme_txt(&version)).context("writing README.txt")?;

    third_party_licenses(&root, &dist_dir.join("THIRD-PARTY-LICENSES.txt"))?;

    // Sign the bare exe before it is wrapped by the installer.
    sign_if_configured(&dist_dir.join(exe_name))?;

    // 3. Build the host-appropriate package. Each runner produces its own:
    //    Windows → Inno Setup installer; Linux → .tar.gz; macOS → .dmg. Each step
    //    self-skips off its platform with a message, so the same `cargo xtask
    //    dist` command works unchanged on every runner.
    let installer = build_installer(&root, &version, &dist_dir)?;
    build_linux_tarball(&version, &dist_dir)?;
    build_macos_dmg(&root, &version, &dist_dir)?;

    // 4. SHA256SUMS over the shipped artefacts.
    write_sha256sums(&dist_dir, installer.as_deref())?;

    println!("dist: artefacts assembled in {}", dist_dir.display());
    Ok(())
}

/// Copy `src` into directory `dir`, preserving the file name.
fn copy_into(src: &Path, dir: &Path) -> Result<()> {
    let name = src
        .file_name()
        .with_context(|| format!("source path {} has no file name", src.display()))?;
    let dest = dir.join(name);
    fs::copy(src, &dest).with_context(|| format!("copying {} -> {}", src.display(), dest.display()))?;
    Ok(())
}

/// Cargo's binary directory (`$CARGO_HOME/bin`, else `~/.cargo/bin`). This is
/// where `cargo install` puts subcommands like `cargo-about`. CI installs the
/// tool here, but the directory isn't always on the inherited `PATH` (seen on
/// the macOS runner), so we resolve it explicitly.
fn cargo_bin_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        return Some(PathBuf::from(home).join("bin"));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".cargo").join("bin"))
}

/// Locate the `cargo-about` subcommand binary: `PATH` first, then Cargo's bin
/// dir. Returns `None` only when the tool is genuinely not installed.
fn find_cargo_about() -> Option<PathBuf> {
    if let Some(p) = which("cargo-about") {
        return Some(p);
    }
    let name = if cfg!(windows) {
        "cargo-about.exe"
    } else {
        "cargo-about"
    };
    let candidate = cargo_bin_dir()?.join(name);
    candidate.is_file().then_some(candidate)
}

/// Generate `THIRD-PARTY-LICENSES.txt` via `cargo about`, warning (not failing)
/// when the tool is absent so a dev-sandbox `dist` still completes. When the
/// tool *is* installed but `~/.cargo/bin` is missing from `PATH`, that directory
/// is prepended for the invocation so Cargo can still resolve the subcommand.
fn third_party_licenses(root: &Path, dest: &Path) -> Result<()> {
    if find_cargo_about().is_none() {
        eprintln!(
            "dist: warning — `cargo about` not found; skipping THIRD-PARTY-LICENSES.txt.\n\
             dist:           install with `cargo install cargo-about` to include it."
        );
        fs::write(
            dest,
            "Third-party license text was not generated for this build because\n\
             `cargo about` was unavailable. Run `cargo install cargo-about` and\n\
             rebuild to produce the full notices.\n",
        )
        .context("writing THIRD-PARTY-LICENSES placeholder")?;
        return Ok(());
    }

    let mut cmd = Command::new("cargo");
    cmd.current_dir(root).args(["about", "generate", "about.hbs"]);
    // Make sure Cargo can find the `cargo-about` subcommand even when
    // `~/.cargo/bin` isn't on the inherited PATH.
    if let Some(bin) = cargo_bin_dir() {
        let existing = std::env::var_os("PATH").unwrap_or_default();
        let mut paths: Vec<PathBuf> = std::env::split_paths(&existing).collect();
        if !paths.contains(&bin) {
            paths.insert(0, bin);
            if let Ok(joined) = std::env::join_paths(paths) {
                cmd.env("PATH", joined);
            }
        }
    }
    let out = run_capture(&mut cmd).context("running `cargo about generate`")?;
    fs::write(dest, out).with_context(|| format!("writing {}", dest.display()))?;
    println!("dist: wrote {}", dest.display());
    Ok(())
}

/// Compile the Inno Setup installer if possible; returns the installer path when
/// one was produced. No-ops with a message off-Windows or when `iscc` is absent.
fn build_installer(root: &Path, version: &str, dist_dir: &Path) -> Result<Option<PathBuf>> {
    let script = root.join("installer").join("installer.iss");
    if !script.exists() {
        eprintln!("dist: warning — {} not found; skipping installer.", script.display());
        return Ok(None);
    }
    let Some(iscc) = which("iscc").or_else(|| which("ISCC")) else {
        eprintln!(
            "dist: Inno Setup compiler (iscc) not found on PATH; skipping installer build.\n\
             dist: run `cargo xtask dist` on Windows with Inno Setup installed to produce it."
        );
        return Ok(None);
    };

    // The .iss reads these via /D defines so the script stays version-agnostic.
    run(Command::new(iscc)
        .arg(format!("/DAppVersion={version}"))
        .arg(format!("/DDistDir={}", dist_dir.display()))
        .arg(&script))
    .context("Inno Setup compilation failed")?;

    let installer = root
        .join("installer")
        .join(format!("tonesmithy-{version}-windows-x64.exe"));
    if !installer.exists() {
        bail!("iscc reported success but {} is missing", installer.display());
    }
    // Sign the installer itself, then drop a copy into the dist dir.
    sign_if_configured(&installer)?;
    copy_into(&installer, dist_dir)?;
    println!("dist: built installer {}", installer.display());
    Ok(Some(installer))
}

/// Package the Linux release as `tonesmithy-<version>-linux-x64.tar.gz`, a
/// gzipped tar whose single top-level `tonesmithy-<version>/` directory holds
/// the binary, licences, and docs — so it unpacks tidily. Runs on Linux hosts
/// only (the binary built is for the host); a no-op elsewhere. Requires `tar`.
fn build_linux_tarball(version: &str, dist_dir: &Path) -> Result<Option<PathBuf>> {
    if !cfg!(target_os = "linux") {
        return Ok(None);
    }
    let Some(tar) = which("tar") else {
        eprintln!("dist: `tar` not found on PATH; skipping Linux tarball.");
        return Ok(None);
    };

    // Stage the shipped files under tonesmithy-<version>/. `fs::copy` preserves
    // the executable bit on Unix, so the binary stays runnable after unpacking.
    let stage_name = format!("tonesmithy-{version}");
    let stage = dist_dir.join(&stage_name);
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing stale stage dir {}", stage.display()))?;
    }
    fs::create_dir_all(&stage).with_context(|| format!("creating stage dir {}", stage.display()))?;
    for name in [
        "tonesmithy",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "CHANGELOG.md",
        "README.txt",
        "THIRD-PARTY-LICENSES.txt",
    ] {
        let src = dist_dir.join(name);
        if src.exists() {
            copy_into(&src, &stage)?;
        }
    }

    let tarball_name = format!("tonesmithy-{version}-linux-x64.tar.gz");
    run(Command::new(tar)
        .arg("-czf")
        .arg(dist_dir.join(&tarball_name))
        .arg("-C")
        .arg(dist_dir)
        .arg(&stage_name))
    .context("creating Linux tarball")?;

    // The loose staging copy is no longer needed once it is inside the archive.
    fs::remove_dir_all(&stage).with_context(|| format!("removing stage dir {}", stage.display()))?;

    let tarball = dist_dir.join(&tarball_name);
    println!("dist: built Linux tarball {}", tarball.display());
    Ok(Some(tarball))
}

/// Package the macOS release as `tonesmithy-<version>-macos.dmg`. The dmg's root
/// holds a drag-installable `Tone Smithy.app` bundle (the binary under
/// `Contents/MacOS/`, an `Info.plist`, and an optional `.icns`), the licences and
/// docs, and an `/Applications` symlink. Runs on macOS hosts only (the binary
/// built is for the host); a no-op elsewhere. Requires `hdiutil` (always present
/// on macOS).
///
/// The bundle is code-signed and the dmg notarized when the corresponding
/// environment is configured (see [`sign_macos_if_configured`] /
/// [`notarize_macos_if_configured`]); otherwise it ships unsigned with the
/// Gatekeeper-bypass note carried in `README.txt`.
fn build_macos_dmg(root: &Path, version: &str, dist_dir: &Path) -> Result<Option<PathBuf>> {
    if !cfg!(target_os = "macos") {
        return Ok(None);
    }
    let Some(hdiutil) = which("hdiutil") else {
        eprintln!("dist: `hdiutil` not found on PATH; skipping macOS dmg.");
        return Ok(None);
    };

    // Stage the dmg root under dmg-staging/: the .app bundle, loose docs, and an
    // /Applications symlink so the volume supports the usual drag-to-install.
    let stage = dist_dir.join("dmg-staging");
    if stage.exists() {
        fs::remove_dir_all(&stage).with_context(|| format!("clearing stale dmg staging dir {}", stage.display()))?;
    }
    let app = stage.join("Tone Smithy.app");
    let macos_dir = app.join("Contents").join("MacOS");
    let resources_dir = app.join("Contents").join("Resources");
    fs::create_dir_all(&macos_dir).with_context(|| format!("creating {}", macos_dir.display()))?;
    fs::create_dir_all(&resources_dir).with_context(|| format!("creating {}", resources_dir.display()))?;

    // Executable into Contents/MacOS/. fs::copy preserves the +x bit on Unix.
    copy_into(&dist_dir.join("tonesmithy"), &macos_dir)?;

    // Optional icon, authored separately as a binary art asset; absence is
    // tolerated like the Windows .ico so builds stay green until it lands.
    let icns = root.join("assets").join("icons").join("tonesmithy.icns");
    let has_icon = icns.exists();
    if has_icon {
        let dest = resources_dir.join("tonesmithy.icns");
        fs::copy(&icns, &dest).with_context(|| format!("copying {} -> {}", icns.display(), dest.display()))?;
    }

    fs::write(
        app.join("Contents").join("Info.plist"),
        macos_info_plist(version, has_icon),
    )
    .context("writing Info.plist")?;
    // PkgInfo: the classic 8-byte type/creator stub macOS still expects.
    fs::write(app.join("Contents").join("PkgInfo"), "APPL????").context("writing PkgInfo")?;

    // Loose docs alongside the .app so the mounted volume is self-describing.
    for name in [
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "CHANGELOG.md",
        "README.txt",
        "THIRD-PARTY-LICENSES.txt",
    ] {
        let src = dist_dir.join(name);
        if src.exists() {
            copy_into(&src, &stage)?;
        }
    }

    // /Applications symlink (via `ln`, so this compiles on every host even though
    // it only ever runs on macOS).
    run(Command::new("ln")
        .arg("-s")
        .arg("/Applications")
        .arg(stage.join("Applications")))
    .context("creating /Applications symlink in dmg staging")?;

    // Sign the finished bundle (deep, hardened runtime) before it is imaged.
    sign_macos_if_configured(&app)?;

    let dmg_name = format!("tonesmithy-{version}-macos.dmg");
    let dmg = dist_dir.join(&dmg_name);
    if dmg.exists() {
        fs::remove_file(&dmg).with_context(|| format!("removing stale dmg {}", dmg.display()))?;
    }
    run(Command::new(hdiutil)
        .args(["create", "-volname", "Tone Smithy", "-srcfolder"])
        .arg(&stage)
        .args(["-ov", "-format", "UDZO"])
        .arg(&dmg))
    .context("creating macOS dmg")?;

    // Notarize + staple the image when credentials are present.
    notarize_macos_if_configured(&dmg)?;

    // The staging tree is no longer needed once it is imaged into the dmg.
    fs::remove_dir_all(&stage).with_context(|| format!("removing dmg staging dir {}", stage.display()))?;

    println!("dist: built macOS dmg {}", dmg.display());
    Ok(Some(dmg))
}

/// `Info.plist` body for the `Tone Smithy.app` bundle. `CFBundleIconFile` is
/// emitted only when an `.icns` was bundled, so an icon-less build is still valid.
/// Declares the `.tsmith` preset document type for parity with the Windows file
/// association.
fn macos_info_plist(version: &str, has_icon: bool) -> String {
    let icon_entry = if has_icon {
        "    <key>CFBundleIconFile</key>\n    <string>tonesmithy</string>\n"
    } else {
        ""
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         {icon_entry}\
             <key>CFBundleName</key>\n    <string>Tone Smithy</string>\n\
             <key>CFBundleDisplayName</key>\n    <string>Tone Smithy</string>\n\
             <key>CFBundleIdentifier</key>\n    <string>com.tonesmithy.ToneSmithy</string>\n\
             <key>CFBundleExecutable</key>\n    <string>tonesmithy</string>\n\
             <key>CFBundlePackageType</key>\n    <string>APPL</string>\n\
             <key>CFBundleInfoDictionaryVersion</key>\n    <string>6.0</string>\n\
             <key>CFBundleVersion</key>\n    <string>{version}</string>\n\
             <key>CFBundleShortVersionString</key>\n    <string>{version}</string>\n\
             <key>LSMinimumSystemVersion</key>\n    <string>11.0</string>\n\
             <key>NSHighResolutionCapable</key>\n    <true/>\n\
             <key>CFBundleDocumentTypes</key>\n\
             <array>\n\
                 <dict>\n\
                     <key>CFBundleTypeName</key>\n            <string>Tone Smithy Preset</string>\n\
                     <key>CFBundleTypeExtensions</key>\n            <array>\n                <string>tsmith</string>\n            </array>\n\
                     <key>CFBundleTypeRole</key>\n            <string>Editor</string>\n\
                 </dict>\n\
             </array>\n\
         </dict>\n\
         </plist>\n"
    )
}

/// Code-sign a macOS `.app` with `codesign` when `APPLE_SIGNING_IDENTITY` names a
/// Developer ID Application identity present in the keychain. A no-op when unset
/// (the unsigned-release path) or off macOS, mirroring the Windows `signtool`
/// gate.
fn sign_macos_if_configured(app: &Path) -> Result<()> {
    // An unset GitHub Actions secret is passed through as an empty string, not
    // absent, so treat empty the same as unset — otherwise we'd codesign with a
    // blank identity.
    let identity = std::env::var("APPLE_SIGNING_IDENTITY").unwrap_or_default();
    if identity.is_empty() {
        return Ok(());
    }
    if !cfg!(target_os = "macos") {
        eprintln!(
            "dist: APPLE_SIGNING_IDENTITY set but not on macOS; skipping signing of {}",
            app.display()
        );
        return Ok(());
    }
    let Some(codesign) = which("codesign") else {
        bail!("APPLE_SIGNING_IDENTITY is set but `codesign` was not found on PATH");
    };
    run(Command::new(codesign)
        .args(["--force", "--deep", "--options", "runtime", "--timestamp", "--sign"])
        .arg(&identity)
        .arg(app))
    .with_context(|| format!("codesigning {}", app.display()))?;
    println!("dist: signed {}", app.display());
    Ok(())
}

/// Notarize and staple a `.dmg` via `xcrun notarytool` when the notarization
/// credentials are all present (`APPLE_NOTARY_APPLE_ID`, `APPLE_NOTARY_PASSWORD`
/// — an app-specific password — and `APPLE_NOTARY_TEAM_ID`). A no-op when any is
/// unset or off macOS.
fn notarize_macos_if_configured(dmg: &Path) -> Result<()> {
    // An unset GitHub Actions secret arrives as an empty string (not absent), so
    // `env::var` returns `Ok("")` rather than `Err`. Treat any empty credential
    // as "notarization not configured" and skip — otherwise `notarytool` is
    // called with blank values and fails ("Team ID must be at least 3 chars").
    let apple_id = std::env::var("APPLE_NOTARY_APPLE_ID").unwrap_or_default();
    let password = std::env::var("APPLE_NOTARY_PASSWORD").unwrap_or_default();
    let team_id = std::env::var("APPLE_NOTARY_TEAM_ID").unwrap_or_default();
    if apple_id.is_empty() || password.is_empty() || team_id.is_empty() {
        return Ok(());
    }
    if !cfg!(target_os = "macos") {
        eprintln!(
            "dist: notarization credentials set but not on macOS; skipping notarization of {}",
            dmg.display()
        );
        return Ok(());
    }
    let Some(xcrun) = which("xcrun") else {
        bail!("notarization credentials are set but `xcrun` was not found on PATH");
    };
    run(Command::new(&xcrun).args(["notarytool", "submit"]).arg(dmg).args([
        "--apple-id",
        &apple_id,
        "--password",
        &password,
        "--team-id",
        &team_id,
        "--wait",
    ]))
    .with_context(|| format!("notarizing {}", dmg.display()))?;
    run(Command::new(&xcrun).args(["stapler", "staple"]).arg(dmg))
        .with_context(|| format!("stapling {}", dmg.display()))?;
    println!("dist: notarized + stapled {}", dmg.display());
    Ok(())
}

/// Sign `target` with `signtool` when `TONESMITHY_CERT` points at a `.pfx`.
/// Silently a no-op when unset (the unsigned-release path) or off-Windows.
fn sign_if_configured(target: &Path) -> Result<()> {
    // Empty (an unset CI secret) is treated the same as unset — the unsigned path.
    let cert = std::env::var("TONESMITHY_CERT").unwrap_or_default();
    if cert.is_empty() {
        return Ok(());
    }
    if !cfg!(windows) {
        eprintln!(
            "dist: TONESMITHY_CERT set but not on Windows; skipping signing of {}",
            target.display()
        );
        return Ok(());
    }
    let Some(signtool) = which("signtool") else {
        bail!("TONESMITHY_CERT is set but `signtool` was not found on PATH");
    };

    let mut cmd = Command::new(signtool);
    cmd.args(["sign", "/fd", "SHA256", "/tr"])
        .arg("http://timestamp.digicert.com")
        .args(["/td", "SHA256", "/f"])
        .arg(&cert);
    if let Ok(pw) = std::env::var("TONESMITHY_CERT_PASSWORD") {
        cmd.arg("/p").arg(pw);
    }
    cmd.arg(target);
    run(&mut cmd).with_context(|| format!("signing {}", target.display()))?;
    println!("dist: signed {}", target.display());
    Ok(())
}

/// Write a `SHA256SUMS` file covering every regular file in `dist_dir` (the
/// installer included when one was built). Relative names keep it portable.
fn write_sha256sums(dist_dir: &Path, _installer: Option<&Path>) -> Result<()> {
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(dist_dir).context("reading dist dir for checksums")? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name != "SHA256SUMS" {
                names.push(name);
            }
        }
    }
    names.sort();

    let mut body = String::new();
    for name in &names {
        let digest = sha256_hex(&dist_dir.join(name))?;
        // Two-space separator matches the `sha256sum -c` convention.
        body.push_str(&format!("{digest}  {name}\n"));
    }
    fs::write(dist_dir.join("SHA256SUMS"), body).context("writing SHA256SUMS")?;
    println!("dist: wrote SHA256SUMS ({} file(s))", names.len());
    Ok(())
}

/// Hex-encoded SHA-256 of a file's contents.
fn sha256_hex(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("reading {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

/// Body of the bundled `README.txt`. The "getting started" and platform-note
/// sections are tailored to the host the package is built on (Windows installer,
/// Linux tarball, or macOS app), so each download ships accurate instructions.
fn readme_txt(version: &str) -> String {
    // Platform-specific launch instructions + any platform caveat.
    let (getting_started, platform_note) = if cfg!(target_os = "windows") {
        (
            "Run the installer and launch Tone Smithy from the Start Menu, or run\n\
             tonesmithy.exe directly from this folder.",
            "SmartScreen note\n\
             ----------------\n\
             This build may be unsigned. Windows SmartScreen can show \"Windows\n\
             protected your PC\". Click \"More info\" then \"Run anyway\" to continue.\n\
             \n",
        )
    } else if cfg!(target_os = "macos") {
        (
            "Drag Tone Smithy.app to Applications, then launch it from there.",
            "Gatekeeper note\n\
             ---------------\n\
             This build may be unsigned/unnotarized. macOS can show \"can't be\n\
             opened because the developer cannot be verified\". Right-click the\n\
             app and choose \"Open\", then confirm, to run it the first time.\n\
             \n",
        )
    } else {
        (
            "Unpack the archive and run ./tonesmithy from the extracted folder.\n\
             Audio uses PipeWire/ALSA and MIDI uses ALSA sequencer via the system\n\
             libraries; no extra setup is needed on a typical desktop.",
            "",
        )
    };

    format!(
        "Tone Smithy v{version}\n\
         =====================\n\
         \n\
         A hybrid (subtractive + FM) standalone software synthesizer.\n\
         \n\
         Getting started\n\
         ---------------\n\
         {getting_started} On first launch a short wizard helps you pick an audio\n\
         output and a MIDI input. No MIDI device? Play from your computer keyboard\n\
         (A-K = white keys, W-U = black keys).\n\
         \n\
         {platform_note}\
         Licences\n\
         --------\n\
         Tone Smithy is dual-licensed MIT OR Apache-2.0 (see LICENSE-MIT and\n\
         LICENSE-APACHE). Bundled third-party components are listed in\n\
         THIRD-PARTY-LICENSES.txt. Changes are recorded in CHANGELOG.md.\n"
    )
}

/// Locate an executable on PATH, returning its full path. Tries the bare name
/// and, on Windows, the `.exe` form.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let candidates: &[String] = &if cfg!(windows) && !name.ends_with(".exe") {
        vec![name.to_string(), format!("{name}.exe")]
    } else {
        vec![name.to_string()]
    };
    for dir in std::env::split_paths(&path) {
        for cand in candidates {
            let full = dir.join(cand);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// Run a command, inheriting stdio, and fail if it exits non-zero.
fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn {:?}", cmd.get_program()))?;
    if !status.success() {
        bail!("command {:?} exited with {status}", cmd.get_program());
    }
    Ok(())
}

/// Run a command, capturing stdout as a `String`; fail if it exits non-zero.
fn run_capture(cmd: &mut Command) -> Result<String> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to spawn {:?}", cmd.get_program()))?;
    if !output.status.success() {
        bail!(
            "command {:?} exited with {}\n{}",
            cmd.get_program(),
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
    }
    String::from_utf8(output.stdout).context("command output was not valid UTF-8")
}
