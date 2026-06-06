# Effective node facade owns per-node persistability

Status: accepted (extends ADR-0004 and ADR-0007)
Date: 2026-05-31

## Context

ADR-0004 introduced `effectiveLayoutStore` as the single read model that
projects the four edit-layer stores plus `nodeTreeStore` into the values
the UI renders. Its surface is intentionally value-shaped:
`effectiveValue`, `effectiveRole`, `slotsByRole`, `isSlotFree`,
`effectiveBowties`. It has no per-node projection.

ADR-0007 introduced the **full-capture threshold** for promoting
discovered nodes into a saved layout:
`fullyCaptured(nodeId) ≡ nodeTreeStore.trees.has(nodeId) ∧ ¬ partialCaptureNodes.has(nodeId)`.
The threshold was located inline in `+page.svelte` as
`fullyCapturedNodeIds` / `unsavedInMemoryNodeIds` derivations, with the
same projection mirrored into `layoutStore.setUnsavedInMemoryNodeIds(...)`
so that `layoutStore.isDirty` could fold it in. `configReadNodesStore`
was not part of the threshold and not an input to the layout facade.

Manual testing of the Spec 014 build surfaced three regressions:

| # | Symptom |
|---|---------|
| R5 | Empty layout + connect → top bar shows "N unsaved changes"; Save promotes nodes whose config was never read. Offline reopen warns the values were not captured. |
| R6 | Clicking an unread real node shows the orange in-memory-changes dot. |
| R7 | Close a layout containing a placeholder, then create a new layout: the old placeholder reappears in the new layout. |

R5 and R6 share a root cause: there is no shared `isPersistableInLayout`
predicate. Save (`canSaveLayoutAction`), the orange dot, the
unsaved-changes count, and the unsaved-new badge each compute their own
slice in different files — they have drifted, and none of them consult
`configReadNodesStore`.

R7 is a related symptom of the same architectural gap: no single owner
enumerates the stores that constitute "a layout's worth of state."
`resetLayoutStateForNoLayout` calls `nodeRoster.replaceLiveRoster([])`,
which deliberately preserves placeholders (correct for refresh /
disconnect), instead of `nodeRoster.clearLayoutScope()`, which fully
clears the layout's roster (correct for close / new). The right method
exists; the wrong one is wired in. When Spec 014 moved placeholders into
`nodeInfoStore`, this reset path silently drifted because no test
enumerates what a layout-close must clear.

## Decision

Extend the layout facade to own **per-node projection** across the same
three layers the value facade already projects, and move lifecycle
**reset enumeration** behind a single orchestrator.

### Per-node facade

Introduce `effectiveNodeStore` (sibling to `effectiveLayoutStore` in
`app/src/lib/layout/`) with the following getters, exposed through
`$lib/layout`:

- `nodeOrigin(key): 'live-only' | 'layout-only' | 'both' | 'placeholder'`
- `isFullyCaptured(key)` — `nodeTreeStore.has(key) ∧ ¬ partialCapture.has(key)` (pins ADR-0007).
- `isConfigRead(key)` — `configReadNodesStore.has(canonical(key))`.
- `isPersistableInLayout(key)` — `isFullyCaptured(key) ∧ (isConfigRead(key) ∨ key.kind === 'placeholder')`.
- `unsavedInMemoryNodeIds` — live keys that are `isPersistableInLayout` and absent from `layoutStore.activeContext.layoutNodeIds`.
- `isDirty` — any persistable in-memory addition OR any draft / metadata / offline-change / layout-struct edit.

Inputs: `nodeTreeStore`, `nodeInfoStore`, `configReadNodesStore`,
`partialCaptureNodesStore` (extracted from a route-local `$state`),
`layoutStore`, `configChangesStore`, `bowtieMetadataStore`,
`offlineChangesStore`. The facade reads only; writes stay with the
existing edit-layer stores.

`partialCaptureNodes` is promoted from a `+page.svelte` `$state` to a
real `partialCaptureNodesStore` so the facade's input set is uniform
and the lifecycle owner has a concrete store to reset.

The four pre-existing per-node-persistability sites all migrate to read
through the facade:

- `+page.svelte`: `fullyCapturedNodeIds` / `unsavedInMemoryNodeIds`
  derivations are deleted; the route and other consumers read
  `effectiveNodeStore.unsavedInMemoryNodeIds` / `.isDirty` directly.
  `layoutStore.isDirty` is narrowed to its true domain — LayoutFile
  struct edits only — and the previous
  `setUnsavedInMemoryNodeIds` mirror sink is removed. The facade is the
  single source of truth for the aggregate in-memory-change signal.
- `configSidebarPresenter.shouldShowConfigNotReadBadge` and
  `buildSidebarNodeEntries.isUnsavedNew`: callers pre-route through
  the facade's `isPersistableInLayout` / `isConfigRead`.
- `saveControlsPresenter.deriveSaveControlsViewState` and
  `changeTrackerStore`: continue to receive a single `layoutIsDirty` /
  `unsavedInMemoryNodeCount` pair; the source of those values is now the
  facade.

### Lifecycle owner

Introduce `layoutLifecycleOrchestrator` with two intent-named methods:

- `resetForNewLayout()` — close / discard / new-layout path. Calls
  `nodeRoster.clearLayoutScope()` (the R7 fix), clears
  `partialCaptureNodesStore`, `bowtieMetadataStore`, `offlineChangesStore`,
  `connectorSelectionsStore`, `configSidebarStore`, and resets
  `layoutStore`. Reprobes live nodes when connected.
