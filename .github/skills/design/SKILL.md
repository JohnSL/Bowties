---
name: design
description: Feature-scoped architecture assessment and vertical slice planning. Evaluates a feature's design against existing module depth, placement rules, and ADRs, then defines vertical slices for TDD implementation. Use when starting implementation after /plan, or when user says "design", "architecture assessment", or "slice planning".
---

# Feature Architecture Design

Assess a feature's architectural impact and plan vertical slices for TDD implementation. Runs after `/plan`, before `/slices`.

Use [LANGUAGE.md](../improve-codebase-architecture/LANGUAGE.md) vocabulary for architecture. Use `product/glossary.md` vocabulary for the domain.

## Process

### 1. Load context

1. Detect current feature from branch name or `$env:SPECIFY_FEATURE`
2. Read `specs/<feature>/plan.md` and `specs/<feature>/spec.md`
3. Read `product/architecture/code-placement-and-ownership.md`, `product/architecture/adr/`, `product/glossary.md`
4. Read `aiwiki/owners.md` (module inventory, shared conventions) and `aiwiki/flows.md`
5. Scan `specs/ideas/` for prior work matching the feature's area

### 2. Identify affected modules

From plan.md, identify which existing modules the feature touches. For each:
- Map to its layer and current owner (from `aiwiki/owners.md`)
- Identify which workflows are affected (from `aiwiki/flows.md`)
- Flag any new modules the plan proposes

### 3. Assess

Evaluate each affected and proposed module using the criteria in [ASSESSMENT.md](ASSESSMENT.md). The assessment covers depth, locality, seam placement, placement compliance, ADR compliance, duplication, cross-layer coupling, testability, existing debt, and deepening opportunities.

### 4. Define vertical slices

Using the methodology in [SLICING.md](SLICING.md), divide the feature into vertical slices. Each slice cuts through all necessary layers and is independently testable. Classify each as HITL or AFK. Order risk-first.

### 5. Scale and present

Scale the output to what the assessment found. See [ASSESSMENT.md](ASSESSMENT.md) scaling rules.

Present findings using domain vocabulary (from `product/glossary.md`) and architecture vocabulary (from [LANGUAGE.md](../improve-codebase-architecture/LANGUAGE.md)). For each finding: what it is, why it matters, recommended action.

**STOP and wait for user input.** The user decides for each finding: include in slices, defer as idea, or reject.

### 6. Update plan

After user decisions:

1. Append an **Architecture Assessment** section to plan.md (format in [ASSESSMENT.md](ASSESSMENT.md))
2. Include slice definitions with HITL/AFK labels
3. Capture deferred improvements as `specs/ideas/` entries (see `specs/ideas/README.md` for format)
4. Draft ADRs for rejected approaches with load-bearing reasons (see [ADR-FORMAT.md](../grill-with-docs/ADR-FORMAT.md))
5. Update `product/glossary.md` if terms were sharpened (see [GLOSSARY-FORMAT.md](../grill-with-docs/GLOSSARY-FORMAT.md))

### Handoff

Tell the user: "Architecture assessment complete. Run `/slices` to generate the slice task file."
