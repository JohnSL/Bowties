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

**Run once per session, not per slice.** On the first slice, delegate these checks to an `Explore` subagent and store the results in session memory (`/memories/session/build-checks-<feature>.md`). For subsequent slices, reference the cached results — only re-run a check if the current slice touches a module not covered by the cache.

1. Check `aiwiki/owners.md` — does shared logic already exist for what you're building?
2. Check `product/architecture/code-placement-and-ownership.md` — is each file in the right layer?
3. Check `product/architecture/adr/` — does the approach conflict with past decisions?
4. Check open GitHub issues labeled `kind/idea` filtered by relevant `area/*` labels (`gh issue list --repo JohnSL/Bowties --label kind/idea --state open`) for prior work on this area. Also glance at any residual `specs/ideas/**` files until migration completes.

## Per-Slice Workflow

### HITL Slices

Present the slice to the user as a **single chat message** with two parts. Do NOT use `vscode_askQuestions` — batch presentation enables the user to spot cross-cutting architectural smells across decisions that sequential questioning destroys.

**Part 1 — Architectural context** (for the architect / product owner):

- **Before/after mermaid diagrams** showing the module-level shape change this slice introduces. Show responsibilities and data flow, not code details.
- **Pattern names** — name each architectural pattern being introduced or changed, with a one-sentence explanation of what it means in this context.
- **Module-level change table** — columns: Module | Today | After. Describe responsibility shifts, not implementation details.

**Part 2 — Numbered decisions** (for principle-level trade-offs):

Present ALL decisions for the slice as a numbered list. Each entry follows this structure:

```
**D{N}: {Short title}** — {Principle at stake: DRY / SOLID / YAGNI / Depth / Locality / ADR-compliance / etc.}
Options: (A) {option} | (B) {option}
Recommend: {A|B} — {why, tied to the named principle}
Impact: {scope — which downstream slices, how many modules, load-bearing or contained}
```

The user reviews the full picture and responds with approvals, overrides, or questions — e.g., "1: approved, 2: option B instead, 3: question — doesn't this violate ADR-0002?"

Wait for user direction on all decisions, then proceed with TDD.

### AFK Slices

Implement autonomously following established patterns. Present the result when done.

**Mid-slice stop condition.** If during implementation you discover the planned approach conflicts with an ADR, sits in the wrong layer per `product/architecture/code-placement-and-ownership.md`, requires coordinating state across layers that the slice did not anticipate, or duplicates logic that already has a shared owner, **stop**. Load and follow the `architecture-first-fix` skill to present options to the user before continuing. Do not patch through the surprise to keep the slice moving — unanticipated complications usually mean the slice's design needs to be revisited, not worked around.

### REFACTOR Slices

Implement autonomously. These slices produce no user-visible change — they restructure internals while preserving existing behavior. Present the result with a focus on what invariant was preserved and what architectural debt was reduced.

**Mid-slice stop condition.** Same as AFK slices: if the refactor reveals a deeper seam problem than the slice anticipated (an invariant that doesn't actually hold, a layer boundary that's wrong, a duplication wider than the slice's scope), load and follow the `architecture-first-fix` skill before continuing. A refactor that quietly absorbs a bug or scope creep defeats its own purpose.

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
