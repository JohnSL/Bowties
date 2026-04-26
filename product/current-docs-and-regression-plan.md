# Current Docs And Regression Plan

## Purpose

This document captures the durable documentation and governance plan for keeping current behavior and architecture documented outside the temporary feature `specs/` workflow.

The feature-specific regression test plan that was originally drafted here now lives in `specs/010-offline-layout-editing/regression-test-plan.md` so it stays with the active feature work.

This file is intended to survive beyond the current feature session and act as a handoff document for future work.

## Why This Exists

The existing `specs/` tree is useful while building a feature, but it is not a reliable long-term source of truth once implementation, follow-up fixes, and refactors begin to diverge from the original plan.

The current regressions are a symptom of two gaps:

- user-visible behavior is not captured in one current place
- architecture boundaries are not enforced consistently enough during refactors

This plan addresses both gaps.

## Source Of Truth Model

### Temporary Planning Artifacts

The following remain useful, but are treated as feature-build artifacts rather than durable truth:

- `specs/`
- feature `plan.md`, `tasks.md`, and related contracts
- archived specs and historical planning notes

### Durable Current Documentation

A new root-level `product/` area will hold the current behavioral and architectural truth for the application.

Rules:

- `product/` documents current expected behavior and active architecture boundaries
- `docs/user/` contains end-user documentation
- `docs/project/` contains developer-process documentation
- `specs/` documents how a feature was planned and built
- when `specs/` and `product/` disagree, `product/` wins unless the code has been intentionally changed and the product docs are stale
- when behavior changes intentionally, tests and `product/` should be updated together

## Proposed Product Structure

Initial structure:

```text
product/
  README.md
  current-docs-and-regression-plan.md
  user-stories/
    offline-sync-workflows.md
  workflows/
    connect-and-discover.md
    open-layout-and-offline-mode.md
    read-configuration.md
    sync-offline-changes.md
  architecture/
    frontend-boundaries.md
    lifecycle-and-state-ownership.md
    naming-and-normalization.md
  quality/
    engineering-principles.md
    testing-strategy.md
  adr/
    0001-source-of-truth.md
    0002-frontend-boundaries.md
```

Recommended first files after this one:

- `product/README.md`
- `product/architecture/code-placement-and-ownership.md`
- `product/documentation-gap-and-reorganization-plan.md`
- `product/user-stories/README.md`
- `product/user-stories/connect-discover-read-configuration.md`
- `product/user-stories/bowties-viewing-and-editing.md`
- `product/user-stories/offline-capture-edit-sync.md`
- `product/user-stories/offline-sync-workflows.md`
- `product/architecture/frontend-boundaries.md`
- `product/quality/engineering-principles.md`
- `product/quality/testing-strategy.md`

The initial product spine is now in place for:

- `product/README.md`
- `product/architecture/code-placement-and-ownership.md`
- `product/documentation-gap-and-reorganization-plan.md`
- `product/architecture/frontend-boundaries.md`
- `product/quality/engineering-principles.md`
- `product/quality/testing-strategy.md`

The initial validated current user-story set is now in place for:

- `product/user-stories/README.md`
- `product/user-stories/connect-discover-read-configuration.md`
- `product/user-stories/bowties-viewing-and-editing.md`
- `product/user-stories/offline-capture-edit-sync.md`

## Engineering Principles To Enforce

These principles should be expressed as repo rules, not slogans.

### SOLID Applied To Bowties

- Route files compose screens, wire events, and coordinate visible UI state.
- Components render state and emit intent events; they should not own multi-step business sequencing.
- Stores own durable frontend state and local state transitions.
- Orchestrators own async sequencing, cross-store workflows, and lifecycle-sensitive transitions.
- API adapter modules own Tauri IPC calls and translation of backend response shapes into frontend-friendly types.

### DRY Applied To Bowties

- one Node ID normalization rule
- one display-name fallback rule
- one lifecycle transition owner per workflow
- one documented acceptance rule per user-visible behavior

### YAGNI Applied To Bowties

- prefer small explicit helpers over generic frameworks
- only introduce a shared abstraction when at least two real call sites need the same concept
- avoid speculative layers that do not remove a current bug or reduce current complexity

## Proposed Copilot Governance

The repository already has `.github/agents/`, `.github/instructions/`, `.github/prompts/`, and `.github/skills/`.

