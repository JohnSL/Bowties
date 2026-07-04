# Architecture-First Fix — Option Drafting Companion

Detailed procedure and rules for drafting the option set. Loaded by
`change-analyze` when composing options, not by the main conversation.

The main conversation loads only the spine (`SKILL.md`); this file contains
the full option format, self-check, banned-language list, philosophy, and
failure-mode catalogue.

## Option format

Present **two or more options** that fix the change at its architectural
root cause. Options must differ by **architectural direction** — which seam
becomes the owner, which invariant is enforced where — **not** by scope,
code volume, or "how much of the codebase is touched." Every option must be
an honest fix at the right seam.

Typical axes of variation:

- Repair at the facade vs. push the invariant to the store boundary vs.
  generalize the mutation pattern.
- Pull the lifecycle fixup into the orchestrator vs. introduce a derived
  store the orchestrator updates.
- Add a typed pending-set today vs. introduce a general mutation log
  (YAGNI tension).

Each option uses this format:

```
**Option N — {short title}**

Seam: {layer / module / file the option repairs}
ADR(s) upheld: {ADR numbers, or "none directly applicable"}
Principle(s) at stake: {DRY | SOLID/SRP | SOLID/<other> | YAGNI | Depth | Locality | ADR-compliance}
  — one-line explanation of how the current code violates this principle,
    and how the option restores it.
Regression class prevented: {a named class of future bugs this option makes
  impossible or materially harder to write, and *why* — e.g. "prevents any
  future Consumer from re-implementing this guard because the invariant is
  enforced at the Owner's mutation boundary", or "makes the invalid state
  unrepresentable in the type: X ∈ {A,B,C} instead of a nullable flag"}
Tradeoff: {what this option preserves and what it commits to —
  architectural direction, not scope}
```

Frame the options for an architect / product owner who understands design
patterns but does not know the code.

### Required fields — non-negotiable

- **Named principle** is required, not optional. Without it options drift
  into "technically correct fixes" without diagnostic clarity. Use the same
  vocabulary as the `design` skill and `build` HITL decisions.
- **`Regression class prevented:`** is required, not optional. It is the
  load-bearing field. A shallow fix has no honest answer to "what class of
  future bugs does this make impossible or hard to write?" If you cannot
  fill this field with a specific, named class of bugs and *why* the option
  prevents them, the option is not an option — it is a stopgap and falls
  under the stopgap rule. "Reduces the chance of this bug recurring" is not
  a regression class; "eliminates the state shape that makes this bug
  expressible" is.

### Seam-symmetry rule

When the change touches a seam in `aiwiki/seams.md`, every option must
address Owner / Contributor / Consumer symmetry at the documented Owner —
not at the symptom site. A patch at one Consumer that leaves other Consumers
diverging from the Owner is not an option; it is a stopgap.

**`Locality` is a valid sole principle only** when the symptom site
genuinely is the invariant owner. If the seam has multiple Consumers and
the option fixes at one of them, `Locality` alone does not justify it —
name the seam-level principle it addresses at the Owner, or the option is
a stopgap.

## Recommendation criterion — prevention breadth, not scope

Recommend the option that prevents the **widest and most consequential
class of future regressions** at the Owner of the seam. Justify the
recommendation by named prevention and named principle, and by **nothing
else**.

### Banned language in the recommendation and option text

- "smallest change", "smaller move", "smallest move that ships"
- "least risky", "safest short-term"
- "least scope", "cheapest to implement", "quickest"
- "quick fix", "for now", "we can revisit later"
- "just add", "just guard", "defensive check", "just handle this case here"

If any of these appear in your own draft recommendation *or* in an option's
description, rewrite until they don't. Their presence signals the
recommendation is being justified by cost, not by architectural direction.

### Principle over expedience

When the "smallest move that ships" creates a known regression hazard,
entrenches existing technical debt, or forecloses a near-term architectural
direction, recommend the principle-honoring path and name what the smaller
move would cost. Do not present "scope-minimal" as a virtue in itself — it
is only a virtue when it is also principle-honoring.

### Type-level safety over runtime cleverness

Prefer designs that make invalid states unrepresentable in the type system
over designs that rely on runtime conventions, callers "knowing not to,"
or flat enums that mix vocabularies. The question is "what stops the next
contributor from writing the wrong thing?" not "what's the fewest lines of
code today?"

### Reject ADR-violating options

Options that violate an active ADR or a documented invariant are marked as
rejected with the reference and removed from real consideration. They do
not appear as peer trade-offs; if they surfaced during drafting, list them
in the audit's "options rejected by self-check" section.

## Pre-present self-check

Before returning the option set, walk through this list. If any check
fails, rewrite before returning.

- [ ] Every option has a `Regression class prevented:` field with a named
      class of future bugs and *why* the option prevents them.
- [ ] No option is differentiated primarily by scope or code volume; each
      option commits to a different architectural direction.
- [ ] No option uses `Locality` as its sole principle unless the symptom
      site genuinely is the invariant owner (justified in the option's own
      text).
- [ ] No option's text uses banned-language red flags.
- [ ] The recommendation is justified by prevention breadth and principle,
      not by cost/scope/risk minimization.
- [ ] For any seam in `aiwiki/seams.md`, every option addresses Owner /
      Contributor / Consumer symmetry at the Owner.

## Stopgap rule

Do **not** include a "just patch the symptom" option unless one of:

- The analysis shows the symptom site genuinely *is* the root cause (the
  invariant owner), and the small change is the correct fix. Say so
  explicitly with reasoning, and fill in `Regression class prevented:`
  honestly — usually "no future contributor can regress this because the
  invariant lives here and every path passes through it."
- The user has explicitly asked for a quick patch.
- A stopgap is unavoidable for external reasons (release pressure, demo).
  Label it as a stopgap, state the underlying issue it leaves unresolved,
  and propose the follow-up as a `kind/idea` issue.

"Cheapness" is not a reason to include a shallow option. Treat "the
cheapest local change" as a red flag, not a default. An option that cannot
honestly fill `Regression class prevented:` is by definition a stopgap and
must be labeled as such.

## Common failure modes this discipline prevents

- **Render-site guard accretion (DRY).** Adding a fourth `if (isPlaceholder)`
  check at a render site because the previous three "looked fine in
  context."
- **Incomplete lifecycle ownership (SOLID/SRP).** Orchestrator owns the data
  mutation; route owns the post-mutation menu/selection fixup; nothing owns
  the coordination. Bugs surface as stale UI state after the orchestrator
  finishes.
- **Asymmetric dirty tracking (SOLID/SRP + ADR-compliance).** Mutation A
  flips `isDirty`; symmetric mutation B doesn't. Caused by deriving dirty
  from one side of the state shape only.
- **Over-correction (YAGNI).** Replacing a typed pending-set with a
  "general mutation log framework" because it sounds principled. Match the
  depth of the seam to the depth of the actual problem.
- **Shallow module accretion (Depth).** Adding a thin pass-through to
  "centralize" something with one caller. A real centralization has
  multiple call sites today.
- **Resurrecting a rejected approach (ADR-compliance).** Re-proposing a
  design that an ADR already evaluated and rejected. The ADR check in step
  1 of the spine is what catches this.
- **Recommend-by-scope drift (this discipline's own failure mode).**
  Presenting a small-scope Consumer-side patch as Option A and justifying
  it by "least risk" or "smallest change." The `Regression class
  prevented:` field and the prevention-breadth criterion exist to catch
  this before the user sees it.
