# M15 — Installer and Release

**Status:** In progress (branch `milestone/m15-installer-release`)

## Goal

Turn the working build into something a stranger can download, install, launch,
and play in under a minute — per the **Done when** in
[`milestones.md`](milestones.md) and the spec in
[`../07-distribution/packaging.md`](../07-distribution/packaging.md).

---

## Done when

- A clean Windows machine can download the installer, install, launch, and play
  a preset within 60 seconds.
- `cargo xtask dist` produces the full artefact set under `target/dist/<version>/`.
- `installer/installer.iss` builds a working Inno Setup installer (signed if a
  cert is configured, unsigned otherwise).
- `CHANGELOG.md`, a getting-started doc, and README download/run instructions
  exist and are current.
- The workspace version is `1.0.0` and the `v1.0.0` tag + GitHub Release are cut.

---

## Environment split

This repo is developed in a Linux sandbox; the release is a **Windows x64**
artefact. Tasks divide into:

- **Doable here (Linux):** all the tooling and content — `xtask dist` logic, the
  `.iss` script, `about.toml` + third-party-license generation wiring, the
  release CI workflow, CHANGELOG, getting-started doc, README updates, the
  `.tsmith` file-association argv handling in the app.
- **Needs the user's Windows host:** running `ISCC.exe` (Inno Setup) to actually
  compile the installer, the manual clean-VM install/uninstall tests, code
  signing with a real certificate, and creating the GitHub Release. `xtask dist`
  and the release workflow are written so these run on the user's machine / CI
  unchanged.

---

## Open decisions (flagged — not resolved unilaterally)

These touch [`../01-vision/open-questions.md`](../01-vision/open-questions.md):

1. **Code signing certificate.** EV/OV cert obtained, or ship v1.0 unsigned with a
   SmartScreen note? Tooling is built **signing-optional**: `xtask dist` invokes
   `signtool` only when a cert is configured (env/args), so either path works
   without a rewrite. Need the user's call before the public release.
2. **Application icon.** No `.ico` asset exists yet. Ship with a placeholder /
   Inno default for now, or wait for a real icon? An icon is a binary art asset
   the agent can't author.
3. **Distribution channel(s).** GitHub Releases is the baseline; itch.io / a
   website page are optional extras that only affect README links.

---

## Progress

All Linux-doable phases are implemented on `milestone/m15-installer-release`:

- [x] **Phase 1 — `xtask dist`.** Builds the release binary and stages
  `target/dist/<version>/` (exe, both licences, CHANGELOG, README.txt,
  THIRD-PARTY-LICENSES.txt, SHA256SUMS). Installer + signing steps skip with a
  message off-Windows. Verified end-to-end in the sandbox.
- [x] **Phase 2 — `installer/installer.iss`.** Per-user install, Start Menu +
  optional desktop shortcut, opt-in `.tsmith` association, user-data-preserving
  uninstall. Icon guarded with `FileExists`. *Not yet compiled (needs `iscc`).*
- [x] **Phase 3 — `.tsmith` argv handling.** `main()` opens a preset path passed
  on the command line via `ToneSmithyApp::open_preset_file`.
- [x] **Phase 4 — third-party licences.** `about.toml` + `about.hbs`; wired into
  `xtask dist` (graceful warn when `cargo about` is absent).
- [x] **Phase 5 — docs + version.** Workspace bumped to `1.0.0`; CHANGELOG v1.0.0
  entry; `docs/getting-started.md`; README download/install + build sections.
- [x] **Phase 6 — release CI.** `.github/workflows/release.yml` builds the
  installer and publishes the GitHub Release on a `v*` tag; signing gated on a
  secret.

### Remaining — needs the user's Windows host

- Compile the installer (`cargo xtask dist` with Inno Setup installed) and smoke
  test it.
- Manual clean-VM install / launch / uninstall tests (Win 10 + 11).
- Provide `assets/icons/tonesmithy.ico` (optional; builds are green without it).
- Decide signing (cert vs. ship unsigned with the SmartScreen note).
- After sign-off: merge to `main`, tag `v1.0.0`, push to trigger the release.

## Work breakdown

### Phase 1 — `xtask dist`
Add a `dist` subcommand that, reading the version from workspace metadata:
1. `cargo build --release` (the `tonesmithy` binary).
2. Assembles `target/dist/<version>/` with: the exe, `LICENSE-MIT` + `LICENSE-APACHE`,
   `THIRD-PARTY-LICENSES.txt` (generated via `cargo about`; graceful warn if the
   tool is absent), a `CHANGELOG.md` snapshot, and `README.txt`.
3. Invokes Inno Setup (`iscc`) on `installer/installer.iss` if available, else
   warns and skips — producing `installer/tonesmithy-<version>-windows-x64.exe`.
4. Writes `SHA256SUMS` over the installer.
Cross-platform-aware: the build/assemble steps run anywhere; installer + signing
steps no-op with a clear message off-Windows.

### Phase 2 — Inno Setup script
`installer/installer.iss` per the packaging spec: install to `%ProgramFiles%\Tone
Smithy\`, Start Menu shortcut, optional desktop shortcut (default off), `.tsmith`
file association, clean uninstall that preserves `%APPDATA%\Tone Smithy\` unless
the user opts in, no prerequisites/services.

### Phase 3 — `.tsmith` file association handling
Make the file association meaningful: when launched with a preset path argument,
the app loads that preset on startup. Currently `main()` ignores argv.

### Phase 4 — Third-party licences
Add `about.toml` and wire `cargo about generate` into `xtask dist` to produce
`THIRD-PARTY-LICENSES.txt`.

### Phase 5 — Docs + version
`CHANGELOG.md` (v1.0.0 entry), `docs/getting-started.md`, README install/run
section (incl. the SmartScreen note for the unsigned case), and bump the
workspace version to `1.0.0`.

### Phase 6 — Release CI
`.github/workflows/release.yml`: on a `v*` tag, build on `windows-latest`, run
`cargo xtask dist`, and attach the installer + `SHA256SUMS` + changelog to the
GitHub Release. Signing step gated on a secret being present.

---

## Out of scope (per packaging spec §"Out of scope for v1")

Auto-update, signing automation as a hard requirement, ARM/non-x64 builds,
per-machine install, silent-install switch testing.
