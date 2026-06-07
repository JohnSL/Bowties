# Module Ownership

## Summary

| Layer | Count | Key shared logic |
|-------|-------|-----------------|
| Routes | 3 pages + layout | Screen composition, tab wiring |
| Components | ~37 across 8 dirs | Rendering, intent emission |
| Orchestrators | 14 | Async workflows, lifecycle transitions |
| Stores | ~21 | Durable frontend state, derived values |
| Utils | ~13 | Normalization, formatting, serialization |
| API | 8 | Tauri IPC bindings |
| bowties-core | 19 modules | Node tree, layout persistence, profile, registry, placeholder, bowtie catalog |
| Backend (Tauri shell) | ~15 modules | IPC commands, state, placeholder factory, events |
| lcc-rs | ~20 modules | Protocol encoding, transport, CDI parsing |

Governing docs: `product/architecture/code-placement-and-ownership.md`, `product/architecture/frontend-boundaries.md`

---

## Routes (`app/src/routes/`)

| File | Purpose | Test |
|------|---------|------|
| `+page.svelte` | Main app page; tabs for layout/discovery/config/traffic. **Picker gate (Spec 013 / S6):** when no layout is active (and bootstrap is finished and no open-in-progress), `+page.svelte` renders `LayoutPicker` instead of the toolbar + main-content; disconnecting does NOT re-show the picker (layout stays active). | `page.route.test.ts` |
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
| `SaveControls.svelte` | Save/discard buttons for batch writes. **Spec 014 / Step 7 Option H:** thin delegate — only owns its local `saveProgress` UI state machine and invokes `onSave` / `onSaveAs` props. All cleanup wiring (drafts, metadata, mark-clean, offline reload) is owned by `saveLayoutOrchestrator` via callbacks composed in `+page.svelte`. The component MUST NOT reach into `layoutStore`, `bowtieMetadataStore`, `configChangesStore`, or `offlineChangesStore` directly. | `SaveControls.test.ts`, `saveControlsPresenter.test.ts` |

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

### LayoutPicker/ — Startup gate (Spec 013 / S6)
| File | Purpose | Test |
|------|---------|------|
| `LayoutPicker.svelte` | Fullscreen startup picker. Renders known-layout list + `New Layout` + `Browse…`. Emits `onOpen` / `onBrowse` / `onCreate` / `onRemove` — no async work owned here. | — |
| `LayoutEntry.svelte` | Single known-layout row with name, path, locale-formatted `lastOpened`, and a Remove (✕) action. | — |
| `NewLayoutDialog.svelte` | Modal for creating a new layout. Folder picker via `@tauri-apps/plugin-dialog`, sanitised filename, preview of derived `<dir>/<name>.layout` path. | — |

