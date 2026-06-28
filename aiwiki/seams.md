# aiwiki/seams.md

Cross-cutting contracts where divergence between participants is the failure mode. Referenced by the `/design`, `/slices`, `/build`, `architecture-first-fix`, and `/feature-finish` skills.

## What counts as a seam

A seam is a contract with:

- **One Owner** — the single module / function / store that owns the contract.
- **≥2 Contributors OR ≥2 Consumers** — multiple participants on one side or the other.
- **Divergence is the failure mode** — if a Contributor isn't registered, or a Consumer re-derives locally, the contract silently breaks and the symptom surfaces far from the cause.

If a contract has only one Contributor and one Consumer, it isn't a seam — it's just an interface. Revisit if a second participant ever appears.

## Per-entry schema

```markdown
## <Seam name>

- **Governing ADR(s)**: ADR-NNNN [, ADR-NNNN]
- **Owner**: <file + symbol + line(s)>
- **Contributors**:
  - <store / module> — <what it contributes> (`path:line` if useful)
- **Consumers**:
  - <reader> (`path:line`) — <what it reads>
- **Per-slice plumbing rule**: <the explicit rule for new work touching this seam>
- **Last-modified**: YYYY-MM-DD  <!-- bumps on any edit to this entry -->
- **Last-audited**: YYYY-MM-DD   <!-- bumps only on a full Owner/Contributors/Consumers re-grep -->

### Notes (optional)
<gotchas, regression history, common bypass patterns>
```

`Last-modified` is cheap and automatic. `Last-audited` is the staleness key — only bump it after grepping current code end-to-end for every Contributor and Consumer. Entries whose `Last-audited` is > 60 days warrant a re-audit.

## Maintenance touchpoints

This file is kept current through normal work, not graduation audits:

- **`copilot-instructions.md` Post-Work Enrichment** — update an entry if you added a Contributor or Consumer.
- **`/build` post-implementation enrichment** — update Contributor / Consumer lists for any seam the slice touched.
- **`architecture-first-fix` post-implementation** — update when the chosen option restores or extends a seam's contract; propose a new entry if the bug exposed an undocumented seam.
- **`/feature-finish` audit** — safety-net pass against the feature diff.
- **Enrichment-gate hook** — blocks turn completion when production code touches a seam-listed Owner or Contributor without a `seams.md` update (or explicit `[seams-unchanged]` override tag).

## Entries

---

## Dirty Aggregation

- **Governing ADR(s)**: ADR-0011, ADR-0004
- **Owner**: `effectiveNodeStore` ([app/src/lib/layout/effectiveNodeStore.svelte.ts](../app/src/lib/layout/effectiveNodeStore.svelte.ts))
  - `isDirty` getter (lines 217–229)
  - `dirtyBreakdown` getter (lines 187–212) — typed snapshot with per-bucket counts
- **Contributors** (each feeds one bucket into `dirtyBreakdown`):
  - `configChangesStore` ([app/src/lib/stores/configChanges.svelte.ts](../app/src/lib/stores/configChanges.svelte.ts)) — `config` bucket
  - `bowtieMetadataStore` ([app/src/lib/stores/bowtieMetadata.svelte.ts](../app/src/lib/stores/bowtieMetadata.svelte.ts)) — `metadata` bucket
  - `channelsStore` ([app/src/lib/stores/channels.svelte.ts](../app/src/lib/stores/channels.svelte.ts)) — `channels` bucket
  - `facilitiesStore` ([app/src/lib/stores/facilities.svelte.ts](../app/src/lib/stores/facilities.svelte.ts)) — `facilities` bucket
  - `connectorSelectionsStore` ([app/src/lib/stores/connectorSelections.svelte.ts](../app/src/lib/stores/connectorSelections.svelte.ts)) — `connectorSelections` bucket
  - `offlineChangesStore` ([app/src/lib/stores/offlineChanges.svelte.ts](../app/src/lib/stores/offlineChanges.svelte.ts)) — `offlineDrafts` + `offlineRevertedPersisted` buckets
  - `layoutStore` ([app/src/lib/stores/layout.svelte.ts](../app/src/lib/stores/layout.svelte.ts)) — `layoutStruct` bucket
  - Per-node in-memory state — `unsavedNewNodes` + `unsavedRemovedNodes` buckets
