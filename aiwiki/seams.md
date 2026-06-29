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

## Railroad Channels Panel

- **Governing ADR(s)**: ADR-0013 (channel role / style / ownership / binding); ADR-0004 (`effectiveLayoutStore` derivations); ADR-0003 (`nodeName` adapter)
- **Owner**: `app/src/lib/components/Railroad/RailroadPanel.svelte` composes the Railroad-tab content — Facilities section (`<FacilitiesSection>`) above the hardware-organised Channels table (`<ChannelsPanel>` → `<ChannelRow>` per channel)
- **Contributors** (sources of the panel's rendered state):
  - [app/src/lib/stores/channels.svelte.ts](../app/src/lib/stores/channels.svelte.ts) — `channels` list and the `groupedByHardware` derivation that drives the per-(node, subsystem) group-header rows
  - [app/src/lib/stores/eventState.svelte.ts](../app/src/lib/stores/eventState.svelte.ts) — live occupancy state via `deriveChannelState` and the route-supplied `resolvedEventIds` map
  - [app/src/lib/utils/channelStyles.ts](../app/src/lib/utils/channelStyles.ts) — style-id → producer event-leaf mapping (used by the route when building `resolvedEventIds`)
  - [app/src/lib/stores/facilities.svelte.ts](../app/src/lib/stores/facilities.svelte.ts) — slot-binding state surfaced through `effectiveLayoutStore.channelUsageMap`; populates the "Used by" cell on each `ChannelRow` (Spec 018 / S4)
  - [app/src/lib/layout/effectiveLayoutStore.svelte.ts](../app/src/lib/layout/effectiveLayoutStore.svelte.ts) — `channelUsageMap` getter (single-merge derivation owner per ADR-0004); the route passes a `usedBy(channelId)` resolver down to `ChannelsPanel`
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte) — supplies `nodeName` (per ADR-0003), `daughterboardName(nodeKey, connector)`, and (S4) the `usedBy(channelId)` resolver
- **Consumers** (user-visible surfaces that depend on this panel):
  - The Railroad tab itself — the panel is the canonical hardware-verification surface (US2): a user with a TowerLCC + BOD-* selection can exercise hardware end-to-end without any facility
  - The "Used by" column went from always-em-dash (S3) to live facility-slot consumers (S4); S6 will additionally drive the Wired-status pill from bowtie presence
- **Per-slice plumbing rule**: New channel kinds (new `binding.kind` variants) MUST extend `channelsStore.groupedByHardware` (and the `hardwareGroupKey` helper) and `ChannelsPanel.svelte`'s `groupLabel` so they land in their own subsystem row without parallel rendering paths. New Consumers of the panel (a new column, a new badge) MUST source from `channelsStore` / `eventStateStore` / `effectiveLayoutStore` rather than re-deriving from raw stores. New "Used by" sources (anything beyond facility slots) MUST flow through `effectiveLayoutStore` so the resolver prop's signature stays stable.
- **Last-modified**: 2026-06-28 (S4 — `facilitiesStore` added as Contributor via `effectiveLayoutStore.channelUsageMap`; "Used by" column now active)
- **Last-audited**: 2026-06-28

### Notes

Replaces the Spec 015 card-grid layout (`ChannelGroup` / `ChannelCard`, retired in S3). The table layout is grouped by hardware locality so the panel reads as "what hardware is selected + what channels does it expose", which is the question US2 asks. The Used-by column intentionally rendered `—` in S3 because no facility-slot binding existed yet; S4 wired `facilitiesStore` through `effectiveLayoutStore.channelUsageMap` so the column now shows `{facilityName} / {slotLabel}` (semicolon-joined for multi-binding entries) the moment a slot is bound.

## Slot Binding

