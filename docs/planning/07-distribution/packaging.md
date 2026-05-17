# Packaging

How the v1 build reaches the user.

## Installer

**Tool:** Inno Setup.

- Free for commercial use, mature, widely-used on Windows.
- Produces a single `.exe` installer; familiar UX (Welcome → Licence → Destination → Components → Install → Finish).
- Lightweight script (`installer/installer.iss`) — no XML, no MSI tooling burden.
- Alternative considered and rejected: WiX/MSI (more powerful, more ceremony, less developer-friendly for our scale).

## Installer contents

```
%ProgramFiles%\Tone Smithy\
    tonesmithy.exe
    presets\
        factory\
            Bass\
            Lead\
            Pad\
            Pluck\
            Keys\
            FX\
    assets\
        fonts\
        icons\
    THIRD-PARTY-LICENSES.txt
    LICENSE
    README.txt
```

## Installer behaviour

- **Install location**: `%ProgramFiles%\Tone Smithy\` by default; user-changeable.
- **Start Menu shortcut** under a "Tone Smithy" folder.
- **Optional desktop shortcut** (checkbox, default off).
- **File association** for the preset extension `.tsmith`.
- **No registry pollution** beyond what's needed for the file association and uninstall registration.
- **Clean uninstall** — removes the install directory, the Start Menu folder, the file association, and the uninstall registry entry. **Does not** touch `%APPDATA%\Tone Smithy\` (user data + user presets) without the user explicitly confirming during uninstall.
- **No prerequisites** — Rust binaries are statically linked against `msvcrt` requirements; if any VC++ runtime is needed, it's bundled.
- **No services, no scheduled tasks, no background processes.**

## Code signing

**Status:** Deferred — see [`../01-vision/open-questions.md`](../01-vision/open-questions.md).

- Without a signing certificate, Windows SmartScreen will warn users on first launch ("Windows protected your PC"). This is a real friction point.
- An EV (Extended Validation) certificate eliminates this warning immediately. Cost: ~$300–$500/year (varies by issuer). It also requires a hardware token in many issuer setups.
- A standard OV (Organization Validation) certificate is cheaper but builds reputation gradually rather than starting trusted.
- For v1.0:
  - **Preferred**: EV cert obtained, installer and binary both signed.
  - **Fallback**: ship unsigned with a clear note in the README and a one-paragraph "About the SmartScreen warning" section.

When signing is in place, the `xtask installer` step invokes `signtool` automatically.

## Distribution channels

- **Project website / itch.io page / GitHub Releases** — primary download location, kept up to date.
- **GitHub Releases** — release artefacts and the changelog live here regardless of where the primary download page is.
- **KVR Audio** — listing the synth on KVR (kvraudio.com) gives access to the largest audio community for plugin discovery. Worthwhile after v1.0.

## Updates

- **No auto-update in v1.** Users check the website / Releases page.
- **v1.1 candidate:** a lightweight update check — at app start, ask GitHub Releases for the latest tag and show a non-blocking notice if a newer version exists. Strictly opt-in, no telemetry, no auto-download.

## Versioning

- **SemVer:** `MAJOR.MINOR.PATCH`.
- `MAJOR` bumps on intentional breaking changes (engine state format breakage, parameter id renames that can't be migrated).
- `MINOR` bumps for features (e.g. v1.1).
- `PATCH` bumps for fixes.
- The version is set once in the workspace `Cargo.toml`; `xtask` reads it for installer and artefact naming.

## Build artefacts

The `xtask dist` target produces, under `target/dist/<version>/`:

- `tonesmithy.exe` (release build, stripped where applicable).
- `installer/tonesmithy-<version>-windows-x64.exe`.
- `LICENSE`.
- `THIRD-PARTY-LICENSES.txt` — generated from `cargo about` or equivalent at build time.
- `CHANGELOG.md` snapshot.
- A `SHA256SUMS` file for the installer (so users / package indexes can verify the download).

## Release checklist (for M15 and each subsequent release)

- [ ] Version bumped in workspace `Cargo.toml`.
- [ ] `CHANGELOG.md` updated.
- [ ] All CI green.
- [ ] Soak test run locally — 8 hours, no crashes / leaks / glitches.
- [ ] Factory bank reviewed; loudness within target; descriptions present.
- [ ] `xtask dist` produces a clean artefact set.
- [ ] Installer manually tested on a clean Windows 10 and Windows 11 VM.
- [ ] Uninstall manually tested; verify clean removal (user data preserved unless opted in).
- [ ] (If signing) installer and exe signed; `signtool verify` passes.
- [ ] GitHub Release drafted with changelog and SHA256SUMS.
- [ ] Website / itch.io page updated.

## Out of scope for v1 packaging

- Auto-update.
- Code-signing automation as a hard requirement (deferred).
- Per-architecture builds beyond x64 (no ARM Windows builds in v1).
- Per-user vs per-machine install (per-user install is the default and only option in v1).
- Silent / unattended install switches (Inno Setup supports them, but they aren't part of the v1 feature set we're testing).
