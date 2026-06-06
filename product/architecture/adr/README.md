# Architecture Decision Records

ADRs live here and use sequential numbering: `0001-slug.md`, `0002-slug.md`, etc.

## Template

```md
# {Short title of the decision}

{1-3 sentences: what's the context, what did we decide, and why.}
```

An ADR can be a single paragraph. The value is recording *that* a decision was made and *why* — not filling out sections.

## Optional sections

Only include when they add genuine value. Most ADRs won't need them.

- **Status** frontmatter (`proposed | accepted | deprecated | superseded by ADR-NNNN`) — useful when decisions are revisited
- **Considered Options** — only when rejected alternatives are worth remembering
- **Consequences** — only when non-obvious downstream effects need to be called out

## Numbering

Scan this directory for the highest existing number and increment by one.

## When to write an ADR

All three must be true:

1. **Hard to reverse** — the cost of changing your mind later is meaningful
2. **Surprising without context** — a future reader will look at the code and wonder "why?"
3. **The result of a real trade-off** — there were genuine alternatives and you picked one for specific reasons

If a decision is easy to reverse, skip it. If it isn't surprising, nobody will wonder. If there was no real alternative, there's nothing to record.

### What qualifies

- Architectural shape decisions
- Integration patterns between layers or contexts
- Technology choices with lock-in cost
- Boundary and scope decisions (ownership, explicit no-s)
- Deliberate deviations from the obvious path
- Constraints not visible in the code
- Rejected alternatives with non-obvious reasoning

## How ADRs evolve

ADRs are **append-only** and **seam-scoped**, not per-change. One ADR per durable architectural seam — a module boundary, an ownership rule, an integration pattern — and that file accrues the design rationale for that seam over time.

**Extend an existing ADR** with a new dated section when a new commitment refines or builds on the same principle in the same seam. Use the heading shape `## YYYY-MM-DD extension: <short title>` and mirror the original structure (Context / Decision / Consequences) scoped to the new commitment. Original sections stay intact; keep each section to the 1–3-sentence template discipline.

**Write a new ADR** only when one of these is true:

1. A genuinely different seam appears that the existing ADR doesn't cover.
2. You are reversing a prior commitment — mark the old ADR `superseded by ADR-NNNN` and explain in the new one what changed.
3. The new decision crosses a boundary the existing ADR explicitly excluded.

**Don't write an ADR** for implementation details that aren't load-bearing commitments. Use a code comment or an `aiwiki/` note instead.

When an absorbed ADR is folded into another, replace its file with a one-line tombstone (`Status: folded into ADR-MMMM on YYYY-MM-DD`) rather than deleting it — the number stays reserved and existing links keep working.
