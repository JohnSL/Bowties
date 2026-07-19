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
- **Invariant — live-only inputs:** `handleDiscoveredNode`, `refreshReinitializedNode`, and `reconcileRefreshState` must receive only live-node `DiscoveredNode[]` arrays (i.e. `liveNodes`, not `nodes`/`allEntries`). Placeholder entries have `node_id: []`, which crashes `keyOf()`→`nodeKey("")`. `replaceLiveRoster` also skips entries with empty `node_id` as a belt-and-braces defense.
- **Invariant — off-bus saved node visibility:** After each `replaceLiveRoster` in the discovery and refresh handlers, `nodeRoster.injectOffBusSavedNodes(savedNodeIds)` synthesizes `NotResponding` entries for saved layout nodes not present on the bus. Also called after `probeForNodes()` to cover the "all nodes offline" case where no discovery events fire. Idempotent; overwritten when a node later responds.

## Disconnect / Offline Fallback
- **Orchestrator:** `syncSessionOrchestrator.svelte.ts` (`disconnect()` → `disconnectWithOfflineFallback()`)
- **Transition matrix:** `lifecycleTransitionMatrix.ts` (`resolveDisconnectTransition`)
- **Paths:** `rehydrated_offline` (layout + snapshots → rehydrate), `preserved_layout` (layout, no snapshots → clear live state), `cleared_to_connection` (no layout → clear + show dialog)
- **Sidebar invariant:** The `rehydrateOffline` callback in `+page.svelte` calls `configSidebarStore.pruneToAvailableNodes()` *after* hydration to keep the selection if the node survived into the offline roster. The `clearLiveState` callback calls `configSidebarStore.reset()` unconditionally (no nodes survive when there are no snapshots).

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
- **Backend:** `commands/cdi.rs` (`download_cdi`, `get_cdi_xml`, `cancel_cdi_download`) — thin intent translator. Resolves a `PeerSessionHandle` via `state.sessions` (fetch-per-call) and dispatches `PeerCommand::DownloadCdi`; per-peer FIFO serialisation is structural inside the actor (`PeerSession::cdi_pending`). `cancel_cdi_download` snapshots every session via `PeerSessionRegistry::snapshot_handles()` and dispatches `PeerCommand::Cancel` to each (global-cancel semantic preserved). **S3 (2026-07-08, ADR-0018):** retired `bowties_core::cdi_inflight::CdiInflightRegistry` and `AppState::cdi_download_cancel`.
- **lcc-rs:** `peer_session.rs::ActiveExchange::CdiDownload` (multi-chunk state machine, DR-with-resend-OK retry, OIR-terminal decode, per-chunk timeout with one `TerminateDueToError` emission per exchange), `protocol/memory_config.rs`, `protocol/datagram.rs`, `cdi/parser.rs`. `discovery.rs::read_cdi_cancellable_with_stats` is `#[deprecated]` — remains as a shim for external `LccConnection` consumers, retired in S5.

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
- **Backend:** `commands/bowties.rs` (`load_layout`, `save_layout`, `build_bowtie_catalog_command` — offline fallback via `OfflineBowtieData`), `commands/layout_capture.rs` (`create_new_layout_capture`, `capture_layout_snapshot`, `build_offline_node_tree`, `close_layout`; snapshot tree-walking delegates to `bowties_core::layout::capture`)
- **State:** `state.rs` (`OfflineBowtieData` — config values, profile roles, CDI XML accumulated per node during offline tree build). `node_registry.rs` (`saved_trees` — config trees built from saved snapshots during layout open; seeded into live proxies on bus rediscovery so previously-captured config is the base layer).
- **Sidebar clearing:** `openOfflineLayoutWithReplay` resets sidebar before hydration; `resetLayoutStateForNoLayout` resets sidebar during teardown. Both use injected `resetSidebar` callback.
- **Save invariant:** `saveCurrentCaptureToFile` must call `buildBowtieCatalog` after `saveLayoutFile` to rebuild the catalog with merged metadata (names, tags, role classifications). Without this, the stale pre-save catalog is used and bowties appear incomplete.
- **Save invariant — no partial downgrade:** `save_layout_directory` never persists a `Partial` snapshot when a `Complete` previous snapshot exists for the same node. The previous snapshot is preserved as-is. This prevents data loss when a saved node is on the bus but hasn't been config-re-read.
- **Save invariant — snapshot cache:** After a successful save, `saveLayoutOrchestrated` returns `nodeSnapshots` (from the backend `SaveLayoutResult`). `+page.svelte` caches these in `currentLayoutSnapshots` so the disconnect transition matrix sees `hasSnapshots: true` and takes the `rehydrated_offline` path. Without this, saves that create new snapshots leave the cache stale and disconnect falls through to `preserved_layout` (which clears all nodes).
- **Drafts-cleared-on-save invariant (ADR-0004 / spec 013 S2c):** `saveLayoutOrchestrator` clears `configChangesStore` drafts after the catalog has been rebuilt and persisted (`clearPersistedDrafts` callback injected from `+page.svelte`). The merge in `buildEffectiveBowtiePreview` no longer has a fast/slow branch — it is one derivation, so stale drafts can no longer pin a stale tree-scan view while the catalog swap is in flight. This eliminates the "blank diagram during save" failure mode.
- **Journaled in-place writes (ADR-0006 / spec 013 S2e):** every layout mutation (full save and partial offline-change / snapshot updates) routes through `layout/journal.rs::execute`. The companion directory is never renamed during a save; files are overwritten in place under a `.save-in-progress` marker + `.restore/` backup mirror. `read_capture` calls `recover_if_needed` first and surfaces `recovery_occurred` up to `OpenLayoutResult`, which `+page.svelte` translates into a "Previous save was interrupted and has been restored." toast.
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
- **Orchestrator:** `syncSessionOrchestrator.svelte.ts`
- **Store:** `syncPanel.svelte.ts`
- **API:** `sync.ts`
- **Backend:** `commands/sync_panel.rs` (thin coordinator; delegates scoring to `bowties_core::sync::classifier`, CDI field resolution to `bowties_core::sync::field_meta`, change helpers to `bowties_core::sync::changes`)

