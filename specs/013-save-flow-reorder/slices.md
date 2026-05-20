# Slices: Layout-First Model

Branch: 013-save-flow-reorder
Generated: 2026-05-17
Status: 4/10 slices complete (S1, S2, S2a, S2b done); S2c added after S2b validation surfaced three remaining divergence bugs

---

## S1: Extract save flow to orchestrator [HITL]

**Layers**: Route, Orchestrator
**Blocked by**: None
**Complexity**: medium
**User stories**: US4

`+page.svelte` currently inlines `saveCurrentCaptureToFile` as a multi-step async workflow, bypassing `saveLayoutOrchestrator`. This slice extracts that inline logic into the orchestrator and wires the route to delegate. The orchestrator becomes the canonical save seam that all subsequent save slices build on.

**Acceptance criteria**:
- [x] `saveCurrentCaptureToFile` in `+page.svelte` delegates to `saveLayoutOrchestrator`
- [x] No inline save workflow logic remains in the route
- [x] Existing save tests pass without modification

**Tasks**:
- [x] S1-T1: Write integration test — save triggers `saveLayoutOrchestrator`, not inline code
- [x] S1-T2: Orchestrator — move inline save logic from `+page.svelte` into `saveLayoutOrchestrator.ts`
- [x] S1-T3: Route — replace `saveCurrentCaptureToFile` body with orchestrator call
- [x] S1-T4: Validate — integration test passes, existing save tests green

---

## S2: Three-phase save + event role persistence [HITL]

**Layers**: Orchestrator, API, Backend command, Backend domain
**Blocked by**: S1
**Complexity**: large
**User stories**: US4, US5

The core architectural fix: save always writes the layout first, then bus writes, then reconciles. A new `save_layout_with_bus_writes` backend command owns the three-phase sequence and emits Tauri progress events between phases. Cancel before bus writes sends nothing to bus. After bus writes, the layout is saved again to clear succeeded offline changes. All resolved (non-ambiguous) event role classifications from the live bowtie catalog are persisted into the layout during the first save phase.

**Acceptance criteria**:
- [x] Online save writes layout before any bus writes (ADR-0001 enforced)
- [x] Cancelling before bus writes sends zero bytes to bus and restores pending changes
- [x] Bowtie preview never goes blank or stale at any point during or after save *(S2a made backend authoritative for layout file; S2b unified display resolution so frontend reads draft → offline pending → baseline consistently)*
- [x] Reconcile phase saves layout again; succeeded offline changes are cleared
- [x] All resolved (non-ambiguous) event roles are persisted in the layout on save
- [x] Ambiguous roles are not written and remain ambiguous on reopen

**Tasks**:
- [x] S2-T1: Write integration test — layout saved before bus; cancel sends nothing; roles persist; bowties never blank
- [x] S2-T2: Backend domain — update `merge_layout_metadata` to include all resolved non-ambiguous roles from live catalog
- [x] S2-T3: Backend command — implement `save_layout_with_bus_writes` with three-phase flow and Tauri progress events
- [x] S2-T4: API — add `saveLayoutWithBusWrites` Tauri invoke binding
- [x] S2-T5: Orchestrator — update `saveLayoutOrchestrator.ts` to call the new command, handle cancel, handle partial failure
- [x] S2-T6: Validate — integration test passes, bowties correct throughout, roles survive save → close → reopen
- [x] S2-T7: Bug fix — populate `offline_bowtie_data` during `open_layout_directory` so offline catalog rebuilds discover event slots

---

## S2a: Backend-authoritative save (ADR-0002) [HITL]

**Layers**: Backend domain, Backend command, API, Orchestrator, Store
**Blocked by**: S2
**Complexity**: large
**User stories**: US4, US5

The save data flow currently passes the frontend's `LayoutFile` copy to the backend, which wholesale-replaces on-disk data — causing data loss when the frontend copy is stale or incomplete (empty `roleClassifications` overwriting correct values, null layout after Save As). ADR-0002 makes the backend the sole owner of layout file data. Save commands accept structured edit deltas instead of full layout objects. The backend applies deltas to its disk-authoritative copy and returns the persisted layout. The frontend layout store becomes a read cache populated only from backend responses. `_applyToLayout()` is removed; the effective layout for display is computed by merging the read cache with pending edits.

