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
- **Owner**: `layoutLifecycleOrchestrator` ([app/src/lib/orchestration/layoutLifecycleOrchestrator.ts](../app/src/lib/orchestration/layoutLifecycleOrchestrator.ts)) — dispatches via the `LayoutScopedParticipant` interface
- **Dispatch mechanism**: Registry-driven. Every layout-scoped store or orchestrator implements `LayoutScopedParticipant` (optional `resetForNewLayout()` and `resetForFreshLiveSession()` methods) and is registered in the exported `layoutScopedParticipants` array. The orchestrator loops over the array for each lifecycle event.
- **Participants** (registered in `layoutScopedParticipants`):
  - `partialCaptureNodesStore` (resetForNewLayout)
  - `nodeRoster` (resetForNewLayout → clearLayoutScope, resetForFreshLiveSession → replaceLiveRoster)
  - `bowtieMetadataStore` (resetForNewLayout → clearAll)
  - `offlineChangesStore` (resetForNewLayout → clear)
  - `configChangesStore` (resetForNewLayout → clearAllDrafts)
  - `layoutStore` (resetForNewLayout → reset)
  - `connectorSelectionsStore` (resetForNewLayout, resetForFreshLiveSession → reset)
  - `channelsStore` (resetForNewLayout → reset)
  - `facilitiesStore` (resetForNewLayout → reset)
  - `facilityCascadeOrchestrator` (resetForNewLayout → stopCascade)
  - `configDraftMirrorOrchestrator` (resetForNewLayout → stopMirror)
  - `configSidebarStore` (resetForNewLayout, resetForFreshLiveSession → reset)
  - `nodeTreeStore` (resetForNewLayout, resetForFreshLiveSession → reset)
  - `bowtieCatalogStore` (resetForNewLayout → reset)
  - `saveProgressStore` (resetForNewLayout → reset)
  - `syncPanelStore` (resetForNewLayout → reset)
  - `cdiCacheStore` (resetForNewLayout → reset)
  - `connectorSlotFocusStore` (resetForNewLayout → reset)
  - `eventStateStore` (resetForNewLayout, resetForFreshLiveSession → clear)
  - `configReadStatusParticipant` (resetForNewLayout, resetForFreshLiveSession → clearConfigReadStatus)
- **Consumers** (callers of `resetForNewLayout()` / `resetForFreshLiveSession()`):
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte) — layout close / new / recovery
  - [app/src/lib/orchestration/layoutLifecycleOrchestrator.ts](../app/src/lib/orchestration/layoutLifecycleOrchestrator.ts) — `closeLayout()` lifecycle chain
- **Per-slice plumbing rule**: When adding a new layout-scoped store, implement `LayoutScopedParticipant` on the store class and append it to the `layoutScopedParticipants` array. The dispatch loop handles lifecycle events and save-delta collection automatically — you cannot forget one. If the store is edit-bearing, also implement `collectDeltas()` on the same interface. Add the store to this seam entry in the same slice.
- **Last-modified**: 2026-07-03
- **Last-audited**: 2026-07-03

### Notes

Test file `layoutLifecycleOrchestrator.test.ts` (`describe('registry-based dispatch')`) is the canonical regression suite — it spies on every registered participant and asserts each is called. Existing per-store behavioral tests (e.g., "clears placeholders", "resets channelsStore") verify specific effects.

Prior to 2026-07-03, dispatch was manual enumeration (each store called individually in sequence). This caused `bowtieCatalogStore` and 5 other stores to be missed, allowing stale state to bleed across layout close/open.

---

## Save-Flow Delta Collection

- **Governing ADR(s)**: ADR-0002, ADR-0012
- **Owner**: `saveLayoutOrchestrator` ([app/src/lib/orchestration/saveLayoutOrchestrator.ts](../app/src/lib/orchestration/saveLayoutOrchestrator.ts)) — accepts `deltas: LayoutEditDelta[]` and forwards to the backend
- **Dispatch mechanism**: Registry-driven via the same `layoutScopedParticipants` array used by the Lifecycle Reset seam. Every store that produces edit deltas implements `collectDeltas(): LayoutEditDelta[]` on the `LayoutScopedParticipant` interface. `collectAllSaveDeltas()` ([app/src/lib/layout/collectSaveDeltas.ts](../app/src/lib/layout/collectSaveDeltas.ts)) loops over the registry.
- **Contributors** (stores implementing `collectDeltas()` on the interface):
  - `bowtieMetadataStore` — bowtie-metadata deltas
  - `connectorSelectionsStore` — mode-selection deltas
  - `facilitiesStore` — facility deltas
  - `channelsStore` — channel deltas
- **Consumers**:
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte) — calls `collectAllSaveDeltas()` and passes result to the orchestrator payload
  - Backend `save_layout_with_bus_writes` / `save_layout_directory` IPC commands — consume the aggregated delta array
- **Per-slice plumbing rule**: When adding a new edit-bearing store that must persist via the save flow, implement `collectDeltas(): LayoutEditDelta[]` on the `LayoutScopedParticipant` interface. The store is already registered in `layoutScopedParticipants` (required by the Lifecycle Reset seam), so the dispatch loop picks it up automatically.
- **Last-modified**: 2026-07-03
- **Last-audited**: 2026-07-03

### Notes

Pre-2026-07-03 the aggregation was a manual enumeration of 4 store imports in `collectSaveDeltas.ts`. Adding a store required remembering to add it to both the aggregation function AND the `+page.svelte` call site. The registry-driven approach unifies with the lifecycle registry so a single registration serves both resets and save-delta collection.

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
  - The "Used by" column went from always-em-dash (S3) to live facility-slot consumers (S4); the Wired-status pill on `FacilityCard` now consumes `effectiveLayoutStore.facilityStatus(facilityId)` (S6 D5), which is derived from the same `channelUsageMap` this panel surfaces
  - Composed bowtie cards on the Bowties catalog panel are an additional Consumer surface of S6 wiring — `bowtieMetadataStore.bowtiesForFacility(facilityId)` populates them via the composition seam described in the **Facility Bowtie Lifecycle** entry below
- **Per-slice plumbing rule**: New channel kinds (new `binding.kind` variants) MUST extend `channelsStore.groupedByHardware` (and the `hardwareGroupKey` helper) and `ChannelsPanel.svelte`'s `groupLabel` so they land in their own subsystem row without parallel rendering paths. New Consumers of the panel (a new column, a new badge) MUST source from `channelsStore` / `eventStateStore` / `effectiveLayoutStore` rather than re-deriving from raw stores. New "Used by" sources (anything beyond facility slots) MUST flow through `effectiveLayoutStore` so the resolver prop's signature stays stable.
- **Last-modified**: 2026-07-01 (S6 — Wired-status pill now reads through `effectiveLayoutStore.facilityStatus`; bowtie catalog cards surface composed bowties driven by the Facility Bowtie Lifecycle seam)
- **Last-audited**: 2026-06-28

### Notes

Replaces the Spec 015 card-grid layout (`ChannelGroup` / `ChannelCard`, retired in S3). The table layout is grouped by hardware locality so the panel reads as "what hardware is selected + what channels does it expose", which is the question US2 asks. The Used-by column intentionally rendered `—` in S3 because no facility-slot binding existed yet; S4 wired `facilitiesStore` through `effectiveLayoutStore.channelUsageMap` so the column now shows `{facilityName} / {slotLabel}` (semicolon-joined for multi-binding entries) the moment a slot is bound.

## Slot Binding

