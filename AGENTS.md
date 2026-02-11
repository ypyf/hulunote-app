# AGENTS.md

## How Agents Are Expected to Work Here

Agents operating in this repository must follow a strict, engineering-driven workflow:

- Work from an explicit TODO list
- Complete **one task at a time**
- Prefer **test-driven development** when behavior changes
- Verify work against:
  - Design documents (PRD, specs, invariants)
  - Tests and acceptance criteria
- Commit all code changes
- Preserve reusable lessons in this file

Avoid leaving long-lived uncommitted work. However, do not create low-signal commits just to “check a box”.

- It’s OK to keep local WIP while iterating.
- Before handing work off (PR/review), changes must be committed in coherent, reviewable units.
- Only record lessons in this file when they are genuinely reusable (avoid noise).

If a task is ambiguous, underspecified, or conflicts with existing design:

- Stop
- Explain the issue clearly
- Request clarification
- Do **not** guess or invent requirements

---

## Definition of Done (Agent-Level)

A task is considered complete **only if all applicable conditions are met**:

- Expected behavior is defined and verified by tests (when applicable)
- Code changes are committed to Git
- Reusable lessons have been recorded in `AGENTS.md` (when applicable)

Speed is never prioritized over correctness, test coverage, or clarity.

---

## Design and Architecture Constraints

Agents must respect the following high-level constraints:

- Design documents are the source of truth, not existing code behavior
- Architectural boundaries must not be crossed casually
- Refactors, optimizations, or stylistic changes require explicit intent
- Do not expand scope beyond the current task

If a task requires changing the design itself:

- Propose the design change explicitly
- Do not silently adjust behavior

---

## Test-Driven Development Expectations

For tasks involving behavior changes or new functionality:

- Prefer writing or identifying tests **before** implementation
- Tests define expected behavior and completion criteria
- Tests should fail before implementation and pass after (when applicable)

If TDD is not feasible:

- Explicitly state why (e.g. pure refactor, documentation-only change)
- Never skip tests silently

Tests are treated as executable design constraints.

---

## Writing & Documentation Guidelines

- Avoid brand/business references in code, comments, and docs; describe behavior and implementation neutrally.

---

## UI Rule (Rust/UI)

We use Rust/UI (rust-ui.com) for a Tailwind component-library style.

Rules:

- Compose pages using Rust/UI components only (copy/paste registry components). Do not reinvent styling per page.
- Prefer Tailwind **semantic utilities** (`bg-muted`, `text-muted-foreground`, `border-border`, etc.) over bespoke class strings.
- Theme must come from Rust/UI CSS variables (`--background`, `--foreground`, `--primary`, etc.) with a single, centralized mechanism for theme variants (e.g. dark mode) at the theme layer.
- **Avoid hardcoding** presentation values in components:
  - No hex colors
  - No `rgb(...)` / `rgba(...)` literals
  - Avoid inline `style=` for colors/spacing/radius/shadows (use tokens/utilities)
- Avoid introducing a parallel token system. If a new token is truly needed, add it in the theme layer and map it to the existing semantic tokens.
- If the UI looks wrong, fix it by adjusting theme tokens (e.g. in `style/tailwind.css`), not by adding page-level CSS.

---

## Version Control Discipline

This repository is Git-managed.

Rules:

- Any task that modifies code **must** end with a commit
- Each TODO item maps to at most one commit
- Commits must:
  - Be scoped to the current task
  - Include relevant tests
  - Have clear, imperative commit messages explaining WHAT and WHY
- **Never revert/restore/reset user-authored docs (AGENTS.md, docs/*) as part of an unrelated change.** If doc diffs appear unexpectedly, stop and ask for confirmation; if separation is needed, move doc changes into a dedicated commit instead of discarding them.

### PR discipline (avoid PR spam)

Pull Requests are **not** the default unit of work.

### Prefer committing directly on `main` for small changes

When the change is small, low-risk, and clearly scoped (especially documentation-only updates), prefer committing directly on `main` rather than creating a separate long-lived branch/PR. This keeps the repo workflow lightweight and avoids review overhead.

If the change is risky, cross-cutting, or user-facing, use a branch + PR as usual.

- Only open a PR when:
  - implementing a complex feature/requirement, or
  - fixing a tracked bug/issue (e.g. a GitHub Issue), or
  - the change is risky enough that it needs review before merge.
- Do **not** open a new PR for minor refactors, small cleanups, or purely incidental tweaks discovered while working on something else.
- If you need to capture a small improvement that is *not* an issue/bug fix:
  - bundle it into the current relevant PR **only if** it is directly related, otherwise
  - write it down as a TODO (or open an Issue) and defer.

### Commit message format (required)

Use a consistent, grep-friendly format:

```
<area>: <imperative summary>
```

Where:
- `<area>` is one of:
  - `auth`, `layout`, `db`, `notes`, `outline`, `search`, `ui`, `docs`, `tests`, `build`, `chore`
- `<imperative summary>`:
  - starts with a verb: `Add`, `Fix`, `Update`, `Refactor`, `Remove`, `Document`
  - <= 72 chars
  - describes **what changed**, not the task history

Optional body (recommended for non-trivial changes):

- **Why** the change is needed
- **How** to verify (tests, manual steps)
- Any behavior changes / risk

Examples:
- `outline: Fix blur-save race when switching nodes`
- `outline: Add Alt+Up/Down reorder among siblings`
- `docs: Update user manual for outline shortcuts`
- `tests: Add regression test for storage roundtrip`

Uncommitted work is considered unfinished work.

---

## Common Pitfalls to Avoid

### Avoid interactive Git flows (editor/TTY prompts)

In this environment, interactive commands can hang waiting for an editor/TTY input.
This has previously happened with:

- `git add -p` (especially choosing `e` to edit a hunk)
- `git rebase -i` / rebase flows that invoke an editor
- any command that prints: `Waiting for your editor to close the file...`

Rules:

- Prefer **non-interactive** Git operations.
  - Use `git add <paths>` (explicit paths) or `git add -A`.
  - To split changes, use separate branches/stashes, or apply patches (`git diff > file.patch`, `git apply --cached file.patch`).
- If a rebase is absolutely necessary, make it **non-interactive**:

```bash
GIT_SEQUENCE_EDITOR=true GIT_EDITOR=true git rebase origin/main
# resolve conflicts
git add <files>
GIT_EDITOR=true git rebase --continue
```

- Prefer `git merge origin/main` over `rebase` when the only goal is to resolve PR conflicts.

Agents should be especially careful to avoid:

- Making assumptions about undocumented behavior
- Treating existing behavior as correct without design confirmation
- Refactoring or optimizing “while you’re here”
- Skipping tests because changes seem “small”
- Forgetting to persist lessons learned after a difficult task
- Doing UI work without actually **opening the page in a browser**

If something was confusing, non-obvious, or surprising once,
it will be confusing again unless documented here.

---

## Updating This File

Agents **must** update `AGENTS.md` when:

- An incorrect assumption was made
- A library or framework behaved in a non-obvious way
- A specific mental model was required to succeed
- A mistake is likely to be repeated in the future

When adding entries:

- Capture **distilled conclusions**, not debugging timelines
- Clearly state:
  - The incorrect assumption or pitfall
  - The correct mental model
  - Why the incorrect approach fails
  - The correct approach to use next time
- Keep entries concise, factual, and actionable

If no reusable knowledge was gained:

- Explicitly state that no update is required
- Do not add filler content

---

## Final Principle

This file exists so that:
> **The second attempt is always easier than the first.**

Agents are expected to leave the codebase
more understandable, more predictable, and less fragile
than they found it.