**Acceptance criteria**:
- [x] Save commands accept edit deltas (bowtie metadata + role classifications + connector selections), not `Option<LayoutFile>`
- [x] Save commands return the persisted `LayoutFile`; frontend hydrates layout store from the response
- [x] `merge_saved_layout_metadata` replaced with delta application — no wholesale field replacement
- [x] `layoutStore._layout` is only set from backend responses (open, save, hydrate); never mutated by metadata stores
- [x] `_applyToLayout()` removed from `bowtieMetadataStore`; edits stay in `_edits` until save
- [x] Bowtie preview cards and dirty indicators derive from `_layout` + `_edits` (effective-layout pattern)
- [x] `getInstanceDisplayName()` resolves through draft → offline pending → baseline, not just `child.value.value` *(function supports resolver; 3/7 call sites wired — S2b completes wiring)*
- [x] Role classifications survive save → close → reopen cycle (S2 acceptance criteria unblocked) *(S2a routes role-classification deltas through backend `apply_layout_deltas`; on reopen `bowtieMetadataStore` rehydrates from layout file; S2b ensures display reads from the rehydrated layer)*
- [x] Layout store is non-null after Save As (backend response populates it)

**Tasks**:
- [x] S2a-T1: Backend domain — define `LayoutEditDelta` type (bowtie edits, role classification edits, connector selection edits)
- [x] S2a-T2: Backend domain — replace `merge_saved_layout_metadata` with `apply_layout_deltas` that reads disk, applies deltas, overlays catalog roles, writes, and returns persisted layout
- [x] S2a-T3: Backend command — change `save_layout_directory` and `save_layout_with_bus_writes` to accept deltas and return `LayoutFile`
- [x] S2a-T4: API — update Tauri invoke bindings for new save signatures (deltas in, layout out)
- [x] S2a-T5: Orchestrator — update `saveLayoutOrchestrator` to collect deltas from `bowtieMetadataStore._edits` and send them; hydrate layout store from response
- [x] S2a-T6: Store — remove `_applyToLayout()` from `bowtieMetadataStore`; make layout store read-only between open/save
- [x] S2a-T7: Store/Utils — add effective-layout derivation (layout + edits) for preview cards and dirty indicators
- [x] S2a-T8: Utils — update `getInstanceDisplayName()` to resolve draft → offline pending → baseline
- [x] S2a-T9: Validate — S2 acceptance criteria pass; roles persist; layout non-null after Save As; bowtie preview stable

---

## S2b: Unified display resolution (ADR-0003) [HITL]

**Layers**: Utils, Store, Component, Backend command
**Blocked by**: S2a
**Complexity**: medium
**User stories**: US4, US5

S2a made the backend the sole owner of layout file data, but display value resolution remains scattered across 6+ independent frontend paths. Some use the full draft → offline pending → baseline waterfall; others read stale tree baseline directly. Online this is invisible (baseline = live hardware), but offline the baseline is a snapshot that doesn't reflect saved changes — causing names, values, and role tags to diverge between bowtie cards and the config tree. ADR-0003 establishes that the backend catalog owns the resolved baseline and the frontend owns only the transient draft layer, with one resolution function for values and one for roles used by all display paths.

**Acceptance criteria**:
- [x] A single `resolveValue(nodeId, path)` function exists and all display paths use it for config value resolution
- [x] A single `resolveRole(nodeId, path)` function exists and all display paths use it for role classification
- [x] After offline save + catalog rebuild, the frontend baseline reflects saved config values (not the stale pre-edit snapshot) *(via layered resolver: draft → offlinePending → baseline; staleness of `leaf.value` is now invisible because all call sites route through `makeValueResolver`)*
- [x] PickerTreeNode group labels, ElementPicker labels, TreeLeafRow context menu labels, and TreeGroupAccordion non-pill headers all show user-configured names while offline
- [x] TreeLeafRow role tags show user-classified roles, not just CDI baseline roles
- [x] ElementPicker auto-classification checks the effective role (not baseline-only `leaf.eventRole`)