- **Governing ADR(s)**: ADR-0012 (draft layer — pending edits diffed to deltas); ADR-0013 (role-based binding); ADR-0004 (single-merge derivation owner)
- **Owner**: `app/src/lib/orchestration/facilityOrchestrator.ts` — `selectChannelForSlot({ facilityId, slotLabel, channelId })` (attach-only post-S6) + `removeFromSlot({ facilityId, slotLabel, channelId })` + `addChannelForSlot({...})`. Validates `channel.role === slot.requiredRole` up-front and rejects attaches that would exceed `slot.maxChannels` via `SlotAtMaxError` — the cardinality guard the store's docstring always advertised but the pre-S6 code left unenforced. Rebind was retired in S6 (2026-07-01) per D4: changing a slot's channel is now a two-step Remove-from-slot + Select/Add sequence, and there is no rebind-specific delta or mode flag anywhere in the seam.
- **Contributors** (sources of the binding state):
  - [app/src/lib/stores/facilities.svelte.ts](../app/src/lib/stores/facilities.svelte.ts) — `attachChannel(facilityId, slotLabel, channelId)` / `detachChannel(...)`; pending slot bindings live in `_pendingSlotBindings: Map<facilityId, Map<slotLabel, string[]>>`; `collectDeltas()` diffs against baseline as a set and emits `attachChannelToSlot` / `detachChannelFromSlot` per change.
  - [bowties-core/src/layout/types.rs](../bowties-core/src/layout/types.rs) — `LayoutEditDelta::AttachChannelToSlot { facilityId, slotLabel, channelId }` + `LayoutEditDelta::DetachChannelFromSlot { ... }` (camelCase serde).
  - [bowties-core/src/layout/facilities.rs](../bowties-core/src/layout/facilities.rs) — `apply_facility_deltas(doc, deltas) -> Result<(), FacilityApplyError>` enforces per-slot `max_channels` (rejects attach above cap; idempotent on re-attach; idempotent detach when absent). Called by `app/src-tauri/src/commands/layout_capture.rs` alongside `apply_layout_deltas` in the save flow.
  - [bowties-core/src/behavior_templates.rs](../bowties-core/src/behavior_templates.rs) — `SlotDefinition { label, kind, requiredRole, minChannels, maxChannels }` is the source of cardinality + role data the orchestrator + apply use to validate.
  - [app/src/lib/stores/behaviorTemplates.svelte.ts](../app/src/lib/stores/behaviorTemplates.svelte.ts) — frontend read-only mirror; the orchestrator looks up `requiredRole` + `maxChannels` here for role-match + cardinality validation.
- **Consumers** (user-visible surfaces driven by slot bindings):
  - [app/src/lib/components/Facilities/FacilitySlot.svelte](../app/src/lib/components/Facilities/FacilitySlot.svelte) — empty state shows role-branched "Select channel" / "Add channel" button; filled state shows the bound channel name + live-state label + a Remove-from-slot action (no Rename per S4 D6; no Rebind per S6 D4 — rename lives in the Channels panel; swap = Remove + Select/Add).
  - [app/src/lib/components/Facilities/SelectChannelPicker.svelte](../app/src/lib/components/Facilities/SelectChannelPicker.svelte) — modal picker; route supplies candidates from `effectiveLayoutStore.unboundChannelsForRole(role)`. Confirm enables the moment any candidate is selected; no rebind-mode / current-channel pre-select branch.
  - [app/src/lib/components/Railroad/ChannelRow.svelte](../app/src/lib/components/Railroad/ChannelRow.svelte) — "Used by" cell reads through the `usedBy(channelId)` resolver (wired by the route from `effectiveLayoutStore.channelUsageMap`); see the **Railroad Channels Panel** seam.
  - [app/src/lib/layout/effectiveLayoutStore.svelte.ts](../app/src/lib/layout/effectiveLayoutStore.svelte.ts) — `channelUsageMap` getter + `unboundChannelsForRole` helper; the route's only read entry-point.
- **Per-slice plumbing rule**: All slot-binding mutations MUST go through `facilityOrchestrator.{selectChannelForSlot, removeFromSlot, addChannelForSlot}`; component layers do not call `facilitiesStore.{attachChannel, detachChannel}` or `channelsStore.{createUserOwnedChannel, removeUserOwnedChannel}` directly. The wire form is always `Vec<String>` bounded by template `max_channels` — never reintroduce `Option<String>` even if a slot is documented as max-1. New facility behaviors that need multi-channel bindings (ABS aspect-slot repeaters) MUST raise their slot's `max_channels` and update the picker filter. No Rename / re-label affordance ever lives on the slot (channel rename is a Channels-panel-only operation); no atomic "rebind" action lives on the slot either (swap = Remove + Select/Add — S6 D4). Every attach path now hooks the Facility Bowtie Lifecycle seam via `composeBowtiesIfWired`; every detach path fires `tearDownFacilityBowties` **before** the detach mutation so teardown sees the still-Wired shape.
- **Last-modified**: 2026-07-01 (S6 — attach paths now trigger `composeBowtiesIfWired`; detach paths trigger `tearDownFacilityBowties` before the mutation; `deleteFacility` orchestrator wrapper composes teardown + facility delete atomically. Phase R Rebind retirement remains in force.)
- **Last-audited**: 2026-06-28

### Notes

Cardinality enforcement (D8 in S4) is a three-layer contract: the picker hides ineligible candidates up-front via `unboundChannelsForRole`; the orchestrator throws `RoleMismatchError` on a mismatched role and `SlotAtMaxError` on an over-cap attach before any store write; the backend `apply_facility_deltas` is the last line of defence (rejects attach above `max_channels` even if a stale frontend delta sneaks through). Rebind's retirement (S6 D4) tightened this contract — pre-S6 code side-stepped the orchestrator max-check by relying on Rebind's implicit detach-first, so the guard sat unwritten. Post-S6, the guard is the single mechanism keeping a saved-baseline slot from silently double-binding, so it MUST stay on both attach paths (`selectChannelForSlot` and `addChannelForSlot`). Save flow plumbing lives in `layout_capture.rs::save_layout_directory`, which calls `apply_facility_deltas` immediately before `apply_layout_deltas` against the same delta list (D7).

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
  - [app/src-tauri/src/commands/layout_capture.rs](../app/src-tauri/src/commands/layout_capture.rs) `save_layout_directory` — refreshes `LayoutState.saved` (facilities / channels / bowties / offline_changes) from the just-written documents and calls `clear_drafts()` inline. Without this the saved layer would drift from disk after every save. Introduced by the 2026-07-03 draft-layer activation (ADR-0015 extension). **Atomic-save fold (2026-07-03, ADR-0002 extension):** now applies channel deltas via `layout::channels::apply_channel_deltas` beside `apply_facility_deltas` before writing — the previous parallel post-save write path (`create_channels` / `rename_channel` / `delete_channels` IPCs) was removed. All layout edits — bowtie / connector / facility / channel — travel one delta list through this command.
  - [app/src-tauri/src/commands/layout_drafts.rs](../app/src-tauri/src/commands/layout_drafts.rs) `sync_layout_drafts` / `clear_layout_drafts` — the *frontend-drafts mirror* half of the seam. `sync_layout_drafts(deltas)` calls `LayoutState::sync_drafts` (clones saved facilities+channels, applies deltas via the same `apply_*_deltas` the save flow uses, stores in `drafts.pending_*`). `clear_layout_drafts` drops the drafts. Idempotent w.r.t. any delta set — callers ship the complete current set on every sync. Frontend caller is `facilityOrchestrator.composeBowtiesIfWired` / `tearDownFacilityBowties` and `SaveControls.handleConfirmDiscard`. Introduced by the 2026-07-03 draft-layer activation.
  - [app/src-tauri/src/commands/cdi.rs](../app/src-tauri/src/commands/cdi.rs) `download_cdi` — calls `LayoutState::record_captured(key, CapturedNode { cdi_xml: Some(...) })` after a successful CDI download.
  - [app/src-tauri/src/commands/cdi.rs](../app/src-tauri/src/commands/cdi.rs) `get_cdi_xml` (file-cache hit branch) — same call shape for CDI loaded from the file cache.
  - `node_registry.saved_trees` ([bowties-core/src/node_registry.rs](../bowties-core/src/node_registry.rs)) — NOT a Contributor of the persisted-data seam; it is a separate **load-once seeding cache** populated by the same loop in `open_layout_directory` that feeds `LayoutState`, consumed once when `get_or_create` spawns a fresh `LiveNodeProxy`. Listed here to document the *non*-contribution: any change that promotes `saved_trees` to a parallel source of truth for save is drift.
