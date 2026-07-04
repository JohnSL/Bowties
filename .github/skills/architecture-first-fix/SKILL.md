---
name: architecture-first-fix
description: Stop-and-propose-options procedure for bugfixes, behavior changes, and mid-implementation surprises. Forces root-cause analysis at the right seam, with options named in terms of the principle at stake (DRY / SOLID / YAGNI / Depth / Locality / ADR-compliance). Use for any bugfix or behavior change (chat or slash-commanded), and any mid-slice surprise during /build.
---

# Architecture-First Fix — Spine

The procedure to run before editing code for a bugfix, behavior change, or
mid-implementation surprise. The goal is to prevent the slow architectural
decay that results from a sequence of locally-reasonable patches at symptom
sites.

This file is the always-loaded **spine**: when this skill applies, the
required outputs, the delegation contract, and the stop-and-wait gate.
The detailed option format, self-check, banned-language list, philosophy,
and failure-mode catalogue live in the companion file
[option-drafting.md](option-drafting.md), which is loaded only by the
`change-analyze` subagent when it actually composes options.

## When this skill applies

- Any bugfix request, whether triggered by `/bugfix`, `/quickchange`, or a
  freeform chat message ("fix this", "this is wrong", "it should…").
- Any behavior change to existing code (as opposed to net-new feature work,
  which uses `design` + `slices` + `build`).
- Any mid-slice surprise during `build`: a planned approach conflicts with
  an ADR, a test fails because the slice's design violates an invariant, or
  coordinating state across layers turns out to be more involved than the
  slice anticipated.

**Exempt:** trivial mechanical edits (typo, comment, import sort,
formatting), and cases where the user has explicitly said "just patch it",
"skip the architecture check", or equivalent.

## Delegation contract — invoke `change-analyze`

Do **not** perform seam identification, options-drafting, or the self-check
inline in the main conversation. Delegate to the
[`change-analyze`](../../agents/change-analyze.agent.md) subagent, which:

- Loads the option-drafting companion internally.
- Runs seam identification via Explore subagents (dead-end reads stay
  inside).
- Drafts options with the required fields, including the mandatory
  `Regression class prevented:` field.
- Runs the pre-present self-check and rewrites options that fail it.
- Returns a single structured summary block plus investigation audit
  metadata (hypotheses ruled out, ADRs scanned, options rejected by
  self-check, prior-work issues, assumptions).

Pass the appropriate **mode** (`bugfix`, `quickchange`, `hitl-decision`,
`mid-slice-escalation`) so the return contract matches how you'll present
it to the user. See the agent file for the mode-specific behavior.

The main conversation never reads source files, ADRs, or `aiwiki/` files
during architecture-first analysis. Every investigation lives inside the
subagent.

## Required properties of every option

The subagent enforces these; the main conversation must not accept a
returned option that lacks them:

- **Named principle** — DRY / SOLID/SRP / SOLID/other / YAGNI / Depth /
  Locality / ADR-compliance.
- **`Regression class prevented:`** — a named class of future bugs the
  option makes impossible or materially harder to write, and *why*. This
  is the load-bearing field. Missing or vague → the option is a stopgap.
- **Seam-symmetry at the Owner** when the change touches a documented seam
  in `aiwiki/seams.md`. Consumer-side patches without Owner-side symmetry
  are stopgaps.

## Recommendation criterion — prevention breadth, not scope

The recommendation must be justified by **prevention breadth and named
principle**. Cost/scope/risk minimization is not a valid justification.
The subagent enforces the banned-language list; the main conversation must
not weaken it when presenting.

## Stop and wait

After presenting the returned block to the user, **stop**. Do not start
implementing the recommended option speculatively. Wait for the user to
choose an option (or override the recommendation).

If the user's choice materially changes the direction (picks Option B over
your recommendation, or asks for a hybrid), it is fine to proceed — the
options are peers, not a preference ranking. If the user asks a follow-up
question the returned summary can't answer, re-invoke `change-analyze` with
the corrected framing rather than reconstructing an answer inline.

## Relationship to other skills and prompts

- [bugfix.prompt.md](../../prompts/bugfix.prompt.md) and
  [quickchange.prompt.md](../../prompts/quickchange.prompt.md) — both
  invoke `change-analyze` at their options step (mode `bugfix` /
  `quickchange`). The prompts add workflow concerns around it (TDD
  regression encoding, test runs, enrichment).
- [`build` SKILL](../build/SKILL.md) — HITL Part 2 invokes `change-analyze`
  once per numbered decision (mode `hitl-decision`). Mid-slice surprises
  from `tdd-cycle` invoke it with mode `mid-slice-escalation`.
- `design` SKILL — uses the same option/principle vocabulary for slice
  planning.
- `improve-codebase-architecture` SKILL — when the analysis identifies that
  a seam is broken across many sites (not just the change under
  investigation), the recommendation may cite `improve-codebase-architecture`
  as a follow-up.

## What this skill does not do

- It does not implement the fix. Implementation happens after the user
  picks an option, under the chosen prompt (`/bugfix`, `/quickchange`,
  `/build`).
- It does not run tests. The calling prompt owns the test loop.
- It does not write ADRs by itself. If the chosen option warrants an ADR
  extension, that happens during post-implementation enrichment in the
  calling prompt.