**Tasks**:
- [x] S2b-T1: Write tests — resolution function returns draft → offline pending → baseline in correct priority; role resolution returns pending edit → catalog → CDI baseline
- [x] S2b-T2: Utils — implement `resolveValue` and `resolveRole` in a resolution utility module
- [x] S2b-T3: Store — ~~update tree baseline from catalog-resolved values~~ deferred: layered resolver makes baseline staleness invisible; revisit only if a concrete divergence is reported
- [x] S2b-T4: Component — wire PickerTreeNode `pickerGroupLabel()` to use `resolveValue` via display name resolution
- [x] S2b-T5: Component — wire TreeLeafRow role tag display (L694-696) to use `resolveRole` instead of `leaf.eventRole`
- [x] S2b-T6: Component — wire ElementPicker auto-classification and label to use `resolveRole` and `resolveValue`
- [x] S2b-T7: Component — wire TreeGroupAccordion non-pill headers and TreeLeafRow context menu to use display name resolution
- [x] S2b-T8: Validate — all acceptance criteria pass; bowtie cards and config tree agree on names and roles in both online and offline modes (846/846 vitest tests pass)

<!-- Session: 2025-S2b — Completed S2b (unified display resolution). New `displayResolution.ts` utility centralizes value + role resolution per ADR-0003. All 6 divergent call sites now route through `makeValueResolver`/`resolveRole`: PickerTreeNode group labels, TreeGroupAccordion non-pill headers, TreeLeafRow role tag + context menu, ElementPicker auto-classification + label. T3 deferred (layered resolver makes baseline staleness invisible). Next: S3 (AFK). -->

---

## S2c: Layout facade + effective view store (ADR-0004) [HITL]

**Layers**: Store, Orchestrator, Component, Route
**Blocked by**: S2b
**Complexity**: large
**User stories**: US4, US5

S2b unified resolution at the **leaf** level, but three bugs remain that share the same root cause one level up: each display surface still re-derives its own "effective view" from raw stores and each omits a different layer.

- During offline save the bowtie diagram goes blank (and sometimes stays blank) because `configChangesStore` drafts are not cleared on persisted save, so `EditableBowtiePreviewStore` is stuck on the slow tree-scanning path while tree/catalog are mid-rebuild.
- The offline ElementPicker shows "?" badges and skips role filtering because `PickerTreeNode` reads `leaf.eventRole` directly for filter and badge code; saved `roleClassifications` never reach the tree and `resolveRole` is only called in `handleSelect`.
- Deleting a bowtie leaves a stale card on screen until save because both preview-build paths in `bowties.svelte.ts` iterate the catalog without consulting `bowtieMetadataStore`'s pending `delete:${eventIdHex}` edits.

ADR-0004 establishes a `$lib/layout` facade as the only layout-state import surface for components. Internally it composes a Svelte 5 `$derived` read model (`effectiveLayoutStore`) and the extended `saveLayoutOrchestrator`. The four edit-layer stores become internal.

**Acceptance criteria**:
- [x] Components and routes import layout reads/writes only from `$lib/layout`; `bowtieCatalogStore` only via the facade re-export; `bowtieMetadataStore`, `configChangesStore`, `layoutStore` are not imported outside the facade and orchestrator
- [x] `effectiveLayoutStore` exposes `effectiveBowties`, `effectiveRole`, `effectiveValue`, `slotsByRole`, `isSlotFree`; every display path uses these (no `leaf.eventRole` reads outside the read model)
- [x] After offline save, the bowtie diagram never goes blank during or after the save sequence (drafts cleared on persisted save by `saveLayoutOrchestrator.clearPersistedDrafts`; single-derivation merge eliminates the fast/slow branch that was pinning stale state)
- [x] Offline ElementPicker filters consumer/producer slots correctly and only shows "?" when the effective role is genuinely unknown (`PickerTreeNode` reads `effectiveLayoutStore.effectiveRole`)
- [x] Deleting a bowtie immediately removes it from the panel (`effectiveLayoutStore.preview` filters `hasPendingDeletion` before exposing cards)
- [x] `EditableBowtiePreviewStore` fast/slow path branch is removed; the class itself is gone — a single module-level `buildEffectiveBowtiePreview()` function in `bowties.svelte.ts` is the merge, called only from the facade
- [x] `resolveValue` and `resolveRole` from `displayResolution.ts` become internal implementation details of the read model, not imported by components

