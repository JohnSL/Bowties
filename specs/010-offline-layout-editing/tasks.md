# Tasks: Offline Layout Editing

**Input**: Design documents from `/specs/010-offline-layout-editing/`
**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/tauri-ipc.md`, `quickstart.md`

**Tests**: No explicit TDD or test-first requirement was requested in the feature spec, so this task list focuses on implementation and validation tasks.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the feature scaffolding and command/module entry points.

- [x] T001 Create layout module skeleton in `app/src-tauri/src/layout/mod.rs`
- [x] T002 [P] Create capture command module skeleton in `app/src-tauri/src/commands/layout_capture.rs`
- [x] T003 [P] Create sync command module skeleton in `app/src-tauri/src/commands/sync_panel.rs`
- [x] T004 [P] Create CDI bundle module skeleton in `app/src-tauri/src/cdi/bundle.rs`
- [x] T005 Wire new command modules into command registration in `app/src-tauri/src/lib.rs`
- [x] T006 Create frontend layout API wrapper file in `app/src/lib/api/layout.ts`
- [x] T007 [P] Create frontend sync API wrapper file in `app/src/lib/api/sync.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Build shared persistence and state infrastructure required by all user stories.

**CRITICAL**: Complete this phase before any user-story implementation.

- [x] T008 Implement manifest and schema version types in `app/src-tauri/src/layout/manifest.rs`
- [x] T009 [P] Implement node snapshot data structures in `app/src-tauri/src/layout/node_snapshot.rs`
- [x] T010 [P] Implement offline change row types and status enums in `app/src-tauri/src/layout/offline_changes.rs`
- [x] T011 Implement deterministic YAML serialization helpers in `app/src-tauri/src/layout/io.rs`
- [x] T012 Implement staging-and-swap directory save utility in `app/src-tauri/src/layout/io.rs`
- [x] T013 Extend global app state for active layout context in `app/src-tauri/src/state.rs`
- [x] T014 [P] Add frontend layout state store for active/offline mode in `app/src/lib/stores/layout.svelte.ts`
- [x] T015 [P] Add frontend offline changes store for baseline/planned row state in `app/src/lib/stores/offlineChanges.svelte.ts`
- [x] T016 Add startup route integration for active layout bootstrap in `app/src/routes/+page.svelte`

**Checkpoint**: Shared layout schema/state/persistence foundation is ready.

---

## Phase 3: User Story 1 - Capture a Full Layout for Offline Use (Priority: P1) MVP

**Goal**: Capture discovered bus state to a YAML layout directory with per-node snapshots and preserved metadata.

**Independent Test**: Save a connected multi-node layout and verify `manifest.yaml`, `nodes/<NODE_ID>.yaml`, SNIP/config values, producer-identified events, preserved bowtie metadata, partial capture markers, and atomic replacement behavior.

- [x] T017 [US1] Implement capture-to-snapshot transformation pipeline in `app/src-tauri/src/commands/layout_capture.rs`
- [x] T018 [P] [US1] Implement capture status and missing-elements generation for partial reads in `app/src-tauri/src/layout/node_snapshot.rs`
- [x] T019 [P] [US1] Persist producer-identified event lists per node snapshot in `app/src-tauri/src/layout/node_snapshot.rs`
- [x] T020 [US1] Persist CDI reference fields (cache key/version/fingerprint) in `app/src-tauri/src/layout/node_snapshot.rs`
- [x] T021 [US1] Implement `save_layout_directory` command with atomic directory swap in `app/src-tauri/src/commands/layout_capture.rs`
- [x] T022 [P] [US1] Implement canonical node filename derivation (`nodes/<NODE_ID>.yaml`) in `app/src-tauri/src/layout/io.rs`
- [x] T023 [US1] Persist and merge existing bowtie names/tags/role classifications during capture save in `app/src-tauri/src/layout/io.rs`
- [x] T024 [US1] Add Save Layout action wiring to capture command flow in `app/src/routes/+page.svelte`

**Checkpoint**: Layout capture works end-to-end and is inspectable on disk.

---

## Phase 4: User Story 2 - Open a Captured Layout Without Connecting to the Bus (Priority: P1)

**Goal**: Load captured layout directories into full offline browsing mode.

**Independent Test**: Open a valid captured layout with no bus connected and verify nodes/config/bowties load from disk plus persistent offline capture banner.