We should use them more deliberately.

### Always-On Workspace Instructions

Add a stable shared instruction file for rules that should apply across most implementation work.

That instruction should require:

- treat `product/` as the durable source of truth for current behavior and architecture
- treat `specs/` as temporary planning artifacts unless explicitly stated otherwise
- keep components declarative
- move side-effect sequencing and lifecycle coordination into orchestrators or focused stores
- reuse shared normalization helpers rather than creating local variants
- when behavior changes, update tests and `product/` together

### File-Scoped Instructions

Add focused instructions for specific areas:

- `product/**`
  - documents must be concise, current, behavioral, and implementation-aware without being code-changelog prose
- `app/src/routes/**` and `app/src/lib/components/**`
  - components and routes should avoid owning multi-step business sequencing
- `app/src/lib/orchestration/**` and `app/src/lib/stores/**`
  - prefer explicit contracts, lifecycle guardrails, and testable pure helpers where possible

### Skills

Add small workflow skills rather than large general-purpose ones.

Recommended skills:

- `maintain-current-docs`
  - update `product/` after approved behavior or architecture changes
- `architecture-boundary-review`
  - check a proposed change against current frontend/store/orchestrator boundaries
- `regression-contracts`
  - convert a bug report or risky seam into behavior contracts and test targets

## Architecture Guardrails To Document

The following should be documented in `product/architecture/frontend-boundaries.md` and reinforced in instructions.

### Frontend Ownership Boundaries

- `+page.svelte` should compose view state and delegate multi-step workflows
- orchestrators should own discovery upgrade flow, sync trigger flow, offline layout replay flow, and post-apply rebuild flow
- stores should own persistent frontend state and deterministic transitions
- pure helpers should own normalization, formatting, and reusable value translation logic

### Specific Current Risk Areas

- discovery naming and SNIP/PIP enrichment
- CDI gating and download prompting
- connect-dialog and layout-open interaction
- offline pending-change save/discard layering
- sync session auto-trigger and manual reopen behavior
- pending-value restamping after discard, reconnect, and partial apply

## Regression Test Plan

The feature-specific regression test plan now lives in `specs/010-offline-layout-editing/regression-test-plan.md`.

The product docs should continue to describe durable documentation and governance rules. The spec-local test plan should continue to describe:

- behavior contracts to protect during the offline/sync/discovery repair work
- target test files and test layers
- regression priorities and execution order
- known review-only gaps that should stay visible during the fix pass

## Proposed Execution Order

### Phase A: Durable Documentation Setup

1. Keep `product/README.md` current as the entry point to durable product docs.
2. Keep `product/quality/engineering-principles.md` current as the concrete engineering-rules document.
3. Keep `product/architecture/frontend-boundaries.md` current as the frontend ownership document.
4. Create the first current user-story and workflow documents for offline/sync behavior.

### Phase B: Copilot Governance Setup

5. Add a stable workspace instruction file for current-doc and architecture-boundary rules.
6. Add file-scoped instructions for `product/**` and frontend boundary areas.
7. Add a small skill for maintaining current docs and one for regression-contract review.

### Phase C: Regression Test Pass

8. Implement the spec-local regression test plan in `specs/010-offline-layout-editing/regression-test-plan.md`.
9. Start with the Priority 1 route-level regressions.
10. Add the Priority 2 rendering and gating tests.
11. Add the Priority 3 lifecycle and pending-value tests.
12. Use the failing tests to drive fixes.

### Phase D: Follow-Up Architecture Work

13. Resume refactor work only after the high-value regression contracts are passing.
14. Update `product/` whenever behavior or ownership boundaries change.

## Approval Checklist

Before implementation, confirm these decisions:

- `product/` is accepted as the durable source of truth for current behavior and architecture
- `specs/` remains a temporary planning workflow, not the long-term truth source
- the repo will enforce behavior-plus-doc updates together for intentional changes
- the first test pass should focus on behavior contracts, not diff coverage by file count
- frontend boundary rules should explicitly separate routes, components, stores, orchestrators, and helpers

## Session Handoff Notes

When resuming this work in a later session:

- start with this document
- confirm whether the `product/` structure and naming are still approved
- implement documentation and Copilot governance before or alongside the first test additions
- keep the regression test plan behavior-first and avoid broad speculative rewrites