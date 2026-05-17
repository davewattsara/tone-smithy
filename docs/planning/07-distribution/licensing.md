# Licensing

**Status:** Decision deferred. Logged as an open question — see [`../01-vision/open-questions.md`](../01-vision/open-questions.md). Must be resolved before milestone M15 (first public release).

This document exists so that when the decision is made, the trade-offs have already been weighed.

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

## What this writer would lean toward

Two reasonable choices given the current direction:

1. **GPL-3.0 + a CLA-free contribution policy** — matches the "free synth, open project, no funny business" positioning, prevents commercial repackaging, and puts us in good company (Surge XT). Best if the freemium model is the long-term plan.

2. **MIT** — maximises adoption and contributor friction is lowest. Best if we want this to be the easiest "first open Rust synth" for others to learn from and remix.

The choice is yours. The current draft leaves it open.

## Decision checklist (to fill in at the time of decision)

- [ ] Licence chosen: ___________
- [ ] `LICENSE` file added at repo root
- [ ] Copyright holder and year set
- [ ] `cargo deny` config updated if needed (e.g. GPL would require denying GPL-incompatible dependencies on tighter terms)
- [ ] Mentioned in README, About dialog, and installer EULA screen
- [ ] Open-questions doc updated to reflect the resolution
