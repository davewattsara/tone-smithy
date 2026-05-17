# Tone Smithy — agent instructions

This is the Tone Smithy repo: a hybrid (subtractive + FM) standalone software synthesizer for Windows, written in Rust. The full design plan is at [`docs/planning/README.md`](docs/planning/README.md).

## Hard rules

### After making file changes
- **Commit immediately.** Don't wait to be asked.
- Use the per-command identity override for your commits:
  `git -c user.name="Claude Opus 4.7" -c user.email="noreply@anthropic.com" commit ...`
- Multiple commits per turn are encouraged when changes are logically distinct.
- Full git workflow: [`docs/working-conventions.md`](docs/working-conventions.md#git-workflow).

### On which branch
- **`development` is the default working branch.** All day-to-day commits go here.
- **`main` is only updated at milestone boundaries** via `git merge --no-ff` from `development` (see the milestone workflow in [`docs/working-conventions.md`](docs/working-conventions.md#milestone-completion-‐-merge-development-to-main)).
- Always check the current branch (`git status`) before committing.
- Flag any branch switch to the user explicitly — branch state is shared.
- Full branching rules: [`docs/working-conventions.md`](docs/working-conventions.md#branching).

### After every Claude response
- Append the exchange to today's log: `docs/conversations/YYYY-MM-DD.md`.
- Format spec: [`docs/conversations/README.md`](docs/conversations/README.md).
- Commit the log update as a **separate commit** after work commits, with subject `Log conversation: ...`.

### When writing Rust code (M0 onward)
- Follow [`docs/planning/04-tech-stack/code-style.md`](docs/planning/04-tech-stack/code-style.md): doc comments on every public item, audio-domain unit suffixes (`_hz`, `_cents`, etc.), prescribed file structure.
- Follow [`docs/planning/03-architecture/design-patterns.md`](docs/planning/03-architecture/design-patterns.md): hexagonal layering, command-pattern events, single source of truth for parameters. **Real-time safety rules in Part 2 are non-negotiable** (no alloc / no lock / no syscall on the audio thread).
- Add new files following [`docs/planning/06-implementation/project-structure.md`](docs/planning/06-implementation/project-structure.md).

### When considering a new feature or change
- Check [`docs/planning/02-scope/features-v1.md`](docs/planning/02-scope/features-v1.md) and [`docs/planning/02-scope/out-of-scope.md`](docs/planning/02-scope/out-of-scope.md) first. Don't expand scope without updating the plan.

## Don'ts

- **Don't commit directly to `main`** outside an explicit milestone merge. All routine work goes on `development`.
- **Don't update git config** (`git config --global ...` or local). Use the per-command override for your commits.
- **Don't create top-level `README.md` unprompted.** A top-level README has been intentionally deferred. CLAUDE.md (this file) serves the agent-onboarding role for now.
- **Don't resolve items in [`docs/planning/01-vision/open-questions.md`](docs/planning/01-vision/open-questions.md) unilaterally.** Flag the question and ask the user; once decided, update both the open-questions file and the relevant plan doc.
- **Don't propose architecture changes that violate** the [design patterns doc](docs/planning/03-architecture/design-patterns.md). If you think a pattern is wrong, raise it explicitly — don't quietly break it.

## Where to look

| You need to know… | Read |
|---|---|
| What we're building | [`docs/planning/01-vision/overview.md`](docs/planning/01-vision/overview.md) |
| The full plan index | [`docs/planning/README.md`](docs/planning/README.md) |
| How we work in this repo | [`docs/working-conventions.md`](docs/working-conventions.md) |
| Conversation log format | [`docs/conversations/README.md`](docs/conversations/README.md) |
| What's still undecided | [`docs/planning/01-vision/open-questions.md`](docs/planning/01-vision/open-questions.md) |

## Project state at a glance

- **Stage:** Planning complete. No code yet. M-1 (learning ramp) is the first milestone; M0 (scaffold) is the first repo-affecting milestone.
- **Licence:** `MIT OR Apache-2.0` dual.
- **Product name:** Tone Smithy.
- **Branches:** `main` (stable; ends at the "planning complete" snapshot until M0 finishes); `development` (active work — commit here).
- **v1 scope:** Path B — trimmed to ship in ~12–15 months. One filter (12 dB/oct), one mod envelope (Env2), 8-slot mod matrix, arpeggiator only, ~60-preset factory bank. The remaining engine features (second filter, 24 dB/oct, Env3, 16 slots, step sequencer, full factory bank) are restored in v1.1.
- **Open questions remaining:** code signing certificate, MPE/microtuning/oversampling scope (now v1.3+), factory content authoring approach, auto-update mechanism (all flagged in `docs/planning/01-vision/open-questions.md`).
