# Working conventions

How we work in this repo. Applies to humans and to AI agents (Claude or otherwise). These are the rules an automated tool can't infer from reading the code.

The design plan itself lives at [`planning/README.md`](planning/README.md). This doc covers the workflow *around* the plan: how we commit, how we log, what decisions are pending.

---

## Git workflow

### Identity

Global git config is **intentionally unset** in this repo. Authorship is established per-commit, so that Claude's commits and the user's commits are clearly distinguishable in `git log`.

- **Claude commits** (made by an AI agent in a sandbox) must use the per-command identity override:

  ```bash
  git -c user.name="Claude Opus 4.7" -c user.email="noreply@anthropic.com" commit -m "..."
  ```

- **User commits** rely on whatever identity the user has configured on their host or in their local repo config. The user is responsible for their own identity setup.

- **Never run `git config --global ...`** — this is an explicit rule from the sandbox environment.

### When to commit

- **After every file change.** Don't wait to be asked.
- **Multiple commits per turn are encouraged** when changes are logically distinct (e.g. one commit for a new doc and a separate commit to update an index that references it).
- **Skip commits only for**: exploration, running tests, files outside the repo (memory files, etc.), or work that didn't actually change tracked files.
- **Never commit suspected secrets** (`.env`, credentials, API keys). Pause and ask if a user explicitly stages one.

### Commit message style

- Imperative subject line, under ~70 characters.
- Body explaining the *why* with bullets where useful.
- Use a HEREDOC to keep formatting clean.
- End Claude commits with the `Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>` trailer.

Example:

