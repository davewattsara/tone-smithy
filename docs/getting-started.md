# Getting started with Tone Smithy

This guide takes you from download to your first sound. Tone Smithy is a
standalone application for Windows, Linux, and macOS — there's no DAW or plugin
host to set up.

## 1. Download

Grab the latest package for your platform from the
[Releases page](https://github.com/davewattsara/tone-smithy/releases):

- `tonesmithy-<version>-windows-x64.exe` — Windows installer.
- `tonesmithy-<version>-linux-x64.tar.gz` — Linux tarball.
- `tonesmithy-<version>-macos.dmg` — macOS disk image (Apple Silicon).
- `SHA256SUMS` — checksums for all of the above, if you want to verify the download.

To verify (optional), compare the file's SHA-256 against the matching line in
`SHA256SUMS`:

```powershell
# Windows (PowerShell)
Get-FileHash .\tonesmithy-<version>-windows-x64.exe -Algorithm SHA256
```

```bash
# Linux / macOS
sha256sum tonesmithy-<version>-linux-x64.tar.gz      # Linux
shasum -a 256 tonesmithy-<version>-macos.dmg         # macOS
```

## 2. Install

### Windows

Run the installer and follow the wizard:

- It installs **per-user**, so there's no administrator prompt.
- A **Start Menu** shortcut is added under "Tone Smithy".
- A **desktop shortcut** is offered (off by default).
- You can opt in to associating **`.tsmith`** preset files, so double-clicking a
  preset opens it in Tone Smithy.

If the build is unsigned, Windows SmartScreen may show **"Windows protected your
PC"** on first launch. This is expected for new, unsigned software — it isn't a
virus warning. Click **More info**, then **Run anyway**. Once the project has a
code-signing certificate, this prompt disappears.

### Linux

Unpack the tarball and run the binary from the extracted folder:

```bash
tar -xzf tonesmithy-<version>-linux-x64.tar.gz
cd tonesmithy-<version>
./tonesmithy
```

Audio goes through PipeWire/ALSA and MIDI through the ALSA sequencer via the
system libraries — no extra setup on a typical desktop. Built and tested on
Ubuntu 24.04; other modern distributions should work.

### macOS

Open the `.dmg` and drag **Tone Smithy.app** to the **Applications** folder, then
launch it from there. If the build is unsigned/unnotarized, Gatekeeper may refuse
the first launch with **"can't be opened because the developer cannot be
verified"**. Right-click (or Control-click) the app, choose **Open**, then
confirm in the dialog — macOS then remembers the choice for future launches.

## 3. First launch

On the first run a short wizard asks you to choose:

1. An **audio output** device (your speakers or headphones interface).
2. A **MIDI input** device, if you have one.

You can change both later under **Settings**. The status line at the top of the
window shows the active sample rate, channel count, and MIDI port.

## 4. Make a sound

- **With a MIDI keyboard:** play — that's it.
- **Without one:** use your **computer keyboard**. The bottom row plays white
  keys (`A`–`K`), the row above plays black keys (`W`, `E`, `T`, `Y`, `U`). `Z`
  and `X` shift the octave.

If a note ever sticks, click **Panic** in the header (or send MIDI All Notes
Off / All Sound Off, CC 123 / CC 120) to silence everything.

## 5. Explore presets

Open the **Presets** tab to browse the factory bank by category (Bass, Lead,
Pad, Pluck, Keys, FX). Click a preset to load it. Several presets respond to the
**mod wheel** — push it up for vibrato or movement.

Save your own sounds with **Save As**; they land in your user preset folder and
show up under **USER** in the browser. That folder lives under your platform's
application-data directory:

- **Windows:** `%APPDATA%\Tone Smithy\presets\`
- **Linux:** `~/.local/share/tonesmithy/presets/`
- **macOS:** `~/Library/Application Support/` (under the Tone Smithy data folder)

## Uninstalling

- **Windows:** uninstall from **Settings → Apps → Tone Smithy**.
- **Linux:** delete the unpacked folder.
- **macOS:** drag **Tone Smithy.app** from Applications to the Trash.

Your settings and user presets in the application-data directory above are left
untouched so a reinstall keeps your work; delete that folder by hand if you want
a clean slate.

## Troubleshooting

- **No sound:** check **Settings** has the right audio output selected, and that
  your system volume / output device isn't muted.
- **MIDI keyboard not responding:** make sure it was connected, then selected
  under **Settings → MIDI**. If it was plugged in after launch, re-select the
  port (or unplug and replug the device) so Tone Smithy picks it up.
- **Stuck note:** click **Panic** in the header.
