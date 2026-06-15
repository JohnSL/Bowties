---
name: build
description: TDD-first vertical implementation with multi-session support. Implements feature slices one at a time using red-green-refactor, with cross-session progress tracking. Use when user says "build", "implement slices", "start building", or after running /slices.
---

# Build

Implement feature slices using TDD. One slice at a time, test-first, across sessions.

## Session Startup

1. Detect current feature from branch name or `$env:SPECIFY_FEATURE`
2. Read `specs/<feature>/slices.md` — read the **Roadmap** table and find the next slice that is not `done` (respecting blocked-by order)
3. If resuming: read the session note at the bottom of slices.md for context
4. Re-orient via `aiwiki/owners.md` for modules you'll touch

If slices.md doesn't exist, tell the user to run `/slices` first.

The roadmap is a thin, two-tier file: every slice starts as a one-line `sketched` row, and you expand exactly one slice to task detail at a time (see [Just-In-Time Tasking](#just-in-time-tasking) below).

## Pre-Implementation Checks

**Run once per session, not per slice.** On the first slice, delegate these checks to an `Explore` subagent and store the results in session memory (`/memories/session/build-checks-<feature>.md`). For subsequent slices, reference the cached results — only re-run a check if the current slice touches a module not covered by the cache.

1. Check `aiwiki/owners.md` — does shared logic already exist for what you're building?
2. Check `product/architecture/code-placement-and-ownership.md` — is each file in the right layer?
3. Check `product/architecture/adr/` — does the approach conflict with past decisions?
4. Check open GitHub issues labeled `kind/idea` filtered by relevant `area/*` labels (`gh issue list --repo JohnSL/Bowties --label kind/idea --state open`) for prior work on this area. Also glance at any residual `specs/ideas/**` files until migration completes.

## Just-In-Time Tasking

`/slices` produces a **roadmap** of slice cards: each carries intent, boundary, blocked-by, acceptance criteria, and (for HITL/new-seam slices) an architecture note — but **no per-layer task breakdown**. You author the task breakdown **one slice at a time**, just before implementing that slice — never all up front. This keeps a mid-feature pivot cheap: it edits a slice card plus the single slice in flight, not a whole task list.

**At the start of each slice:**

1. Take the next roadmap slice that is not `done`. Confirm its blocked-by slices are `done`.
2. **Re-read the roadmap** and adjust this slice's card in light of what earlier slices revealed — its boundary, and its acceptance criteria or architecture note if the learning changed them. If the learning changes the slice's intent, label, or ordering — or splits/merges it — update the card(s) first. This per-slice re-cut is an **expected checkpoint**, not an exception.
3. Append the **per-layer task breakdown** to the slice's card, following [SLICE-FORMAT.md](../slices/SLICE-FORMAT.md). The acceptance criteria are already on the card — do not rewrite them; derive the tasks from them. Task 1 is always the integration test; implementation tasks in dependency order (deepest layer first); the last task is always validation. Add complexity / user-stories to the card if not already present.
4. Flip the slice's status `sketched → tasked`.
5. Proceed with the Per-Slice Workflow below.

Only **one** slice is `tasked` at a time. Do not author the task breakdown for a downstream slice until the current one is `done`. If a pivot during implementation invalidates a downstream slice, edit that slice's card — it costs a sentence, not a rewrite, precisely because its tasks were never authored.

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

#### Delegating the loop to the TDD coordinator (optional, context-saving)

Red→green→refactor accumulates failing-test output, stack traces, and abandoned
attempts that fill the main window fast. To keep the main conversation lean, you
may run the loop for the **current, already-tasked slice** through the `tdd-build`
coordinator agent, which delegates each phase to a context-isolated worker
(`tdd-red`, `tdd-green`, `tdd-refactor`) and returns one summary per cycle.

Constraints when delegating:
- The coordinator runs **strictly downstream of `/design`** and **inside** a
  single slice that is already `status: tasked` (and, for HITL slices, already
  approved). It never re-decides architecture or re-cuts slices — that stays here.
- The **Refactor** worker is bound to `architecture-first-fix`: if cleanup reveals
  a deeper seam problem, it stops and surfaces options instead of patching.
- The coordinator stops at the slice boundary and writes back to `slices.md`, then
  hands control here to re-read the roadmap and task the next slice (per
  [Just-In-Time Tasking](#just-in-time-tasking)).

Delegation is optional. For a small slice it is often cheaper to run the loop
inline; for a long or test-heavy slice, prefer the coordinator. Either way the
TDD rules above are unchanged.

### After Each Slice

1. Check off all tasks in slices.md (`[x]`)
2. Set the slice's roadmap status `tasked → done`
3. Update the status line (`N/total slices complete`)
4. Present the result: what was built, what the test proves, any surprises
5. **Re-read the roadmap and adjust the next slice's card** in light of what this slice revealed (the just-in-time re-cut) — its boundary, and acceptance criteria/architecture note if the learning changed them. Do not author the next slice's task breakdown until you actually start it.

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