- **Governing ADR(s)**: ADR-0012 (draft layer — pending edits diffed to deltas); ADR-0013 (role-based binding); ADR-0004 (single-merge derivation owner)
- **Owner**: `app/src/lib/orchestration/facilityOrchestrator.ts` — `selectChannelForSlot({ facilityId, slotLabel, channelId, mode, previousChannelId? })` + `removeFromSlot({ facilityId, slotLabel, channelId })`. Validates `channel.role === slot.requiredRole` up-front; on `mode: 'rebind'` runs detach-then-attach atomically.
- **Contributors** (sources of the binding state):
  - [app/src/lib/stores/facilities.svelte.ts](../app/src/lib/stores/facilities.svelte.ts) — `attachChannel(facilityId, slotLabel, channelId)` / `detachChannel(...)`; pending slot bindings live in `_pendingSlotBindings: Map<facilityId, Map<slotLabel, string[]>>`; `collectDeltas()` diffs against baseline as a set and emits `attachChannelToSlot` / `detachChannelFromSlot` per change.
  - [bowties-core/src/layout/types.rs](../bowties-core/src/layout/types.rs) — `LayoutEditDelta::AttachChannelToSlot { facilityId, slotLabel, channelId }` + `LayoutEditDelta::DetachChannelFromSlot { ... }` (camelCase serde).
  - [bowties-core/src/layout/facilities.rs](../bowties-core/src/layout/facilities.rs) — `apply_facility_deltas(doc, deltas) -> Result<(), FacilityApplyError>` enforces per-slot `max_channels` (rejects attach above cap; idempotent on re-attach; idempotent detach when absent). Called by `app/src-tauri/src/commands/layout_capture.rs` alongside `apply_layout_deltas` in the save flow.
  - [bowties-core/src/behavior_templates.rs](../bowties-core/src/behavior_templates.rs) — `SlotDefinition { label, kind, requiredRole, minChannels, maxChannels }` is the source of cardinality + role data the orchestrator + apply use to validate.
  - [app/src/lib/stores/behaviorTemplates.svelte.ts](../app/src/lib/stores/behaviorTemplates.svelte.ts) — frontend read-only mirror; the orchestrator looks up `requiredRole` here for role-match validation.
- **Consumers** (user-visible surfaces driven by slot bindings):
  - [app/src/lib/components/Facilities/FacilitySlot.svelte](../app/src/lib/components/Facilities/FacilitySlot.svelte) — empty state shows "Select channel" button; filled state shows the bound channel name + live-state label + Rebind / Remove-from-slot actions (no Rename per D6 — rename lives in the Channels panel).
  - [app/src/lib/components/Facilities/SelectChannelPicker.svelte](../app/src/lib/components/Facilities/SelectChannelPicker.svelte) — modal picker; route supplies candidates from `effectiveLayoutStore.unboundChannelsForRole(role, { excludeIds })`.
  - [app/src/lib/components/Railroad/ChannelRow.svelte](../app/src/lib/components/Railroad/ChannelRow.svelte) — "Used by" cell reads through the `usedBy(channelId)` resolver (wired by the route from `effectiveLayoutStore.channelUsageMap`); see the **Railroad Channels Panel** seam.
  - [app/src/lib/layout/effectiveLayoutStore.svelte.ts](../app/src/lib/layout/effectiveLayoutStore.svelte.ts) — `channelUsageMap` getter + `unboundChannelsForRole` helper; the route's only read entry-point.
- **Per-slice plumbing rule**: All slot-binding mutations MUST go through `facilityOrchestrator.{selectChannelForSlot, removeFromSlot}`; component layers do not call `facilitiesStore.{attachChannel, detachChannel}` directly. The wire form is always `Vec<String>` bounded by template `max_channels` — never reintroduce `Option<String>` even if a slot is documented as max-1. New facility behaviors that need multi-channel bindings (ABS aspect-slot repeaters) MUST raise their slot's `max_channels` and update the picker filter; no Rename / re-label affordance ever lives on the slot (channel rename is a Channels-panel-only operation).
- **Last-modified**: 2026-06-28 (S4 — first slice with active slot bindings; UI is max-1 even though the wire form is plural)
- **Last-audited**: 2026-06-28

### Notes

Cardinality enforcement (D8 in S4) is a three-layer contract: the picker hides ineligible candidates up-front via `unboundChannelsForRole`; the orchestrator's role-match validation throws `RoleMismatchError` before any store write; the backend `apply_facility_deltas` is the last line of defence (rejects attach above `max_channels` even if a stale frontend delta sneaks through). Rebind is a derived workflow — no separate "rebind" delta, just a `DetachChannelFromSlot` + `AttachChannelToSlot` pair (D3 / D4). Save flow plumbing lives in `layout_capture.rs::save_layout_directory`, which calls `apply_facility_deltas` immediately before `apply_layout_deltas` against the same delta list (D7).

## Style Constraint Contract