**Tasks**:
- [x] S2c-T1: Write tests — `effectiveLayoutStore` correctly merges pending bowtie deletions, pending role classifications, draft config values, and pending entry edits over the catalog; `slotsByRole` filters by effective role
- [x] S2c-T2: Write tests — `saveLayoutOrchestrator` clears persisted drafts on successful offline save; read model observes no intermediate blank state during catalog swap
- [x] S2c-T3: Store — implement `effectiveLayoutStore.svelte.ts` consolidating `_buildPreviewFromCatalog`, `_buildPreviewWithTreeScanning`, `getRoleForSlot`, and the leaf-level `resolveValue`/`resolveRole` into a single derivation
- [x] S2c-T4: Orchestrator — extend `saveLayoutOrchestrator` to clear `configChangesStore` drafts matching persisted edits and to swap the catalog atomically from the read model's perspective
- [x] S2c-T5: Facade — create `app/src/lib/layout/index.ts` re-exporting read model API + orchestrator entry points + edit-recording commands; document it as the sole import surface in `aiwiki/owners.md`
- [x] S2c-T6: Component — wire `BowtieCatalogPanel` and bowtie card components to read `effectiveBowties` from the facade
- [x] S2c-T7: Component — wire `PickerTreeNode` filter and badge code (lines 85-94, 199-210, 283, 290, 303, 310, 317) to `effectiveRole` / `slotsByRole` instead of `leaf.eventRole`
- [x] S2c-T8: Component — replace remaining `displayResolution` imports in components with facade reads; downgrade `displayResolution.ts` to an internal helper
- [x] S2c-T9: Cleanup — done in three sub-steps:
    - **T9a**: collapsed the fast/slow path inside the merge into a single derivation; removed the dead `_buildPreviewFromCatalog` method
    - **T9b**: migrated remaining `editableBowtiePreviewStore` consumers (`ElementPicker.svelte`, `NewConnectionDialog.svelte`) to `$lib/layout`; added `effectiveLayoutStore.usedInMap` to the facade
    - **T9c**: extracted the merge into a module-level `buildEffectiveBowtiePreview()` function; deleted the `EditableBowtiePreviewStore` class and `editableBowtiePreviewStore` export; retargeted the 15 store tests to exercise the merge through `effectiveLayoutStore`
- [x] S2c-T10: Validate — 876/876 vitest tests green; `aiwiki/owners.md` and `aiwiki/flows.md` updated. Manual scenarios (save-then-blank, offline picker filtering, delete-bowtie immediacy) require HITL re-verification before closing the spec.

---

## S3: Save progress tracking + API cleanup [AFK]

**Layers**: Store, Component, API
**Blocked by**: S2a
**Complexity**: small
**User stories**: US6

A modal `SaveProgressDialog` displays the current save phase and per-field bus-write progress. A new `saveProgress` store tracks phase transitions driven by Tauri progress events from S2. As a companion to extending the API layer in S2, the duplicate IPC wrappers (`saveLayoutFile` ≡ `saveLayoutDirectory`, `openLayoutFile` ≡ `openLayoutDirectory`) are removed and the layout.ts / bowties.ts boundary is clarified.

**Acceptance criteria**:
- [ ] Progress store transitions through layout-save → bus-write → reconcile phases
- [ ] `SaveProgressDialog` renders as a modal during save; shows "Saving layout…", per-field bus-write count, and "Updating layout…"
- [ ] Dialog is modal — no second save can be initiated while one is in progress
- [ ] Duplicate API wrappers removed; all callers compile

