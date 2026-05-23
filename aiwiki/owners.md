# Module Ownership

## Summary

| Layer | Count | Key shared logic |
|-------|-------|-----------------|
| Routes | 3 pages + layout | Screen composition, tab wiring |
| Components | ~34 across 7 dirs | Rendering, intent emission |
| Orchestrators | 13 | Async workflows, lifecycle transitions |
| Stores | ~20 | Durable frontend state, derived values |
| Utils | ~13 | Normalization, formatting, serialization |
| API | 8 | Tauri IPC bindings |
| Backend commands | 60+ across 7 modules | IPC boundary, error translation |
| Backend domain | ~15 modules | Node registry, proxy actors, layout persistence |
| lcc-rs | ~20 modules | Protocol encoding, transport, CDI parsing |

Governing docs: `product/architecture/code-placement-and-ownership.md`, `product/architecture/frontend-boundaries.md`

---

## Routes (`app/src/routes/`)

| File | Purpose | Test |
|------|---------|------|
| `+page.svelte` | Main app page; tabs for layout/discovery/config/traffic | `page.route.test.ts` |
| `+layout.svelte` | Root layout wrapper | — |
| `+layout.ts` | Disables SSR (SPA-only for Tauri) | — |
| `config/+page.svelte` | Config editor; renders ConfigSidebar + ElementCardDeck | — |
| `traffic/+page.svelte` | Live CAN traffic monitor | — |

---

## Components (`app/src/lib/components/`)

### Bowtie/ — Event-centric connection UI
| File | Purpose | Test |
|------|---------|------|
| `BowtieCard.svelte` | Bowtie card with producer/consumer columns | `BowtieCard.test.ts` |
| `BowtieCatalogPanel.svelte` | Full catalog panel with editable metadata | `BowtieCatalogPanel.test.ts` |
| `ElementEntry.svelte` | Event slot entry (producer or consumer) | — |
| `ElementPicker.svelte` | Searchable node/slot picker | `ElementPicker.test.ts` |
| `PickerTreeNode.svelte` | Tree node in picker hierarchy | — |
| `EmptyState.svelte` | Empty catalog prompt | `EmptyState.test.ts` |
| `AddElementDialog.svelte` | Add node/slot to existing bowtie | — |
| `NewConnectionDialog.svelte` | Create new bowtie connection | — |
| `ConnectorArrow.svelte` | Visual arrow connecting producers to consumers | — |
| `RoleClassifyPrompt.svelte` | Resolve ambiguous event roles | — |

### ConfigSidebar/ — Node/section navigation
| File | Purpose | Test |
|------|---------|------|
| `ConfigSidebar.svelte` | Left-side nav; nodes and CDI segments | `ConfigSidebar.test.ts` |
| `configSidebarPresenter.ts` | Presenter logic for sidebar state | `configSidebarPresenter.test.ts` |
| `NodeEntry.svelte` | Clickable node with status badge | `NodeEntry.test.ts` |
| `SegmentEntry.svelte` | Clickable CDI segment entry | `SegmentEntry.test.ts` |
| `ConnectorSlotSelector.svelte` | Daughterboard slot dropdown | — |

### ElementCardDeck/ — Config value editing
| File | Purpose | Test |
|------|---------|------|
| `ElementCardDeck.svelte` | Container for top-level group cards | `ElementCardDeck.test.ts` |
| `ElementCard.svelte` | Single CDI group card | `ElementCard.test.ts` |
| `SegmentView.svelte` | All cards in a segment | `SegmentView.test.ts` |
| `TreeLeafRow.svelte` | Single config field row | `TreeLeafRow.test.ts`, `TreeLeafRow.offline.test.ts` |
| `TreeGroupAccordion.svelte` | Accordion for replicated groups | `TreeGroupAccordion.test.ts` |
| `SubGroupAccordion.svelte` | Nested non-root group accordion | — |
| `EventSlotRow.svelte` | Event ID slot field | — |
| `FieldRow.svelte` | Generic field row wrapper | — |
| `SaveControls.svelte` | Save/discard buttons for batch writes | `SaveControls.test.ts`, `saveControlsPresenter.test.ts` |

