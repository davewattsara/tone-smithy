# Out of scope (v1)

Explicit non-features. Each is listed with a short rationale so we do not revisit the decision casually. Anything reconsidered should move to [`roadmap.md`](roadmap.md) with a target version.

## Platforms

- **macOS / Linux builds** — Out of scope for v1.0 only; delivered in v1.1 (M19). Linux tarball and macOS DMG ship alongside the Windows installer from v1.1.0.
- **Plugin formats (VST3, CLAP, AU, AAX)** — Standalone gets the engine to users fastest. CLAP is the planned first plugin target in v2.

## Synthesis engines

- **Wavetable synthesis** — Worthwhile, but a different product. Adding it to v1 dilutes the hybrid (subtractive + FM) identity.
- **Granular / sample playback** — Same rationale.
- **Physical modelling** — Out of scope; significant DSP investment.
- **Additive synthesis** — Out of scope.

## Modulation / playability

- **MPE (per-note expression)** — Voice manager will keep the door open architecturally, but channel routing and per-note pitch/timbre/pressure are not implemented in v1.
- **Microtuning / Scala** — Standard 12-TET only in v1.
- **Per-voice unison spread / chord modes** — Unison exists per oscillator; full unison/chord modes deferred.

## Audio features

- **Global oversampling** — No user-controlled oversampling. Internal oversampling may be used inside specific modules (FM operators in particular) where audibly necessary.
- **Multiple audio outputs** — Single stereo out in v1.
- **Sidechain input** — Not supported.
- **Built-in audio recording / bouncing** — Defer to v1.x.

## UX

- **Theming / skinning** — Single dark theme in v1. Theme system deferred.
- **Resizable / pop-out preset browser** — Browser is a fixed panel/sidebar in v1.
- **Drag-and-drop modulation assignment** — v1 uses an explicit matrix view. Drag-drop is a nice v1.x polish.
- **Tutorial / onboarding flow** — A static getting-started note in the help menu is sufficient for v1.

## Distribution / commerce

- **Telemetry, analytics, "phone home"** — Never. Crash logs are local-only unless the user attaches them to an issue.
- **Cloud presets, accounts, sign-in** — No.
- **In-app store for expansion packs** — Even if a freemium model is later adopted, the v1 release does not include any storefront UI. Expansions would install as ordinary preset packs.
- **Auto-update** — Deferred to v1.1.
