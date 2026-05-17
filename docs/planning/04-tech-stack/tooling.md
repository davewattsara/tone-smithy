# Tooling

Developer tooling, build process, CI, profiling, and packaging.

## Toolchain

- Pinned via `rust-toolchain.toml`:
  - `channel = "stable"`
  - MSRV decided at the start of M0 and reviewed at each milestone.
- `rustup` is the assumed toolchain manager.
- All builds are 64-bit (`x86_64-pc-windows-msvc`).

## Formatting

- `rustfmt` with a project `rustfmt.toml`:
  - `edition = "2024"` (or the latest stable edition at project start)
  - `max_width = 120`
  - Default everything else.
- `cargo fmt --check` runs in CI; failures block merge.

## Linting

- `clippy` with **deny on warnings** in CI.
- Project-wide allows for documented exceptions in a `clippy.toml` (kept tight; each exception explained inline).
- Aim for zero `#[allow(...)]` attributes outside `clippy.toml`; if one is unavoidable, comment with the reason.

## Testing

- `cargo test --workspace` runs:
  - Unit tests per crate.
  - Integration tests for engine snapshots and preset round-trips.
  - `assert_no_alloc` audio-path tests.
- `cargo test --workspace --release` runs once per CI build to catch optimisation-only bugs (denormals, fast-math behaviours).

## Benchmarks

- `criterion` benchmarks live in `crates/synth-engine/benches/`.
- Benchmarks are not part of CI gating (too noisy on shared runners). They are run locally before/after work on DSP hot paths, with results recorded in PR descriptions.

## Pre-commit

- A simple Git hook (committed to `.githooks/pre-commit` and enabled via `core.hooksPath`) runs:
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets`
- The hook can be bypassed with `--no-verify` (discouraged); CI is the source of truth.

## Continuous integration

GitHub Actions, single workflow file `.github/workflows/ci.yml`. Jobs:

1. **Build & test** — `windows-latest`, stable toolchain, `cargo build --workspace`, `cargo test --workspace`, `cargo test --workspace --release`.
2. **Lint** — `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
3. **Licence & advisory** — `cargo deny check` (licences, advisories, bans, sources).
4. **Audio safety** — runs the `assert_no_alloc` test suite specifically (covered by job 1 but called out so a regression here fails loudly).

CI runs on push to any branch and on PR open / update. Merging to `main` is gated on all jobs passing.

## Profiling

- **`tracing-tracy`** bridge for live profiling sessions during development. Tracy gives per-frame and per-callback breakdowns.
- **Superluminal** as an alternative when sampling profiling is preferred.
- Audio callback profiling is gated by a feature flag (`profile`) so production builds don't pay for it.

## Logging

- `tracing` is configured to:
  - Debug builds: log to stderr at `INFO`, optionally `DEBUG` per module.
  - Release builds: log to a rolling file (max 5 files, 5 MB each) under `%APPDATA%\Tone Smithy\logs\`, at `INFO`.
- Audio thread does not log in release builds. A non-blocking ring may be used in debug builds and flushed by a separate thread.

## Build / release artefacts

- A small `xtask` crate orchestrates release steps:
  - `xtask build --release` — workspace build.
  - `xtask installer` — produce the installer (Inno Setup invocation).
  - `xtask dist` — collect artefacts (exe + installer + license bundle) into `target/dist/`.
- Versioning follows SemVer. The version is set in the workspace `Cargo.toml`; `xtask` reads it.

## Installer

- **Inno Setup** is the chosen installer tool for v1 (over WiX/MSI):
  - Easier to script and maintain.
  - Familiar UX for end users.
  - Free for commercial use.
- Installer is built from `installer/installer.iss`.
- Code signing is deferred — see [`../07-distribution/packaging.md`](../07-distribution/packaging.md).

## Documentation

- Public-facing user docs (a getting-started README and a brief manual) live under `docs/user/` (created as needed).
- Architecture and design docs live under `docs/planning/` (this folder).
- API docs via `cargo doc` are useful internally even though the crates are not published.

## Editor / IDE

- VS Code with `rust-analyzer` and `CodeLLDB` is the assumed default.
- `.vscode/settings.json` is **not** committed — personal config.
- A `.editorconfig` is committed to enforce shared whitespace defaults.

## Reproducibility

- Lockfile (`Cargo.lock`) committed.
- CI uses a clean checkout; no cached `target/` shared across PRs.
- Builds are reproducible up to compiler / linker timestamps; bit-for-bit reproducibility is not a goal for v1.