### Sync/ — Layout reconciliation
| File | Purpose | Test |
|------|---------|------|
| `SyncPanel.svelte` | Modal for resolving offline/online mismatches | `SyncPanel.display.test.ts`, `SyncPanel.lifecycle.test.ts` |
| `ConflictRow.svelte` | Conflict row with resolution controls | — |
| `CleanSummarySection.svelte` | Summary of clean (uncontended) rows | — |

### PillSelector/ — Instance selector
| File | Purpose | Test |
|------|---------|------|
| `PillSelector.svelte` | Dropdown pill for replicated group instances | `PillSelector.test.ts` |

### Layout/ — Offline mode indicators
| File | Purpose | Test |
|------|---------|------|
| `OfflineBanner.svelte` | Offline status banner with sync trigger | — |
| `MissingCaptureBadge.svelte` | Badge for missing snapshot values | — |

### Root-level components
| File | Purpose | Test |
|------|---------|------|
| `TrafficMonitor.svelte` | Live traffic frame display | — |
| `NodeList.svelte` | Discovered nodes with CDI viewer access | — |
| `ErrorDialog.svelte` | Error modal with Escape-to-close | — |
| `DiscardConfirmDialog.svelte` | Confirm discard of unsaved edits | — |
| `DiscoveryProgressModal.svelte` | Progress during discovery phases (reading, building-catalog, complete, cancelled) | — |
| `RefreshButton.svelte` | Refresh discovered nodes | — |
| `CdiDownloadDialog.svelte` | CDI download prompt | — |
| `CdiXmlViewer.svelte` | Syntax-highlighted CDI XML viewer | — |

---

## Orchestrators (`app/src/lib/orchestration/`)

| File | Purpose | Test |
|------|---------|------|
| `discoveryOrchestrator.ts` | Node discovery workflow: probe → SNIP/PIP → publish | `discoveryOrchestrator.test.ts` |
| `configReadOrchestrator.ts` | Per-node CDI read preflight and eligibility checks | `configReadOrchestrator.test.ts` |
| `configReadSessionOrchestrator.ts` | Multi-node config read session lifecycle (cancel/phase) | `configReadSessionOrchestrator.test.ts` |
| `configDraftOrchestrator.ts` | Mirrors config drafts to backend IPC or offline persistence | — |
| `cdiDialogOrchestrator.ts` | CDI download/cache/redownload state machine | `cdiDialogOrchestrator.test.ts` |
| `connectorSelectionOrchestrator.ts` | Connector slot selection + compatibility recompute | `connectorSelectionOrchestrator.test.ts` |
| `offlineLayoutOrchestrator.ts` | Offline layout hydration, snapshot replay, and layout-transition resets (sidebar, nodes, trees, metadata). Three reset functions: `resetLayoutStateForNoLayout` (full teardown), `resetFreshLiveSessionState` (live-session guard), `openOfflineLayoutWithReplay` (clears sidebar before hydrating new layout) | `offlineLayoutOrchestrator.test.ts` |
| `saveLayoutOrchestrator.ts` | Full save lifecycle: flush pending → persist layout → rebuild catalog → update context & partial nodes → clear metadata → mark clean | `saveLayoutOrchestrator.test.ts` |
| `syncSessionOrchestrator.ts` | Sync session lifecycle: classify → mode → reconcile | `syncSessionOrchestrator.test.ts` |
| `syncApplyOrchestrator.ts` | Post-apply reconciliation: rebuild offline trees | `syncApplyOrchestrator.test.ts` |
| `syncPanelViewOrchestrator.ts` | Sync panel user interactions (mode, deselect, apply) | `syncPanelViewOrchestrator.test.ts` |
| `lifecycleTransitionMatrix.ts` | App lifecycle transition decision logic | `lifecycleTransitionMatrix.test.ts` |
| `unsavedChangesGuard.ts` | Navigation guard for unsaved edits | `unsavedChangesGuard.test.ts` |

---

## Stores (`app/src/lib/stores/`)