- [x] T025 [US2] Implement `open_layout_directory` command and manifest validation in `app/src-tauri/src/commands/layout_capture.rs`
- [x] T026 [P] [US2] Implement YAML directory read and in-memory hydration in `app/src-tauri/src/layout/io.rs`
- [x] T027 [P] [US2] Build Offline banner component with capture timestamp in `app/src/lib/components/Layout/OfflineBanner.svelte`
- [x] T028 [US2] Add no-layout/open-layout startup UX and active layout identity display in `app/src/routes/+page.svelte`
- [x] T029 [P] [US2] Implement missing-capture placeholder badge component in `app/src/lib/components/Layout/MissingCaptureBadge.svelte`
- [x] T030 [US2] Render `(Not captured)` non-editable state in config view integration at `app/src/routes/+page.svelte`

**Checkpoint**: Offline browsing from disk is fully functional without network activity.

---

## Phase 5: User Story 3 - Edit Configuration and Bowties While Offline (Priority: P2)

**Goal**: Allow offline edits with distinct indicators and persisted pending change rows.

**Independent Test**: Edit config and bowties offline, save, restart app, reopen layout, and verify pending offline changes/indicators restore correctly.

- [x] T031 [US3] Implement `set_offline_change` and `revert_offline_change` commands in `app/src-tauri/src/commands/layout_capture.rs` (implemented in `app/src-tauri/src/commands/sync_panel.rs`)
- [x] T032 [P] [US3] Persist pending offline change rows in `offline-changes.yaml` via `app/src-tauri/src/layout/io.rs`
- [x] T033 [P] [US3] Add offline-change visual markers distinct from online dirty state in `app/src/routes/+page.svelte` (implemented across config/sidebar components)
- [x] T034 [US3] Extend bowtie edit flow to emit offline change rows in `app/src-tauri/src/commands/bowties.rs`
- [x] T035 [US3] Persist offline bowtie metadata edits in `app/src-tauri/src/layout/io.rs`
- [x] T036 [US3] Implement offline-save path preserving baseline/planned separation in `app/src-tauri/src/commands/layout_capture.rs`
- [x] T037 [US3] Add field-level revert-to-baseline action in `app/src/lib/stores/offlineChanges.svelte.ts`

**Checkpoint**: Offline edits persist and remain clearly distinguishable from online unsaved state.

---

## Phase 6: User Story 4 - Sync Offline Changes to the Bus (Priority: P2)

**Goal**: Compare pending changes with live values, resolve conflicts, and apply selected rows safely.

**Independent Test**: With one clean, one conflict, and one already-applied row, verify sync classification, resolution gating, selective apply, non-fatal continuation, and read-only row clearing behavior.

- [x] T038 [US4] Implement preliminary overlap classification (`likely same/uncertain/likely different`) in `app/src-tauri/src/commands/sync_panel.rs`
- [x] T039 [P] [US4] Implement `build_sync_session` row triage logic (conflict/clean/already-applied/node-missing) in `app/src-tauri/src/commands/sync_panel.rs`
- [x] T040 [P] [US4] Implement sync panel state store for conflict resolution and clean row selection in `app/src/lib/stores/syncPanel.svelte.ts`
- [x] T041 [P] [US4] Build Sync Panel UI container and sections in `app/src/lib/components/Sync/SyncPanel.svelte`
- [x] T042 [P] [US4] Build conflict row component with baseline/planned/bus comparison in `app/src/lib/components/Sync/ConflictRow.svelte`
- [x] T043 [P] [US4] Build clean summary section with per-row deselection in `app/src/lib/components/Sync/CleanSummarySection.svelte`
- [x] T044 [US4] Implement explicit sync mode selection (`target layout bus` vs `bench/other bus`) in `app/src-tauri/src/commands/sync_panel.rs` (backend `set_sync_mode`)
- [x] T045 [US4] Implement `apply_sync_changes` with continue-on-error row processing in `app/src-tauri/src/commands/sync_panel.rs`
- [x] T046 [US4] Implement read-only write-reply handling to clear row and reset displayed value in `app/src-tauri/src/commands/sync_panel.rs`
- [x] T047 [US4] Integrate startup auto-load, close layout, and new layout capture actions in `app/src/routes/+page.svelte`

**Checkpoint**: Sync panel prevents silent pushes and supports safe selective application.

---

## Phase 6b: Bug Fixes — Sync/Offline Lifecycle (Spec 010, Phase 6 follow-up)