### Root-level components
| File | Purpose | Test |
|------|---------|------|
| `TrafficMonitor.svelte` | Live traffic frame display | — |
| `NodeList.svelte` | Discovered nodes with CDI viewer access | — |
| `ErrorDialog.svelte` | Error modal with Escape-to-close | — |
| `DiscardConfirmDialog.svelte` | Confirm discard of unsaved edits | — |
| `AddBoardDialog.svelte` | Spec 014 / S8: modal picker for adding a placeholder board to the active offline layout. Lists bundled board-model profiles via `listBundledProfiles()` and calls `placeholderBoardOrchestrator.addPlaceholderBoard` on submit. Entry point is the native `File → Add Placeholder Board…` menu item (gated on `offlineActive && !busy`); mounted from `+page.svelte` behind a `showAddBoardDialog` state flag. | — |
| `DiscoveryProgressModal.svelte` | Progress during discovery phases (reading, building-catalog, complete, cancelled) | — |
| `SaveProgressDialog.svelte` | Modal during the three-phase save flow; reads phase + per-field bus-write counters from `saveProgressStore`. Auto-dismisses on `complete` / `error` after 2 s. | — |
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
| `placeholderBoardOrchestrator.ts` | Spec 014 / S8.10: placeholder board lifecycle. Calls `addPlaceholderBoardIpc(profileStem)` (backend factory mints UUID, builds proxy, registers in backend), then reads tree via `getNodeTree` IPC and seeds `nodeRoster`. Delete gates on caller-supplied `confirm: () => Promise<boolean>` (FR-017a). UUID minting moved to backend factory in S8.10. | `placeholderBoardOrchestrator.test.ts` |
| `offlineLayoutOrchestrator.ts` | Offline layout hydration and snapshot replay. Exposes `buildOfflineDiscoveryNodes`, `buildOfflineTreesFromSnapshots`, `rehydrateOfflineStateFromSnapshots`, `restoreRecentOfflineLayout`, `openOfflineLayoutWithReplay`. The legacy standalone `resetLayoutStateForNoLayout` / `resetFreshLiveSessionState` exports remain for back-compat but new code MUST go through `layoutLifecycleOrchestrator` (ADR-0011) — the route's two wrappers now delegate. | `offlineLayoutOrchestrator.test.ts` |
| `layoutLifecycleOrchestrator.ts` | **Single owner of in-memory layout-lifecycle resets** (ADR-0011, Step 11). Three named entry points: `resetForNewLayout({connected, reprobeLiveNodes, probeForNodes, afterReset})` (full teardown — close, discard, new-layout, no-layout recovery; calls `nodeRoster.clearLayoutScope()` so placeholders do NOT bleed across layouts — R7 fix), `resetForFreshLiveSession()` (disconnect/reconnect within the same live session; preserves placeholders because they are layout-scoped), and `closeLayout({activeMode, closeLayoutIpc, clearRecentLayout, ...})` which sequences backend close → frontend reset. Imports stores directly (not callback bags) so a new `effectiveNodeStore` input cannot be added without also extending the reset path. | `layoutLifecycleOrchestrator.test.ts` |
| `saveLayoutOrchestrator.ts` | Full save lifecycle: flush pending → persist layout → rebuild catalog → update context & partial nodes → clear metadata → mark clean. **S8.11:** accepts `inMemorySnapshotKeys?: string[]` (unified real-node + placeholder keys) and appends `{type:'addNode', nodeKey}` deltas; surfaces `persistedNodeIds` into the updated `ActiveLayoutContext.layoutNodeIds`. | `saveLayoutOrchestrator.test.ts` |
| `startupOrchestrator.ts` | Layout picker lifecycle (Spec 013 / S6): `loadKnownLayouts`, `openLayoutFromRegistry` (open existing → register), `createNewLayout` (capture → save → reopen → register), `removeKnownLayout`, `deriveLayoutNameFromPath`. Pure functions with injected callbacks — no direct store or IPC imports. Registry refresh failures are non-fatal (logged via `onError`). | `startupOrchestrator.test.ts` (14 tests) |
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
| `configChanges.svelte.ts` | Layered change state (draft/offlinePending/baseline). S8.12: `commitForSave()` replaces `clearNonPlaceholderDrafts()` — uniform draft clearing post-save. | `configChanges.test.ts` |
| `configEditor.svelte.ts` | Entry point for user-initiated config edits | `configEditor.test.ts` |
| `configFocus.svelte.ts` | Navigation request: bowtie → config field | `configFocus.test.ts` |
| `configReadStatus.ts` | Tracks nodes with successful config reads | `configReadStatus.test.ts` |
| `layout.svelte.ts` | Layout file state: open/save/save-as/recent; `hydrateFromBackend()` sets layout from backend response. **ADR-0011:** the per-node `isDirty` projection (fully-captured live nodes + persistability) is owned by `effectiveNodeStore`. `layoutStore.isDirty` carries only the LayoutFile-struct edits (`_hasFileEdits`); aggregate dirty signal lives on `effectiveNodeStore.isDirty`. Consumers (`SaveControls` presenter, top-bar dot, unsaved-changes count) MUST read through the facade, not directly off `layoutStore.isDirty`. | `layout.svelte.test.ts` |
| `layoutOpenLifecycle.ts` | Phase machine for layout open (opening→hydrating→ready) | — |
| `offlineChanges.svelte.ts` | Offline change row tracking (persisted/draft layers) | `offlineChanges.store.test.ts` |
| `syncPanel.svelte.ts` | Sync session state: conflict/clean row tracking | `syncPanel.store.test.ts` |
| `connectorSelections.svelte.ts` | Connector slot selections per-node. **Spec 014 / S6:** writes through `set_node_mode_selection` IPC (`$lib/api/layout`) — the unified `node_mode_selections` seam on `LayoutFile`. Identity mapping for Tower-LCC: slot_id ≡ mode_id, daughterboardId ≡ variantId. `fromNodeModeSelections(nodeId, profile, selections)` projects the unified map back onto the slot list for display. IPC save failures are logged via `console.warn` (not stored in the node-level error channel, which is reserved for "can't load this connector"). Selector changes are wrapped by `connectorSelectionOrchestrator.applyConnectorSelectionChange` which awaits `nodeTreeStore.refreshTree(nodeId)` so the backend re-runs `annotate_tree` with the new selection and the rendered tree re-shapes. | `connectorSelections.s6.test.ts` |
| `placeholderBoards.svelte.ts` | Spec 014 / S8: read-only projection of `layoutStore.layout.placeholderBoards`. Exposes `list()` (sorted by name, case-insensitive), `get(id)`, `exists(id)`, `displayName(id)`. Owns no state; all mutations go through `placeholderBoardOrchestrator`. Consumed by `ConfigSidebar.svelte` to render placeholder entries with the `isPlaceholder` badge. | `placeholderBoards.svelte.test.ts` |
| `bowtieFocus.svelte.ts` | Currently focused bowtie card (keyboard nav) | — |
| `connectorSlotFocus.svelte.ts` | Focused connector slot per-node | — |
| `connectionRequest.svelte.ts` | Cross-tab connection request (config→bowtie) | — |
| `changeTracker.svelte.ts` | Unified "unsaved changes" snapshot for save controls | `changeTracker.svelte.test.ts` |
| `saveProgress.svelte.ts` | Phase tracker for the three-phase save flow. Listens to `save-progress` Tauri events (`saving-layout` / `writing-config` / `reconciling` / `complete`) emitted by `save_layout_with_bus_writes` + per-iteration events from `write_modified_values`. Also exposes `begin()` / `apply()` / `fail()` / `reset()` so the offline-save path (driven by `+page.svelte`) can flip phases without backend events. Consumed by `SaveProgressDialog.svelte` and by `isMenuBusy()` to gate concurrent saves. | `saveProgress.svelte.test.ts` |
| `nodes.ts` | Global discovered nodes list (Svelte 5 runes) | — |
| `nodeInfo.ts` | nodeId → DiscoveredNode map for display-name resolution. **Spec 014 / S8.7:** kept as internal backing storage; the canonical single source of truth for "the set of nodes the user sees" is now `nodeRoster.svelte.ts`. New consumers should read from the roster facade. | — |
| `nodeRoster.svelte.ts` | **Spec 014 / S8.7, S8.12:** Unified facade over `nodeInfoStore`, `configReadNodesStore`, `nodeTreeStore`, and an internal `_profileStems` map (S8.12 — previously `inMemoryPlaceholdersStore`, now deleted). Exposes `allEntries` / `liveEntries` / `placeholderEntries` / `liveNodes` / `hasAnyEntries` / `has(nodeKey)` as reactive views, and `upsertLive`, `replaceLiveRoster` (preserves placeholders), `addPlaceholder`, `removePlaceholder`, `markPlaceholdersPersisted`, `setTree`, `markRead`, `clearLayoutScope` as mutators. | `nodeRoster.svelte.test.ts` |
| `pillSelection.ts` | Replicated group instance selections | `pillSelection.test.ts` |
| `traffic.ts` | Live traffic message stream | — |
| `knownLayouts.svelte.ts` | `knownLayoutsStore` singleton — frontend mirror of `known-layouts.json` (Spec 013 / S6). Exposes `entries`, `loaded`, `busy`; setters tolerate undefined backend payloads. Written through by `startupOrchestrator`; read by `LayoutPicker`. | — |