- `resetForFreshLiveSession()` — disconnect / reconnect within the same
  layout. Clears live-only state and explicitly preserves placeholders
  via `nodeRoster.replaceLiveRoster([])`.

The two existing `resetLayoutStateForNoLayout` /
`resetFreshLiveSessionState` functions in `offlineLayoutOrchestrator.ts`
move into the lifecycle owner with their callback shapes collapsed —
since `nodeRoster.clearLayoutScope()` already bundles the
nodeInfo / nodeTree / configReadStatus fan-out, most callbacks are no
longer needed.

A test asserts that the orchestrator's reset list matches the facade's
declared input set; adding a new input to the facade without a
corresponding reset entry fails the test.

## Consequences

- **Single source of truth for persistability.** Save, the orange dot,
  the unsaved-count, and the unsaved-new badge all flow from one
  predicate. The R5 / R6 misfires become structurally impossible —
  a node without `configRead` is never `isPersistableInLayout`.
- **R7 is fixed by naming.** The wrong-default
  (`replaceLiveRoster([])`) is replaced by `clearLayoutScope()` in the
  named `resetForNewLayout()` method; callers cannot pick the wrong
  mechanism because the mechanism is no longer at the call site.
- **`partialCaptureNodes` becomes a store.** The route writes to it via
  `partialCaptureNodesStore.replace(set)` instead of a local
  `$state<Set>`. One reactive source, one reset point.
- **`layoutStore.isDirty` is narrowed to struct-only.** The aggregate
  "any in-memory change" signal that the Save button and unsaved-changes
  guard need lives on `effectiveNodeStore.isDirty`; `layoutStore`
  reports only its own domain (the LayoutFile struct). The previous
  `setUnsavedInMemoryNodeIds` mirror — a back-channel that invited
  temporal-coupling bugs via `$effect` ordering — is removed.
- **Lifecycle reset enumeration is testable.** The orchestrator's
  reset list is a single, inspectable structure that the facade's input
  list is pinned against. New inputs to the facade fail loudly if their
  reset is missed.

## Considered alternatives

- **Extend `effectiveLayoutStore` in place.** Rejected: the existing
  store is value-shaped (`effectiveValue`, `slotsByRole`) and already
  near a coherent size. Splitting per-node projection into a sibling
  keeps each file focused; both surface through `$lib/layout`.
- **Backend-owned persistability.** Rejected for the same reason as
  ADR-0004: drafts and config-read status are inherently UI state, and
  the threshold composes them with `nodeTreeStore` / `layoutStore`
  state that already lives on the frontend.
- **Per-store reset registration without an orchestrator.** Rejected:
  decentralises the enumeration that the R7 regression argues should
  be in one place. The orchestrator is the one place a reviewer reads
  to know what "close this layout" means.

## Related

- ADR-0004: Layout facade (effective view store).
- ADR-0007: Full-capture threshold for promoting discovered nodes.
- specs/014-config-modes-placeholders/regression-fix-plan.md (Phase 9).

## 2026-05-31 extension: lifecycle ownership crosses the IPC boundary

### Context

The layout-close path was split across three modules: the route
(`clearActiveLayout`), `offlineLayoutOrchestrator.clearActiveLayoutWithReset`
(which owns the backend `closeLayout` IPC), and
`layoutLifecycleOrchestrator.resetForNewLayout` (which owns the frontend
store wipe). `createNewLayout` in `startupOrchestrator` bypassed the
close path entirely. Backend `close_layout` cleared the active-layout
context but left `node_registry` populated; `save_layout_directory` then
wrote every surviving registry handle (including stale placeholders)
into the new layout file when no `AddNode` deltas existed (the
"brand-new layout, no persisted file" exception). The placeholder
round-tripped back through `open_layout_directory` →
`hydrateOfflineSnapshots`.

### Decision

`layoutLifecycleOrchestrator` owns the **full** layout-close sequence,
both sides of the IPC boundary. Add `layoutLifecycleOrchestrator.closeLayout({ ... })`
that runs (in order) the backend `closeLayout('discard')` IPC, the
backend's registry-clear contract, and the frontend store wipe currently
in `resetForNewLayout`. Fold `clearActiveLayoutWithReset` into the
orchestrator (it has one caller). `createNewLayout` calls
`layoutLifecycleOrchestrator.closeLayout(...)` as its first step so
both sides are clean before `createNewLayoutCapture` /
`saveLayoutDirectory` / `openLayout` run.

Backend: `close_layout` in `layout_capture.rs` calls a new
`node_registry` method that drops placeholder proxies (and any other
layout-scoped registry state). Live discovered nodes either survive
or get re-registered by the next discovery probe — decide based on
whether bus connection state is preserved across the close. The
"brand-new layout, no persisted file → write every handle" exception
in `save_layout_directory` is removed; a new layout with no `AddNode`
deltas writes zero snapshots.

### Consequences

- One named entry point (`closeLayout`) for routes and orchestrators
  to call. No more multi-module assembly at the call site.
- `createNewLayout` cannot leak prior-layout state by construction.
- Backend `close_layout` becomes the single owner of "what does it
  mean for the backend to forget a layout." A test pins the registry
  state after close.
- The R7 regression (placeholder reappears in new layout) is
  structurally impossible: the registry is empty when the new layout
  is saved.
- The save-layout exception that bypassed `permitted_node_keys` for
  brand-new layouts is gone; tests that relied on it need to add
  explicit `AddNode` deltas.
