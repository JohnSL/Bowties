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
- **Route:** `+page.svelte`
- **Orchestrator:** `offlineLayoutOrchestrator.ts`
- **Store:** `layout.svelte.ts`, `bowtieMetadata.svelte.ts`, `layoutOpenLifecycle.ts`
- **API:** `layout.ts`, `bowties.ts`
- **Backend:** `commands/bowties.rs` (`load_layout`, `save_layout`), `commands/layout_capture.rs` (`create_new_layout_capture`, `capture_layout_snapshot`, `build_offline_node_tree`, `close_layout`)
- **No protocol** — YAML snapshot I/O

## Bowtie Catalog Build
- **Route:** `+page.svelte` (trigger on startup/changes)
- **Store:** `bowties.svelte.ts`
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
