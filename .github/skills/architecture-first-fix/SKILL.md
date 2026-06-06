---
name: architecture-first-fix
description: Stop-and-propose-options procedure for bugfixes, behavior changes, and mid-implementation surprises. Forces root-cause analysis at the right seam, with options named in terms of the principle at stake (DRY / SOLID / YAGNI / Depth / Locality / ADR-compliance). Use for any bugfix or behavior change (chat or slash-commanded), and any mid-slice surprise during /build.
---

# Architecture-First Fix

The procedure to run before editing code for a bugfix, behavior change, or mid-implementation surprise. The goal is to prevent the slow architectural decay that results from a sequence of locally-reasonable patches at symptom sites.

## When this skill applies

- Any bugfix request, whether triggered by `/bugfix`, `/quickchange`, or a freeform chat message ("fix this", "this is wrong", "it should…").
- Any behavior change to existing code (as opposed to net-new feature work, which uses `design` + `slices` + `build`).
- Any mid-slice surprise during `build`: a planned approach conflicts with an ADR, a test fails because the slice's design violates an invariant, or coordinating state across layers turns out to be more involved than the slice anticipated.

**Exempt:** trivial mechanical edits (typo, comment, import sort, formatting), and cases where the user has explicitly said "just patch it", "skip the architecture check", or equivalent.

## Procedure

### 1. Identify the seam

**Delegate the research to an `Explore` subagent** to conserve main-conversation context. The subagent should gather module ownership, ADR coverage, and test file mapping, then return a structured summary. Work from that summary to state, explicitly:

- Which layer(s) own the affected behavior, per `product/architecture/code-placement-and-ownership.md`.
- Which ADR(s) in `product/architecture/adr/` govern the seam. If none apply, say so.
- Which `aiwiki/owners.md` modules are involved, and which test files cover them.

### 2. Distinguish symptom from root cause

The root cause is the point where a contract, invariant, or ownership rule was violated — not the place where the wrong value surfaces. Common patterns:

- Symptom in a component or render site, root cause in a missing facade contract or a store that exposes raw state.
- Symptom in a route's menu/button state, root cause in an orchestrator whose lifecycle transition is incomplete.
- Symptom in a "the user did X and nothing happened" complaint, root cause in a mutation that didn't flip a dirty/persist flag.
- Symptom in a duplicated guard at three call sites, root cause in a missing shared helper or an invariant that should hold at the source.

If symptom site and root cause are the same place, say so explicitly with the reasoning — do not assume.

### 3. Present options at the root cause

Present **two or more options** that fix the bug at its architectural root cause. Options differ in depth/scope, **not** in "shallow patch vs. real fix" — every option must be an honest fix at the right seam. Typical axes:

- Repair at the facade vs. push the invariant to the store boundary vs. generalize the mutation pattern.
- Pull the lifecycle fixup into the orchestrator vs. introduce a derived store the orchestrator updates.
- Add a typed pending-set today vs. introduce a general mutation log (YAGNI tension).

Each option uses this format:

```
**Option N — {short title}**

Seam: {layer / module / file the option repairs}
ADR(s) upheld: {ADR numbers, or "none directly applicable"}
Principle(s) at stake: {DRY | SOLID/SRP | SOLID/<other> | YAGNI | Depth | Locality | ADR-compliance}
  — one-line explanation of how the current code violates this principle, and how the option restores it.
Tradeoff: {scope, risk, what this option defers or leaves for later, what it preserves}
```

Frame the options for an architect / product owner who understands design patterns but does not know the code. Recommend one and explain why.

**Naming the principle is required, not optional.** Without it the options drift into "technically correct fixes" without diagnostic clarity. Use the same vocabulary as the `design` skill and `build` HITL decisions so the language is consistent across the workflow.

### 4. Stopgap rule

Do **not** include a "just patch the symptom" option unless one of:

- The analysis shows the symptom site genuinely *is* the root cause, and the small change is the correct fix. Say so explicitly with reasoning.
- The user has explicitly asked for a quick patch.
- A stopgap is unavoidable for external reasons (release pressure, demo). In that case label it as a stopgap, state the underlying issue it leaves unresolved, and propose the follow-up as a `kind/idea` issue.

"Cheapness" is not a reason to include a shallow option. Treat "the cheapest local change" as a red flag, not a default.

### 5. Stop and wait

Stop and wait for the user to choose an option before editing code. Do not start implementing the recommended option speculatively.

## Common failure modes this skill prevents

- **Render-site guard accretion (DRY).** Adding a fourth `if (isPlaceholder)` check at a render site because the previous three "looked fine in context."
- **Incomplete lifecycle ownership (SOLID/SRP).** Orchestrator owns the data mutation; route owns the post-mutation menu/selection fixup; nothing owns the coordination. Bugs surface as stale UI state after the orchestrator finishes.
- **Asymmetric dirty tracking (SOLID/SRP + ADR-compliance).** Mutation A flips `isDirty`; symmetric mutation B doesn't. Caused by deriving dirty from one side of the state shape only.
- **Over-correction (YAGNI).** Replacing a typed pending-set with a "general mutation log framework" because it sounds principled. Match the depth of the seam to the depth of the actual problem.
- **Shallow module accretion (Depth).** Adding a thin pass-through to "centralize" something with one caller. A real centralization has multiple call sites today.
- **Resurrecting a rejected approach (ADR-compliance).** Re-proposing a design that an ADR already evaluated and rejected. The ADR check in step 1 is what catches this.

## Relationship to other skills and prompts

- `bugfix.prompt.md` and `quickchange.prompt.md` — both load this skill at the analysis step. Those prompts add workflow concerns around it (TDD regression encoding, test runs, aiwiki/backlog updates).
- `build` SKILL — loads this skill mid-slice when an AFK or REFACTOR slice surfaces an unanticipated complication that touches a seam.
- `design` SKILL — uses the same option/principle format for slice planning. The vocabulary here matches `design` deliberately.
- `improve-codebase-architecture` SKILL — when this skill identifies that a seam is broken across many sites (not just the bug under investigation), recommend invoking `improve-codebase-architecture` on that seam as part of the recommended option or as a follow-up `kind/idea` issue.

## What this skill does not do

- It does not implement the fix. Implementation happens after the user picks an option, under the chosen prompt (`/bugfix`, `/quickchange`, `/build`).
- It does not run tests. The calling prompt owns the test loop.
- It does not write ADRs by itself. If the chosen option warrants an ADR extension or new ADR, that happens during post-implementation enrichment in the calling prompt.