```bash
git -c user.name="Claude Opus 4.7" -c user.email="noreply@anthropic.com" commit -m "$(cat <<'EOF'
Subject line in imperative mood

Brief explanation of why this change is being made.

- Specific point 1
- Specific point 2

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Branching

### Long-lived branches

- **`main`** — stable, milestone-tagged. Only updated when a milestone completes.
- **`development`** — default working branch for all day-to-day work.

All routine commits go to `development`. `main` represents shipped / milestone-complete state. A fresh clone defaults to `main`, which means anyone landing on the repo sees the most recent stable snapshot rather than work in flight.

### When to branch off `development`

Most work stays directly on `development`. Create a short-lived branch off `development` only when:

- The work will **span multiple sessions** and would leave `development` in a non-working state in between.
- The work is **experimental** and might be thrown away.
- The work is a **substantial milestone implementation** that touches many files and is safer to land as a unit.
- The user **explicitly asks** for a branch.

### Don't branch for

Doc updates, conversation log entries, single-commit fixes, routine planning iterations, anything you'd be comfortable landing on `development` as-is. Just commit.

### Branch naming

- `feat/<short-description>` — new feature
- `fix/<short-description>` — bug fix
- `docs/<short-description>` — large docs work
- `chore/<short-description>` — tooling, build, maintenance
- `experiment/<short-description>` — exploratory, may be discarded
- `milestone/m<NN>-<short>` — when a whole milestone is isolated (rare; usually milestone work goes directly on `development`)

### Merging short-lived branches back to `development`

- Default: **regular merge** (preserve history; we commit carefully so individual commits are valuable).
- Squash only when a branch has obvious WIP noise ("wip", "fix typo", "oops").
- Delete the branch after merge (`git branch -d <name>`).

### Milestone completion → merge `development` to `main`

1. Verify the milestone's success criteria are met (per [`planning/06-implementation/milestones.md`](planning/06-implementation/milestones.md)).
2. Final commit on `development` — tick off the milestone in the plan, update any version numbers.
3. Merge into `main` with a visible merge commit:
   ```bash
   git checkout main
   git merge --no-ff development -m "Milestone M0X: <name>"
   ```
   `--no-ff` forces a merge commit even when fast-forward is possible. This makes the milestone boundary obvious in `main`'s history — every commit on `main` (after the initial planning-era commits) represents a completed milestone.
4. Tag the milestone:
   ```bash
   git tag -a m0X -m "Milestone M0X: <name>"
   ```
5. `git checkout development` and continue work.

### Tagging

- **Milestone tags** — `m00`, `m01`, …, `m15`. Lightweight markers, one per milestone completion.
- **Release tags** — SemVer (`v1.0.0`, `v1.1.0`, …). First applied at M15.

### Hot-fix flow (rare, post-milestone)

If a critical bug is found in a milestone-tagged `main`:

1. Branch from `main`: `git checkout main && git checkout -b fix/<short>`.
2. Make the fix and commit.
3. Merge to `main` (regular merge); tag if it's a release-worthy patch.
4. Merge `main` back into `development` so the two branches don't drift:
   ```bash
   git checkout development
   git merge main
   ```

### Claude-specific

- **Always check the current branch** before committing (`git status` shows it on the first line).
- **Never commit directly to `main`** outside an explicit milestone merge step you've talked through with the user.
- **Flag any branch switch** — especially `main` ↔ `development` or creating a new branch — to the user explicitly. Branch state is shared and a surprise switch is unpleasant.
- **Don't switch branches with uncommitted changes.** Commit (on the current branch) or stash first.

---

## Conversation logging

Every exchange between the user and Claude must be appended to today's conversation log file.

- **Where:** `docs/conversations/YYYY-MM-DD.md`, one file per local-date day.
- **Format spec:** [`conversations/README.md`](conversations/README.md). Read this before writing entries — the format is specific (visual distinction for the user, heading-level shifts for Claude's content, commit-hash cross-references).
- **When:** after each Claude response, before reporting completion.
- **Commit cadence:** the log update is a **separate commit** from work commits, and comes **after** them so the log can reference the work commit hashes. Subject style: `Log conversation: <short description>`.

---

## Following the plan

When writing code (eventually — M0 onward), the prescriptive docs are not optional reading:

| Doc | When to read |
|---|---|
| [`planning/04-tech-stack/code-style.md`](planning/04-tech-stack/code-style.md) | Before writing or reviewing any Rust code in this repo. |
| [`planning/03-architecture/design-patterns.md`](planning/03-architecture/design-patterns.md) | Before designing engine or host code. Real-time safety rules in Part 2 are non-negotiable. |
| [`planning/06-implementation/project-structure.md`](planning/06-implementation/project-structure.md) | When adding a new file, module, or crate. |
| [`planning/02-scope/features-v1.md`](planning/02-scope/features-v1.md) | Before proposing a new feature — confirm it's in scope or explicitly add it to the plan first. |
| [`planning/02-scope/out-of-scope.md`](planning/02-scope/out-of-scope.md) | Confirm a request isn't already deferred. |

---

## Keeping the README up to date

The top-level [`README.md`](../README.md) is the public face of the project — it's what anyone visiting the repo sees first. Keep it accurate.

### Update README.md when

- **v1 scope changes** — something added, removed, or moved between v1 and v1.1+.
- **The current milestone changes** (M0 → M1, etc.) — update the status line.
- **The build, run, lint, or test commands change** (new flags, new prerequisites, new OS support).
- **A new top-level directory** is added that anyone exploring the repo should know about.
- **The licence changes** (anything affecting `LICENSE-MIT` or `LICENSE-APACHE`).
- **A new system dependency** is introduced that someone building from a clean machine would need to install (Linux audio libs, signing tools, etc.).

### Don't update README.md for

- Internal refactors that don't change behaviour or public commands.
- Doc-only changes inside `docs/planning/` that don't shift v1 scope.
- New conversation log entries.
- Routine commits that don't surface in the public-facing summary.

### How

Touch only the section affected by your change, in the **same commit** as the change. Don't batch unrelated README updates. The README has a small fixed set of sections:

| Section | Update when… |
|---|---|
| Status line | Current milestone or timeline estimate shifts. |
| Features | v1 scope or roadmap split changes. |
| Quick start | Build/run/lint/test commands or system dependencies change. |
| Project layout | A top-level folder or workspace member is added or removed. |
| Architecture | The major-piece split (engine / host / ui / presets / app) changes. |
| Licence | The licence policy changes. |
| Acknowledgements | A new major dependency or reference belongs in the credits. |

Don't add screenshots, badges, or marketing copy until v1.0 is actually shippable.

---

## Open questions and pending decisions

- All currently-deferred decisions live in [`planning/01-vision/open-questions.md`](planning/01-vision/open-questions.md).
- **Don't resolve them unilaterally.** Flag the question and ask the user. If a decision is made in conversation, update the open-questions file and the relevant plan doc in the same commit.

---

## Recommended reading order for a new agent or contributor

1. [`/CLAUDE.md`](../CLAUDE.md) (auto-loaded for AI agents)
2. [`planning/01-vision/overview.md`](planning/01-vision/overview.md) — what we're building.
3. [`planning/README.md`](planning/README.md) — the plan index.
4. This file (`working-conventions.md`) — how we work.
5. [`conversations/README.md`](conversations/README.md) — how to log exchanges.

After that, dip into specific planning docs as the task requires.