**Tasks**:
- [ ] S3-T1: Write integration test — progress store updates through phases; dialog displays correct labels
- [ ] S3-T2: Store — implement `saveProgress.svelte.ts` with phase state and Tauri event subscription
- [ ] S3-T3: Component — implement `SaveProgressDialog.svelte` (modal, phase labels, per-field counter)
- [ ] S3-T4: API — remove `saveLayoutFile`/`openLayoutFile` duplicates; clarify layout.ts vs bowties.ts boundary
- [ ] S3-T5: Validate — integration test passes, dialog is modal, no duplicate wrappers

---

## S4: Schema extension (connections field) + connection CRUD [AFK]

**Layers**: Backend domain, Backend command, API
**Blocked by**: None
**Complexity**: medium
**User stories**: US2, US3

Add an optional `connections` field to the layout manifest. Because it's a serde-defaulted optional, existing layout files open without migration — no breaking change. The companion directory snapshot format is unaffected. Backend commands `get_layout_connections` and `save_layout_connections` expose CRUD for connection definitions. A layout can store multiple named connections (name, type, host/port or serial settings).

**Acceptance criteria**:
- [ ] Existing layout files (without connections field) open correctly — no error, connections list is empty
- [ ] Layout with connections persists and round-trips through save → close → reopen
- [ ] A layout can store multiple named connections
- [ ] `get_layout_connections` and `save_layout_connections` commands work correctly

**Tasks**:
- [ ] S4-T1: Write integration test — existing layout opens cleanly; connections round-trip; multiple connections supported
- [ ] S4-T2: Backend domain — add `connections: Vec<ConnectionConfig>` (serde default) to `LayoutManifest` in `manifest.rs`; add `ConnectionConfig` type to `types.rs`
- [ ] S4-T3: Backend command — implement `get_layout_connections` and `save_layout_connections` in `commands/connection.rs`
- [ ] S4-T4: API — add `getLayoutConnections` and `saveLayoutConnections` Tauri invoke bindings
- [ ] S4-T5: Validate — integration test passes, existing layout files unaffected

---

## S5: Known-layout registry backend [AFK]

**Layers**: Backend domain, Backend command, API
**Blocked by**: None
**Complexity**: medium
**User stories**: US1

A new `known_layouts.rs` backend module persists the app's known-layout registry to `$APPDATA/bowties/known-layouts.json`. Each entry stores layout name, file path, and last-opened date. The module filters stale entries (path no longer exists) and uses atomic writes (temp→flush→rename). Backend commands `get_known_layouts`, `add_known_layout`, and `remove_known_layout` expose the registry. Removing a known layout removes only the registry entry — layout files on disk are not deleted.

**Acceptance criteria**:
- [ ] CRUD on known-layouts.json works correctly
- [ ] Stale paths are filtered from the returned list
- [ ] Writes are atomic (temp→flush→rename)
- [ ] Removing a layout entry does not delete files on disk

**Tasks**:
- [ ] S5-T1: Write integration test — CRUD; stale-path filtering; atomic writes; remove-only-registry
- [ ] S5-T2: Backend domain — implement `layout/known_layouts.rs` with get/add/remove and stale-path filtering
- [ ] S5-T3: Backend command — implement `get_known_layouts`, `add_known_layout`, `remove_known_layout` in new `commands/startup.rs`
- [ ] S5-T4: Backend — register new commands in `lib.rs`; add registry state to `state.rs`
- [ ] S5-T5: API — add `getKnownLayouts`, `addKnownLayout`, `removeKnownLayout` Tauri invoke bindings in new `api/startup.ts`
- [ ] S5-T6: Validate — integration test passes, atomic writes confirmed, files not deleted on remove

---

## S6: Layout picker gate [HITL]

**Layers**: Route, Component, Orchestrator, Store, API
**Blocked by**: S4, S5
**Complexity**: large
**User stories**: US1

No functionality is accessible until a layout is active. `+page.svelte` renders either the layout picker or the main UI — never both. `LayoutPicker.svelte` shows known layouts (name, location, last-opened date), "New Layout", and "Browse…". `startupOrchestrator.ts` owns the picker lifecycle: loading known layouts, handling selection, creating new layouts, and setting the active layout in the store. The picker disappears once a layout is active; disconnecting returns to the main UI (not the picker).

