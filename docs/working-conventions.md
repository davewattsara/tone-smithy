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
