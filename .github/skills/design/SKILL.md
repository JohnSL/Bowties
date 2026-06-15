---
name: design
description: Feature-scoped architecture assessment and vertical slice planning. Evaluates a feature's design against existing module depth, placement rules, and ADRs, then defines vertical slices for TDD implementation. Use when starting implementation after /plan, or when user says "design", "architecture assessment", or "slice planning".
---

# Feature Architecture Design

Assess a feature's architectural impact and plan vertical slices for TDD implementation. Runs after `/plan`, before `/slices`.

Use [LANGUAGE.md](../improve-codebase-architecture/LANGUAGE.md) vocabulary for architecture. Use `product/glossary.md` vocabulary for the domain.

## Process

### 1. Load context

**Delegate to an `Explore` subagent** to conserve main-conversation context. The subagent should fetch all of the following and return a structured summary (affected modules, relevant ADRs, terminology notes, prior `kind/idea` issues, placement rules excerpt):

1. Detect current feature from branch name or `$env:SPECIFY_FEATURE`
2. Read `specs/<feature>/plan.md` and `specs/<feature>/spec.md`
3. Read `product/architecture/code-placement-and-ownership.md`, `product/architecture/adr/`, `product/glossary.md`
4. Read `aiwiki/owners.md` (module inventory, shared conventions) and `aiwiki/flows.md`
5. Search open GitHub issues labeled `kind/idea` filtered by the feature's `area/*` labels (`gh issue list --repo JohnSL/Bowties --label kind/idea --state open`) for prior work. Also glance at any residual `specs/ideas/**` files until migration completes.

Work from the subagent's summary for subsequent steps.

### 2. Identify affected modules

From plan.md, identify which existing modules the feature touches. For each:
- Map to its layer and current owner (from `aiwiki/owners.md`)
- Identify which workflows are affected (from `aiwiki/flows.md`)
- Flag any new modules the plan proposes

### 3. Assess

Evaluate each affected and proposed module using the criteria in [ASSESSMENT.md](ASSESSMENT.md). The assessment covers depth, locality, seam placement, placement compliance, ADR compliance, duplication, cross-layer coupling, testability, existing debt, and deepening opportunities.

### 4. Define vertical slices

Using the methodology in [SLICING.md](SLICING.md), divide the feature into vertical slices. Each slice cuts through all necessary layers and is independently testable. Classify each as HITL or AFK. Order risk-first.

Apply the **Vertical-Slice Gate** from [SLICING.md](SLICING.md): reject any horizontal slice (one that fails to cut all needed layers or yields nothing the user can exercise) and reshape it before continuing. "Testable" means *user-demoable*, not merely *test-covered*.

**Validate at the slice-set (roadmap) level — do not write per-layer task breakdowns here.** `/design` owns the whole-feature view: it checks that the *set* of slices and their seams are sound (no cycles, no depth/locality violations, no slice that can't be demoed). It defines each slice as a card — title, one-line intent, layer boundary, HITL/AFK/REFACTOR label, acceptance criteria, and (for HITL/new-seam slices) a short architecture note. The per-layer task breakdown is authored later, one slice at a time, by `/build` (just-in-time tasking) — writing it now would commit pivot-fragile detail that earlier slices may invalidate. The architecture firewall lives here at the roadmap level; the per-layer task detail does not.

### 5. Scale and present

Scale the output to what the assessment found. See [ASSESSMENT.md](ASSESSMENT.md) scaling rules.

Present findings as a **single chat message** with three sections. Do NOT use `vscode_askQuestions` — batch presentation enables the user to spot cross-cutting architectural smells that sequential questioning destroys.

**Section 1 — Architectural shape** (for the architect / product owner):

- **Before/after mermaid diagrams** showing the module-level architecture today vs. after the feature lands. Show responsibilities and data flow between modules, not code details.
- **Pattern names** — name each architectural pattern being introduced or changed, with a one-sentence explanation of what it means in this feature's context.
- **Module-level change table** — columns: Module | Today | After. Describe responsibility shifts, not implementation details.

**Section 2 — Findings** (for principle-level review):

For each finding: what principle is at stake (DRY, SOLID, YAGNI, Depth, Locality, etc.), why it matters at the architectural level, and the recommended action. Use domain vocabulary (from `product/glossary.md`) and architecture vocabulary (from [LANGUAGE.md](../improve-codebase-architecture/LANGUAGE.md)).

**Section 3 — Proposed slices** (for behavioral review):

A **behavior summary table** — one row per slice, columns: Slice title | User-visible change | Demoable? This gives the product manager a 30-second scan of when to pay attention. Slices where "User-visible change" is empty are flagged as `[REFACTOR]` slices.

**STOP and wait for user input.** The user decides for each finding: include in slices, defer as idea, or reject.

### 6. Update plan

After user decisions:

1. Append an **Architecture Assessment** section to plan.md (format in [ASSESSMENT.md](ASSESSMENT.md))
2. Include slice definitions with HITL/AFK labels
3. For each deferred improvement, **propose** a GitHub issue (title, `kind/*` plus relevant `area/*` labels, body using the idea template fields: Summary, Areas, Origin, Prior Work, Open Questions). Present the proposals to the user and **wait for explicit confirmation** before creating any issue. Do not pre-apply a `status/*` label.
4. Draft ADRs for rejected approaches with load-bearing reasons (see [ADR-FORMAT.md](../grill-with-docs/ADR-FORMAT.md))
5. Update `product/glossary.md` if terms were sharpened (see [GLOSSARY-FORMAT.md](../grill-with-docs/GLOSSARY-FORMAT.md))

### Handoff

Tell the user: "Architecture assessment complete. Run `/slices` to generate the slice task file."