**Purpose**: Fix five correctness bugs discovered during end-to-end testing of Phase 6 and add unit/component test coverage for the underlying behaviors.

- [x] T047b [US4] **Bug 1a** — `revert_offline_change` writes updated `offline-changes.yaml` to disk via new `persist_offline_changes` helper in `app/src-tauri/src/commands/sync_panel.rs`. Without this fix, reverted changes reappeared on next open.
- [x] T047c [US4] **Bug 1b** — Remove `layoutStore.markDirty()` from persisted-row revert onclick in `TreeLeafRow.svelte`. Calling it caused false Save/Discard buttons ("0 unsaved changes") after going online.
- [x] T047d [US3] **Bug 2** — Add `forceSyncPanel()` function and `menu-sync-to-bus` Tauri menu item so the user can re-open the sync panel after dismissing it. Add dismiss guard in `maybeTriggerSync` to prevent settle-timer auto-triggers from re-opening the panel.
- [x] T047e [US3] **Bug 3** — Apply `isOfflinePending` flag and `applyOfflinePendingValues()` to show the planned offline value (not the bus value) as the field's `modifiedValue` when online with pending persisted changes. Update annotation from "Pending apply: X → Y" to "Bus: X | Pending: Y".
- [x] T047f [US4] **Bug 4** — Persist `currentLayoutSnapshots` state on the page; `disconnect()` re-hydrates the node tree from those snapshots instead of clearing to empty when a layout is open.
- [x] T047g [US1] **Bug 5** — Filter non-CDI nodes (fingerprint `"not_supported"` or `"missing"`) from the snapshot list in `save_layout_directory` so their YAML is not written and does not produce "(Not captured)" banners on reload.
- [x] T047h [US4] **Tests** — `syncPanel.store.test.ts`: 11 store-level tests covering `dismiss()` persistence, settle-timer guard invariant, `loadSession()` reset contract, and `reset()` cleanup (`app/src/lib/stores/syncPanel.store.test.ts`).
- [x] T047i [US3/US4] **Tests** — `TreeLeafRow.offline.test.ts`: 13 component tests covering draft/persisted offline row annotations, revert button clearing both store and tree, `markDirty` absence, and lifecycle suppression during layout open (`app/src/lib/components/ElementCardDeck/TreeLeafRow.offline.test.ts`).
- [x] T047j [US3] **Tests** — `nodeTree.store.test.ts`: 5 tests for `applyOfflinePendingValues` covering address match, no-match, non-pending skip, empty store, and string value parsing.
- [x] T047k [US3] **Tests** — `nodeTree.test.ts`: 4 tests for `countModifiedLeaves` excluding `isOfflinePending` leaves from the dirty count.
- [x] T047l [US4] **Tests** — `offlineChanges.store.test.ts`: 2 tests verifying `pendingApplyCount` drops to 0 after persisted revert and that draft reverts never call backend IPC.

---

## Phase 6c: Bug Fixes — Revert/Save Lifecycle & Disconnect Corrections (Spec 010, Session 2026-04-20)

**Purpose**: Correct behaviors from Phase 6b that conflict with the updated revert/save model and generalized disconnect behavior defined in spec Session 2026-04-20. T047b and T047c are directly superseded; T047f scope is extended.

- [ ] T047m [US3] **Fix T047b** — Remove the `persist_offline_changes` auto-write call from `revert_offline_change` in `app/src-tauri/src/commands/sync_panel.rs`. Revert must remove the row from in-memory state only and signal the frontend to mark the layout dirty. The on-disk `offline-changes.yaml` is updated only when the user explicitly saves the layout, not at revert time.
- [ ] T047n [US3] **Fix T047c** — Restore `layoutStore.markDirty()` in the revert onclick handler in `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`. A revert is a pending modification until saved; the layout must be dirty so Save/Discard buttons appear and the user can either commit or undo the revert.
- [ ] T047o [US3] **New** — Introduce saved-vs-in-memory layering to the offline changes store (`app/src/lib/stores/offlineChanges.svelte.ts`): track the last-saved snapshot of offline change rows separately from current in-memory rows. Discard must restore in-memory rows to the last-saved snapshot, which re-instates any planned offline values that were removed by an unsaved revert (US3 AS-7). Save must promote in-memory rows to the saved snapshot and write to disk.
- [ ] T047p [US4] **Fix T047f** — Extend the disconnect handler in `app/src/routes/+page.svelte` to preserve the node tree for online-mode layouts as well as offline-mode layouts: when any layout is open at disconnect, re-hydrate from captured snapshots (or leave session-read values visible if no snapshots exist). When no layout is open at disconnect, transition to the connection dialog.
- [ ] T047q **Tests** — Update `TreeLeafRow.offline.test.ts` to reflect restored `markDirty` on revert and no backend IPC call at revert time. Update `offlineChanges.store.test.ts` to cover the saved-vs-in-memory layering: discard restores a reverted planned value, save promotes in-memory to saved snapshot; add a test for the no-layout-open disconnect → connection dialog path.

