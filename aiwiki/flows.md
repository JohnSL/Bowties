# Workflow Module Participation

Which modules participate in each major workflow. For full ownership rules, see `product/architecture/`.

---

## Node Discovery
- **Route:** `+page.svelte` (discovery modal lifecycle)
- **Orchestrator:** `discoveryOrchestrator.ts`
- **Store:** `nodeInfo.ts`, `nodes.ts`
- **API:** `tauri.ts`
- **Backend:** `commands/discovery.rs` (`discover_nodes`, `register_node`, `query_snip_*`, `query_pip_*`)
- **lcc-rs:** `discovery.rs` (alias allocation, node probing)

## SNIP / PIP Query
- **Orchestrator:** `discoveryOrchestrator.ts` (embedded in discovery; also `reconcileRefreshState()`)
- **Store:** `nodeInfo.ts`
- **API:** `tauri.ts`
- **Backend:** `commands/discovery.rs` (`query_snip_single/batch`, `query_pip_single/batch`, `verify_node_status`)
- **lcc-rs:** `snip.rs`, `pip.rs`

## CDI Download
- **Component:** `CdiDownloadDialog.svelte`
- **Orchestrator:** `cdiDialogOrchestrator.ts`
- **API:** `cdi.ts`
- **Backend:** `commands/cdi.rs` (`download_cdi`, `get_cdi_xml`, `cancel_cdi_download`)
- **lcc-rs:** `protocol/memory_config.rs`, `cdi/parser.rs`

## Config Read Session
- **Route:** `+page.svelte` (progress modal lifecycle)
- **Component:** `DiscoveryProgressModal.svelte` (per-node progress + building-catalog phase)
- **Orchestrator:** `configReadSessionOrchestrator.ts` (session lifecycle, phase transitions), `configReadOrchestrator.ts` (eligibility, sequential read + tree reload)
- **Store:** `configReadStatus.ts`
- **API:** `config.ts`
- **Backend:** `commands/cdi.rs` (`read_all_config_values` — emits `BuildingCatalog` status on last node before event-role exchange and catalog build, `get_node_tree`, `cancel_config_reading`)
- **lcc-rs:** `protocol/memory_config.rs`, `protocol/datagram.rs`

## Config Editing (Online)
- **Route:** `config/+page.svelte`
- **Component:** `ElementCardDeck/`, `ConfigSidebar/`
- **Orchestrator:** `configDraftOrchestrator.ts`
- **Store:** `configChanges.svelte.ts`, `configEditor.svelte.ts`
- **API:** `config.ts`
- **Backend:** `commands/cdi.rs` (`set_modified_value`, `write_modified_values`, `discard_modified_values`)
- **lcc-rs:** `protocol/memory_config.rs`

## Config Editing (Offline)
- **Route:** `config/+page.svelte`
- **Orchestrator:** `configDraftOrchestrator.ts` (`stageDraftsForOfflineSave()`)
- **Store:** `configChanges.svelte.ts`, `offlineChanges.svelte.ts`
- **API:** `sync.ts`
- **Backend:** `commands/sync_panel.rs` (`set_offline_change`, `revert_offline_change`)
- **No protocol** — local-only until sync apply