| File | Purpose | Test |
|------|---------|------|
| `bowties.svelte.ts` | `bowtieCatalogStore` singleton + `buildEffectiveBowtiePreview()` (the catalog×tree×metadata×layout merge). The merge is consumed only by `$lib/layout/effectiveLayoutStore`; components do not import from here directly. | `bowties.svelte.test.ts` (tests exercise the merge through `effectiveLayoutStore`) |
| `bowtieMetadata.svelte.ts` | Pending bowtie name/tag/role edits; `collectDeltas()` converts edits to `LayoutEditDelta[]` for save | `bowtieMetadata.svelte.test.ts` |
| `nodeTree.svelte.ts` | Unified node config tree (CDI + addresses + values) | `nodeTree.store.test.ts` |
| `configChanges.svelte.ts` | Layered change state (draft/offlinePending/baseline) | `configChanges.test.ts` |
| `configEditor.svelte.ts` | Entry point for user-initiated config edits | `configEditor.test.ts` |
| `configFocus.svelte.ts` | Navigation request: bowtie → config field | `configFocus.test.ts` |
| `configReadStatus.ts` | Tracks nodes with successful config reads | `configReadStatus.test.ts` |
| `layout.svelte.ts` | Layout file state: open/save/save-as/recent; `hydrateFromBackend()` sets layout from backend response | `layout.svelte.test.ts` |
| `layoutOpenLifecycle.ts` | Phase machine for layout open (opening→hydrating→ready) | — |
| `offlineChanges.svelte.ts` | Offline change row tracking (persisted/draft layers) | `offlineChanges.store.test.ts` |
| `syncPanel.svelte.ts` | Sync session state: conflict/clean row tracking | `syncPanel.store.test.ts` |
| `connectorSelections.svelte.ts` | Connector slot selections per-node | — |
| `bowtieFocus.svelte.ts` | Currently focused bowtie card (keyboard nav) | — |
| `connectorSlotFocus.svelte.ts` | Focused connector slot per-node | — |
| `connectionRequest.svelte.ts` | Cross-tab connection request (config→bowtie) | — |
| `changeTracker.svelte.ts` | Unified "unsaved changes" snapshot for save controls | `changeTracker.svelte.test.ts` |
| `nodes.ts` | Global discovered nodes list (Svelte 5 runes) | — |
| `nodeInfo.ts` | nodeId → DiscoveredNode map for display-name resolution | — |
| `pillSelection.ts` | Replicated group instance selections | `pillSelection.test.ts` |
| `traffic.ts` | Live traffic message stream | — |

---

## Layout facade (`app/src/lib/layout/`)

**Public import surface for all layout state reads/writes (ADR-0004).** Components and routes should import only from `$lib/layout`; the underlying stores (`bowtieCatalogStore`, `bowtieMetadataStore`, `configChangesStore`, `layoutStore`) are internal collaborators of the facade and its orchestrator.

| File | Purpose | Test |
|------|---------|------|
| `index.ts` | Public facade. Re-exports `effectiveLayoutStore`, `bowtieCatalogStore`, `makeValueResolver`, `saveLayoutOrchestrated` + types, and the edit-recording commands (`recordBowtieDeletion`, `recordRoleClassification`, `recordConfigDraft`). | — |
| `effectiveLayoutStore.svelte.ts` | Single read model that merges all edit layers into the user-visible view. `preview` / `effectiveBowties` (catalog × tree × metadata × layout, with `hasPendingDeletion` filter); `effectiveRole(nodeId, leaf)` (pending classify → catalog → leaf baseline); `effectiveValue(nodeId, leaf)` (draft override → leaf baseline); `slotsByRole(nodeId, role)`; `isSlotFree(nodeId, leaf)`; `usedInMap`. Composes `buildEffectiveBowtiePreview()` from `bowties.svelte.ts`. | `effectiveLayoutStore.svelte.test.ts` |

### Layout facade conventions

- **Single import surface.** New components MUST import layout reads via `$lib/layout`. Direct imports from `$lib/stores/bowties.svelte`, `$lib/stores/bowtieMetadata.svelte`, `$lib/stores/configChanges.svelte`, or `$lib/stores/layout.svelte` from components/routes are a code smell that should be reviewed against ADR-0004.
- **Single merge derivation.** `buildEffectiveBowtiePreview()` in `bowties.svelte.ts` is the only place the catalog×tree×metadata×layout merge happens. The previous fast/slow path branch and `EditableBowtiePreviewStore` class are gone; collapsing them was the structural fix for the "blank diagram during save" bug (ADR-0004 / spec 013 S2c).
- **Pending deletions are facade-level.** The merge function does not consult `bowtieMetadataStore.hasPendingDeletion`; the facade applies that filter on top so the in-class merge remains pure with respect to deletion intent.

---

## Utils (`app/src/lib/utils/`)