## Sync Apply
- **Orchestrator:** `syncApplyOrchestrator.ts`, `syncPanelViewOrchestrator.ts`
- **Store:** `offlineChanges.svelte.ts`, `syncPanel.svelte.ts`
- **API:** `sync.ts`
- **Backend:** `commands/sync_panel.rs` (bus I/O and AppState coordination; value conversion via `bowties_core::sync::field_meta`)
- **lcc-rs:** `protocol/memory_config.rs` (applies writes)

## Connector Selection
- **Route:** `+page.svelte` (confirmation dialog before board change when channels affected)
- **Orchestrator:** `connectorSelectionOrchestrator.ts`
- **Store:** `connectorSelections.svelte.ts`, `nodeTree.svelte.ts`, `channels.svelte.ts` (step 4)
- **Util:** `connectorConstraints.ts`, `connectorLeafDecision.ts`, `connectorSlotSelectors.ts`, `channelDefaults.ts` (step 4)
- **API:** `connectorProfiles.ts`, `channels.ts` (save-time only per ADR-0012)
- **Backend:** `commands/connector_profiles.rs`, `commands/channels.rs` (`create_channels`, `rename_channel`, `delete_channels` — invoked only by save workflow), `profile/loader.rs`, `profile/resolver.rs`
- **Steps:** 1. Update slot selection → 2. Refresh node tree → 3. Recompute compatibility → 4. Remove old channels for slot + auto-create new channels from board `channelInputs` metadata in-memory (Spec 015 / S3+S5, ADR-0012). Route shows confirmation dialog before step 1 when existing channels would be affected. Backend IPC is deferred to save.

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

## Placeholder Board Lifecycle (Spec 014 / S8.8–S8.13)
- **Route:** `+page.svelte` (add/delete menu items, `canAddPlaceholderBoard`/`canDeletePlaceholderBoard` gates)
- **Component:** `AddBoardDialog.svelte` (profile picker)
- **Orchestrator:** `placeholderBoardOrchestrator.ts` (`addPlaceholderBoard` calls factory IPC + seeds roster; `deletePlaceholderBoard` with confirm gate)
- **Store:** `nodeRoster.svelte.ts` (`addPlaceholder`, `removePlaceholder`, `markPlaceholdersPersisted`, internal `_profileStems` map)
- **API:** `layout.ts` (`addPlaceholderBoardIpc`, `getNodeTree`)
- **Backend:** `placeholder.rs` (factory: `synthesize`, `reconstitute`), `commands/placeholders.rs` (`add_placeholder_board` IPC)
- **Registry:** `node_registry.rs` / `node_proxy.rs` — `Synthesized(SynthesizedNodeProxy)` variant inserted by factory
- **Save path:** unified with real nodes via `AddNode { node_key }` delta → `layout_capture.rs` one-arm save flow
- **No protocol** — factory synthesizes what the bus would have read