---

## Layout facade (`app/src/lib/layout/`)

**Public import surface for all layout state reads/writes (ADR-0004).** Components and routes should import only from `$lib/layout`; the underlying stores (`bowtieCatalogStore`, `bowtieMetadataStore`, `configChangesStore`, `layoutStore`) are internal collaborators of the facade and its orchestrator.

| File | Purpose | Test |
|------|---------|------|
| `index.ts` | Public facade. Re-exports `effectiveLayoutStore`, `effectiveNodeStore`, `bowtieCatalogStore`, `makeValueResolver`, `saveLayoutOrchestrated` + types, and the edit-recording commands (`recordBowtieDeletion`, `recordRoleClassification`, `recordConfigDraft`). | — |
| `effectiveLayoutStore.svelte.ts` | Single read model that merges all edit layers into the user-visible **value** view. `preview` / `effectiveBowties` (catalog × tree × metadata × layout, with `hasPendingDeletion` filter); `effectiveRole(nodeId, leaf)` (pending classify → catalog → leaf baseline); `effectiveValue(nodeId, leaf)` (draft override → leaf baseline); `slotsByRole(nodeId, role)`; `isSlotFree(nodeId, leaf)`; `usedInMap`. Composes `buildEffectiveBowtiePreview()` from `bowties.svelte.ts`. | `effectiveLayoutStore.svelte.test.ts` |
| `effectiveNodeStore.svelte.ts` | **Per-node** layout facade sibling to `effectiveLayoutStore` (ADR-0011, Step 10). Projects `nodeTreeStore`, `nodeInfoStore`, `configReadNodesStore`, `partialCaptureNodesStore`, `layoutStore.activeContext`, and the edit-layer stores into `nodeOrigin(key)`, `isFullyCaptured(key)`, `isConfigRead(key)`, `isPersistableInLayout(key)` (= `isFullyCaptured ∧ (isConfigRead ∨ placeholder)` — R5 fix), `unsavedInMemoryNodeIds`, `unsavedRemovedNodeIds`, `isDirty` (R6 fix). Reads only — never writes through. The lifecycle reset path is `layoutLifecycleOrchestrator`, which MUST enumerate every store this facade reads. | `effectiveNodeStore.svelte.test.ts` |