| File | Purpose | Test |
|------|---------|------|
| `nodeId.ts` | Node ID normalization: dotted-hex ↔ canonical | — |
| `nodeDisplayName.ts` | Display name resolution via fallback chain | — |
| `formatters.ts` | Config value display formatting (int/string/eventId/float) | `formatters.test.ts` |
| `serialize.ts` | Serialize TreeConfigValue to raw bytes for writes | `serialize.test.ts` |
| `eventIds.ts` | Event ID placeholder detection; fresh event ID generation | — |
| `editKey.ts` | Canonical edit key construction: `nodeId:space:address` | `editKey.test.ts` |
| `displayResolution.ts` | **INTERNAL** (ADR-0004) — leaf-level resolution primitives consumed only by `$lib/layout` and the closely-related structural helpers. `resolveValue` waterfalls draft → offlinePending → baseline; `resolveRole` waterfalls pending edit → saved layout → catalog → CDI baseline; `makeValueResolver(nodeId)` is re-exported from `$lib/layout` for `buildElementLabel`/`getInstanceDisplayName`. | `displayResolution.test.ts` |
| `connectorConstraints.ts` | Evaluate connector slot constraints | `connectorConstraints.test.ts` |
| `connectorLeafDecision.ts` | Leaf value compatibility under slot constraints | `connectorLeafDecision.test.ts` |
| `connectorSlotSelectors.ts` | View model for connector slot selector UI | `connectorSlotSelectors.test.ts` |
| `cardTitle.ts` | Card title from CDI group name + user names | `cardTitle.test.ts` |
| `treeLeafViewState.ts` | Display state for tree leaf rows (offline, compatibility) | `treeLeafViewState.test.ts` |
| `treeConfigValuePersistence.ts` | TreeConfigValue ↔ offline-stored string format | — |
| `layoutPath.ts` | Layout file path utilities: `normalizeLayoutTitle` strips extensions to get display name | `layoutPath.test.ts` |
| `xmlFormatter.ts` | Pretty-print XML for CDI viewer | — |

---

## API Layer (`app/src/lib/api/`)

| File | Purpose |
|------|---------|
| `tauri.ts` | Low-level Tauri IPC bindings (discovery, SNIP, PIP, registration) |
| `cdi.ts` | CDI download/cache operations |
| `config.ts` | Config read/write IPC (setModifiedValue, writeModifiedValues, discard) |
| `bowties.ts` | Bowtie catalog building and layout persistence |
| `layout.ts` | Layout file open/close, offline snapshot hydration |
| `sync.ts` | Sync session and offline change reconciliation IPC |
| `connectorProfiles.ts` | Connector profile queries, slot selection, compatibility preview |
| `types.ts` | Shared API type definitions |

---

## Backend Commands (`app/src-tauri/src/commands/`)

| Module | Key Commands | Purpose |
|--------|-------------|---------|
| `connection.rs` | `list_serial_ports`, `load_connection_prefs`, `save_connection_prefs` | Connection management |
| `discovery.rs` | `discover_nodes`, `probe_nodes`, `register_node`, `query_snip_*`, `query_pip_*`, `verify_node_status`, `refresh_all_nodes` | Node discovery and metadata |
| `bowties.rs` | `query_event_roles`, `build_bowtie_catalog_command`, `get_bowties`, `set_bowtie_metadata`, `load_layout`, `save_layout`, `*_recent_layout` | Bowtie catalog and layout files |
| `cdi.rs` | `download_cdi`, `get_cdi_xml`, `get_cdi_structure`, `read_config_value`, `read_all_config_values`, `write_config_value`, `set_modified_value`, `write_modified_values`, `discard_modified_values`, `trigger_action`, `cancel_*` | CDI download, config read/write |
| `layout_capture.rs` | `capture_layout_snapshot`, `save_layout_directory`, `open_layout_directory`, `close_layout`, `create_new_layout_capture`, `build_offline_node_tree`, `save_layout_with_bus_writes` | Layout snapshot persistence; save commands accept `LayoutEditDelta[]` and return persisted `LayoutFile` |
| `sync_panel.rs` | `set_offline_change`, `revert_offline_change`, `list_offline_changes` | Offline change staging |
| `connector_profiles.rs` | `get_connector_profile`, `get_connector_selections`, `put_connector_selections`, `preview_connector_compatibility` | Connector profile and slot constraints |
| `diagnostics.rs` | `get_diagnostic_report` | Ring-buffer logs and troubleshooting |