- **Governing ADR(s)**: ADR-0013 (style owns the constraint contract; no transitional double-source)
- **Owner**: `collect_validity_rules` ([bowties-core/src/profile/mod.rs](../bowties-core/src/profile/mod.rs)) — single backend producer of the pre-baked `slot.supported_daughterboard_constraints[].validity_rules` projection that the frontend evaluator reads
- **Contributors** (sources of constraint rules):
  - [app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml](../app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml) — top-level `styles:` catalog (e.g. `bod-block-detector-input` for BOD-* daughterboards). Style rules without `lineOrdinals` inherit from `channelInputs[].inputs`.
  - Same file — `daughterboards[].validityRules` + `constraintVariants` for non-styled daughterboards (OI-IB-8, OI-OB-8). Fallback path; ignored as soon as a daughterboard's `channelInputs[].style` resolves against the styles catalog.
- **Consumers** (readers of the constraint projection):
  - [app/src/lib/utils/connectorConstraints.ts](../app/src/lib/utils/connectorConstraints.ts) `evaluateConnectorConstraintsForPath` — reads only the pre-baked projection
  - [app/src/lib/components/ElementCardDeck/SegmentView.svelte](../app/src/lib/components/ElementCardDeck/SegmentView.svelte) — applies constraint state to CDI field rendering
  - [app/src/lib/components/ElementCardDeck/TreeGroupAccordion.svelte](../app/src/lib/components/ElementCardDeck/TreeGroupAccordion.svelte), [TreeLeafRow.svelte](../app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte) — visibility / disable / pick-restrict behaviour per field
- **Per-slice plumbing rule**: New daughterboards whose channels share a style MUST declare the style in the `styles:` catalog (not as inline `validityRules`). Existing daughterboards may carry inline `validityRules` ONLY if they have no `channelInputs[].style` entry. Style additions land in the SAME change as the daughterboard's `channel_inputs[].style` reference — never as a separate transitional commit.
- **Last-modified**: 2026-06-28
- **Last-audited**: 2026-06-28

### Notes

Introduced in Spec 018 / S3 (ADR-0013). The migration was backend-only: the frontend evaluator and CDI field renderers were untouched because rules continue to land in the same `slot.supported_daughterboard_constraints[].validity_rules` projection. The "no double-source" rule is structurally enforced by `collect_validity_rules` returning style rules **instead of** daughterboard rules whenever any `channel_inputs[].style` resolves.

## Layout In-Memory Persistence