---

## Phase 6d: Bug Fixes — Pending-Value Display (Spec 010, Session 2026-04-20)

**Purpose**: Fix a cluster of defects that prevent pending offline values from being visibly applied to the config tree's `modifiedValue`/`isOfflinePending` leaf flags. These bugs make pending offline changes invisible to the user after going online, after discard, and after partial sync apply — despite the offline change rows being correctly stored. Also adds "already applied" auto-clearing and post-apply snapshot refresh.

**Root cause summary**: `applyOfflinePendingValues` always silently no-ops in practice because (a) NodeID formats between tree keys (dotted hex `"05.02.01.02.00.00"`) and change-row `nodeId` (canonical no-dots `"050201020200"`) never match, and (b) the function is called before `reloadFromBackend` populates the rows it needs, so even a fixed format comparison would find an empty list.

- [ ] T047r [US3/US4] **EC-1/2** — Fix NodeID format mismatch in `applyOfflinePendingValues` (`app/src/lib/stores/nodeTree.svelte.ts`): normalize both the tree key and the change-row `nodeId` using the same `normalizeNodeId` helper (strip dots, uppercase) before comparing, matching the convention already used in `offlineChanges.svelte.ts`.
- [ ] T047s [US3/US4] **EC-7/8** — Fix `applyOfflinePendingValues` call-site ordering and coverage in `app/src/routes/+page.svelte` and `app/src/lib/stores/offlineChanges.svelte.ts`: (a) remove the premature call inside `hydrateOfflineSnapshots` (rows are not yet loaded at that point); (b) add a single guaranteed call after `offlineChangesStore.reloadFromBackend()` completes at every layout-open site (startup restore, manual open, and the online discovery path); (c) add a call after every direct `nodeTreeStore.setTree()` that occurs outside the `node-tree-updated` listener path (such as the offline fallback tree build in `hydrateOfflineSnapshots`), since these paths do not trigger `startListening`'s callback.
- [ ] T047t [US3] **EC-3** — After `revertAllPending` (Discard path) restores `_persistedRows` from `_savedRows`, call `nodeTreeStore.applyOfflinePendingValues` with the restored rows so tree leaf `modifiedValue`/`isOfflinePending` flags are re-stamped to match the restored state. Without this, the field shows the snapshot baseline instead of the restored planned value after discard (breaks US3 AS-7).
- [ ] T047u [US4] **EC-4** — After `handleApply` in `app/src/lib/components/Sync/SyncPanel.svelte` rebuilds trees for applied nodes via `buildOfflineNodeTree` + `nodeTreeStore.setTree`, call `nodeTreeStore.applyOfflinePendingValues(offlineChangesStore.persistedRows)` over all trees so any remaining pending rows on those (or other) nodes retain their `modifiedValue`/`isOfflinePending` display. Without this, a partial apply wipes the pending-value indicator for all other pending rows.
- [ ] T047v [US4] **EC-5** — Auto-clear "already applied" rows from backend in-memory cache during `build_sync_session` in `app/src-tauri/src/commands/sync_panel.rs`: rows whose bus value already equals the planned value must be removed from `offline_changes_cache` inside `build_sync_session`, not merely counted. This matches the spec intent "silently cleared and shown only as a count" (US4 AS-5) and prevents already-applied rows from persistently re-appearing in future sessions.
- [ ] T047w [US4] **EC-6** — After `apply_sync_changes` completes in `app/src-tauri/src/commands/sync_panel.rs`, update the per-node snapshot YAML files in the companion directory for each successfully applied row: write the applied `planned_value` into that node's snapshot config entry at the matching `space`/`offset`, and update the node's `captured_at` timestamp. Skipped, deselected, and failed rows are left unchanged. This prevents the snapshot from showing a stale pre-apply baseline on the next offline open.
- [ ] T047x **Tests** — Add/update tests: `nodeTree.store.test.ts` — `applyOfflinePendingValues` matches when nodeId uses canonical format (EC-1/2 regression); `offlineChanges.store.test.ts` — `revertAllPending` triggers `applyOfflinePendingValues` with restored rows (EC-3); `SyncPanel` component test — `applyOfflinePendingValues` called after partial apply and after `handleApply` tree rebuild (EC-4).