### Layout facade conventions

- **Single import surface.** New components MUST import layout reads via `$lib/layout`. Direct imports from `$lib/stores/bowties.svelte`, `$lib/stores/bowtieMetadata.svelte`, `$lib/stores/configChanges.svelte`, or `$lib/stores/layout.svelte` from components/routes are a code smell that should be reviewed against ADR-0004.
- **Single merge derivation.** `buildEffectiveBowtiePreview()` in `bowties.svelte.ts` is the only place the catalog×tree×metadata×layout merge happens. The previous fast/slow path branch and `EditableBowtiePreviewStore` class are gone; collapsing them was the structural fix for the "blank diagram during save" bug (ADR-0004 / spec 013 S2c).
- **Pending deletions are facade-level.** The merge function does not consult `bowtieMetadataStore.hasPendingDeletion`; the facade applies that filter on top so the in-class merge remains pure with respect to deletion intent.

---

## Utils (`app/src/lib/utils/`)

| File | Purpose | Test |
|------|---------|------|
| `nodeId.ts` | Node ID normalization: dotted-hex ↔ canonical | — |
| `nodeKey.ts` | **NodeKey** (Spec 014, ADR-0008, ADR-0010) — branded discriminated union covering live `NodeID` and `placeholder:<uuidv4>`. Exports `type NodeKey = LiveNodeKey | PlaceholderNodeKey`, constructor `nodeKey(input)`, `nodeKeyEquals`, `nodeKeyToString`, `nodeKeyToDisplay`, `toCanonicalNodeKey(input: string | NodeKey | null \| undefined)`, `isPlaceholderInput(input)`, and the transitional shim `NodeKeyInput = string \| NodeKey`. Mirrors the backend prefix predicate `layout::types::is_placeholder` exactly. Consumed by `configChanges.svelte.ts`, `editKey.ts`, and any store that legitimately widens to placeholders. | `nodeKey.test.ts` |
| `nodeRoster.ts` | S8: pure helpers comparing the active layout's saved-node roster against the currently-visible node set — `canonicalizeNodeId`, `computeDiscoveredOnlyNodeIds` (badge predicate, no threshold), `computeUnsavedInMemoryNodeIds` (threshold-gated by full capture — feeds `layoutStore.isDirty` and `addNode` save deltas), `isUnsavedDiscoveredNode`, `isSavedOffBusNode`. Consumed by `+page.svelte` (sidebar badge + dirty signal + save deltas), `configSidebarPresenter` (unsaved-new badge), and any future "off-bus saved" surfaces. | `nodeRoster.test.ts` |
| `nodeDisplayName.ts` | Display name resolution via fallback chain | — |
| `formatters.ts` | Config value display formatting (int/string/eventId/float) | `formatters.test.ts` |
| `serialize.ts` | Serialize TreeConfigValue to raw bytes for writes | `serialize.test.ts` |
| `eventIds.ts` | Event ID placeholder detection; fresh event ID generation | — |
| `uuid.ts` | RFC 4122 v4 UUID generation via `crypto.getRandomValues` (broader Tauri-target compatibility than `crypto.randomUUID`). Single helper `generateUuidV4()`. No longer used by `placeholderBoardOrchestrator` (UUID minting moved to backend factory in S8.10); available for any future v4 UUID need (saved-connection ids, etc.). | — |
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
| `connection.rs` | `list_serial_ports`, `get_layout_connections`, `save_layout_connections` | Per-layout connection registry — stored inside each layout manifest (Spec 013 / S4, S7). The global `$APPDATA/bowties/connections.json` registry was removed in S7; connections now live with the layout they belong to. |
| `startup.rs` | `get_known_layouts`, `add_known_layout`, `remove_known_layout` | Known-layout registry (`$APPDATA/bowties/known-layouts.json`) for the layout picker |
| `discovery.rs` | `discover_nodes`, `probe_nodes`, `register_node`, `query_snip_*`, `query_pip_*`, `verify_node_status`, `refresh_all_nodes` | Node discovery and metadata |
| `bowties.rs` | `query_event_roles`, `build_bowtie_catalog_command`, `get_bowties`, `set_bowtie_metadata`, `load_layout`, `save_layout`, `*_recent_layout`. Re-exports pure catalog functions from `bowties_core::bowtie::catalog`. | Bowtie catalog and layout files |
| `cdi.rs` | `download_cdi`, `get_cdi_xml`, `get_cdi_structure`, `read_config_value`, `read_all_config_values`, `write_config_value`, `set_modified_value`, `write_modified_values`, `discard_modified_values`, `trigger_action`, `cancel_*` | CDI download, config read/write |
| `layout_capture.rs` | `capture_layout_snapshot`, `save_layout_directory`, `open_layout_directory`, `close_layout`, `create_new_layout_capture`, `build_offline_node_tree`, `save_layout_with_bus_writes` | Layout snapshot persistence; save commands accept `LayoutEditDelta[]` and return persisted `LayoutFile` plus (S8) `persistedNodeIds` — the canonical roster after applying any `AddNode` deltas. `save_layout_directory` computes a permitted-node set = previously-persisted ∪ explicitly-added; live handles outside the permitted set are skipped, and previously-saved snapshots for permitted off-bus nodes are carried forward from `prev.node_snapshots`. First-save (`previous == None`) passes all live handles through for backward compatibility. **Manifest reconstruction goes through `layout::manifest::build_save_manifest(previous, layout_id, captured_at, last_saved_at, companion_dir)`** — preserves `connections` / `match_thresholds` / `active_mode` from `previous`, never falls back to `LayoutManifest::new(...)` on re-save (that path silently zeroed per-layout LCC connections; see `resave_preserves_existing_connections_via_build_save_manifest` test in `layout/io.rs`). |
| `sync_panel.rs` | `set_offline_change`, `revert_offline_change`, `list_offline_changes` | Offline change staging |
| `connector_profiles.rs` | `get_connector_profile`, `preview_connector_compatibility` | Connector profile and slot constraints. Selection persistence (`get_connector_selections` / `put_connector_selections`) was removed in Spec 014; the replacement seam is `node_mode_selections` written through `placeholders.rs`. |
| `placeholders.rs` | `add_placeholder_board`, `delete_placeholder_board`, `set_placeholder_config_value`, `set_node_mode_selection`, `rename_placeholder_board` | Spec 014 / S3: thin wrappers that validate input, build a single `LayoutEditDelta`, and delegate to `save_layout_directory` (ADR-0002 delta pipeline). |
| `diagnostics.rs` | `get_diagnostic_report` | Ring-buffer logs and troubleshooting |

