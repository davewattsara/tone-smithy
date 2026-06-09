# Getting started with Tone Smithy

This guide takes you from download to your first sound. Tone Smithy is a
standalone Windows application — there's no DAW or plugin host to set up.

## 1. Download

Grab the latest installer from the
[Releases page](https://github.com/OWNER/REPO/releases):

- `tonesmithy-<version>-windows-x64.exe` — the installer.
- `SHA256SUMS` — checksums, if you want to verify the download.

To verify (optional), in PowerShell:

```powershell
Get-FileHash .\tonesmithy-<version>-windows-x64.exe -Algorithm SHA256
```

and compare the result against the matching line in `SHA256SUMS`.

## 2. Install

Run the installer and follow the wizard:

- It installs **per-user**, so there's no administrator prompt.
- A **Start Menu** shortcut is added under "Tone Smithy".
- A **desktop shortcut** is offered (off by default).
- You can opt in to associating **`.tsmith`** preset files, so double-clicking a
  preset opens it in Tone Smithy.

### The SmartScreen warning

If v1.0 ships unsigned, Windows SmartScreen may show **"Windows protected your
PC"** on first launch. This is expected for new, unsigned software — it isn't a
virus warning. Click **More info**, then **Run anyway**. Once the project has a
code-signing certificate, this prompt disappears.

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

Save your own sounds with **Save As**; they land in your user preset folder
(`%APPDATA%\Tone Smithy\presets\`) and show up under **USER** in the browser.

## Uninstalling

Uninstall from **Settings → Apps → Tone Smithy**. Your settings and user presets
in `%APPDATA%\Tone Smithy\` are left untouched so a reinstall keeps your work;
delete that folder by hand if you want a clean slate.

## Troubleshooting

- **No sound:** check **Settings** has the right audio output selected, and that
  your system volume / output device isn't muted.
- **MIDI keyboard not responding:** make sure it was connected, then selected
  under **Settings → MIDI**. If it was plugged in after launch, re-select the
  port (or unplug and replug the device) so Tone Smithy picks it up.
- **Stuck note:** click **Panic** in the header.
