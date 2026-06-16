# Packaging

How the v1 build reaches the user.

## Installer

**Tool:** Inno Setup.

- Mature, widely-used on Windows. The licence permits free commercial use — a purchase is *not strictly required* — but the author's FAQ requests that commercial users buy a licence. Treat that as a goodwill ask to honour **if/when Tone Smithy earns revenue**, not a blocker to shipping. (Fully free, no-ask alternatives if that request is unwelcome: NSIS or WiX/MSI — both more work.)
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

When signing is in place, the `xtask dist` step invokes `signtool` automatically (gated on the
`TONESMITHY_CERT` environment variable).

## Linux package (v1.1, M19)

**Format:** a gzipped tar, `tonesmithy-<version>-linux-x64.tar.gz`.

- Built by `xtask dist` on a Linux host (CI: `ubuntu-latest`). The archive's single top-level
  `tonesmithy-<version>/` directory holds the binary (executable bit preserved), the licences,
  `README.txt`, `CHANGELOG.md`, and `THIRD-PARTY-LICENSES.txt`, so it unpacks tidily.
- **No installer.** Users unpack and run `./tonesmithy`. Audio via `cpal` (PipeWire/ALSA); MIDI via
  the ALSA sequencer through `midir`. Build/runtime system libs: ALSA, udev, libxkbcommon, Wayland
  (the same set CI installs to build).
- Built and tested against **Ubuntu 24.04**; expected to run on other modern desktops.
- `.tsmith` file association is not wired into a desktop entry in v1.1 — manual association only.

## macOS package (v1.1, M19)

**Format:** a `.dmg` disk image, `tonesmithy-<version>-macos.dmg` (Apple Silicon / arm64).

- Built by `xtask dist` on a macOS host (CI: `macos-latest`). The image's root holds a
  drag-installable `Tone Smithy.app` bundle plus the loose licences/docs and an `/Applications`
  symlink. The bundle carries an `Info.plist` (bundle id `com.tonesmithy.ToneSmithy`, the binary
  under `Contents/MacOS/`, an optional `.icns`, and a `.tsmith` document type for parity with the
  Windows file association) and a `PkgInfo` stub. Imaged via `hdiutil`. Audio via CoreAudio; MIDI
  via CoreMIDI (through `cpal` / `midir`).
- **Code signing + notarization** mirror the gated Windows path: `xtask dist` codesigns the bundle
  with `codesign` (deep, hardened runtime) when `APPLE_SIGNING_IDENTITY` is set, and notarizes +
  staples the dmg via `xcrun notarytool` when `APPLE_NOTARY_APPLE_ID` / `APPLE_NOTARY_PASSWORD` (an
  app-specific password) / `APPLE_NOTARY_TEAM_ID` are present. Absent those, it ships
  unsigned/unnotarized with a Gatekeeper-bypass note in `README.txt` (right-click → Open).

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

The `xtask dist` target stages a common set under `target/dist/<version>/` (the binary, `LICENSE-MIT`,
`LICENSE-APACHE`, `README.txt`, a `CHANGELOG.md` snapshot, and a `THIRD-PARTY-LICENSES.txt` generated
from `cargo about`), then builds the **host-appropriate** package — each runner produces only its own:

- **Windows:** `tonesmithy-<version>-windows-x64.exe` (Inno Setup installer).
- **Linux:** `tonesmithy-<version>-linux-x64.tar.gz`.
- **macOS:** `tonesmithy-<version>-macos.dmg` (holding `Tone Smithy.app`).

Each runner also writes a `SHA256SUMS` over its own dist directory; the release workflow then
collects all three platforms' packages and writes a single combined `SHA256SUMS` for the GitHub
Release (so users / package indexes can verify any download).

## Release checklist (for M15 and each subsequent release)

- [ ] Version bumped in workspace `Cargo.toml`.
- [ ] `CHANGELOG.md` updated.
- [ ] All CI green.
- [ ] Soak test run locally — 8 hours, no crashes / leaks / glitches.
- [ ] Factory bank reviewed; loudness within target; descriptions present.
- [ ] `xtask dist` produces a clean artefact set on each platform (Windows installer, Linux tarball, macOS dmg).
- [ ] Installer manually tested on a clean Windows 10 and Windows 11 VM.
- [ ] Linux tarball manually tested on a clean Ubuntu 24.04 machine (unpack, launch, play a preset).
- [ ] macOS dmg manually tested on a clean macOS machine (drag to Applications, launch, play a preset; note any Gatekeeper prompt).
- [ ] Uninstall manually tested; verify clean removal (user data preserved unless opted in).
- [ ] (If signing) Windows installer and exe signed (`signtool verify` passes); macOS bundle signed + dmg notarized (`spctl -a -t open --context context:primary-signature` passes).
- [ ] GitHub Release drafted with changelog and SHA256SUMS.
- [ ] Website / itch.io page updated.

## Out of scope for v1 packaging

- Auto-update.
- Code-signing automation as a hard requirement (deferred).
- Per-architecture builds beyond x64 (no ARM Windows builds in v1).
- Per-user vs per-machine install (per-user install is the default and only option in v1).
- Silent / unattended install switches (Inno Setup supports them, but they aren't part of the v1 feature set we're testing).