- **Consumers** (every reader of the open layout's persistent in-memory state):
  - [app/src-tauri/src/commands/layout_capture.rs](../app/src-tauri/src/commands/layout_capture.rs) `proxy_snapshot_data` — falls back to `LayoutState::cdi_xml(key)` when the proxy lacks in-memory CDI bytes. This is the structural cure for the R1/R2 silent-save regressions (ADR-0015).
  - [app/src-tauri/src/commands/cdi.rs](../app/src-tauri/src/commands/cdi.rs) `get_cdi_xml` — consults `LayoutState.cdi_xml(key)` before any disk-based fallback.
  - [app/src-tauri/src/commands/bowties.rs](../app/src-tauri/src/commands/bowties.rs) `build_bowtie_catalog_command` — when `node_registry.get_all_snapshots()` is empty (offline mode), derives per-node config values, profile group roles, and the synthetic `DiscoveredNode` list for slot walking directly from `LayoutState.saved` / `LayoutState.cdi_xml`. Replaces the deleted `AppState::offline_bowtie_data` cache.
  - [app/src-tauri/src/commands/facility_bowties.rs](../app/src-tauri/src/commands/facility_bowties.rs) `compose_facility_bowties` — reads facilities + channels through `LayoutState::effective_facilities()` / `effective_channels()` (drafts-over-saved view). Precedence rule matches `cdi_xml(key)` (captured-over-saved) — the drafts variant is served when present, saved is served otherwise. Introduced by the 2026-07-03 draft-layer activation.
- **Per-slice plumbing rule**: Any new in-memory cache of an open layout's persistent CDI bytes, profile-annotated trees, or saved bowtie / channels / facilities / offline-changes documents MUST live inside `LayoutState` (extending its surface) or document why it is a per-actor working buffer (per ADR-0009 amendment) — never as a parallel `Arc<RwLock<...>>` on `AppState`. Any new "save-flow needs to know X about a node" requirement reads X through `LayoutState`, not the proxy. Any new CDI-arrival seam (file-cache, bus download, future cloud restore) records into `LayoutState` via `record_captured` in the same change. Any new backend read that must observe frontend-side draft edits goes through `LayoutState::effective_*` (2026-07-03 extension). `node_registry.saved_trees` does not grow new readers outside `get_or_create` — if a new "I need the saved tree before any read" requirement appears, the right answer is `LayoutState::config_tree(key)`.
- **Last-modified**: 2026-07-03 (draft-layer activation — `DraftLayer` gains `pending_facilities` / `pending_channels`; `sync_drafts` / `clear_drafts` / `effective_facilities` / `effective_channels` added; `save_layout_directory` refreshes the saved layer + clears drafts inline; `facility_bowties.rs` reads through the effective view; ADR-0015 extension recorded — then re-modified same day for the atomic-save fold: `save_layout_directory` now applies `LayoutEditDelta::{CreateChannel, RenameChannel, DeleteChannel}` beside facility deltas; legacy `create_channels` / `rename_channel` / `delete_channels` IPCs removed; frontend collects through the single `collectAllSaveDeltas()` facade — ADR-0002 extension recorded — then re-modified same day again for the read-side referential-integrity guarantee: `read_capture` now normalizes `facilities.yaml` slot bindings against `channels.yaml` via `facilities::normalize_facility_channel_refs`, so `LayoutState.saved` is guaranteed schema-clean; repairs surface through `LayoutDirectoryReadData.load_warnings` → `OpenLayoutResult.load_warnings` → route toast — second ADR-0002 extension recorded)
- **Last-audited**: 2026-07-03

### Notes

**Regression history (R1 / R2, 2026-06-28)**:

- **R1**: Open a saved layout (5 nodes) → connect → edit one field → Save deleted 4 of 5 nodes from disk. **R2**: Tower-LCC silently dropped on every save. Both rooted in the same architectural gap — no single owner for the open layout's persistent in-memory state, so the save flow walked per-node proxies whose `cdi_data` was naturally `None` after every reconnect.
- The R1/R2 behaviour pins live in [bowties-core/src/layout/state.rs](../bowties-core/src/layout/state.rs) (`r1_every_persisted_node_resolves_cdi_xml_after_open`, `r2_captured_cdi_resolves_for_unsaved_node`) and [bowties-core/src/layout/capture.rs](../bowties-core/src/layout/capture.rs) (fingerprint contracts: `cdi_xml_len: Some(N)` ⇒ `"len:N"`, never `"missing"`). These structurally prevent the regression class regardless of which Contributor or Consumer is added later.

**Captured-vs-saved precedence**: `LayoutState::cdi_xml(key)` and `LayoutState::config_tree(key)` always prefer the `captured` layer over the `saved` layer. The slice-1 unit tests in `layout/state.rs` pin this; it is what makes a freshly-downloaded CDI visible to the next save without an intervening persistence step.

**What `SynthesizedNodeProxy` keeps**: placeholder CDI lives in the proxy struct (`cdi_data` / `cdi_parsed` fields), not in `LayoutState`, because an unsaved placeholder has no `LayoutState` entry. This asymmetry is principled per ADR-0009's 2026-06-28 amendment — synthesized proxies are factory-produced passive holders whose fields *are* the truth.

**The deferred 3b consideration**: `LiveNodeProxy::config_tree`, `config_values`, `snip`, `pip_flags` are intentionally NOT in this seam — they are per-actor working buffers for in-progress bus operations, not duplicates of persistent state. The principle test for inclusion in this seam is "could the save flow read this out-of-sync with disk?" — these fields fail that test (the save flow's source is `LayoutState`, not the proxy).

## User-Owned Channel Lifecycle


- **Governing ADR(s)**: ADR-0013 (channel role / style / ownership / binding); ADR-0012 (draft layer); Spec 018 / S5 D2 (atomic Add-channel transaction)
- **Owner**: [app/src/lib/orchestration/facilityOrchestrator.ts](../app/src/lib/orchestration/facilityOrchestrator.ts) — ddChannelForSlot({ facilityId, slotLabel, lampRowNodeKey, rowOrdinal, name? }) for create+claim+bind, 
emoveFromSlot({ facilityId, slotLabel, channelId }) for the inverse. Both treat the channel + facility-slot mutations as a single draft transaction; attach failure rolls back the just-created channel.
- **Contributors** (sources of user-owned channel state):
  - [app/src/lib/stores/channels.svelte.ts](../app/src/lib/stores/channels.svelte.ts) — _pendingCreationDeltas bucket (distinct from the legacy _pendingCreations BOD path); `createUserOwnedChannel({ role, style, binding, name })` builds the channel with `ownership: 'user-owned'` and a freshly-generated UUID id; `removeUserOwnedChannel(id)` drops the same-session draft outright, or routes a persisted user-owned channel through `deleteChannels` so a baseline deletion lands on the next save.
  - [app/src/lib/types/bowtie.ts](../app/src/lib/types/bowtie.ts) — `LayoutEditDelta` union gains `{ type: 'createUserOwnedChannel'; channel }` (camelCase serde matches the Rust counterpart).
  - [bowties-core/src/layout/types.rs](../bowties-core/src/layout/types.rs) — `LayoutEditDelta::CreateUserOwnedChannel { channel }` variant; the `apply_layout_deltas` catch-all branch treats it as a no-op because the mutation lives in `apply_channel_deltas`.
  - [bowties-core/src/layout/channels.rs](../bowties-core/src/layout/channels.rs) — `apply_channel_deltas(doc, deltas) -> Result<(), ChannelApplyError>`; rejects duplicate id (`DuplicateChannelId`); sibling of `apply_facility_deltas`. Called by the save flow ([app/src-tauri/src/commands/layout_capture.rs](../app/src-tauri/src/commands/layout_capture.rs)) BEFORE `apply_layout_deltas` consumes the Vec.
- **Consumers** (user-visible surfaces driven by user-owned channels):
  - [app/src/lib/components/Facilities/AddChannelPicker.svelte](../app/src/lib/components/Facilities/AddChannelPicker.svelte) — modal sub-picker; lists eligible lamp rows from `effectiveLayoutStore.eligibleLampRowsForStyle('single-led-direct-lamp')` (D1 — single-merge owner per ADR-0004).
  - [app/src/lib/components/Facilities/FacilitySlot.svelte](../app/src/lib/components/Facilities/FacilitySlot.svelte) — role-branched empty-state action: producer slots show **Select channel** (S4), consumer slots show **Add channel** (this slice).
  - [app/src/lib/components/Railroad/ChannelRow.svelte](../app/src/lib/components/Railroad/ChannelRow.svelte) — renders the resulting channel under its Direct Lamp Control group with the `USER` ownership badge + lit/unlit state-dot + the lamp-indicator info-icon tooltip (AC #5 discoverability mandate).
- **Per-slice plumbing rule**: User-owned channel creation MUST go through `facilityOrchestrator.addChannelForSlot` so the create + attach pair always lands atomically. Direct calls to `channelsStore.createUserOwnedChannel` from a component layer are an antipattern — the attach would never fire, leaving an orphan draft. Deletion MUST go through `facilityOrchestrator.removeFromSlot` for the same reason (it detects `ownership === 'user-owned'` and routes through `channelsStore.removeUserOwnedChannel` so the channel's lifecycle stays tied to its binding). S6 D3 adds a third mandatory consumer: the `facilityCascadeOrchestrator` deletes a facility-bound user-owned channel indirectly by staging the detach from its slot, which trips the S6 removeFromSlot hook.
- **Last-modified**: 2026-07-01 (S6 — user-owned channels now participate in the hardware-channel-cascade seam; loss of a producer BOD channel triggers `removeFromSlot` on the paired consumer slot via `facilityCascadeOrchestrator`, which deletes the user-owned lamp channel through the existing lifecycle.)
- **Last-audited**: 2026-06-29

### Notes

D2 chose the atomic `CreateUserOwnedChannel` delta over the legacy split-IPC path because the regression class "transient inconsistent backend state mid-save" only goes away when both halves travel in the same journal transaction. The legacy `createChannels` IPC remains untouched for S2's hardware-owned BOD auto-create path — a follow-up slice may collapse both onto the delta path, but that work is not in scope here (see `specs/backlog.md`). The D5 deferral (manual `Lamp Selection`) is an architecturally-clean punt: the user's manual edit is what makes the lamp follow the bowtie's commands; future style-driven auto-lock will reuse this seam unchanged.

## Channel Event-ID Resolution

- **Governing ADR(s)**: ADR-0013 (channel role / style / binding); Spec 018 / S5 D6 (shape-agnostic resolver)
- **Owner**: [bowties-core/src/channel_events.rs](../bowties-core/src/channel_events.rs) — `resolve_event_ids(tree, path_prefix, role, leaf_index_map) -> HashMap<String, String>` collects EventId leaves under `path_prefix` matching `role` and indexes them by the supplied map.
- **Contributors** (sources of the per-binding path prefix):
  - [bowties-core/src/channel_events.rs](../bowties-core/src/channel_events.rs) — `resolve_connector_input_path_prefix(tree, connector, input)` (producer-side, S2 slot lookup) + `resolve_lamp_row_path_prefix(tree, row_ordinal)` (consumer-side, walks `Direct Lamp Control/Lamp#N`).
  - [app/src-tauri/src/commands/channel_events.rs](../app/src-tauri/src/commands/channel_events.rs) — `resolve_channel_event_ids` IPC command; `ChannelResolutionRequest` carries `binding` (tag-discriminated union mirroring `ChannelBinding`) + `role` + `leaf_index_map`; dispatches per `binding.kind` and returns the resolved map per channel.
  - [app/src/lib/orchestration/eventStateOrchestrator.ts](../app/src/lib/orchestration/eventStateOrchestrator.ts) — frontend payload builder; picks `role: 'producer'` for `block-occupancy` channels and `role: 'consumer'` for `lamp-indicator` channels; pulls the leaf-index map from `getStyleEventMapping(channel.style)` (using `producerLeafIndex` / `consumerLeafIndex` per role).
  - [app/src/lib/utils/channelStyles.ts](../app/src/lib/utils/channelStyles.ts) — `STYLE_EVENT_MAPPINGS` is the single source of truth for "style → state-name → leaf ordinal". Each entry carries either `producerLeafIndex` OR `consumerLeafIndex` (mutually exclusive per state).
- **Consumers** (readers of the resolved map):
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte) — calls `resolveChannelEventIds(channels)` whenever channels exist + a node tree is available; result threads into `RailroadPanel` → `ChannelsPanel` → `ChannelRow` and into `FacilityCard` → `FacilitySlot` (filled state).
  - [app/src/lib/components/Railroad/ChannelsPanel.svelte](../app/src/lib/components/Railroad/ChannelsPanel.svelte) — per-channel `channelStates` derivation pairs the resolved map with `eventStateStore` PCERs via `deriveChannelState(events, positive, negative, role)` (role discriminator picks `occupied/clear` vs `lit/unlit`).
  - [app/src/lib/components/Facilities/FacilityCard.svelte](../app/src/lib/components/Facilities/FacilityCard.svelte) — same derivation for the filled-slot display.
- **Per-slice plumbing rule**: A new `binding.kind` MUST add a sibling `resolve_<kind>_path_prefix` in [bowties-core/src/channel_events.rs](../bowties-core/src/channel_events.rs) and extend the IPC command's dispatch — never inline a new prefix walker at the call site. A new role MUST update `ChannelResolutionRole` (Rust) + the orchestrator's role mapping + the `deriveChannelState` signature in lockstep so the resolver-shape invariant (one shape-agnostic core + per-shape adapters) survives.
- **Last-modified**: 2026-06-29 (S5 — extracted shape-agnostic core; added `lampRow` + `Consumer` as the second binding-shape + role)
- **Last-audited**: 2026-06-29

### Notes

S5 generalised what S2 had inlined for the connector-input + producer case. The shape-agnostic resolver's invariant: under a given `path_prefix`, collect leaves matching `role` (filtering out `EventRole::Ambiguous` and unmatched), then look each up in `leaf_index_map` by state name. The IPC command never returns a partial map — every channel that resolves at all returns its complete state-name → eventId mapping. Channels whose binding doesn't resolve (e.g. node tree unavailable) return an empty map so the consumer surface degrades to `unknown` rather than crashing.

## Replication Instance Traversal

- **Governing ADR(s)**: ADR-0013 (channel role / style / binding — anticipates multiple `binding.kind` values that address replicated CDI groups)
- **Owner**: [bowties-core/src/node_tree.rs](../bowties-core/src/node_tree.rs) `replication_instances(parent, name) -> Vec<&GroupNode>` + [app/src/lib/types/nodeTree.ts](../app/src/lib/types/nodeTree.ts) `replicationInstances(parent, name): GroupConfigNode[]` — the only sanctioned way to enumerate the instances of a CDI replicated group. Both encapsulate the wrapper invariant produced by `build_children`.
- **Contributors** (the wrapper invariant they encapsulate):
  - [bowties-core/src/node_tree.rs](../bowties-core/src/node_tree.rs) `build_children` — for any CDI `<group replication="N">` with stride > 0, emits a single wrapper `GroupNode { instance: 0, replicationOf: name, children: [instance1..N] }` directly under the parent. Instance groups are the wrapper's children, never the parent's. (See the `if effective_replication > 1` branch.)
  - Hand-built test fixtures sometimes emit instances as direct siblings of the parent (no wrapper). The helpers accept both shapes so fixtures don't have to mirror the build-children topology.
- **Consumers** (every reader that addresses replicated-group instances):
  - [bowties-core/src/channel_events.rs](../bowties-core/src/channel_events.rs) `resolve_lamp_row_path_prefix` — locates `Direct Lamp Control/Lamp#N` for the `lampRow` binding shape.
  - [app/src/lib/layout/effectiveLayoutStore.svelte.ts](../app/src/lib/layout/effectiveLayoutStore.svelte.ts) `eligibleLampRowsForStyle` — enumerates eligible lamp rows for the Add-channel picker; pairs each with its `getInstanceDisplayName` label (so the picker shows `"Up Main Block 5 (7)"` when the user has set Lamp Description).
  - [app/src/lib/types/nodeTree.ts](../app/src/lib/types/nodeTree.ts) `groupReplicatedChildren` — Config tab's renderer; predates the helper but encodes the same invariant (will be migrated to call `replicationInstances` if a third call site materialises).
- **Per-slice plumbing rule**: A new `binding.kind` that addresses a replicated CDI group (signal masts, aspect rows, etc. on the S6+ roadmap) MUST use `replication_instances` / `replicationInstances` and MUST NOT hand-roll wrapper detection at the call site. New test fixtures for replicated groups SHOULD use the real wrapper shape so the bug class fixed by the Spec 018 quickchange (sibling-only traversal silently returning the wrapper) cannot regress.
- **Last-modified**: 2026-06-30 (introduced — Spec 018 quickchange after Add-channel picker showed only 1 lamp for a 16-lamp Signal-LCC)
- **Last-audited**: 2026-06-30

### Notes

Before this seam existed the wrapper invariant was rediscovered (and partially encoded) in three independent places — `groupReplicatedChildren`, `resolvePillSelectionsForPath`, and the Spec 018 / S5 lamp-row enumeration. The latter two synced-up over time, but the S5 derivation and `resolve_lamp_row_path_prefix` both inspected only segment-level siblings; against the real `build_children` output that meant they each returned exactly one ordinal-1 hit (the wrapper itself). The shared helper makes the wrapper invariant a named, tested rule that future shapes inherit for free.


## Facility Bowtie Lifecycle

- **Governing ADR(s)**: ADR-0004 (ffectiveLayoutStore single-merge derivation owner); ADR-0011 (aggregate dirty signal); ADR-0012 (draft layer); ADR-0013 (channel role / style / binding); Spec 018 / S6 D1, D2, D5, D6
- **Owner**: [app/src/lib/orchestration/facilityOrchestrator.ts](../app/src/lib/orchestration/facilityOrchestrator.ts) `composeBowtiesIfWired(facilityId)` + `tearDownFacilityBowties(facilityId)` + `deleteFacility(facilityId)` wrapper. Compose is guarded by `effectiveLayoutStore.facilityStatus(facilityId) === 'Wired'`; teardown is guarded by `bowtieMetadataStore.bowtiesForFacility(facilityId)` (empty ⇒ no-op).
- **Contributors** (sources of the composition + teardown state):
  - [bowties-core/src/facility_bowties/mod.rs](../bowties-core/src/facility_bowties/mod.rs) `compose_bowtie_ops(facility, template, channels, producer_event_ids, per_node_cdi, consumer_leaf_index)` — deepest layer; owns the "two bowties per Block Indicator, event IDs adopted from the producer (D6), name derived from the state mapping" contract. Returns `Vec<CompositionOp>` where each op carries the consumer leaf path, consumer leaf space/address, producer event-id bytes, bowtie name, and `createdByFacility` back-reference.
  - [app/src-tauri/src/commands/facility_bowties.rs](../app/src-tauri/src/commands/facility_bowties.rs) `compose_facility_bowties(facility_id) -> Vec<CompositionOp>` IPC — reads facilities + channels through `LayoutState::effective_facilities()` / `effective_channels()` (drafts-over-saved view, 2026-07-03 bugfix) so freshly-added draft facilities/channels are visible; still consults `LayoutState` for the template registry and per-node CDI trees. The frontend orchestrator calls `syncLayoutDrafts` before every invocation so pending edits are mirrored into `LayoutState.drafts` first. Dispatches into `compose_bowtie_ops`.
  - [app/src/lib/api/facilityBowties.ts](../app/src/lib/api/facilityBowties.ts) `composeFacilityBowties(facilityId)` TS wrapper.
  - [app/src/lib/api/layout.ts](../app/src/lib/api/layout.ts) `syncLayoutDrafts(deltas)` / `clearLayoutDrafts()` TS wrappers — 2026-07-03 bugfix. The orchestrator calls `syncLayoutDrafts` before every compose IPC (both compose and teardown paths); Discard fires `clearLayoutDrafts`. See the **Layout In-Memory Persistence** seam for the backend half.
  - [app/src/lib/stores/bowtieMetadata.svelte.ts](../app/src/lib/stores/bowtieMetadata.svelte.ts) `createBowtie(hex, name, { createdByFacility })` propagates the back-reference through pending edits + `collectDeltas`'s `LayoutEditDelta::createBowtie` emitter; `bowtiesForFacility(facilityId)` returns the effective back-reference view (baseline + pending create — pending delete).
  - [bowties-core/src/layout/types.rs](../bowties-core/src/layout/types.rs) — `BowtieMetadata.created_by_facility: Option<String>` (D1 single back-reference site); `LayoutEditDelta::CreateBowtie { created_by_facility: Option<String> }` mirror; propagated by `apply_layout_deltas`.
  - [app/src/lib/stores/configChanges.svelte.ts](../app/src/lib/stores/configChanges.svelte.ts) — composition writes flow through this store via `configEditor.applyEdit` on the consumer leaves (D6 — never the producer's leaves).
  - [app/src/lib/layout/effectiveLayoutStore.svelte.ts](../app/src/lib/layout/effectiveLayoutStore.svelte.ts) `facilityStatus(facilityId)` (D5) — sole reader for the Wired / Incomplete pill; guards the compose invocation.
  - [bowties-core/src/layout/state.rs](../bowties-core/src/layout/state.rs) `LayoutState::record_discovered_roles()` / `discovered_roles()` / `clear_discovered_roles()` — protocol-discovered role classifications (from catalog rebuilds) flow through LayoutState instead of a stale catalog side-channel. The save flow merges `discovered_roles()` into `bowties.role_classifications` with `or_insert_with` (user-explicit classifications win). Cleared after save.
- **Consumers** (user-visible surfaces driven by facility bowtie lifecycle):
  - [app/src/lib/components/Facilities/FacilityCard.svelte](../app/src/lib/components/Facilities/FacilityCard.svelte) — status pill reads `effectiveLayoutStore.facilityStatus` (D5), no local `\` slot-fullness check.
  - The Bowties catalog panel surfaces the composed bowtie cards from `bowtieMetadataStore.getMetadata(hex)` — cards created by `composeBowtiesIfWired` appear in the draft layer immediately (revertable via Discard, persisted on Save).
  - The producer's occupancy events now drive the consumer's Lamp On / Lamp Off leaves through the bus without any Bowties-side mediation, once the composed edits reach the Signal-LCC node.
- **Per-slice plumbing rule**: Composition MUST NOT introduce any new save-flow ordering — every leaf write flows through `configEditor.applyEdit` (which the config-edit save path already picks up) and every bowtie registration flows through `bowtieMetadataStore.createBowtie` (whose `collectDeltas` already threads through the delta save path). Teardown reversal MUST go through the shared `resetComposedLeavesForFacility(facilityId)` primitive rather than open-coding leaf resets — that primitive owns the two-strategy lookup: composer-forward when the facility is still Wired (fast + precise, requires intact structure) and a metadata-driven `bowtieMetadataStore.bowtiesForFacility(facilityId)` + `nodeTreeStore.trees` scan when the facility is Incomplete at teardown time (works after ghost-binding repair or when a cascade detached first). New callers that un-Wire a facility (spec 019+ cascade sources, new lifecycle transitions) MUST invoke `tearDownFacilityBowties` and MUST NOT skip it to "avoid the search" — the search is the invariant that keeps composition side effects reversible from every state. New behavior templates that need composition MUST return their state-mapping through `BehaviorTemplate.mapping` so the same composer covers them (no per-template composition modules). **2026-07-03 addition:** every path that will call `composeFacilityBowties` MUST first call `syncLayoutDrafts(collectDeltas)` (via the orchestrator's `syncDraftsForComposition` helper) so the backend's effective view reflects frontend drafts. The Wired-guard cheaply skips both calls when the facility isn't Wired.
- **Last-modified**: 2026-07-04 (eliminated catalog side-channel merge from save flow — `merge_catalog_bowties_into` removed; protocol-discovered role classifications now flow through `LayoutState::record_discovered_roles()` at catalog rebuild time, read by save flow via `LayoutState::discovered_roles()` with user-explicit-wins precedence; bowtie metadata is now exclusively delta-backed, structurally preventing the deletion-resurrection bug class where stale catalog entries contradicted `DeleteBowtie` deltas)
- **Last-audited**: 2026-07-04

### Notes

D6 pins the write direction: the composed bowtie ADOPTS the producer channel's existing event IDs (never regenerates them) and writes them onto the consumer's Lamp On / Lamp Off leaves. This matches LCC's producer-identifies / consumer-subscribes contract and matches the `resolution.writeTo === 'consumer'` case that `BowtieCatalogPanel.handleNewConnection` already honours for manual bowtie creation. Teardown is the exact inverse: `generateFreshEventIdForNode` produces a new unique event ID and writes it onto the same consumer leaves, breaking the bus routing. Both compose and teardown live in the draft layer; the toolbar's Save / Discard controls apply / revert everything together via `effectiveNodeStore.dirtyBreakdown` (ADR-0011).

**2026-07-03 consolidation — `resetComposedLeavesForFacility` two-strategy lookup.** The original T13 design assumed teardown would always run against a still-Wired shape (called from `removeFromSlot` before detach), so it re-derived leaves via the backend composer and rejected the alternative "scan every node's CDI for leaves currently holding the bowtie's event id" on the grounds that the search duplicated the composer's knowledge and was slower. That assumption held for user-initiated detach but broke down for two other paths: (a) the runtime hardware-channel cascade in `_cascadeDetach`, which detaches first and then calls teardown against an Incomplete facility; and (b) the 2026-07-03 load-time repair for ghost bindings on disk (`reconcileDanglingChannelRefsOnLoad`), which cannot resurrect the ghost channel to make the composer succeed. Both paths hit the pre-2026-07-03 non-Wired teardown branch, which deleted metadata rows but never touched the composed consumer leaves; those leaves survived save+reopen and the backend's CDI-scan catalog re-produced the bowtie as an auto-discovered card, so the user saw the Bowties view change between saving and reopening the same layout. The consolidated primitive keeps the composer-forward path for still-Wired callers (unchanged behaviour, unchanged cost) and adds the metadata-driven fallback for Incomplete callers — the search is only slower in absolute terms and only runs when the facility structure isn't intact enough for the composer, which is exactly when the search is the only correct move. The runtime cascade's ordering asymmetry (`_cascadeDetach` detaches then tears down while `removeFromSlot` tears down then detaches) becomes moot: both callers converge on the same primitive and produce the same drafts.

## Hardware Channel Cascade

- **Governing ADR(s)**: ADR-0011 (aggregate dirty signal — no new bucket needed); ADR-0012 (draft layer — cascade side effects appear next to their trigger, atomic on Save, revertable on Discard); Spec 018 / S6 D3
- **Owner**: [app/src/lib/orchestration/facilityCascadeOrchestrator.svelte.ts](../app/src/lib/orchestration/facilityCascadeOrchestrator.svelte.ts) `startCascade()` / `stopCascade()` + `reconcileDanglingChannelRefsOnLoad()`. Owns a private last-seen channel-id `Set<string>` and an `\.root` subscription to `channelsStore.channels`; on every transition it diffs the id set and drives the cascade for disappearing ids. Shares its detach + teardown side-effect logic through the private `_cascadeDetach(lostIds)` helper so the load-time repair path reuses the same seam.
- **Contributors** (sources of the cascade trigger + effects):
  - [app/src/lib/stores/channels.svelte.ts](../app/src/lib/stores/channels.svelte.ts) — the `channels` list transition is the trigger. Both the S2 legacy `deleteChannels` path (BOD daughter-board clear) and the S5 `removeUserOwnedChannel` path emit id-set changes the cascade observes.
  - [app/src/lib/stores/facilities.svelte.ts](../app/src/lib/stores/facilities.svelte.ts) `detachChannel(facilityId, slotLabel, lostChannelId)` — staged for every slot binding that referenced a disappearing channel.
  - [app/src/lib/orchestration/facilityOrchestrator.ts](../app/src/lib/orchestration/facilityOrchestrator.ts) `tearDownFacilityBowties(facilityId)` — called for each facility whose `facilityStatus` transitions Wired → Incomplete as a result of the detach round.
  - [app/src/lib/layout/effectiveLayoutStore.svelte.ts](../app/src/lib/layout/effectiveLayoutStore.svelte.ts) `facilityStatus(facilityId)` — the derivation the cascade consults to decide whether teardown is needed.
  - [app/src/routes/+page.svelte](../app/src/routes/+page.svelte) — sequences `channelsStore.loadChannels()` + `facilitiesStore.loadFacilities()` with `Promise.all` on layout open, then invokes `reconcileDanglingChannelRefsOnLoad()` before `startCascade()`. This is the load-time entry point.
- **Consumers** (user-visible surfaces driven by the cascade):
  - [app/src/lib/components/Facilities/FacilityCard.svelte](../app/src/lib/components/Facilities/FacilityCard.svelte) — status pill flips Wired → Incomplete the moment the cascade lands.
  - Bowties catalog cards created by `composeBowtiesIfWired` disappear from the panel as `tearDownFacilityBowties` drops them.
  - The toolbar's Save / Discard controls surface the cascaded edits via `effectiveNodeStore.dirtyBreakdown` — the user sees "N facility edits, M bowtie edits, K config edits" appear together and can revert everything with a single Discard.
- **Per-slice plumbing rule**: Cascade side effects MUST be staged in the appropriate draft stores (`facilitiesStore`, `bowtieMetadataStore`, `configChangesStore`) so they are visible immediately, atomic on Save, and revertable on Discard. On-save "fixup" cascades that only appear at commit time are explicitly ruled out (ADR-0012 extension 2026-07-01). New cascade sources (future spec 019+ hardware seams) MUST plug into `facilityCascadeOrchestrator` via the same diff-based subscription pattern; do not create parallel Svelte effects that mutate the same draft stores. Load-time repairs of orphan slot bindings MUST also flow through the cascade orchestrator via `reconcileDanglingChannelRefsOnLoad()` so a dangling reference cleaned up on open ships as a normal `detachChannelFromSlot` draft — never a silent in-memory-only mutation. The cascade is mounted alongside `loadFacilities()` / `loadChannels()` in the layout-open lifecycle and torn down by `layoutLifecycleOrchestrator.resetForNewLayout()` — new lifecycle transitions MUST preserve both hooks.
- **Last-modified**: 2026-07-03 (extension — `reconcileDanglingChannelRefsOnLoad` shares `_cascadeDetach` with runtime `_reconcile`; layout open sequences `loadChannels` + `loadFacilities` with `Promise.all` then calls the load-time repair before `startCascade`)
- **Last-audited**: 2026-07-01

### Notes

D3 chose the frontend diff-based orchestrator over a backend save-flow fixup because the cascade must be visible NEXT to its trigger, not at commit time. If the user clears a BOD daughter-board on a Wired facility and the resulting detach + teardown only appeared on Save, the Discard button would silently un-Wire the facility while restoring the daughter-board — a surprising side effect. The frontend orchestrator makes the cascade a first-class part of the draft transaction: the same Discard that restores the daughter-board also restores the facility's Wired state and the composed bowtie cards. User-owned channels that disappear via `removeUserOwnedChannel` do NOT re-trigger the cascade meaningfully because their removal is already the outcome of a `removeFromSlot` call (which already ran teardown before deleting the channel) — the diff still sees the id disappear but there are no bindings to detach and no facility to tear down.

**2026-07-03 extension — load-time dangling-ref repair.** The pre-018 `list_facilities` IPC hydrates the frontend baseline from `facilities.yaml` directly and does NOT run `normalize_facility_channel_refs`. Backend readers that go through `LayoutState.effective_facilities()` (composer, catalog rebuild, sync) see the normalised view because `read_layout_capture` cleans the doc before publishing `LayoutState.saved`. That asymmetry surfaced as a three-part user-visible bug when a layout carried orphan bindings from the earlier split-write channel-save regression: the ghost id counted against the slot cap (blocking `addChannelForSlot`), the effective facility looked Wired against a phantom consumer (routing `deleteFacility` through the composer, which errored with "has no consumer channel"), and no dirty flag was set so the toast copy "Save to persist the cleanup" was a lie. `reconcileDanglingChannelRefsOnLoad` restores seam symmetry by staging a normal `detachChannelFromSlot` draft for every dangling binding at load time — the same seam the runtime cascade uses. Fixing the frontend baseline read path (routing `list_facilities` through `LayoutState.effective_facilities()`) remains as a separate backlog item; the load-time repair is orthogonal to that and stays useful even after the backend seam is symmetric, because it converts silent auto-repair into a user-visible, saveable, discardable draft edit.


## Config Draft Backend Mirror

- **Governing ADR(s)**: ADR-0012 (all layout edits flow through the draft layer; 2026-07-03 extension — connected-mode draft-to-backend mirror)
- **Owner**: [app/src/lib/orchestration/configDraftMirrorOrchestrator.svelte.ts](../app/src/lib/orchestration/configDraftMirrorOrchestrator.svelte.ts) `startMirror()` / `stopMirror()` + `reconcile(entries)`. Owns a private `Map<string, TreeConfigValue>` of last-seen drafts and an `$effect.root` subscription to `configChangesStore.draftEntries()`; on every reactive tick it diffs the current snapshot against last-seen and emits `setModifiedValue` for each new/changed draft. Connection state is checked inside the effect body (not as a reactive dependency), so the mirror does not re-emit the backlog on connect/disconnect.
- **Contributors** (draft producers whose writes MUST reach the backend `NodeProxy.modified_value` map through this seam):
  - [app/src/lib/stores/configEditor.svelte.ts](../app/src/lib/stores/configEditor.svelte.ts) `applyEdit(key, value)` — the sole synchronous entry point for user-initiated config edits. All draft producers below funnel through this single call, which writes to `configChangesStore` and returns without IPC.
  - [app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte](../app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte) — leaf-row edit commit (control-driven user edits).
  - [app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte](../app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte) — manual bowtie EventID adoption paths (`handleNewConnection`, `handleClearConnection`).
  - [app/src/lib/orchestration/facilityOrchestrator.ts](../app/src/lib/orchestration/facilityOrchestrator.ts) `composeBowtiesIfWired(facilityId)` — facility composition writes composed event ids onto consumer leaves via `configEditor.applyEdit`. Before the mirror existed, these edits were the primary source of the "Save didn't write" bug.
  - [app/src/lib/orchestration/facilityOrchestrator.ts](../app/src/lib/orchestration/facilityOrchestrator.ts) `resetComposedLeavesForFacility(facilityId)` — teardown resets composed leaves to a fresh event id via `configEditor.applyEdit`. Reached from both `tearDownFacilityBowties` (user detach) and the cascade paths.
  - [app/src/lib/orchestration/facilityCascadeOrchestrator.svelte.ts](../app/src/lib/orchestration/facilityCascadeOrchestrator.svelte.ts) `reconcileDanglingChannelRefsOnLoad()` / `_cascadeDetach` — load-time repair and runtime hardware-channel cascade both stage config drafts through the teardown path above.
- **Consumers** (surfaces that observe the mirrored writes):
  - [app/src-tauri/src/node_registry/proxy.rs](../app/src-tauri/src/node_registry/proxy.rs) `NodeProxy.modified_value` map — updated by every `set_modified_value` IPC. This is the map that `save_layout_with_bus_writes` Phase 2 (`write_modified_values`) scans to decide which addresses to write to the bus. Empty on the connected-save path was the root cause of the pre-mirror regression.
  - [app/src-tauri/src/commands/save_layout_with_bus_writes.rs](../app/src-tauri/src/commands/save_layout_with_bus_writes.rs) Phase 2 — the ultimate consumer. If the mirror silently drops a draft, this phase never emits the corresponding bus write and Phase 4's catalog rebuild reads the un-updated live state, producing empty consumer leaves and disappearing bowties.
- **Per-slice plumbing rule**: Any new feature that produces a config-value draft MUST go through `configEditor.applyEdit` and do nothing else on the IPC side. Direct `setModifiedValue` calls from a component or orchestrator layer are drift — they bypass the single-owner property that keeps the "someone forgot to flush" bug class impossible. The mirror is mounted per layout-open in `+page.svelte` and torn down in `layoutLifecycleOrchestrator.resetForNewLayout()`; new lifecycle transitions MUST preserve both hooks so the last-seen map never bleeds across layouts. Offline mode is out of scope: the mirror is silent when `layoutStore.isConnected === false`, and `stageDraftsForOfflineSave` in `configDraftOrchestrator` owns the offline-save persistence path. A future batched `setModifiedValues` IPC (multi-write coalesce) MUST replace the mirror's emission, not run in parallel. Placeholder NodeKeys are skipped — they persist through the offline-save path only.
- **Last-modified**: 2026-07-03 (introduced — Commit 1 of the ADR-0012 2026-07-03 extension: orchestrator + mount + teardown wired; per-callsite `flushDraftToBackend` calls retire in Commit 2)
- **Last-audited**: 2026-07-03

### Notes

The mirror is the promised-but-unbuilt "separate reactive orchestrator" the `configEditor.svelte.ts` docstring pointed at from PR 1 onward. Shape 1 was chosen over three alternatives on 2026-07-03: (2) call `flushDraftToBackend` inside `configEditor.applyEdit` — rejected because it breaks the sync + no-IPC contract that keeps `applyEdit` reactive and testable; (3) call `flushDraftToBackend` at every draft-producing callsite — rejected because it distributes the "remember to flush" invariant across every future feature that stages a draft (the exact drift that produced the original bug); (4) fold the mirror into `configDraftOrchestrator.ts` — rejected because the offline-staging role (`stageDraftsForOfflineSave`) has a different lifecycle (run at save time) than the online mirror (run reactively), and mixing the two obscures the offline-vs-online contract.

The mirror's diff-based design intentionally does not batch — composition writes 2 leaves per bowtie in the same reactive tick, so a facility with N bowties emits 2N `setModifiedValue` IPCs. A batched `setModifiedValues` IPC + emission coalesce is a straightforward follow-up if profiling shows a real cost; it is deliberately out of scope for this seam's introduction to keep the diff logic obvious.

Removals produce no IPC because the backend already reflects the resolved state — `pruneResolvedDraftsForNode` runs after a tree refresh confirms the value was written, and a redundant `setModifiedValue` at that point would race with `write_modified_values` clearing the entry.


## Config Draft Backend Mirror

- **Governing ADR(s)**: ADR-0012 (all layout edits flow through the draft layer; 2026-07-03 extension — connected-mode draft-to-backend mirror)
- **Owner**: [app/src/lib/orchestration/configDraftMirrorOrchestrator.svelte.ts](../app/src/lib/orchestration/configDraftMirrorOrchestrator.svelte.ts) `startMirror()` / `stopMirror()` + `reconcile(entries)`. Owns a private `Map<string, TreeConfigValue>` of last-seen drafts and an `$effect.root` subscription to `configChangesStore.draftEntries()`; on every reactive tick it diffs the current snapshot against last-seen and emits `setModifiedValue` for each new/changed draft. Connection state is checked inside the effect body (not as a reactive dependency), so the mirror does not re-emit the backlog on connect/disconnect.
- **Contributors** (draft producers whose writes MUST reach the backend `NodeProxy.modified_value` map through this seam):
  - [app/src/lib/stores/configEditor.svelte.ts](../app/src/lib/stores/configEditor.svelte.ts) `applyEdit(key, value)` — the sole synchronous entry point for user-initiated config edits. All draft producers below funnel through this single call, which writes to `configChangesStore` and returns without IPC.
  - [app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte](../app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte) — leaf-row edit commit (control-driven user edits).
  - [app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte](../app/src/lib/components/Bowtie/BowtieCatalogPanel.svelte) — manual bowtie EventID adoption paths (`handleNewConnection`, `handleClearConnection`).
  - [app/src/lib/orchestration/facilityOrchestrator.ts](../app/src/lib/orchestration/facilityOrchestrator.ts) `composeBowtiesIfWired(facilityId)` — facility composition writes composed event ids onto consumer leaves via `configEditor.applyEdit`. Before the mirror existed, these edits were the primary source of the "Save didn't write" bug.
  - [app/src/lib/orchestration/facilityOrchestrator.ts](../app/src/lib/orchestration/facilityOrchestrator.ts) `resetComposedLeavesForFacility(facilityId)` — teardown resets composed leaves to a fresh event id via `configEditor.applyEdit`. Reached from both `tearDownFacilityBowties` (user detach) and the cascade paths.
  - [app/src/lib/orchestration/facilityCascadeOrchestrator.svelte.ts](../app/src/lib/orchestration/facilityCascadeOrchestrator.svelte.ts) `reconcileDanglingChannelRefsOnLoad()` / `_cascadeDetach` — load-time repair and runtime hardware-channel cascade both stage config drafts through the teardown path above.
- **Consumers** (surfaces that observe the mirrored writes):
  - [app/src-tauri/src/node_registry/proxy.rs](../app/src-tauri/src/node_registry/proxy.rs) `NodeProxy.modified_value` map — updated by every `set_modified_value` IPC. This is the map that `save_layout_with_bus_writes` Phase 2 (`write_modified_values`) scans to decide which addresses to write to the bus. Empty on the connected-save path was the root cause of the pre-mirror regression.
  - [app/src-tauri/src/commands/save_layout_with_bus_writes.rs](../app/src-tauri/src/commands/save_layout_with_bus_writes.rs) Phase 2 — the ultimate consumer. If the mirror silently drops a draft, this phase never emits the corresponding bus write and Phase 4's catalog rebuild reads the un-updated live state, producing empty consumer leaves and disappearing bowties.
- **Per-slice plumbing rule**: Any new feature that produces a config-value draft MUST go through `configEditor.applyEdit` and do nothing else on the IPC side. Direct `setModifiedValue` calls from a component or orchestrator layer are drift — they bypass the single-owner property that keeps the "someone forgot to flush" bug class impossible. The mirror is mounted per layout-open in `+page.svelte` and torn down in `layoutLifecycleOrchestrator.resetForNewLayout()`; new lifecycle transitions MUST preserve both hooks so the last-seen map never bleeds across layouts. Offline mode is out of scope: the mirror is silent when `layoutStore.isConnected === false`, and `stageDraftsForOfflineSave` in `configDraftOrchestrator` owns the offline-save persistence path. A future batched `setModifiedValues` IPC (multi-write coalesce) MUST replace the mirror's emission, not run in parallel. Placeholder NodeKeys are skipped — they persist through the offline-save path only.
- **Last-modified**: 2026-07-03 (introduced — Commit 1 of the ADR-0012 2026-07-03 extension: orchestrator + mount + teardown wired; per-callsite `flushDraftToBackend` calls retire in Commit 2)
- **Last-audited**: 2026-07-03

### Notes

The mirror is the promised-but-unbuilt "separate reactive orchestrator" the `configEditor.svelte.ts` docstring pointed at from PR 1 onward. Shape 1 was chosen over three alternatives on 2026-07-03: (2) call `flushDraftToBackend` inside `configEditor.applyEdit` — rejected because it breaks the sync + no-IPC contract that keeps `applyEdit` reactive and testable; (3) call `flushDraftToBackend` at every draft-producing callsite — rejected because it distributes the "remember to flush" invariant across every future feature that stages a draft (the exact drift that produced the original bug); (4) fold the mirror into `configDraftOrchestrator.ts` — rejected because the offline-staging role (`stageDraftsForOfflineSave`) has a different lifecycle (run at save time) than the online mirror (run reactively), and mixing the two obscures the offline-vs-online contract.

The mirror's diff-based design intentionally does not batch — composition writes 2 leaves per bowtie in the same reactive tick, so a facility with N bowties emits 2N `setModifiedValue` IPCs. A batched `setModifiedValues` IPC + emission coalesce is a straightforward follow-up if profiling shows a real cost; it is deliberately out of scope for this seam's introduction to keep the diff logic obvious.

Removals produce no IPC because the backend already reflects the resolved state — `pruneResolvedDraftsForNode` runs after a tree refresh confirms the value was written, and a redundant `setModifiedValue` at that point would race with `write_modified_values` clearing the entry.
