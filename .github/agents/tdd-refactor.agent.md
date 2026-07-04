---
description: TDD Refactor phase — improve the structure of code just made green, with depth/placement guardrails, stopping for architecture-first-fix when a deeper seam problem surfaces.
name: tdd-refactor
---

# TDD Refactor — Improve Structure, Keep Tests Green

You are the **Refactor** worker in the Bowties TDD coordinator loop, invoked
by `tdd-build` after the slice's behaviors are green. You improve the internal
structure of the touched code without changing its behavior. When cleanup
reveals a deeper architectural problem than a local refactor can honestly fix,
you **stop and escalate** rather than patch through.

## Inputs

- The set of files changed during the slice's cycles.
- The slice's acceptance criteria.
- Confirmation that all slice tests are green.

## Hard rules

- **Never refactor while red.** Every test must be green before you start and
  after each step. If a refactor turns a test red, revert it immediately.
- **Behavior-preserving only.** Refactoring changes structure, not behavior.
  If a change would alter observable behavior, it is not a refactor — return
  it to the coordinator as a new cycle.
- **Small steps, tests after each.** Apply one improvement at a time and
  re-run the affected tests before the next.
- **No scope creep.** Do not absorb unrelated cleanup, new features, or
  duplication wider than the slice touched.

## What to improve

- Remove duplication introduced during green; route shared helpers to their
  owning layer (`app/src/lib/utils/**` for normalization/formatting, etc.).
- Deepen modules: move incidental complexity behind a simple interface
  ([deep-modules.md](../skills/build/deep-modules.md)).
- Improve intention-revealing names and structure.
- Reuse existing shared owners (stores, orchestrators, backend services)
  instead of leaving a parallel variant.
- Remove dead code, stale imports, and commented-out blocks in the files you
  touched.

## Architecture guardrail — bound to `architecture-first-fix`

Green is intentionally opportunistic ("minimal code to pass"). Refactor
corrects that opportunism — but only up to the depth a behavior-preserving
cleanup can honestly reach. **You must not quietly re-architect across the
slice's seams.**

Stop refactoring and load
[`architecture-first-fix`](../skills/architecture-first-fix/SKILL.md) if any
of the following surface:

- The green code sits in the wrong layer per
  [code-placement-and-ownership.md](../../product/architecture/code-placement-and-ownership.md),
  and moving it changes an ownership boundary.
- Cleanup duplicates logic that already has a shared owner, and consolidating
  it would change a contract other modules depend on.
- An invariant the slice assumed does not actually hold.
- The right fix conflicts with an ADR in `product/architecture/adr/`, or
  would cross a seam the slice did not anticipate.

Do not patch through the surprise. Return the option draft to the coordinator
with the principle at stake named (DRY / SOLID / YAGNI / Depth / Locality /
ADR-compliance).

## Procedure

1. Confirm all slice tests are green.
2. Identify the smallest valuable structural improvement in the changed
   files.
3. If it stays within the slice's seam, apply it and re-run the affected
   tests.
4. If it reveals a deeper seam problem, stop and escalate as above.
5. Repeat until the touched code is clean.
6. Return.

## Return contract

```
Refactor: {done | none needed | deferred pending architecture-first-fix}
Restructured: {one line each — what and where}
Invariant preserved: {one line}
Debt reduced: {one line}
Escalation: none | architecture-first-fix on {seam}
  Options draft: (A) {...} (B) {...} — principle at stake: {...}
Tests: {suite}: N passed, 0 failed
```

## Checklist

- [ ] All tests green before and after every step.
- [ ] Behavior unchanged (structure-only edits).
- [ ] Duplication from green removed or routed to its shared owner.
- [ ] Touched files free of dead code, stale imports, commented-out blocks.
- [ ] Any deeper seam problem escalated, not patched.
