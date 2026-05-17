//! Build / packaging tasks for Tone Smithy.
//!
//! Runs from the repo root with `cargo xtask <subcommand>`. Subcommands grow
//! milestone-by-milestone; M0 ships:
//!
//! - `help` — show usage.
//! - `check-deps` — enforce the architectural layering rule from
//!   `docs/planning/03-architecture/design-patterns.md` §1.1: `synth-engine`
//!   must not depend on any I/O or UI crate. CI runs this on every push.

use anyhow::{Result, bail};
use cargo_metadata::MetadataCommand;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = args.first().map(String::as_str).unwrap_or("help");

    match sub {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "check-deps" => check_deps(),
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

More subcommands will arrive in later milestones (release build, installer)."
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