---

## Phase 7: User Story 5 - Prepare New Uninstalled Nodes at Home (Priority: P2)

**Goal**: Support staged nodes that are added, edited, and later validated/synced when discovered on target bus.

**Independent Test**: Add staged node offline, persist it, read values on bench bus, then sync/validate on target bus while absent nodes remain non-blocking.

- [ ] T048 [US5] Implement staged-node creation and persistence as first-class node snapshots in `app/src-tauri/src/commands/layout_capture.rs`
- [ ] T049 [P] [US5] Add staged-node metadata fields (`origin`, validation state) to snapshot model in `app/src-tauri/src/layout/node_snapshot.rs`
- [ ] T050 [US5] Implement identity-only discovered state until read success for staged nodes in `app/src/routes/+page.svelte`
- [ ] T051 [US5] Include staged-node pending rows in sync session generation in `app/src-tauri/src/commands/sync_panel.rs`
- [ ] T052 [US5] Preserve non-discovered staged nodes as non-blocking pending rows during apply in `app/src-tauri/src/commands/sync_panel.rs`

**Checkpoint**: Staged node preparation and deferred validation are fully supported.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Finalize portability, migration, docs, and validation across all stories.

- [ ] T053 [P] Implement CDI export flow command in `app/src-tauri/src/cdi/bundle.rs`
- [ ] T054 [P] Implement CDI import flow command and missing-reference recovery in `app/src-tauri/src/cdi/bundle.rs`
- [x] T055 ~~Implement legacy layout migration entrypoint for older single-file persistence~~ — Schema v2 capture directory support was added as temporary import-only compatibility and then removed; only schema v3 (`.layout` + `.layout.d`) is accepted. Migration from the Feature 009 single `.bowties.yaml` layout file is out of scope for this feature (no node snapshot data to migrate).
- [ ] T056 [P] Add deterministic serialization normalization pass for stable ordering in `app/src-tauri/src/layout/io.rs`
- [ ] T057 Update user workflow documentation for capture/offline/sync in `docs/user/using.md`
- [ ] T058 Execute and record quickstart validation scenarios in `specs/010-offline-layout-editing/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- Setup (Phase 1) has no dependencies.
- Foundational (Phase 2) depends on Setup and blocks all user stories.
- User Story phases depend on Foundational completion.
- Polish (Phase 8) depends on completion of all required user stories.

### User Story Dependencies

- US1 depends only on Foundational phase.
- US2 depends on US1 capture output format and Foundational phase.
- US3 depends on US2 offline load state and Foundational phase.
- US4 depends on US3 persisted offline changes and Foundational phase.
- US5 depends on US3 offline persistence plus US4 sync infrastructure.

### Story Completion Order

1. US1 (MVP capture)
2. US2 (offline open/browse)
3. US3 (offline edit/persist)
4. US4 (sync workflow)
5. US5 (staged nodes)

---

## Parallel Execution Examples

### User Story 1

- Run T018 and T019 in parallel after T017.
- Run T022 and T023 in parallel after T021.

### User Story 2

- Run T027 and T029 in parallel after T025/T026.

### User Story 3

- Run T032 and T033 in parallel after T031.

### User Story 4

- Run T040, T041, T042, and T043 in parallel after T038/T039.

### User Story 5

- Run T049 in parallel with T050 after T048.

---

## Implementation Strategy

### MVP First (US1)

1. Complete Setup and Foundational phases.
2. Deliver US1 capture/save with deterministic per-node YAML snapshots.
3. Validate US1 independently before moving on.

### Incremental Delivery

1. Add US2 for offline browse-only value.
2. Add US3 for offline editing and persistence.
3. Add US4 for controlled sync back to bus.
4. Add US5 staged node preparation workflow.

### Parallel Team Strategy

1. One developer focuses backend persistence (`layout/*.rs`, `layout_capture.rs`).
2. One developer focuses sync backend (`sync_panel.rs`) and CDI portability.
3. One developer focuses frontend stores/components (`layout.svelte.ts`, `syncPanel.svelte.ts`, `Sync/*.svelte`).
4. Integrate per-story checkpoints in priority order.