- **Governing ADR(s)**: ADR-0015 (single in-memory owner — `LayoutState`); ADR-0009 amendment (narrowed `NodeProxy` scope); ADR-0005 extension (layout module surface gained `LayoutState`)
- **Owner**: `bowties_core::layout::state::LayoutState` ([bowties-core/src/layout/state.rs](../bowties-core/src/layout/state.rs)) — owns the three-layer projection (`saved` mirrors disk, `captured` is fresh-from-bus not yet persisted, `drafts` mirrors frontend edits awaiting save). Held by `AppState::layout_state: Arc<RwLock<Option<LayoutState>>>` ([app/src-tauri/src/state.rs](../app/src-tauri/src/state.rs)).
- **Contributors** (the only legitimate writers of the open layout's persistent in-memory state):
  - [app/src-tauri/src/commands/layout_capture.rs](../app/src-tauri/src/commands/layout_capture.rs) `open_layout_directory` — builds `LayoutState::from_loaded(...)` from `read_capture`'s `LayoutDirectoryReadData` plus the per-node CDI-XML + profile-annotated-tree maps assembled in the same function.
  - [app/src-tauri/src/commands/layout_capture.rs](../app/src-tauri/src/commands/layout_capture.rs) `close_layout` — clears `layout_state` to `None`.
  - [app/src-tauri/src/commands/cdi.rs](../app/src-tauri/src/commands/cdi.rs) `download_cdi` — calls `LayoutState::record_captured(key, CapturedNode { cdi_xml: Some(...) })` after a successful CDI download.
  - [app/src-tauri/src/commands/cdi.rs](../app/src-tauri/src/commands/cdi.rs) `get_cdi_xml` (file-cache hit branch) — same call shape for CDI loaded from the file cache.
  - `node_registry.saved_trees` ([bowties-core/src/node_registry.rs](../bowties-core/src/node_registry.rs)) — NOT a Contributor of the persisted-data seam; it is a separate **load-once seeding cache** populated by the same loop in `open_layout_directory` that feeds `LayoutState`, consumed once when `get_or_create` spawns a fresh `LiveNodeProxy`. Listed here to document the *non*-contribution: any change that promotes `saved_trees` to a parallel source of truth for save is drift.
- **Consumers** (every reader of the open layout's persistent in-memory state):
  - [app/src-tauri/src/commands/layout_capture.rs](../app/src-tauri/src/commands/layout_capture.rs) `proxy_snapshot_data` — falls back to `LayoutState::cdi_xml(key)` when the proxy lacks in-memory CDI bytes. This is the structural cure for the R1/R2 silent-save regressions (ADR-0015).
  - [app/src-tauri/src/commands/cdi.rs](../app/src-tauri/src/commands/cdi.rs) `get_cdi_xml` — consults `LayoutState.cdi_xml(key)` before any disk-based fallback.
  - [app/src-tauri/src/commands/bowties.rs](../app/src-tauri/src/commands/bowties.rs) `build_bowtie_catalog_command` — when `node_registry.get_all_snapshots()` is empty (offline mode), derives per-node config values, profile group roles, and the synthetic `DiscoveredNode` list for slot walking directly from `LayoutState.saved` / `LayoutState.cdi_xml`. Replaces the deleted `AppState::offline_bowtie_data` cache.
- **Per-slice plumbing rule**: Any new in-memory cache of an open layout's persistent CDI bytes, profile-annotated trees, or saved bowtie / channels / facilities / offline-changes documents MUST live inside `LayoutState` (extending its surface) or document why it is a per-actor working buffer (per ADR-0009 amendment) — never as a parallel `Arc<RwLock<...>>` on `AppState`. Any new "save-flow needs to know X about a node" requirement reads X through `LayoutState`, not the proxy. Any new CDI-arrival seam (file-cache, bus download, future cloud restore) records into `LayoutState` via `record_captured` in the same change. `node_registry.saved_trees` does not grow new readers outside `get_or_create` — if a new "I need the saved tree before any read" requirement appears, the right answer is `LayoutState::config_tree(key)`.
- **Last-modified**: 2026-06-28 (introduced — ADR-0015 slice 3a)
- **Last-audited**: 2026-06-28

### Notes

**Regression history (R1 / R2, 2026-06-28)**:

- **R1**: Open a saved layout (5 nodes) → connect → edit one field → Save deleted 4 of 5 nodes from disk. **R2**: Tower-LCC silently dropped on every save. Both rooted in the same architectural gap — no single owner for the open layout's persistent in-memory state, so the save flow walked per-node proxies whose `cdi_data` was naturally `None` after every reconnect.
- The R1/R2 behaviour pins live in [bowties-core/src/layout/state.rs](../bowties-core/src/layout/state.rs) (`r1_every_persisted_node_resolves_cdi_xml_after_open`, `r2_captured_cdi_resolves_for_unsaved_node`) and [bowties-core/src/layout/capture.rs](../bowties-core/src/layout/capture.rs) (fingerprint contracts: `cdi_xml_len: Some(N)` ⇒ `"len:N"`, never `"missing"`). These structurally prevent the regression class regardless of which Contributor or Consumer is added later.

**Captured-vs-saved precedence**: `LayoutState::cdi_xml(key)` and `LayoutState::config_tree(key)` always prefer the `captured` layer over the `saved` layer. The slice-1 unit tests in `layout/state.rs` pin this; it is what makes a freshly-downloaded CDI visible to the next save without an intervening persistence step.

**What `SynthesizedNodeProxy` keeps**: placeholder CDI lives in the proxy struct (`cdi_data` / `cdi_parsed` fields), not in `LayoutState`, because an unsaved placeholder has no `LayoutState` entry. This asymmetry is principled per ADR-0009's 2026-06-28 amendment — synthesized proxies are factory-produced passive holders whose fields *are* the truth.

**The deferred 3b consideration**: `LiveNodeProxy::config_tree`, `config_values`, `snip`, `pip_flags` are intentionally NOT in this seam — they are per-actor working buffers for in-progress bus operations, not duplicates of persistent state. The principle test for inclusion in this seam is "could the save flow read this out-of-sync with disk?" — these fields fail that test (the save flow's source is `LayoutState`, not the proxy).