---

## Backend Domain Modules

### bowties-core crate (`bowties-core/src/`)

Pure domain logic extracted from `src-tauri` so that tests can run with
`cargo test` (no Tauri DLL dependency). Re-exported by `src-tauri` via
thin shim modules so existing `crate::` paths compile unchanged.

| Module | Purpose | Test |
|--------|---------|------|
| `node_key.rs` | **`NodeKey`** (ADR-0010) — sum type `Live(NodeID) \| Placeholder(Uuid)` with canonical wire form. | inline `#[cfg(test)]` (10) |
| `node_tree.rs` | Unified config tree: CDI + addresses + values + roles. Also owns `NodeRoles` (per-event producer/consumer sets). | inline `#[cfg(test)]` (24+) |
| `node_proxy.rs` | Polymorphic node handle (`NodeProxyHandle` enum: `Live` + `Synthesized`). `LiveNodeProxy` actor, `SynthesizedNodeProxy` passive state holder. | inline `#[cfg(test)]` |
| `node_registry.rs` | Thread-safe `NodeKey → NodeProxyHandle` map. Owns `saved_trees` cache. | inline `#[cfg(test)]` |
| `layout/mod.rs` | **Deep module** (ADR-0005). Sole owner of companion-directory file structure. Public API: `save_capture`, `read_capture`, `read_node_snapshot`, `update_offline_changes`, etc. | inline `#[cfg(test)]` |
| `layout/types.rs` | YAML data structures, `ConnectionConfig`, `LayoutEditDelta`, `apply_layout_deltas`. | inline `#[cfg(test)]` |
| `layout/io.rs` | Companion-dir / node-file path derivation, full capture read/write, CDI XML resolution. | inline `#[cfg(test)]` |
| `layout/journal.rs` | **Write-ahead journal** (ADR-0006). In-place writes guarded by marker + backup. | inline `#[cfg(test)]` (8) |
| `layout/manifest.rs` | Layout manifest, saved connections. | inline `#[cfg(test)]` |
| `layout/known_layouts.rs` | App-level known-layout registry (Spec 013 / S5). | inline `#[cfg(test)]` (7) |
| `layout/node_snapshot.rs` | Node config snapshot for offline use. | inline `#[cfg(test)]` |
| `layout/offline_changes.rs` | Offline change staging (config diffs). | inline `#[cfg(test)]` |
| `layout/serde_node_id.rs` | Custom serde for NodeID (dotted-hex in YAML). | — |
| `profile/mod.rs` | Profile tree annotation, overlay composition, cache types. | inline `#[cfg(test)]` |
| `profile/types.rs` | Structure profile types (event roles, relevance, connectors). | — |
| `profile/resolver.rs` | Profile conditional resolution (firmware checks). | inline `#[cfg(test)]` |
| `placeholder.rs` | **Placeholder factory helpers** — CDI loading, EventId-zero collection, config-value merging, leaf-default population. All pure (no Tauri deps). | inline `#[cfg(test)]` (5 tests) |
| `bowtie/mod.rs` | Bowtie catalog module root. | — |
| `bowtie/types.rs` | Bowtie catalog types: `BowtieState`, `EventSlotEntry`, `BowtieCard`, `BowtieCatalog`. | — |
| `bowtie/catalog.rs` | **Bowtie catalog builder** — pure algorithm: slot walking, catalog building, layout metadata merging, role extraction. Also owns `SlotInfo`, `WELL_KNOWN_EVENT_IDS`, `node_display_name`, `CdiReadCompletePayload`. | inline `#[cfg(test)]` (25+ tests) |

