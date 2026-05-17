# Licensing

**Status:** **Resolved (2026-05-17): dual-licensed `MIT OR Apache-2.0`.**

Users and contributors may use the code under either licence, at their option. This is the standard Rust ecosystem convention (used by `rustc`, `cargo`, the standard library, and most major crates).

The trade-off analysis that led to the decision is preserved below.

## Practical implications

- **Repository root** contains `LICENSE-MIT` and `LICENSE-APACHE` with the canonical full texts.
- **Every `Cargo.toml`** in the workspace sets `license = "MIT OR Apache-2.0"` (added when crates are scaffolded in M0).
- **Contributions** are implicitly under the same dual licence. **No CLA** is required.
- **`cargo deny`** is configured to allow MIT, Apache-2.0, and compatible licences (BSD, ISC, Zlib, Unicode-DFS-2016, MPL-2.0); GPL family is rejected to keep our linked-dependency surface clean for downstream commercial reuse.
- **`THIRD-PARTY-LICENSES.txt`** is generated at build time (via `cargo about` or equivalent in `xtask`) and shipped with the installer.
- **Freemium expansion packs** (paid preset bundles, etc.) can be sold under any licence the author prefers; the dual licence on the core does not restrict expansion content.
- **Future commercial work** — selling signed builds, hosted versions, support, or proprietary expansion packs — is permitted. The core code stays open; commercial value lives in brand, distribution, and curated content.

## Why this choice

Recap of the deciding factors (full Q&A in `01-vision/open-questions.md`):

| Factor | User answer | Implication |
|---|---|---|
| Should others be able to fork and sell? | "Fine — that's the deal of open source" | Permissive licence (rules out GPL and source-available). |
| Community involvement? | "Welcome but not actively cultivated" | Any licence works; permissive lowers contributor friction. |
| Charging for the core within 2 years? | "Possible — want to keep the option open" | Permissive licences allow this via support/signed-build/SaaS models; source-available would also work but adds friction. |

`MIT OR Apache-2.0` specifically (rather than MIT-only or Apache-only):

- **Maximum compatibility.** Downstream users pick whichever variant suits their context — MIT for simplicity, Apache-2.0 when they need the explicit patent grant.
- **Matches dependency licences.** Most crates listed in [`../04-tech-stack/libraries.md`](../04-tech-stack/libraries.md) are themselves MIT-or-Apache; using the same scheme avoids any awkward licence-mismatch conversations.
- **Patent protection available without enforcing.** The Apache-2.0 patent grant is there when needed; nobody is forced to read Apache's longer text when MIT suffices.

## Options considered

Kept here for completeness — they were ruled out by the analysis above.

## Why the decision matters

- A public binary with no licence statement is legally ambiguous and unfriendly to anyone wanting to redistribute, fork, or contribute.
- The licence interacts with the freemium model: if expansion packs are sold separately, they live in their own world and are unaffected by the core's licence. But the core licence determines whether someone could legally re-package and re-sell the synth itself.
- Some dependencies have licence constraints (mostly MIT/Apache-2.0 in our stack; nothing GPL-incompatible currently planned). This means we are free to pick any licence; we are not forced into GPL by transitive dependencies.

## Option A — MIT or Apache-2.0 (permissive)

**What it is:** Anyone may use, modify, redistribute, and sell the code, including commercial reuse, provided they keep the copyright notice and licence text.

**Pros:**
- Maximum adoption — easiest to embed, fork, learn from.
- Friendliest to potential contributors (no contributor licence agreement needed).
- Compatible with our entire dependency tree.

**Cons:**
- A competitor (or a chancer) can repackage, rebrand, and sell the synth without contributing back. The freemium expansion pack model partially offsets this (the value moves into curated content), but the core is fully reusable.
- No mechanism to require improvements be shared back upstream.

**When to pick:** If we want the broadest community footprint and don't mind that others can monetise our core work.

## Option B — GPL-3.0 (copyleft)

**What it is:** Anyone may use, modify, redistribute. Derivative works must be released under GPL-3.0 too, with source code provided.

**Pros:**
- Forks must stay open — improvements made by others are available to everyone.
- Used by reputable peers in this space (Surge XT, Dexed). Has community legitimacy.
- A commercial competitor can't fork, close-source, and re-sell.

**Cons:**
- Some contributors and integrators avoid GPL.
- Plugin integration into closed-source DAWs is fine (host-plugin relationship is not derivative), but bundling into closed-source products is restricted.
- Slightly more friction to attract drive-by contributors.

**When to pick:** If we want the synth itself to remain open in all its forks, and we are happy with that constraint trading off some adoption.

## Option C — Source-available (e.g. BSL, custom)

**What it is:** Source is published, but reuse is restricted (e.g. no commercial use without a separate licence; or open after N years).

**Pros:**
- Transparency of an open codebase plus commercial protection.
- Allows a future shift to a paid commercial product without rewriting history.

**Cons:**
- Not an "open source" licence by OSI definition; reduces community trust and contribution.
- Adds complexity (custom or BSL-style licences need legal review).
- Mixed track record in the audio community.

**When to pick:** If a paid commercial future is more likely than not, and we still want a published source tree for trust.

## Option D — Proprietary / closed source

**What it is:** Binary only; source is private.

**Pros:**
- Maximum control. No obligations to anyone.
- Compatible with any future business model.

**Cons:**
- Loses every benefit of open source: contributions, audit, transparency, community trust.
- Inconsistent with the v1 positioning (free download with community-friendly intent).

**When to pick:** Only if the project pivots to a commercial-first stance. Not consistent with current direction.

## Decision checklist (completed)

- [x] Licence chosen: **MIT OR Apache-2.0** (dual)
- [x] `LICENSE-MIT` and `LICENSE-APACHE` added at repo root
- [x] Copyright holder set: "The Tone Smithy Contributors" (year 2026). Swap to a personal name if preferred before public release.
- [ ] `cargo deny` config to be added at M0 scaffold (allow MIT/Apache-2.0/BSD/ISC/Zlib/Unicode/MPL-2.0; deny GPL family).
- [ ] To be mentioned in top-level README, About dialog, and installer EULA screen as those artefacts are created.
- [x] `01-vision/open-questions.md` updated to reflect the resolution.
