# Regression Test Plan

**Feature**: `010-offline-layout-editing`

This document tracks the regression test pass for the recent offline/sync/discovery refactor work.

The goal is not to mirror every edited file. The goal is to protect every user-visible behavior and every high-risk orchestration contract changed by the recent work.

See also:

- `specs/010-offline-layout-editing/refactoring-roadmap.md`
- `specs/010-offline-layout-editing/tasks.md`
- `handbook/current-docs-and-regression-plan.md`

## Test Layers

- store tests for deterministic state transitions
- orchestrator tests for multi-step sequencing and boundary logic
- component tests for rendering contracts and intent emission
- route-level tests for cross-component workflow behavior in `app/src/routes/+page.svelte`

## Existing Coverage To Reuse

These already cover important seams and should not be duplicated unnecessarily:

- `app/src/lib/orchestration/discoveryOrchestrator.test.ts`
- `app/src/lib/orchestration/syncSessionOrchestrator.test.ts`
- `app/src/lib/components/Sync/SyncPanel.lifecycle.test.ts`
- `app/src/lib/stores/offlineChanges.store.test.ts`
- `app/src/lib/components/ElementCardDeck/TreeLeafRow.offline.test.ts`
- `app/src/lib/stores/layoutOpenLifecycle.test.ts`

## Priority 1 Regression Tests

These are the first tests to add because they map directly to reported regressions.

1. No-layout live discovery shows friendly node names after SNIP arrives, not only raw Node IDs.
2. Confirmed CDI-less nodes are excluded from CDI download prompts.
3. A CDI-less node does not block reading configuration for other eligible nodes.
4. Opening a layout while the connect dialog is visible dismisses the connect dialog.
5. Closing a layout with only saved pending-sync rows does not trigger the Unsaved Changes dialog.

## Priority 2 Rendering And Gating Tests

These cover closely related behavior that is likely to regress from the same seams.

6. Node display-name fallback order is user name, then manufacturer plus model, then Node ID.
7. Duplicate friendly names in the node list are disambiguated.
8. Selecting a confirmed CDI-less node does not show the per-node Read Configuration CTA.
9. Config sidebar naming uses the same fallback order as the rest of the UI.
10. Config sidebar shows configuration-not-supported behavior for confirmed CDI-less nodes.

## Priority 3 Lifecycle And Pending-Value Tests

These protect the save/discard and sync lifecycle areas changed by the recent refactor.

11. Discard restores the last saved pending offline state and restamps pending values into the tree.
12. Restored saved pending rows do not count as unsaved local edits by themselves.
13. Partial sync apply rebuilds affected trees and preserves visible pending values for unaffected rows.
14. Disconnect with no layout returns to the connection dialog.
15. Disconnect with an open layout preserves or rehydrates layout state according to the active mode and snapshot availability.

## Priority 4 Review-Only Gaps

These are not necessarily refactor regressions, but they should stay visible while implementing the test plan.

- already-applied rows are still not auto-cleared from backend in-memory cache during sync-session build
- snapshot YAML refresh after successful apply is still incomplete
- some CDI preflight failures may still be treated as generic missing-CDI cases rather than differentiated errors

## Suggested Test File Targets

Most likely files to add or extend:

- `app/src/routes/+page.test.ts`
- `app/src/lib/components/NodeList.test.ts`
- `app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts`
- `app/src/lib/orchestration/discoveryOrchestrator.test.ts`
- `app/src/lib/components/Sync/SyncPanel.lifecycle.test.ts`
- `app/src/lib/stores/offlineChanges.store.test.ts`
- `app/src/lib/components/ElementCardDeck/SaveControls.test.ts`

## Proposed Execution Order

1. Add the five Priority 1 regression tests first.
2. Add the Priority 2 rendering and gating tests.
3. Add the Priority 3 lifecycle and pending-value tests.
4. Use the failing tests to drive fixes.

## Notes

- Keep the regression pass behavior-first. Do not chase file-level test symmetry.
- Prefer the narrowest seam that can prove the intended contract.
- When a behavior is both user-visible and lifecycle-sensitive, prioritize a route-level test over duplicating the same assertion across multiple lower-level tests.