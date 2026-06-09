# Tone Smithy — agent instructions

This is the Tone Smithy repo: a hybrid (subtractive + FM) standalone software synthesizer for Windows, written in Rust. The full design plan is at [`docs/planning/README.md`](docs/planning/README.md).

## Hard rules

### Starting a milestone
- **Never begin milestone implementation work without explicit user instruction.** Finishing a previous milestone, reading a plan, or reaching the end of a session does not constitute permission to start the next one. Wait for the user to say "start M16" (or equivalent) before writing any code.

### Closing out a milestone
- When marking a milestone complete in `docs/planning/06-implementation/milestones.md`, always include the date: `— **complete (YYYY-MM-DD, tag \`mXX\`)**`. Use the user's local date (today's conversation log filename, e.g. `docs/conversations/2026-06-09.md` → `2026-06-09`).
- The sign-off goes on the `## MXX — Title` heading line itself, matching the format used for M0 and M1.

### After making file changes
- **Commit immediately.** Don't wait to be asked.
- Use the per-command identity override with **your current model name** (e.g. `Claude Sonnet 4.6`, `Claude Opus 4.7` — whatever model is powering this session):
  `git -c user.name="Claude Sonnet 4.6" -c user.email="noreply@anthropic.com" commit ...`
- **Always use a HEREDOC** for commit messages to avoid shell quoting issues.
- **Before every commit**, run `rm -f .git/COMMIT_EDITMSG` to prevent stale content from polluting the new message body.
- Commit message: imperative subject ≤70 chars; body explains *why*, not *what*.
- Multiple commits per turn are encouraged when changes are logically distinct.
- Full git workflow: [`docs/working-conventions.md`](docs/working-conventions.md#git-workflow).

### On which branch
- **`development` is the default working branch.** Routine work (single-commit fixes, doc updates, planning iterations, conversation logs) commits directly to `development`.
- **Branch off `development` for substantial work.** Before starting a milestone implementation (M1, M2, …), an experiment that might be thrown away, or any work that will span multiple sessions and could leave `development` in a non-working state in between, run `git checkout -b <name>` first. Merge back to `development` with a regular merge when the work is done. *This rule fires on every new milestone — don't skip it.* Full criteria: [`docs/working-conventions.md#when-to-branch-off-development`](docs/working-conventions.md#when-to-branch-off-development).
- **Branch naming:** `feat/`, `fix/`, `docs/`, `chore/`, `experiment/`, or `milestone/m<NN>-<short>`. Milestone work uses the `milestone/` prefix.
- **`main` is only updated at milestone boundaries** via `git merge --no-ff` from `development` (see the milestone workflow in [`docs/working-conventions.md`](docs/working-conventions.md#milestone-completion-‐-merge-development-to-main)).
- Always check the current branch (`git status`) before committing.
- Flag any branch switch to the user explicitly — branch state is shared.
- **Don't switch branches with uncommitted changes.** Commit on the current branch first.

### After every Claude response
- Append the exchange to today's log: `docs/conversations/YYYY-MM-DD.md`.
- The Claude turn heading must identify the model: `### [HH:MM:SS] Claude (Sonnet 4.6)` — use your current model name so the log shows which agent wrote each response.
- Format spec: [`docs/conversations/README.md`](docs/conversations/README.md).
- Commit the log update as a **separate commit** after work commits, with subject `Log conversation: ...`.

### When writing Rust code (M0 onward)
- Follow [`docs/planning/04-tech-stack/code-style.md`](docs/planning/04-tech-stack/code-style.md): doc comments on every public item, audio-domain unit suffixes (`_hz`, `_cents`, etc.), prescribed file structure.
- **`mod.rs` files must only declare submodules and re-export — no implementation code.** Put functions and types in named `.rs` files.
- Follow [`docs/planning/03-architecture/design-patterns.md`](docs/planning/03-architecture/design-patterns.md): hexagonal layering, command-pattern events, single source of truth for parameters. **Real-time safety rules in Part 2 are non-negotiable** (no alloc / no lock / no syscall on the audio thread).
- Add new files following [`docs/planning/06-implementation/project-structure.md`](docs/planning/06-implementation/project-structure.md).
- **Before committing any `.rs` or `Cargo.toml` change**, run `cargo fmt --all --check` and fix any diff it reports. Then run `cargo clippy --workspace --all-targets -- -D warnings` and resolve all warnings. CI enforces both with `-D warnings`; a commit that skips this will fail the lint job.

### Keeping README.md current
Update `README.md` **in the same commit** as any change that:
- Shifts the active milestone (M0 → M1, etc.) or changes the timeline estimate.
- Adds or removes a v1 scope item.
- Changes the build, run, lint, or test commands.
- Adds a top-level directory or a new system dependency a builder would need.
- Changes the licence.

Don't update it for internal refactors, doc-only planning changes, or conversation log entries. Full triggers: [`docs/working-conventions.md#keeping-the-readme-up-to-date`](docs/working-conventions.md#keeping-the-readme-up-to-date).

### When considering a new feature or change
- Check [`docs/planning/02-scope/features-v1.md`](docs/planning/02-scope/features-v1.md) and [`docs/planning/02-scope/out-of-scope.md`](docs/planning/02-scope/out-of-scope.md) first. Don't expand scope without updating the plan.

## Don'ts

- **Don't rewrite git history.** Never use `git filter-branch`, `git rebase -i`, `git commit --amend` on a non-HEAD commit, `git reset --hard` to discard commits, or any other history-rewriting command. If commits are messy, leave them — messy history is better than rewritten history. Only the user may authorise history rewrites, and even then, confirm before acting.
- **Don't commit directly to `main`** outside an explicit milestone merge. All routine work goes on `development`.
- **Don't update git config** (`git config --global ...` or local). Use the per-command override for your commits.
- **Don't add new top-level markdown files unprompted.** The set of top-level docs is intentionally small (`README.md`, `CLAUDE.md`, `LICENSE-MIT`, `LICENSE-APACHE`). Anything else goes under `docs/`.
- **Don't resolve items in [`docs/planning/01-vision/open-questions.md`](docs/planning/01-vision/open-questions.md) unilaterally.** Flag the question and ask the user; once decided, update both the open-questions file and the relevant plan doc.
- **Don't propose architecture changes that violate** the [design patterns doc](docs/planning/03-architecture/design-patterns.md). If you think a pattern is wrong, raise it explicitly — don't quietly break it.

## Where to look

| You need to know… | Read |
|---|---|
| A quick public-facing overview | [`README.md`](README.md) |
| What we're building (more detail) | [`docs/planning/01-vision/overview.md`](docs/planning/01-vision/overview.md) |
| The full plan index | [`docs/planning/README.md`](docs/planning/README.md) |
| How we work in this repo | [`docs/working-conventions.md`](docs/working-conventions.md) |
| When to update README.md | [`docs/working-conventions.md#keeping-the-readme-up-to-date`](docs/working-conventions.md#keeping-the-readme-up-to-date) |
| Conversation log format | [`docs/conversations/README.md`](docs/conversations/README.md) |
| What's still undecided | [`docs/planning/01-vision/open-questions.md`](docs/planning/01-vision/open-questions.md) |

## Project state at a glance

- **Stage:** v1.0.0 shipped (tag `v1.0.0` on `main`). All v1 milestones (M0–M15) done. **Active: v1.1 planning — next milestone M16 (Quick wins).** v1.0 ships unsigned with the default icon (both deferred to a later version).
- **Licence:** `MIT OR Apache-2.0` dual.
- **Product name:** Tone Smithy.
- **Branches:** `main` (stable; v1.0.0 at tag `v1.0.0`); `development` (active work — commit here).
- **v1.1 scope:** Three quick UX wins (K=C keyboard note, alphabetical presets, conditional OSC/Sub panel) + engine expansion (second filter, 24 dB/oct, Env3, 16-slot matrix) + step sequencer + Linux/macOS installers + factory bank expansion to ~120 presets. Milestones M16–M20 defined in `docs/planning/06-implementation/milestones.md`.
- **Release:** v1.0.0 is live on GitHub Releases. pushing the `v1.0.0` tag triggered the release workflow. v1.1 will follow the same workflow via a `v1.1.0` tag.
- **Open questions remaining:** MPE/microtuning/oversampling scope (now v1.3+), factory content authoring approach, auto-update mechanism. Code signing and the custom icon are resolved as "deferred to a later version" (see `docs/planning/01-vision/open-questions.md`).