---

## Backend Domain Modules (`app/src-tauri/src/`)

| Module | Purpose | Test |
|--------|---------|------|
| `lib.rs` | Entry point: connection init, state setup, command registration | — |
| `main.rs` | Tauri desktop app launcher | — |
| `state.rs` | Authoritative app state: connection, registry, caches | inline `#[cfg(test)]` |
| `node_registry.rs` | Thread-safe NodeID → NodeProxyHandle map | inline `#[cfg(test)]` |
| `node_proxy.rs` | Per-node actor: SNIP, PIP, CDI, config values via mailbox | inline `#[cfg(test)]` |
| `node_tree.rs` | Unified config tree: CDI + addresses + values + roles | inline `#[cfg(test)]` |
| `diagnostics.rs` | Ring-buffer logging (`bwlog!`), diagnostic stats | — |
| `events/router.rs` | Event broadcast: transport frames → Tauri events | inline `#[cfg(test)]` |
| `traffic/mod.rs` | Message decoding for traffic monitor display | — |
| `menu.rs` | Desktop app menu | — |
| `layout/mod.rs` | **Deep module** (ADR-0005). Sole owner of the layout companion-directory file structure. Public intent-shaped API: `save_capture`, `read_capture`, `read_node_snapshot`, `update_offline_changes`, `update_node_snapshots`, `resolve_cdi_xml_for_snapshot`. Code outside `layout/` must use this API — never compute companion-dir paths, node-file names, or write YAML directly. | inline `#[cfg(test)]` |
| `layout/types.rs` | YAML data structures (BowtieMetadata, RoleClassification, LayoutEditDelta) + `apply_layout_deltas` | `layout::types::tests` |
| `layout/io.rs` | Internal: companion-dir / node-file path derivation, full capture read/write, CDI XML resolution. Builds `journal::SavePlan` for every mutation and calls `journal::execute`; `read_capture` runs `journal::recover_if_needed` first and surfaces `recovery_occurred`. All companion-dir helpers are `pub(crate)`; only the single-file `load_file`/`save_file` and `cdi_cache_path` remain `pub`. | inline `#[cfg(test)]` |
| `layout/journal.rs` | **Write-ahead journal (ADR-0006).** Sole mutation primitive for the companion directory: in-place file writes guarded by `.save-in-progress` marker + `.restore/` backup mirror. `execute(SavePlan)` and `recover_if_needed(base_file)` are `pub(crate)` and called only from `layout/io.rs` and `layout/mod.rs`. Test seams `FAIL_AFTER_BACKUP` / `FAIL_MID_WRITES` are thread-local `#[cfg(test)]` `Cell<bool>`s. | inline `#[cfg(test)]` (8 tests) |
| `layout/manifest.rs` | Layout manifest tracking | inline `#[cfg(test)]` |
| `layout/node_snapshot.rs` | Node config snapshot for offline use | inline `#[cfg(test)]` |
| `layout/offline_changes.rs` | Offline change staging (config diffs) | inline `#[cfg(test)]` |
| `layout/serde_node_id.rs` | Custom serde for NodeID (dotted-hex in YAML) | — |
| `profile/mod.rs` | Profile loading + tree annotation | — |
| `profile/types.rs` | Structure profile types (event roles, relevance, connectors) | — |
| `profile/loader.rs` | `.profile.yaml` and `.connector.yaml` loading | inline `#[cfg(test)]` |
| `profile/resolver.rs` | Profile conditional resolution (firmware checks) | inline `#[cfg(test)]` |

---

## lcc-rs Protocol Library (`lcc-rs/src/`)

