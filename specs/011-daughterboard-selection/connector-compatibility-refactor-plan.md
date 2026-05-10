# Refactor Plan: Connector Compatibility Ownership Cleanup

**Branch**: `011-daughterboard-selection`  
**Date**: 2026-05-05  
**Scope**: Board-switch compatibility and auto-correction workflow for connector-governed config leaves

## Summary

Refactor the connector board-switch workflow so one pure, testable decision contract owns compatibility and auto-correction outcomes. The current behavior is fragile because compatibility, pending-change staging, effective-value projection, and message rendering are split across multiple layers that can observe different "current" values.

The target architecture keeps only the behavior needed now:

- profile-authored constraints remain the sole hardware-policy input
- when a selected board invalidates a value, Bowties stages the first compatible allowed value
- components render the outcome and do not infer compatibility policy on their own

## Problem Statement

The current workflow is difficult to reason about because one user-visible behavior spans several owners:

- constraint evaluation in `app/src/lib/utils/connectorConstraints.ts`
- board-switch workflow sequencing in `app/src/lib/orchestration/connectorSelectionOrchestrator.ts`
- pending-row layering in `app/src/lib/stores/offlineChanges.svelte.ts`
- tree projection in `app/src/lib/stores/nodeTree.svelte.ts`
- message rendering in `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`

That layering makes it possible for the planner to decide one thing while the renderer shows another. The repeated failures to get an apparently simple behavior working are a signal that ownership is unclear, not just that one helper is wrong.

## Design Goal

Create one canonical decision contract for connector-governed leaf state.

Given:

- the effective current value for a leaf
- the resolved connector constraint state for that leaf
- the leaf metadata needed for scalar conversion

Return exactly one of:

- `compatible`
- `autoCorrect` with the next value to stage
- `unsupported` with a reason explaining why no correction can be derived

This contract should be pure TypeScript and fully unit-testable.

## Target Ownership Model

### Utility Layer

**Owner**: new pure helper module under `app/src/lib/utils/**`

Owns:

- compatibility verdict for one leaf
- first-allowed-value correction policy
- conversion from allowed scalar values to a concrete `TreeConfigValue`

Does not own:

- store mutation
- orchestration sequencing
- rendering

### Store Layer

**Owner**: `app/src/lib/stores/offlineChanges.svelte.ts`

Owns:

- layered persisted and draft pending rows
- effective pending row lookup for a leaf
- explicit "effective current value" selectors needed by workflow and UI

Does not own:

- connector board-switch sequencing
- compatibility policy

### Orchestrator Layer

**Owner**: `app/src/lib/orchestration/connectorSelectionOrchestrator.ts`

Owns:

- board-switch workflow sequencing
- collecting governed leaves
- calling the canonical decision contract
- staging or clearing pending changes based on that decision output

Does not own:

- local fallback ladders beyond invoking the pure contract
- component-facing display decisions

### Component Layer

**Owner**: `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`

Owns:

- rendering the effective value
- rendering the message returned by the shared state/decision model
- emitting user intent

Does not own:

- compatibility policy
- first-allowed fallback decisions
- recomputation of board-switch outcomes from filtered option lists

## Implementation Phases

### Phase 1: Add the Missing Workflow Contract Tests

Add focused tests that prove the actual user workflow:

1. switching to `BOD4` with an incompatible governed value stages the first allowed value
2. switching to `BOD-8-SM` with an incompatible governed value stages the first allowed value
3. switching between those boards preserves a valid effective pending value
4. the incompatible message clears when a compatible staged value exists
5. the incompatible message remains only when no compatible staged value can be produced

These tests should be concentrated at the orchestrator and nearest rendering seams, not only at helper level.

### Phase 2: Introduce a Canonical Leaf Decision Helper

Create a pure helper module, for example `app/src/lib/utils/connectorLeafDecision.ts`, that:

- accepts the effective current value and connector constraint state
- decides whether the value is compatible
- derives the first compatible allowed value when needed
- returns a small discriminated result object

The helper should not know about stores, Svelte, or backend calls.

### Phase 3: Make Effective Current Value Explicit

Refactor the pending-change store so the orchestrator and renderer can ask the same question and get the same answer:

- what is the effective current value for this leaf right now?

This should be a selector-level API, not duplicated ad hoc logic in multiple consumers.

### Phase 4: Refactor Board-Switch Sequencing

In `connectorSelectionOrchestrator.ts`:

- collect governed leaves
- resolve the effective current value from the store/tree contract
- call the canonical leaf decision helper
- if `autoCorrect`, stage the returned value
- if `compatible`, do nothing
- if `unsupported`, surface a warning

The orchestrator should become a workflow coordinator only.

### Phase 5: Refactor Rendering to Consume Shared Outcome

In `TreeLeafRow.svelte` and any adjacent view-state helpers:

- stop deriving incompatibility from filtered dropdown entries alone
- render from the same effective value and decision outcome used by the orchestrator path

This removes presentation-layer policy drift.

### Phase 6: Remove Redundant Compatibility Logic

After the workflow tests are green, delete any duplicated compatibility or fallback logic that remains in:

- components
- view-state helpers
- orchestrator-local helper branches that no longer add unique value

## Non-Goals

This refactor should not:

- reintroduce a broad repair-rule framework
- add new profile schema for explicit repair defaults
- move hardware-policy decisions into Svelte components
- invent backend-owned workflow state for a frontend-owned editing flow

## Acceptance Criteria

The refactor is complete when all of the following are true:

1. A board switch uses one canonical decision path to determine compatibility and correction.
2. The first allowed value is staged when the current value is invalid and a compatible allowed value exists.
3. The renderer and the planner observe the same effective current value.
4. The incompatibility message appears only when no compatible staged value can be produced.
5. No Svelte component owns connector compatibility policy.
6. Focused workflow tests cover the real board-switch behavior that previously regressed.

## Recommended Execution Order For The Next Session

1. Add the failing workflow tests first.
2. Introduce the canonical decision helper.
3. Refactor the effective-value selector contract.
4. Update the orchestrator to use the shared helper and selector.
5. Update rendering to consume the same outcome.
6. Remove duplicate logic after the tests are green.

## Rationale

This plan follows the principles the project cares about:

- **SOLID**: one owner per concern
- **DRY**: compatibility and correction logic defined once
- **YAGNI**: only support the current first-allowed-value behavior
- **KISS**: planner consumes one simple decision contract instead of a layered fallback system
