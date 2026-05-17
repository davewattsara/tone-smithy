//! Build / packaging tasks for Tone Smithy.
//!
//! Runs from the repo root with `cargo xtask <subcommand>`. M0 only defines
//! the entry-point and a `help` subcommand; release/install build commands
//! arrive near M15.

use anyhow::{Result, bail};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = args.first().map(String::as_str).unwrap_or("help");

    match sub {
        "help" | "--help" | "-h" => print_help(),
        other => bail!("unknown xtask subcommand: {other}\n\n{}", help_text()),
    }

    Ok(())
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
    help     Show this message.

More subcommands will arrive in later milestones (release build, installer)."
}