### Tauri app shell (`app/src-tauri/src/`)

| Module | Purpose | Test |
|--------|---------|------|
| `lib.rs` | Entry point: connection init, state setup, command registration | — |
| `main.rs` | Tauri desktop app launcher | — |
| `state.rs` | Authoritative app state: connection, registry, caches. Re-exports `NodeRoles` from `bowties_core::node_tree` and bowtie catalog types from `bowties_core::bowtie::types`. | inline `#[cfg(test)]` |
| `node_key.rs` | Re-export shim → `bowties_core::node_key` | — |
| `node_registry.rs` | Re-export shim → `bowties_core::node_registry` | — |
| `node_proxy.rs` | Re-export shim → `bowties_core::node_proxy` | — |
| `node_tree.rs` | Re-export shim → `bowties_core::node_tree` | — |
| `layout/mod.rs` | Re-export shim → `bowties_core::layout` | — |
| `profile/mod.rs` | Re-export shim → `bowties_core::profile` + keeps `loader` submodule | — |
| `profile/loader.rs` | `.profile.yaml` loading; depends on `tauri::AppHandle` for resource-dir resolution. Bundled-profile listing for placeholder picker. | inline `#[cfg(test)]` |
| `placeholder.rs` | **Placeholder factory orchestrator** (S8.10). Owns `synthesize`/`reconstitute` (Tauri-dependent). Re-exports pure helpers from `bowties_core::placeholder`. | — |
| `diagnostics.rs` | Ring-buffer logging (`bwlog!`), diagnostic stats | — |
| `events/router.rs` | Event broadcast: transport frames → Tauri events | inline `#[cfg(test)]` |
| `traffic/mod.rs` | Message decoding for traffic monitor display | — |
| `menu.rs` | Desktop app menu | — |
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
- Discovery: lcc-rs probes → backend creates LiveNodeProxy actors → frontend receives events
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