- **Consumers**:
  - [app/src/lib/components/ElementCardDeck/SaveControls.svelte](../app/src/lib/components/ElementCardDeck/SaveControls.svelte#L74) — toolbar derivation via `dirtyBreakdown` snapshot
  - [app/src/lib/components/UnsavedChangesDialog.svelte](../app/src/lib/components/UnsavedChangesDialog.svelte) — per-bucket breakdown rendering
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L164) — `promptUnsaved()` reads `isDirty`
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L196) — `hasInMemoryEdits` derivation
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L954) — window-close handler reads `isDirty` + `dirtyBreakdown`
- **Per-slice plumbing rule**: When adding a new edit-bearing store, extend `effectiveNodeStore.dirtyBreakdown` with a new bucket AND update the `isDirty` predicate in the same slice. The store must also register a reset callback in `layoutLifecycleOrchestrator.resetForNewLayout()` (Lifecycle Reset seam). Consumers read exclusively through the `$lib/layout` facade — never re-derive from raw stores.
- **Last-modified**: 2026-06-28
- **Last-audited**: 2026-06-28

### Notes

**Regression history (S1 → S1.2)**:

- Spec 018 / S1.1: `facilitiesStore` landed without being wired into `isDirty` / `dirtyBreakdown`. Facility-only edits silently bypassed the Save toolbar and close-prompt. Root cause: pipeline had no step that grepped for actual Consumers when adding a Contributor.
- Pre-S1.2: three diverging readers existed — `SaveControls.svelte` re-derived counts locally via an enumerated param list; `+page.svelte` used a `hasUnsavedPromptChanges()` guard that mixed `isDirty` reads with per-node iteration; `changeTrackerStore` was a parallel unadopted owner.
- S1.2 fixes: `dirtyBreakdown` became a typed snapshot (adding a bucket requires explicit edits to the interface AND the getter); all Consumers route through the facade; `changeTrackerStore`, `unsavedChangesGuard`, and the enumerated-param `deriveSaveControlsViewState` were deleted.

**Test coverage**: `app/src/lib/layout/dirtyAggregate.integration.test.ts` asserts each bucket independently.

---

## Lifecycle Reset

