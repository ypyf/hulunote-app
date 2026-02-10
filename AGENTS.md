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

Uncommitted changes or undocumented lessons are considered incomplete work.

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

## UI Rule (Rust/UI)

We use Rust/UI (rust-ui.com) for a Tailwind component-library style.

Rules:

- Compose pages using Rust/UI components only (copy/paste registry components). Do not reinvent styling per page.
- Theme must come from Rust/UI CSS variables (`--background`, `--foreground`, `--primary`, etc.) and `.dark` overrides.
- Do NOT hardcode colors (no hex) or introduce a separate token system that conflicts with Rust/UI theme variables.
- If the UI looks wrong, fix it by adjusting the Rust/UI theme variables, not by adding page-level CSS.

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
