---
name: slices
description: Generate a slice-organized task file from a feature's architecture assessment. Produces a cross-session progress tracker with vertical slices, HITL/AFK labels, and checkboxes. Use when user says "slices", "generate slices", or after running /design.
---

# Slice Task Generation

Generate a slice-organized task file from a feature's architecture assessment. Runs after `/design`, before `/build`.

`/slices` emits the **Tier 1 roadmap only** — an ordered set of slice **cards**, each rich enough to review (intent, boundary, acceptance criteria, and an architecture note for slices that shift the architecture). It does **not** write the per-layer task breakdown; `/build` appends that one slice at a time (just-in-time tasking). See [SLICE-FORMAT.md](SLICE-FORMAT.md) for the two-tier model.

## Process

### 1. Load context

1. Detect current feature from branch name or `$env:SPECIFY_FEATURE`
2. Read `specs/<feature>/plan.md` — requires the Architecture Assessment section (from `/design`)
3. Read `specs/<feature>/spec.md` for user stories and acceptance criteria

If plan.md lacks an Architecture Assessment section, tell the user to run `/design` first.

### 2. Draft the roadmap

From the Architecture Assessment's Vertical Slices section, build the **roadmap** — an overview table plus one **roadmap card** per slice. Each card captures enough to review the slice without seeing code:

1. **Title**
2. **Intent** — one line: what the user can see or do after the slice (for `[REFACTOR]`, the invariant preserved)
3. **Boundary** — the set of layers it cuts
4. **Label** — HITL / AFK / REFACTOR
5. **Blocked by** — slice dependencies
6. **Acceptance criteria** — the behavioral, product-manager-verifiable outcomes that say what you'll be able to test/demo (for `[REFACTOR]`, the invariants preserved). **For every seam this slice contributes to per `aiwiki/seams.md`, include at least one behavioural assertion per documented Consumer surface.** If a slice adds a Contributor to the Dirty Aggregation seam, the criteria must cover the Save toolbar AND the close prompt — not just the new store's internal state. The seam entry's Consumer list is the checklist.
7. **Architecture note** — *for HITL / new-seam slices only*: 1–2 lines naming the pattern the slice introduces or the seam it changes, and why it needs review. Omit for AFK/REFACTOR slices that reuse an established pattern.
8. **Status** — `sketched` (every slice starts here)

**Do not write the per-layer task breakdown here.** That is Tier 2, authored by `/build` when it reaches each slice. Acceptance criteria and the architecture note are *not* deferred — they are the slice's contract and impact, which you need to review and approve, and they are pivot-stable. Only the task list (`S1-T2: store…`, `S1-T3: backend…`) is pivot-fragile and deferred.

Apply the **Vertical-Slice Gate** from [SLICING.md](../design/SLICING.md): every slice must cut all needed layers **and** yield a user-exercisable behavior (or a preserved invariant for `[REFACTOR]`). Reject and reshape any horizontal slice — "just the store", "just the types", "all the backend", "all the tests" — by folding it into the first downstream slice that becomes demoable. "Testable" means *user-demoable*, not merely *test-covered*.

### 3. Quiz the user

Present the proposed roadmap as a numbered list. For each slice show:
- Title and HITL/AFK/REFACTOR label
- One-line intent (user-visible change, or invariant preserved)
- Layer boundary
- Blocked-by relationships
- Acceptance criteria (what the user will be able to test/demo)
- Architecture note for HITL/new-seam slices (the pattern/seam under review)

Ask:
- Does the granularity feel right? (too coarse / too fine)
- Are the dependency relationships and ordering correct?
- Are the HITL/AFK/REFACTOR labels right?
- Are the acceptance criteria the right way to verify each slice? Anything missing or untestable?
- For HITL slices, is the architectural impact clear and is the pattern choice sound?
- Is every slice demoable to a product manager? If not, is the non-demoable slice justified as a `[REFACTOR]` or migration?
- Should any slices be merged or split?

Iterate until the user approves. Keep refining the cards — resist the urge to write the per-layer task breakdown into any slice.

### 4. Generate slices.md

Write the approved roadmap to `specs/<feature>/slices.md` using the format in [SLICE-FORMAT.md](SLICE-FORMAT.md): the architecture header (diagrams, patterns, module changes, behavior summary), the overview table, and one **roadmap card** per slice (intent, boundary, blocked-by, acceptance criteria, architecture note where applicable) with every slice at `status: sketched`. Do not write any Tier 2 task breakdowns.

### Handoff

Tell the user: "Roadmap generated. Run `/build` — it expands and implements one slice at a time."