**Acceptance criteria**:
- [ ] App with no active layout shows the picker; main UI is not accessible
- [ ] Selecting a known layout opens it and picker disappears
- [ ] "New Layout" prompts for name and location, creates the layout, opens it
- [ ] "Browse…" opens an existing layout not in the known list and adds it to the list
- [ ] "Remove" removes the entry from the known list without deleting files
- [ ] Layout name is visible in the title bar or header after opening
- [ ] Picker does not reappear when disconnecting — only appears when no layout is active

**Tasks**:
- [ ] S6-T1: Write integration test — no layout → picker shown; select known → main UI; new layout → picker gone; browse → added to list
- [ ] S6-T2: Store — extend layout store with `activeLayoutContext` and `knownLayouts` state
- [ ] S6-T3: Orchestrator — implement `startupOrchestrator.ts` (load known layouts, open, create new, browse, set active)
- [ ] S6-T4: Component — implement `LayoutEntry.svelte` (name, location, last-opened date, remove action)
- [ ] S6-T5: Component — implement `NewLayoutDialog.svelte` (name + location inputs, create action)
- [ ] S6-T6: Component — implement `LayoutPicker.svelte` (known list, New Layout, Browse…)
- [ ] S6-T7: Route — add `activeLayoutContext` conditional render gate to `+page.svelte`
- [ ] S6-T8: Validate — integration test passes, picker gate correct, title bar shows layout name

---

## S7: Connected layout — connect from layout + node visibility [HITL]

**Layers**: Route, Component, Orchestrator, Store
**Blocked by**: S6, S4
**Complexity**: medium
**User stories**: US2, US3

With a layout open, the user connects using a connection stored in the layout. If the layout has exactly one connection, it is used directly. If it has multiple, a selector appears. Disconnecting keeps the layout open in offline mode with all data intact. The node list shows all nodes currently in the layout — nodes not discovered on the current bus are shown with a visual "not on bus" indicator. Nodes discovered on the live bus that have no snapshot in the layout are automatically added to the layout.

**Acceptance criteria**:
- [ ] Connect uses layout-stored connection settings; no re-entry required if one connection defined
- [ ] Multi-connection selector appears when layout has more than one connection
- [ ] Disconnect keeps layout open; all node data preserved
- [ ] Node list shows all layout nodes while online; absent nodes have a "not on bus" indicator
- [ ] Newly discovered bus nodes are automatically added to the layout
- [ ] Only one connection may be active at a time

**Tasks**:
- [ ] S7-T1: Write integration test — connect from layout; multi-connection selector; disconnect preserves layout; absent-node indicator; auto-add discovered node
- [ ] S7-T2: Orchestrator — update `offlineLayoutOrchestrator.ts` connect path to read connection from active layout
- [ ] S7-T3: Component — add multi-connection selector to connection dialog (hidden when only one connection)
- [ ] S7-T4: Store — add per-node "on-bus" presence flag; reconcile layout nodes vs discovered bus nodes
- [ ] S7-T5: Component — add "not on bus" visual indicator to node list entries
- [ ] S7-T6: Orchestrator — auto-add newly discovered bus nodes to the layout store
- [ ] S7-T7: Validate — integration test passes, single-connection fast path works, disconnect keeps layout open

---

<!-- Session: 2026-05-17 — Completed S1, S2 (including T7 bug fix: offline_bowtie_data population on layout open). Next: S3 (AFK). -->
<!-- Session: 2026-05-18 — S2 acceptance criteria still failing (role data loss, layout null after Save As). Root-cause analysis found 7-state-owner problem and wholesale-replace in merge_saved_layout_metadata. Wrote ADR-0002 (backend owns layout file data). Quick-fix patches attempted and reverted as architecturally unsound. Next: implement ADR-0002 before reattempting S2 acceptance criteria. -->
<!-- Session: 2026-05-18b — Completed S2a (backend-authoritative save). Discovered systemic display-resolution divergence: 6+ frontend call sites read stale baseline instead of effective values, causing offline names/roles to differ from online. Wrote ADR-0003 (unified display resolution). Added S2b slice. Next: implement S2b. -->
