---
name: build
description: TDD-first vertical implementation with multi-session support. Implements feature slices one at a time using red-green-refactor, with cross-session progress tracking. Use when user says "build", "implement slices", "start building", or after running /slices.
---

# Build

Implement feature slices using TDD. One slice at a time, test-first, across sessions.

## Session Startup

1. Detect current feature from branch name or `$env:SPECIFY_FEATURE`
2. Read `specs/<feature>/slices.md` — find the next unchecked slice
3. If resuming: read the session note at the bottom of slices.md for context
4. Re-orient via `aiwiki/owners.md` for modules you'll touch

If slices.md doesn't exist, tell the user to run `/slices` first.

## Pre-Implementation Checks

Before implementing each slice, verify:
1. Check `aiwiki/owners.md` — does shared logic already exist for what you're building?
2. Check `product/architecture/code-placement-and-ownership.md` — is each file in the right layer?
3. Check `product/architecture/adr/` — does the approach conflict with past decisions?
4. Check `specs/ideas/**` (all bucket subfolders: `features/`, `refactors/`, `docs/`, `process/`) for prior work on this area

## Per-Slice Workflow

### HITL Slices

Present the architectural pattern question to the user first:
- What pattern is this slice introducing?
- What are the alternatives?
- What's your recommendation and why?

Wait for user direction, then proceed with TDD.

### AFK Slices

Implement autonomously following established patterns. Present the result when done.

### TDD Loop (both types)

For each slice, follow the [TDD methodology](tdd.md):

```
RED:   Write integration test for the slice → test fails
GREEN: Implement minimum code across all layers → test passes
REFACTOR: Improve code quality while tests stay green
```

Implementation order within a slice: **deepest layer first, working up**.

Rules:
- One test at a time. Don't write all tests then all code. See [tdd.md](tdd.md).
- Only enough code to pass the current test
- Design interfaces for [testability](interface-design.md) and [depth](deep-modules.md)
- Mock at [system boundaries only](mocking.md)
- Tests should verify [behavior, not implementation](tests.md)

### After Each Slice

1. Check off all tasks in slices.md (`[x]`)
2. Update the status line (`N/total slices complete`)
3. Present the result: what was built, what the test proves, any surprises

## Session Capacity

After completing a slice, evaluate:
- How much context has accumulated?
- What's the complexity of the next slice?
- Is the next slice HITL (may need extended discussion)?

If the session is getting long, **stop at the slice boundary**. Add a session note to slices.md:

```markdown
<!-- Session: YYYY-MM-DD — Completed S{N}-S{M}. Next: S{M+1} ({HITL|AFK}). -->
```

Summarize: what was completed, what's next, any issues found.

## Post-Implementation Enrichment

After completing all slices (or at session end if substantial work was done):

1. **aiwiki/owners.md** — add new modules, update test mappings, document new conventions
2. **aiwiki/flows.md** — update workflow module participation if changed
3. **product/architecture/adr/** — write ADRs for architecture decisions made during build
4. **specs/backlog.md** — resolve completed items, add newly revealed items