### Component Thin-Delegate Contract Tests
**Canonical example:** `app/src/lib/components/ElementCardDeck/SaveControls.test.ts` (`describe('Step 7 Option H: thin-delegate contract', ...)`)
- When a component is refactored to a thin delegate (presenter says yes/no → component invokes the `onAction` prop; no direct store mutation), the test suite MUST encode the contract directly:
  1. **Presenter-says-yes invokes prop.** Set up the minimum store state the presenter needs to gate `canX: true`, render the real component (not the presenter), click the button, and assert `onAction` was invoked. The Save no-op regression that motivated Option H slipped past mocked-presenter tests because the regression was in the component's parallel gate, not the presenter.
  2. **Component does NOT reach into cleanup stores.** Assert `.not.toHaveBeenCalled()` on `layoutStore.markClean`, `bowtieMetadataStore.clearAll`, `configChangesStore.commitForSave`, `offlineChangesStore.reloadFromBackend`, and any other post-action wiring that orchestrators own.
  3. **Mode parity.** When the component is mode-agnostic but the orchestrator wiring differs (online vs offline), parameterise the contract test over both modes with a `runFor(mode)` helper that mounts and unmounts cleanly.
- Anti-pattern: asserting that the component itself calls `commitForSave`/`markClean`/etc. — that test moves with the cleanup, not with the component, and inverts when the refactor lands.

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