## Layout Open / Save
- **Route:** `+page.svelte` (`saveCurrentCaptureToFile` — save path; `openLayoutAction` — open path)
- **Orchestrator:** `offlineLayoutOrchestrator.ts` (calls `buildBowtieCatalog` after offline hydration), `saveLayoutOrchestrator.ts` (tested save→rebuild→clean sequence)
- **Store:** `layout.svelte.ts`, `bowtieMetadata.svelte.ts`, `layoutOpenLifecycle.ts`, `bowties.svelte.ts` (receives offline catalog via `setCatalog`), `configSidebar.ts` (reset on layout open/close)
- **API:** `layout.ts`, `bowties.ts`
- **Backend:** `commands/bowties.rs` (`load_layout`, `save_layout`, `build_bowtie_catalog_command` — offline fallback via `OfflineBowtieData`), `commands/layout_capture.rs` (`create_new_layout_capture`, `capture_layout_snapshot`, `build_offline_node_tree`, `close_layout`)
- **State:** `state.rs` (`OfflineBowtieData` — config values, profile roles, CDI XML accumulated per node during offline tree build)
- **Sidebar clearing:** `openOfflineLayoutWithReplay` resets sidebar before hydration; `resetLayoutStateForNoLayout` resets sidebar during teardown. Both use injected `resetSidebar` callback.
- **Save invariant:** `saveCurrentCaptureToFile` must call `buildBowtieCatalog` after `saveLayoutFile` to rebuild the catalog with merged metadata (names, tags, role classifications). Without this, the stale pre-save catalog is used and bowties appear incomplete.
- **Drafts-cleared-on-save invariant (ADR-0004 / spec 013 S2c):** `saveLayoutOrchestrator` clears `configChangesStore` drafts after the catalog has been rebuilt and persisted (`clearPersistedDrafts` callback injected from `+page.svelte`). The merge in `buildEffectiveBowtiePreview` no longer has a fast/slow branch — it is one derivation, so stale drafts can no longer pin a stale tree-scan view while the catalog swap is in flight. This eliminates the "blank diagram during save" failure mode.
- **No protocol** — YAML snapshot I/O

## Bowtie Catalog Build
- **Route:** `+page.svelte` (trigger on startup/changes)
- **Orchestrator:** `offlineLayoutOrchestrator.ts` (offline path)
- **Store:** `bowties.svelte.ts` — `bowtieCatalogStore` holds the catalog; `buildEffectiveBowtiePreview()` is the **single** merge derivation (catalog × tree × metadata × layout). Per ADR-0004 the merge is consumed only by `$lib/layout/effectiveLayoutStore`; components and routes never call it directly.
- **Facade:** `$lib/layout/effectiveLayoutStore` is the sole read surface for components/routes. It composes `buildEffectiveBowtiePreview()` with the `hasPendingDeletion` filter and adds the leaf-level resolvers (`effectiveRole`, `effectiveValue`, `slotsByRole`, `isSlotFree`, `usedInMap`).
- **API:** `bowties.ts`
- **Backend:** `commands/bowties.rs` (`build_bowtie_catalog_command`, `query_event_roles`, `get_bowties`)

## Sync Session (Classification)
- **Route:** `+page.svelte`
- **Orchestrator:** `syncSessionOrchestrator.ts`
- **Store:** `syncPanel.svelte.ts`
- **API:** `sync.ts`
- **Backend:** `commands/sync_panel.rs`

## Sync Apply
- **Orchestrator:** `syncApplyOrchestrator.ts`, `syncPanelViewOrchestrator.ts`
- **Store:** `offlineChanges.svelte.ts`, `syncPanel.svelte.ts`
- **API:** `sync.ts`
- **Backend:** `commands/sync_panel.rs`
- **lcc-rs:** `protocol/memory_config.rs` (applies writes)

## Connector Selection
- **Route:** `config/+page.svelte`
- **Orchestrator:** `connectorSelectionOrchestrator.ts`
- **Store:** `connectorSelections.svelte.ts`, `nodeTree.svelte.ts`
- **Util:** `connectorConstraints.ts`, `connectorLeafDecision.ts`, `connectorSlotSelectors.ts`
- **API:** `connectorProfiles.ts`
- **Backend:** `commands/connector_profiles.rs`, `profile/loader.rs`, `profile/resolver.rs`

## Bowtie Metadata Editing
- **Store:** `bowtieMetadata.svelte.ts`
- **API:** `bowties.ts`
- **Backend:** `commands/bowties.rs` (`set_bowtie_metadata`)
- **No orchestrator** — direct store → backend

## Traffic Monitoring
- **Route:** `traffic/+page.svelte`
- **Component:** `TrafficMonitor.svelte`
- **Store:** `traffic.ts` (live stream)
- **Backend:** `traffic/mod.rs`, `events/router.rs`
- **lcc-rs:** `protocol/frame.rs` (GridConnect decode)

## Unsaved Changes Guard
- **Orchestrator:** `unsavedChangesGuard.ts`
- **Store:** `changeTracker.svelte.ts`, `configChanges.svelte.ts`
- **Component:** `DiscardConfirmDialog.svelte`