| Module | Purpose | Test |
|--------|---------|------|
| `lib.rs` | Public crate API; re-exports types and protocol structs | — |
| `types.rs` | Core types: NodeID, EventID, NodeAlias, SNIPData, ProtocolFlags | inline `#[cfg(test)]` |
| `constants.rs` | Protocol constants (timeouts, buffer sizes) | — |
| `discovery.rs` | LccConnection: protocol orchestrator, node probe, BatchReader | inline `#[cfg(test)]` |
| `alias_allocation.rs` | CID7→CID4 alias allocation, conflict detection per S-9.7.2.1 | inline `#[cfg(test)]` |
| `snip.rs` | SNIP query via datagram: manufacturer/model/version retrieval | inline `#[cfg(test)]` |
| `pip.rs` | Protocol Identification Protocol capability query | inline `#[cfg(test)]` |
| `transport_actor.rs` | Dual-path frame I/O actor: mpsc queue + direct send | inline `#[cfg(test)]` |
| `dispatcher.rs` | Inbound frame routing by MTI & alias | inline `#[cfg(test)]` |
| `cdi/mod.rs` | Cdi struct and DataElement enum | — |
| `cdi/parser.rs` | CDI XML → Cdi tree parsing | inline `#[cfg(test)]` |
| `cdi/hierarchy.rs` | Tree traversal: `walk_event_slots()` iterator | inline `#[cfg(test)]` |
| `cdi/role.rs` | Event role classification heuristics | inline `#[cfg(test)]` |
| `protocol/mod.rs` | Protocol facade; re-exports | — |
| `protocol/frame.rs` | GridConnectFrame: CAN header parse/encode | inline `#[cfg(test)]` |
| `protocol/mti.rs` | MTI enum: 60+ message types with encoding | inline `#[cfg(test)]` |
| `protocol/datagram.rs` | DatagramAssembler: multi-frame reassembly | inline `#[cfg(test)]` |
| `protocol/memory_config.rs` | MemoryConfigCmd, address spaces, ReadReply parsing | inline `#[cfg(test)]` |
| `transport/mod.rs` | Transport trait API | — |
| `transport/tcp.rs` | TCP transport with GridConnect framing | inline `#[cfg(test)]` |
| `transport/gridconnect_serial.rs` | Serial transport with GridConnect framing | inline `#[cfg(test)]` |
| `transport/slcan_serial.rs` | Serial transport with SLCAN (Lawicel) framing | inline `#[cfg(test)]` |
| `transport/mock.rs` | Mock transport for unit tests | — |

---

## Integration Boundaries

### Frontend → Backend IPC
- All commands return `Result<T, String>` (error as string)
- State accessed via `tauri::State<'_, AppState>`
- Events emitted: `lcc-node-discovered`, `lcc-message-received`, `lcc-connection-changed`, `cdi-read-complete`, `config-read-progress`

### Backend → lcc-rs API Surface
- `LccConnection` — main protocol orchestrator
- `TransportHandle` — broadcast inbound + mpsc outbound
- Types: `NodeID`, `EventID`, `NodeAlias`, `DiscoveredNode`, `SNIPData`, `ProtocolFlags`
- Protocol: `GridConnectFrame`, `MTI`, `DatagramAssembler`, `MemoryConfigCmd`
- CDI: `Cdi`, `DataElement`, `EventRole`, `classify_event_slot()`, `walk_event_slots()`
- Transport: `LccTransport`, `GridConnectSerialTransport`, `SlcanSerialTransport`

### Lifecycle Ownership Transitions
- Discovery: lcc-rs probes → backend creates NodeProxy actors → frontend receives events
- Config read: frontend orchestrator → backend batch reads → lcc-rs memory config datagrams
- Layout open: frontend orchestrator → backend hydrates snapshots → stores populated
- Sync apply: frontend orchestrator → backend writes changes → lcc-rs memory config

---

## Shared Conventions

### Key Generation
**Canonical implementation:** `app/src/lib/utils/editKey.ts`
- Format: `"${normalizedNodeId}:${space}:${address}"` (e.g., `"050201020300:253:100"`)
- `editKeyForLeaf(nodeId, space, address)` — single source of truth
- `parseEditKey(key)` — inverse
- Used by: `configChanges.svelte.ts`, `offlineChanges.svelte.ts`

### Normalization Rules
**Canonical implementation:** `app/src/lib/utils/nodeId.ts` + `lcc-rs/src/types.rs`
- Node ID canonical: uppercase, no dots (`"050201020000FF"`)
- Node ID display: dotted-hex (`"05.02.01.02.00.FF"`)
- `normalizeNodeId()` — removes dots, uppercases
- `formatNodeId()` — bytes → dotted display
- Anti-pattern: inline `.replace(/\./g, '')` without uppercase → lookup failures
- Governing doc: `product/architecture/naming-and-normalization.md`

