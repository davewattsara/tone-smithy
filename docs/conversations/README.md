# Conversation logs

Daily logs of every conversation between Dave and Claude as Tone Smithy is built. The logs are the project's narrative record — what was discussed, what was decided, and which commits came out of each conversation.

## Folder layout

```
docs/conversations/
├── README.md            ← this file (the instructions)
├── 2026-05-17.md        ← one file per local-date day
├── 2026-05-18.md
└── ...
```

- **One file per day**, named `YYYY-MM-DD.md`, using **local time** (the developer's wall clock, not UTC).
- If a single conversation spans midnight, start a new file at the date boundary.

## Format — daily file

Every daily file starts with a level-1 header naming the date:

```markdown
# 2026-05-17 — Conversation log
```

Then a sequence of **exchanges**, separated by horizontal rules (`---`). Each exchange is one user turn followed by one Claude turn.

## Format — a single exchange

```markdown
### [HH:MM:SS] **You**
> Verbatim text of the user's message, blockquoted.
> Multi-line messages keep the `>` prefix on every line.

### [HH:MM:SS] Claude
Verbatim text of Claude's response, as plain prose.

**Commits:** `abc1234` — Short commit subject, `def5678` — Another subject

---
```

### Visual distinction (important)

The user's turn must stand out at a glance. Two cues enforce this:

1. **`**You**` is bolded in the heading**; Claude's heading is plain.
2. **The user's body is blockquoted** (every line starts with `> `); Claude's body is plain text.

In rendered Markdown, the blockquote shows as a vertical accent bar, so the user's lines pop out when scrolling.

### Heading-level shift in Claude's content

If Claude's response contains its own headings, **shift them down by 3 levels** so they don't conflict with the turn-level `###`:

| In Claude's original response | In the log |
|---|---|
| `# Title`       | `#### Title`   |
| `## Section`    | `##### Section` |
| `### Sub`       | `###### Sub`    |
| `#### Anything` | (already deep enough — leave as-is) |

This keeps the document outline clean and prevents the response from inventing top-level structure inside the conversation log.

Other Markdown (tables, fenced code blocks, lists, links) is preserved verbatim.

## Timestamps

- Format: `HH:MM:SS` (24-hour).
- Source: the local clock at the moment Claude is writing the log entry.
- The user's timestamp is approximate — set it to the time the exchange began (just before Claude's response). Don't try to reconstruct exact send times after the fact.
- Get the current time with `date '+%H:%M:%S'` (Bash).

## Commit tracking

At the end of each Claude turn, list any commits made during that turn:

```markdown
**Commits:** `3c280ef` — Adopt MIT OR Apache-2.0 dual licence, `767b900` — Add code style guide
```

- Use the short hash (first 7 characters).
- Use the subject line of the commit (first line of the commit message), not the full body.
- Multiple commits separated by commas.
- If no commits were made, omit the `**Commits:**` line entirely.

## What to include

- Every user message **verbatim**, including formatting, code blocks, and typos.
- Every Claude text response **verbatim**, with the heading-level shift above.
- Commit hashes + subjects for any commits made during the turn.

## What to omit

- **Tool call invocations** and their raw outputs. These are inferable from the commits and add noise.
- **`<system-reminder>` messages** and other system-injected text.
- **Internal reasoning / thinking**.
- **`AskUserQuestion` prompts** — log the user's answer as their message, prefixed with a short note like "(answered structured question: …)" if context is otherwise lost.

If a Claude turn was almost entirely tool calls with little or no prose, log a one-line summary like "(tool work only — see commits)".

## When to write to the log

- Write the new exchange immediately **after** Claude has finished responding to the user, **before** the "I'm done" wrap-up of that turn.
- Append to the day's file; don't rewrite earlier entries.

## When to commit the log

- Commit the log update as a **separate commit** from any work commits the same turn produced.
- The log commit comes **after** the work commits, so the log entry can reference the work commit hashes.
- Commit subject style: `Log conversation: <short description of the exchange>` — for example `Log conversation: adopt MIT/Apache-2.0 dual licence`.
- Body of the log commit can be empty (or just the Co-Authored-By trailer).
- If the turn produced no work commits, still commit the log update on its own.

## Example file

```markdown
# 2026-05-17 — Conversation log

### [14:32:01] **You**
> can we add a section for conversation logs

### [14:32:15] Claude
Sure — proposing `docs/conversations/` with a README of instructions
and one Markdown file per day.

**Commits:** `abc1234` — Add conversation log scaffolding

---

### [14:35:00] **You**
> looks good, lock it in

### [14:35:09] Claude
Done. The instructions are at `docs/conversations/README.md`; today's
log is `docs/conversations/2026-05-17.md`.

**Commits:** `def5678` — Add conversation logging instructions and first day file

---
```