## Unsaved Changes Guard
- **Orchestrator:** `unsavedChangesGuard.ts`
- **Store:** `changeTracker.svelte.ts`, `configChanges.svelte.ts`
- **Component:** `DiscardConfirmDialog.svelte`
- **Window close:** `+page.svelte` `onCloseRequested` always prevents native close, checks `hasUnsavedPromptChanges`, shows the unsaved dialog when dirty, calls `disconnect_lcc` IPC for graceful LCC teardown before exiting. Menu Exit uses the same `promptUnsaved` + disconnect path.

## Native Menu Wiring (S4)
- **Route:** `+page.svelte` — builds the `MenuEnableInputs` snapshot in a reactive `$effect`, supplies `MenuActionHandlers` bodies, pushes enable bits via `update_menu_state` IPC
- **Util:** `menuEnableState.ts` (`computeMenuEnableState` — pure enable policy)
- **Orchestrator:** `menuListeners.ts` (`registerMenuListeners` — owns the `menu-*` listen/teardown lifecycle)
- **Keyboard:** `menuShortcuts.ts` (`installMenuShortcuts` — Ctrl/Cmd accelerator bindings)
- **Backend:** `update_menu_state` IPC command (native menu item enable/disable)

## Information Channel Inventory (Spec 015)
- **Route:** `+page.svelte` — 3rd tab ("Railroad") in segmented control; renders `RailroadPanel`
- **Component:** `Railroad/RailroadPanel.svelte` → `ChannelGroup.svelte` → `ChannelCard.svelte`
- **Store:** `channels.svelte.ts` (`channelsStore` — hydrated on layout open, reset on close)
- **API:** `channels.ts` (`listChannels`, `createChannels`, `renameChannel`, `deleteChannels`)
- **Backend:** `commands/channels.rs` (`list_channels`, `create_channels`, `rename_channel`, `delete_channels`)
- **Core:** `bowties-core/layout/channels.rs` (`InformationChannel`, `ChannelsDocument`)
- **Lifecycle:** `layoutLifecycleOrchestrator.ts` resets `channelsStore` on layout close (S6)
- **Save flush:** `+page.svelte` save handler calls `deleteChannels` → `createChannels` → `renameChannel` for pending drafts, then re-reads authoritative state

## Live Channel State (Spec 016)
- **Protocol:** `lcc-rs/protocol/mti.rs` — `ProducerConsumerEventReport` (0x195B4) MTI variant
- **Backend event pipeline:** `events/router.rs` — PCER subscription → parses 8-byte event ID → emits `lcc-event-state` Tauri event
- **Backend command:** `commands/channel_events.rs` — `resolve_channel_event_ids` batch IPC: walks cached config trees + profile eventMapping to extract producer event IDs per channel
- **Core domain:** `bowties-core/channel_events.rs` — `resolve_channel_event_ids(tree, connector, input, event_mapping)`: finds Producer-role EventId leaves under the slot's resolved path prefix, indexes by producerLeafIndex
- **Profile schema:** `bowties-core/profile/types.rs` — `EventMappingEntry { producer_leaf_index }` + `event_mapping: HashMap<String, EventMappingEntry>` on `ChannelInputMapping`
- **Frontend store:** `stores/eventState.svelte.ts` — transient, channel-unaware event ledger (eventId → timestamp)
- **Frontend orchestrator:** `orchestration/eventStateOrchestrator.ts` — `startEventStateListening()` subscribes to backend events; `resolveChannelEventIds()` batch-resolves event IDs via IPC
- **Frontend utility:** `utils/channelState.ts` — `deriveChannelState(events, occupiedId, clearId)` pure timestamp comparison
- **Component:** `RailroadPanel.svelte` computes `channelStates` via `$derived.by()` + passes to `ChannelCard` as `occupancyState` prop