### Formatting / Display
**Canonical implementation:** `app/src/lib/utils/formatters.ts`
- `formatConfigValue()` — Int→string, String→as-is, EventId→dotted hex, Float→.toFixed(2)
- `formatEventId()` — 8-byte array → dotted uppercase hex
- `formatTreeConfigValue()` — resolves int enums to labels via mapEntries

### Fallback Chains
**Canonical implementation:** `app/src/lib/utils/nodeDisplayName.ts`
- Display name: user_name → manufacturer+model → model → Node ID hex
- `resolveNodeDisplayName(nodeId, node)` — single entry point
- Anti-pattern: inline fallback `node.user_name || nodeId` — use the helper
- Governing doc: `product/architecture/naming-and-normalization.md`

### CDI File Naming
**Canonical saving:** `layout/io.rs` (layout dir), `commands/cdi.rs` (global cache)
- Cache key: `{sanitize(manufacturer)}_{sanitize(model)}_{sanitize(version)}`
- Global cache filename: `{cache_key}.cdi.xml` (in `app_data_dir/cdi_cache/`)
- Layout directory filename: `{cache_key}.cdi.xml` (in `layout.d/cdi/`)
- Legacy layout files may use `{cache_key}.xml` (without `.cdi` prefix) — readers fall back to this
- `cdi_cache_path_for_snapshot()` in `layout_capture.rs` — global cache path builder
- `get_cdi_path_for_snapshot()` in `layout/io.rs` — layout dir path builder (checks `.cdi.xml` then `.xml`)
- Anti-pattern: constructing layout CDI filenames from sanitized SNIP fields instead of `cache_key` — they use different extensions

### Config Change Layer Resolution
**Canonical implementation:** `app/src/lib/stores/configChanges.svelte.ts`
- `visibleValue(key)` — full resolution: draft → offlinePending → baseline (tree walk)
- `overrideValue(key)` — fast resolution: draft → offlinePending only (no tree walk)
- `_findLeaf` uses a `WeakMap`-backed address index (`addressIndexCache`) so repeated `visibleValue` calls into the same tree are O(1) instead of O(N) recursive walks. The cache is keyed by tree object identity; replaced trees get fresh indexes on next access.
- Anti-pattern: calling `visibleValue()` in bulk scans over `collectEventIdLeaves()` — the baseline tree walk is O(N) per leaf and the caller already has the leaf value.

### Bowtie Entry Enrichment
**Canonical implementation:** `app/src/lib/stores/bowties.svelte.ts`
- `enrichCardEntries(card)` — enriches all three entry arrays (producers, consumers, ambiguous) with `element_label` in one call.
- `enrichEntryLabel(entry)` — computes element_label from the live tree, falling back to `element_path.join('.')`.
- All preview build paths must use `enrichCardEntries` instead of mapping arrays individually — prevents the bug class where a new array is added or forgotten.
- Anti-pattern: calling `enrichEntryLabel` on producers and consumers but not ambiguous entries.

## Skills

Skills that own specific workflow phases. Located in `.github/skills/`.

### design
**Files:** `SKILL.md`, `ASSESSMENT.md`, `SLICING.md`
**Purpose:** Feature-scoped architecture assessment and vertical slice planning. Runs after `/plan`, before `/slices`. Evaluates module depth, placement compliance, ADR compliance, and defines vertical slices with HITL/AFK labels.
**References:** `improve-codebase-architecture/LANGUAGE.md`, `grill-with-docs/ADR-FORMAT.md`, `grill-with-docs/GLOSSARY-FORMAT.md`

### slices
**Files:** `SKILL.md`, `SLICE-FORMAT.md`
**Purpose:** Generates slice-organized task file (`specs/<feature>/slices.md`) for cross-session progress tracking. Each slice has tasks with checkboxes, HITL/AFK labels, blocked-by relationships, and acceptance criteria.
**Output:** `specs/<feature>/slices.md`

### build
**Files:** `SKILL.md`, `tdd.md`, `deep-modules.md`, `interface-design.md`, `mocking.md`, `tests.md`
**Purpose:** TDD-first vertical implementation with multi-session support. Implements one slice at a time using red-green-refactor. AI judges session capacity and stops at slice boundaries. Includes pre-implementation checks and post-implementation enrichment.
**TDD methodology adapted from:** Matt Pocock's TDD skill
