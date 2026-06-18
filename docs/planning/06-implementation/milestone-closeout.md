# Milestone close-out runbook

The canonical checklist for finishing a milestone. This is the procedure to follow
*after* the implementation work is done — it covers user sign-off, the documentation
that must be brought back in sync, and the git mechanics for landing the milestone.

There are two flavours:

- **Every milestone** runs the [standard close-out](#standard-close-out-every-milestone).
- **Version (release) milestones** — the ones that cut a SemVer release and push a
  `v*` tag (e.g. M15 → v1.0.0, M20 → v1.1.0, M24 → v1.2.0) — run the standard
  close-out **plus** the [version close-out](#version-close-out-release-milestones-only)
  steps on top.

The git merge/tag mechanics live in
[`../../working-conventions.md`](../../working-conventions.md#milestone-completion-‐-merge-development-to-main);
this runbook references them rather than duplicating them.

---

## Gate: wait for user sign-off

**Do not start any close-out step until the user has tested the build and explicitly
signed off.** A green test suite and a clean `cargo build` are necessary but not
sufficient — the "done when" criteria for playable milestones are experiential.

Present the completed work, ask the user to run `cargo run --bin tonesmithy` and test
it, and wait for explicit confirmation. Finishing the code, reading the plan, or
reaching the end of a session is **not** sign-off.

---

## Standard close-out (every milestone)

Work through these in order. Don't skip the doc-sync block — keeping it to the end of
the milestone is exactly how docs fall behind.

### 1. Mark the milestone complete in the plan

In [`milestones.md`](milestones.md), add the sign-off to the `## MXX — Title` heading
line itself (matching the format used for M0, M1, …):

```
## MXX — Title — **complete (YYYY-MM-DD, tag `mXX`)**
```

Use the current date from the `currentDate` context, or ask the user if unsure.

### 2. Doc-sync sweep — update anything the milestone made stale

This is the step that has historically been missed. **Before** merging, walk this
table and update every doc whose trigger fired during the milestone. Update each in
the same logical change as the close-out, not "later".

| Doc | Update when… |
|---|---|
| [`milestones.md`](milestones.md) | Always — sign-off line (step 1). Also revisit sizing estimates at the boundary. |
| [`/README.md`](../../../README.md) | The active milestone, scope, build/run/lint/test commands, top-level layout, system deps, or licence changed. See [README triggers](../../working-conventions.md#keeping-the-readme-up-to-date). |
| [`/CLAUDE.md`](../../../CLAUDE.md) | The **"Project state at a glance"** bullet — update stage, active branch, active milestone, and scope summary so the next agent starts from truth. |
| [`open-questions.md`](../01-vision/open-questions.md) | A previously-open question was resolved during the milestone (update here **and** the relevant plan doc). |
| [`mXX-plan.md`](.) | The milestone deviated from its plan — note what changed and why. |
| [`roadmap.md`](../02-scope/roadmap.md) | Scope moved between versions, or a milestone shifted version band. |
| [`features-v1.md`](../02-scope/features-v1.md) / [`out-of-scope.md`](../02-scope/out-of-scope.md) | A feature was added, dropped, or deferred. |
| [`getting-started.md`](../../getting-started.md) | The build, run, or platform-setup steps changed. |
| [`docs/user-manual.md`](../../user-manual.md) | A user-facing feature was added or changed — see the [version close-out](#version-close-out-release-milestones-only) for the hard rule on release milestones. |

If a milestone touched something not in this table that a future reader would be
misled by, fix that too — the table is a floor, not a ceiling.

> Agent memory note: the auto-memory `project-milestone-state.md` also tracks the
> active milestone. Refresh it as part of close-out so the next session's recall is
> accurate.

### 3. Land it

Follow the git steps in
[`working-conventions.md` → Milestone completion](../../working-conventions.md#milestone-completion-‐-merge-development-to-main):
final commit on `development`, `git merge --no-ff` into `main`, then `git tag -a mXX`,
then back to `development`.

For a non-release milestone the tag is the lightweight milestone marker (`mXX`). For a
release milestone, see below — the tag is the SemVer `v*` tag instead/as well.

---

## Version close-out (release milestones only)

A version milestone ships a SemVer release and pushes a `v*` tag that triggers the
release CI (Windows + Linux + macOS artefacts). Do the standard close-out, and also:

### A. Update the user manual BEFORE finalizing the version

**This is mandatory and comes first.** The user manual ([`docs/user-manual.md`](../../user-manual.md))
must describe the release as it actually ships — do not bump the version, tag, or cut
the release until the manual is current. For each user-facing change in the release:

- Update the **version in the manual's title and intro** (`# Tone Smithy vX.Y.Z — User Manual`).
- Add or revise the **section(s)** covering any new or changed feature, and update the
  **Contents list** and its anchors to match.
- Verify cross-references (tab names, menu items, keyboard shortcuts) still match the
  shipping UI.

A release with an out-of-date manual is not finished. Treat the manual as a release
artefact, not documentation that can trail the tag.

### B. CHANGELOG

In [`/CHANGELOG.md`](../../../CHANGELOG.md), move the `[Unreleased]` notes into a new
`## [X.Y.Z] — YYYY-MM-DD` entry (Keep a Changelog format, grouped Added / Changed /
Fixed). Leave a fresh empty `[Unreleased]` section above it.

### C. Version bump

Bump `version` in the workspace `Cargo.toml` (`[workspace.package]`, currently the
`version` field near the top) to the new SemVer, and commit. This is what the
auto-update check and the manual's version string read from.

### D. Tag and release

Per the working-conventions release flow: tag the SemVer release (`v X.Y.Z`) on `main`
and push it. The `v*` tag push triggers the GitHub Release workflow that publishes the
three-platform installers. Confirm the release artefacts actually published.

---

## Quick reference

- **Sign-off gate:** user tests the build first. No exceptions.
- **Every milestone:** sign-off line in `milestones.md` → doc-sync sweep → merge + `mXX` tag.
- **Version milestone:** all of the above, **plus** user manual updated *first*, then
  CHANGELOG, version bump, `v*` tag + release.
