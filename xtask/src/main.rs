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
//!   and, on Windows with Inno Setup available, compile the installer.

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
                  artefacts under target/dist/<version>/. On Windows with Inno
                  Setup (iscc) on PATH, also compiles installer/installer.iss.
                  Set TONESMITHY_CERT (+ optional TONESMITHY_CERT_PASSWORD) to
                  sign the exe and installer via signtool."
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

    // 3. Compile the installer (Windows + Inno Setup only).
    let installer = build_installer(&root, &version, &dist_dir)?;

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

/// Generate `THIRD-PARTY-LICENSES.txt` via `cargo about`, warning (not failing)
/// when the tool is absent so a dev-sandbox `dist` still completes.
fn third_party_licenses(root: &Path, dest: &Path) -> Result<()> {
    if which("cargo-about").is_none() {
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

    let out = run_capture(
        Command::new("cargo")
            .current_dir(root)
            .args(["about", "generate", "about.hbs"]),
    )
    .context("running `cargo about generate`")?;
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

/// Sign `target` with `signtool` when `TONESMITHY_CERT` points at a `.pfx`.
/// Silently a no-op when unset (the unsigned-release path) or off-Windows.
fn sign_if_configured(target: &Path) -> Result<()> {
    let Ok(cert) = std::env::var("TONESMITHY_CERT") else {
        return Ok(());
    };
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

/// Body of the bundled `README.txt` shown alongside the installer.
fn readme_txt(version: &str) -> String {
    format!(
        "Tone Smithy v{version}\n\
         =====================\n\
         \n\
         A hybrid (subtractive + FM) standalone software synthesizer for Windows.\n\
         \n\
         Getting started\n\
         ---------------\n\
         Run the installer and launch Tone Smithy from the Start Menu, or run\n\
         tonesmithy.exe directly from this folder. On first launch a short wizard\n\
         helps you pick an audio output and a MIDI input. No MIDI device? Play\n\
         from your computer keyboard (A-K = white keys, W-U = black keys).\n\
         \n\
         SmartScreen note\n\
         ----------------\n\
         This build may be unsigned. Windows SmartScreen can show \"Windows\n\
         protected your PC\". Click \"More info\" then \"Run anyway\" to continue.\n\
         \n\
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
