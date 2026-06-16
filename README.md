# Tone Smithy

A hybrid (subtractive + FM) standalone software synthesizer for Windows, Linux, and macOS, written in Rust.

> **Status:** v1.0.0 shipped (Windows). v1.1 in progress — M16–M19 complete (engine expansion, step sequencer, and Linux `.tar.gz` + macOS `.dmg` packages alongside the Windows installer, all built via `cargo xtask dist`); **M20 (factory bank expansion to ~120 presets + v1.1.0 release) is next.** Builds ship unsigned for now (see the platform notes below).
> See [`docs/planning/06-implementation/milestones.md`](docs/planning/06-implementation/milestones.md) for the milestone plan.

Tone Smithy combines analog-style subtractive synthesis with 4-operator FM in a single voice — so a patch can layer warm analog character with clean FM bell tones without switching plugins. Free download, open source, no DAW required.

## Features (v1.0 target)

- **Hybrid voice** — each of two oscillator slots can be subtractive (3 osc + sub) or 4-operator FM
- **32-voice polyphony** with oldest-released-then-quietest voice stealing
- **Filter** — multi-mode (LP / HP / BP / Notch) state-variable filter, 12 dB/oct, self-oscillation
- **Modulation** — 8-slot matrix, 2 LFOs, ADSR amp envelope + 1 additional mod envelope
- **Effects chain** — EQ → drive → chorus → delay → FDN-8 reverb
- **Arpeggiator** with sync, swing, octave range
- **Preset browser** — categories, tags, search; ~60-preset factory bank + user folder
- **Input** — MIDI hardware, on-screen virtual keyboard, computer keyboard
- **Modern flat UI** built with egui

A second filter, 24 dB/oct option, second mod envelope, 16-slot matrix, step sequencer, and factory bank expansion to ~120 presets are deferred to v1.1 to keep the v1.0 timeline tractable. See [`docs/planning/02-scope/roadmap.md`](docs/planning/02-scope/roadmap.md).

## Download & install

> Released builds attach to [GitHub Releases](../../releases). Each release
> publishes a Windows installer, a Linux tarball, and a macOS dmg, with a single
> `SHA256SUMS` covering all three.

After downloading, you can (optionally) verify the file against the matching line
in `SHA256SUMS`. On first launch a short wizard picks an audio output and MIDI
input — no MIDI device? Play from your computer keyboard.

Step-by-step help: [`docs/getting-started.md`](docs/getting-started.md).

### Windows

Download `tonesmithy-<version>-windows-x64.exe` and run it. It installs per-user
— no administrator prompt — adds a Start Menu shortcut, and optionally associates
`.tsmith` preset files. Uninstall from **Settings → Apps**.

**SmartScreen note:** builds may ship unsigned, so Windows SmartScreen can show
"Windows protected your PC". Click **More info → Run anyway** to continue. Once a
code-signing certificate is in place this warning goes away.

### Linux

Download `tonesmithy-<version>-linux-x64.tar.gz`, unpack it, and run `./tonesmithy`
from the extracted folder. Audio uses PipeWire/ALSA and MIDI uses the ALSA
sequencer via the system libraries — no extra setup on a typical desktop. Built
and tested on Ubuntu 24.04; other modern distros should work.

### macOS

Download `tonesmithy-<version>-macos.dmg` (Apple Silicon), open it, and drag
**Tone Smithy.app** to Applications. Builds may ship unsigned/unnotarized, so
macOS Gatekeeper can refuse the first launch ("can't be opened because the
developer cannot be verified"). Right-click the app and choose **Open**, then
confirm, to run it the first time.

Your settings and user presets live under your platform's application-data
directory — `%APPDATA%\Tone Smithy\` on Windows, `~/.local/share/tonesmithy/` on
Linux, and under `~/Library/Application Support/` on macOS — and are left in
place unless you remove them yourself.

## Build from source

Prerequisites: a stable Rust toolchain via [rustup](https://rustup.rs/) and a C linker. On Linux, also `libasound2-dev`, `libudev-dev`, `libxkbcommon-dev`, `libwayland-dev` for cpal and eframe. The pinned toolchain comes from `rust-toolchain.toml`.

```bash
# Build the workspace
cargo build --workspace

# Run the app (opens window, starts silent audio)
cargo run --bin tonesmithy

# Lint / test
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Enable the pre-commit hook (formats + clippy on every commit):

```bash
git config core.hooksPath .githooks
```

Package a release build (assembles `target/dist/<version>/`, then builds the
host-appropriate package: on Windows with [Inno Setup](https://jrsoftware.org/isinfo.php)
on `PATH` it compiles the installer; on Linux a `.tar.gz`; on macOS a `.dmg`
holding the `Tone Smithy.app` bundle). Each runner produces only its own package:

```bash
cargo xtask dist
```

## Project layout

```
crates/
  synth-engine/   pure DSP — oscillators, filters, envelopes, FM, effects
  synth-host/    audio (cpal) + MIDI (midir) I/O
  synth-presets/ preset format (RON), browser, file I/O
  synth-ui/      egui front end
  synth-app/     binary; the composition root
xtask/           build / packaging tasks
docs/planning/   the full design plan
docs/conversations/ daily conversation logs
```

Detailed structure: [`docs/planning/06-implementation/project-structure.md`](docs/planning/06-implementation/project-structure.md).

## Architecture

Hexagonal — `synth-engine` sits at the centre with no I/O dependencies. `synth-host`, `synth-ui`, and `synth-presets` are adapters; `synth-app` is the composition root. The audio thread is treated as hard real-time (no allocations, no locks, no syscalls); cross-thread communication runs over lock-free queues and atomic parameter snapshots.

Design plan: [`docs/planning/03-architecture/`](docs/planning/03-architecture/) — start with [`overview.md`](docs/planning/03-architecture/overview.md) and [`design-patterns.md`](docs/planning/03-architecture/design-patterns.md).

## Contributing

- Full design plan: [`docs/planning/`](docs/planning/) (start at the [README](docs/planning/README.md)).
- Working conventions (git workflow, commit cadence, branching, conversation logs): [`docs/working-conventions.md`](docs/working-conventions.md).
- AI agents working in this repo: [`CLAUDE.md`](CLAUDE.md).

Default branch is `development`; `main` is updated only at milestone boundaries.

## Licence

Dual-licensed under either of:

- MIT licence — see [`LICENSE-MIT`](LICENSE-MIT)
- Apache Licence 2.0 — see [`LICENSE-APACHE`](LICENSE-APACHE)

at your option. This is the standard Rust ecosystem convention — downstream users pick whichever variant suits their needs. Unless you explicitly state otherwise, any contribution you submit for inclusion in Tone Smithy shall be dual-licensed as above, without any additional terms or conditions.

## Acknowledgements

The DSP designs draw on Vadim Zavalishin's *The Art of VA Filter Design*, Will Pirkle's *Designing Audio Effect Plug-Ins in C++*, and the body of open-source synthesis work in Surge XT, Vital, and Dexed.

Built with [cpal](https://github.com/RustAudio/cpal), [midir](https://github.com/Boddlnagg/midir), [eframe](https://github.com/emilk/egui) / [egui](https://github.com/emilk/egui), and others — full list in [`docs/planning/04-tech-stack/libraries.md`](docs/planning/04-tech-stack/libraries.md).