- **Governing ADR(s)**: ADR-0011
- **Owner**: `layoutLifecycleOrchestrator.resetForNewLayout()` ([app/src/lib/orchestration/layoutLifecycleOrchestrator.ts](../app/src/lib/orchestration/layoutLifecycleOrchestrator.ts#L80)) — lines 80–109
- **Contributors** (every layout-scoped store reset in sequence):
  - `partialCaptureNodesStore.clear()` (line 99)
  - `nodeRosterStore.clearLayoutScope()` (line 100)
  - `clearConfigReadStatus()` (line 101)
  - `bowtieMetadataStore.clearAll()` (line 102)
  - `offlineChangesStore.clear()` (line 103)
  - `configChangesStore.clearAllDrafts()` (line 104)
  - `layoutStore.reset()` (line 105)
  - `connectorSelectionsStore.reset()` (line 106)
  - `channelsStore.reset()` (line 107)
  - `facilitiesStore.reset()` (line 108)
- **Consumers** (callers of `resetForNewLayout()`):
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L725) — layout close / new / recovery
  - [app/src/lib/orchestration/layoutLifecycleOrchestrator.ts](../app/src/lib/orchestration/layoutLifecycleOrchestrator.ts#L159) — `closeLayout()` lifecycle chain
- **Per-slice plumbing rule**: When adding a new layout-scoped store, register its reset call inside `resetForNewLayout()` in the same slice that introduces the store. Add an assertion to [app/src/lib/orchestration/layoutLifecycleOrchestrator.test.ts](../app/src/lib/orchestration/layoutLifecycleOrchestrator.test.ts#L104) covering the new store. A store not registered here will silently bleed state into the next layout.
- **Last-modified**: 2026-06-28
- **Last-audited**: 2026-06-28

### Notes

Test file `layoutLifecycleOrchestrator.test.ts` (line 104, `describe('resetForNewLayout')`) is the canonical regression suite — every Contributor has an explicit assertion. New stores must extend this suite.

---

## Save-Flow Delta Collection

- **Governing ADR(s)**: ADR-0002, ADR-0012
- **Owner**: `saveLayoutOrchestrator` ([app/src/lib/orchestration/saveLayoutOrchestrator.ts](../app/src/lib/orchestration/saveLayoutOrchestrator.ts)) — accepts `deltas: LayoutEditDelta[]` (line 42) and forwards to the backend
- **Contributors** (every store that exposes `collectDeltas()`):
  - `bowtieMetadataStore.collectDeltas()` — bowtie-metadata deltas
  - `connectorSelectionsStore.collectDeltas()` — mode-selection deltas
  - `facilitiesStore.collectDeltas()` ([app/src/lib/stores/facilities.svelte.ts](../app/src/lib/stores/facilities.svelte.ts#L86)) — facility deltas
- **Consumers**:
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L608) — aggregates contributor `collectDeltas()` calls (lines 608–610) into the orchestrator payload
  - Backend `save_layout_with_bus_writes` / `save_layout_directory` IPC commands — consume the aggregated delta array
- **Per-slice plumbing rule**: When adding a new edit-bearing store that must persist via the save flow, implement `collectDeltas(): LayoutEditDelta[]` on the store and add it to the aggregation in `+page.svelte` (lines ~608–610) in the same slice. A store missing `collectDeltas()` (or not added to the aggregation site) bypasses the save workflow entirely — its edits are lost on save.
- **Last-modified**: 2026-06-28
- **Last-audited**: 2026-06-28

### Notes

Pre-ADR-0012 the save flow used write-through against individual store APIs, which was the root cause of partial saves when new stores landed. The current contract (single delta array, one orchestrator entry point) is brittle in exactly one place: the `+page.svelte` aggregation site. If that site is restructured (e.g., extracted into a dedicated save coordinator), update this entry's Consumer list.

---

## Connector Selection Hydration

- **Governing ADR(s)**: ADR-0012
- **Owner**: `connectorSelectionsStore.hydrateFromLayout(layout: LayoutFile)` ([app/src/lib/stores/connectorSelections.svelte.ts](../app/src/lib/stores/connectorSelections.svelte.ts))
- **Contributors** (every layout-open / recovery path that triggers hydration):
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L499) — primary layout open
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L556) — offline save recovery
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L910) — additional recovery path
- **Consumers** (readers of the hydrated selection state):
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L1346) — reads `selectedConnectorProfile` derived value
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte#L1368) — calls `loadNode(selectedNodeId, selectedConnectorProfile)`
  - [app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte](../app/src/lib/components/ConfigSidebar/ConfigSidebar.svelte) — slot descriptor surfaces depend on selection
  - [app/src/lib/components/ElementCardDeck/SaveControls.svelte](../app/src/lib/components/ElementCardDeck/SaveControls.svelte#L74) — save recovery re-hydration
- **Per-slice plumbing rule**: Any new layout-open or recovery path must call `connectorSelectionsStore.hydrateFromLayout(layout)` before any Consumer reads the selection. New Consumers must read through the store (not re-derive from `layout.connectorSelections`). New persistence formats must round-trip through `hydrateFromLayout` to confirm the contract holds.
- **Last-modified**: 2026-06-28
- **Last-audited**: 2026-06-28

### Notes

Pre-Spec 014 / S6 the selections were not restored on layout open, leaving slot descriptors blank. The fix made `hydrateFromLayout` the single restore path; this entry exists to prevent regression when new recovery paths are added (e.g., crash recovery, version migration).
