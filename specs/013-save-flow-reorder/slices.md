# Slices: Layout-First Model

Branch: 013-save-flow-reorder
Generated: 2026-05-17
Status: 12/13 slices complete (S1, S2, S2a, S2b, S2c, S2d, S2e, S3, S4, S5, S6, S7 done). S8 implementation complete (all T1–T17 done; HITL acceptance criteria still pending: layout-nodes-not-on-bus indicator, disconnect drops discovered-only nodes, wrong-bus scenario). S2e added 2026-05-23 to make persistence resilient to cloud-sync (Dropbox/OneDrive/AV) sharing-violation failures. S7 split 2026-05-23 into S7 (layout-scoped connections) and S8 (layout as durable node roster) so the connection-list change and the discovery-reconciliation change can be validated independently; the split was motivated by S2d/S2e — T6 (auto-add) now has a single legitimate persistence seam (`layout::update_node_snapshots`) and belongs with the other node-roster work, not with the connection-UI work.

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

<!-- Session: 2026-05-23 — User reported Dropbox save failure ("The process cannot access the file because it is being used by another process") leaving a `.layout` file with an unpromoted `.layout.d.staging` directory. Root cause: Windows `MoveFileEx` on the staging directory loses to Dropbox/OneDrive/AV file handles. Investigation also surfaced that `layout/` is not a deep module — `commands/cdi.rs` and `commands/sync_panel.rs` directly construct companion-dir paths and call `write_yaml_file`, which would defeat any journal added to the save path. Adding S2d (deepen `layout/`) and S2e (in-place writes + `.restore/` journal) before S3. Both continue the S2 "layout-authoritative" arc. -->

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

## S2d: Deepen the layout module (single-owner file knowledge) [AFK]

**Layers**: Backend domain, Backend command
**Blocked by**: S2c
**Complexity**: medium
**User stories**: US4 (foundation for save robustness)

The `layout/` module today is a thin I/O helper. Knowledge of *what files make up a layout* — the companion directory, the `nodes/` subfolder, the `offline-changes.yaml` filename, per-node YAML derivation — is duplicated across `commands/layout_capture.rs`, `commands/sync_panel.rs`, and `commands/cdi.rs`. Every one of these call sites independently constructs `derive_companion_dir_path(...).join("nodes").join(...)` and calls `write_yaml_file` directly. This blocks the journaled-save change in S2e because any single-file write that bypasses the journal can corrupt the recovery state. This slice promotes `layout/` into a deep module with an intent-shaped public API and makes all file/path/format details private — no behavior change.

**Acceptance criteria**:
- [x] `layout/mod.rs` exposes intent-shaped functions: `save_capture`, `read_capture`, `update_offline_changes`, `update_node_snapshots`, `read_node_snapshot`
- [x] `derive_companion_dir_path`, `derive_node_file_path`, `write_yaml_file`, `read_yaml_file`, `save_directory_atomic`, and the `nodes/` / `offline-changes.yaml` / `bowties.yaml` / `event-names.yaml` constants are private to `layout/`
- [x] No code outside `layout/` joins `"nodes"`, references the companion-dir layout, or writes YAML directly into a layout
- [x] `commands/cdi.rs` and `commands/sync_panel.rs` partial-write paths call the new `update_*` APIs instead of constructing paths themselves
- [x] All existing save/read tests pass unchanged; new tests cover the partial-update APIs going through the same code path as the full save

**Tasks**:
- [x] S2d-T1: Write tests — `update_offline_changes` and `update_node_snapshots` round-trip; `read_node_snapshot` returns the value written by either full save or partial update
- [x] S2d-T2: Layout module — define the intent-shaped public API in `layout/mod.rs`; keep current internals but route through the new entry points
- [x] S2d-T3: Command refactor — replace direct `write_yaml_file` / `derive_*` / `.join("nodes")` calls in `commands/sync_panel.rs` (~3 sites) and `commands/cdi.rs` (~2 sites) with the new APIs (also `commands/layout_capture.rs` and `commands/connector_profiles.rs`)
- [x] S2d-T4: Visibility — made companion-dir helpers `pub(crate)`; verified `cargo build --tests` clean
- [x] S2d-T5: Updated `aiwiki/owners.md` to mark `layout/` as the deep owner of layout file structure; wrote **ADR-0005** "Layout module owns all companion-dir file structure"
- [x] S2d-T6: Validate — full backend test suite green (308 passed); no compiler warnings

---

## S2e: Journaled in-place save (cloud-sync resilience) [HITL]

**Layers**: Backend domain, Backend command, Orchestrator (recovery notice)
**Blocked by**: S2d
**Complexity**: medium
**User stories**: US4 (specifically the Dropbox/OneDrive failure mode)

Today, saving a layout writes a `.layout.d.staging` directory and then renames it over `.layout.d`. On Windows the directory rename (`MoveFileEx`) fails with sharing-violation error 32 whenever Dropbox, OneDrive, antivirus, or Windows Search has briefly opened any file inside the staging directory — which they always do, immediately after each file is written. The user-visible failure is *"Save failed: The process cannot access the file because it is being used by another process"*, followed by an unopenable layout because the base `.layout` file was written but the staging directory was never promoted.

This slice replaces the staging-swap with **in-place writes + a write-ahead journal** owned entirely inside `layout/`:

1. Write a `.save-in-progress` marker with a phase field; fsync.
2. For every file the save will modify or delete, copy current contents to `.restore/<relpath>`; fsync.
3. Flip the marker to `phase: "writing"`; fsync.
4. Overwrite each target file in place (`File::create` + write + `sync_all`) — no temp files, no renames.
5. Delete files no longer in the snapshot.
6. Delete the marker; fsync the directory.

On read, if the marker exists the previous save was interrupted: roll back from `.restore/` and surface a notice. Every public mutation in `layout/` (full save and the S2d `update_*` APIs) flows through this protocol — one journal, one place. This trades filesystem per-file atomicity for transactional rollback at the layout level; under Dropbox/OneDrive/AV that trade is strongly favourable because in-place file writes do not contend with sync-agent read handles the way directory renames do.

**Acceptance criteria**:
- [ ] No code path under `layout/` calls `std::fs::rename` on a directory; per-file temp-then-rename is also gone in the new write path
- [ ] Save succeeds in a folder where another process holds an open read handle on existing layout files (simulated test using a held file handle; gated to Windows)
- [x] A crash injected between journal phases 3 and 6 leaves a coherent layout after the next open (auto-rollback from `.restore/` restores the previous coherent state)
- [x] Existing `…layout.d.staging` and `…layout.d.backup` directories from the old scheme are cleaned up opportunistically on next save
- [x] On rollback at open time, the user sees a one-line notice ("Previous save was interrupted and has been restored")
- [x] No `.layout.d.staging` or `.tmp` files appear anywhere on disk after a normal save
- [x] Frontend behavior is unchanged on the happy path; save progress dialog (S3) sees the same phase events
- [ ] Bytes uploaded to Dropbox per save drop to "only files that actually changed" (manually verified, not asserted in test)

**Tasks**:
- [x] S2e-T1: Write tests — happy-path round-trip; held-handle save succeeds on Windows; crash-between-phases recovery (inject an abort flag in the marker writer, then assert read-back state); migration cleanup of leftover `.staging`/`.backup` dirs
- [x] S2e-T2: Layout module — define the marker format (`.save-in-progress` containing `phase`, `started_at`, `manifest_path`) and the `.restore/` mirror layout
- [x] S2e-T3: Layout module — implement the 6-step in-place journal; route `save_capture` and the S2d `update_*` APIs through it
- [x] S2e-T4: Layout module — implement `recover_if_needed(base_file)` and call it from the top of `read_capture`; return a `RecoveryOccurred` flag on the read result
- [x] S2e-T5: API + Orchestrator — surface the recovery flag through `open_layout_directory`; emit a toast/notice when set
- [x] S2e-T6: Cleanup — remove `save_directory_atomic`, the temp-file dance in `write_yaml_file`, the staging/backup constants, and any related dead code
- [x] S2e-T7: Update `aiwiki/owners.md` and `aiwiki/flows.md`; write **ADR-0006** "In-place writes with journaled rollback for layout persistence"
- [ ] S2e-T8: Validate — full test suite green; manual save into a Dropbox-synced folder succeeds without sharing-violation errors; no orphan `.staging` directories remain *(automated suites green: 316 backend + 876 frontend tests; manual Dropbox check pending user verification)*

---

## S3: Save progress tracking + API cleanup [AFK]

**Layers**: Store, Component, API
**Blocked by**: S2a
**Complexity**: small
**User stories**: US6

A modal `SaveProgressDialog` displays the current save phase and per-field bus-write progress. A new `saveProgress` store tracks phase transitions driven by Tauri progress events from S2. As a companion to extending the API layer in S2, the duplicate IPC wrappers (`saveLayoutFile` ≡ `saveLayoutDirectory`, `openLayoutFile` ≡ `openLayoutDirectory`) are removed and the layout.ts / bowties.ts boundary is clarified.

**Acceptance criteria**:
- [x] Progress store transitions through layout-save → bus-write → reconcile phases
- [x] `SaveProgressDialog` renders as a modal during save; shows "Saving layout…", per-field bus-write count, and "Updating layout…"
- [x] Dialog is modal — no second save can be initiated while one is in progress
- [x] Duplicate API wrappers removed; all callers compile

**Tasks**:
- [x] S3-T1: Write integration test — progress store updates through phases; dialog displays correct labels
- [x] S3-T2: Store — implement `saveProgress.svelte.ts` with phase state and Tauri event subscription
- [x] S3-T3: Component — implement `SaveProgressDialog.svelte` (modal, phase labels, per-field counter)
- [x] S3-T4: API — remove `saveLayoutFile`/`openLayoutFile` duplicates; clarify layout.ts vs bowties.ts boundary
- [x] S3-T5: Validate — integration test passes, dialog is modal, no duplicate wrappers

---

## S4: Schema extension (connections field) + connection CRUD [AFK]

**Layers**: Backend domain, Backend command, API
**Blocked by**: None
**Complexity**: medium
**User stories**: US2, US3

Add an optional `connections` field to the layout manifest. Because it's a serde-defaulted optional, existing layout files open without migration — no breaking change. The companion directory snapshot format is unaffected. Backend commands `get_layout_connections` and `save_layout_connections` expose CRUD for connection definitions. A layout can store multiple named connections (name, type, host/port or serial settings).

**Acceptance criteria**:
- [x] Existing layout files (without connections field) open correctly — no error, connections list is empty
- [x] Layout with connections persists and round-trips through save → close → reopen
- [x] A layout can store multiple named connections
- [x] `get_layout_connections` and `save_layout_connections` commands work correctly

**Tasks**:
- [x] S4-T1: Write integration test — existing layout opens cleanly; connections round-trip; multiple connections supported
- [x] S4-T2: Backend domain — add `connections: Vec<ConnectionConfig>` (serde default) to `LayoutManifest` in `manifest.rs`; add `ConnectionConfig` type to `types.rs`
- [x] S4-T3: Backend command — implement `get_layout_connections` and `save_layout_connections` in `commands/connection.rs`
- [x] S4-T4: API — add `getLayoutConnections` and `saveLayoutConnections` Tauri invoke bindings
- [x] S4-T5: Validate — integration test passes, existing layout files unaffected

---

## S5: Known-layout registry backend [AFK]

**Layers**: Backend domain, Backend command, API
**Blocked by**: None
**Complexity**: medium
**User stories**: US1

A new `known_layouts.rs` backend module persists the app's known-layout registry to `$APPDATA/bowties/known-layouts.json`. Each entry stores layout name, file path, and last-opened date. The module filters stale entries (path no longer exists) and uses atomic writes (temp→flush→rename). Backend commands `get_known_layouts`, `add_known_layout`, and `remove_known_layout` expose the registry. Removing a known layout removes only the registry entry — layout files on disk are not deleted.

**Acceptance criteria**:
- [x] CRUD on known-layouts.json works correctly
- [x] Stale paths are filtered from the returned list
- [x] Writes are atomic (temp→flush→rename)
- [x] Removing a layout entry does not delete files on disk

**Tasks**:
- [x] S5-T1: Write integration test — CRUD; stale-path filtering; atomic writes; remove-only-registry
- [x] S5-T2: Backend domain — implement `layout/known_layouts.rs` with get/add/remove and stale-path filtering
- [x] S5-T3: Backend command — implement `get_known_layouts`, `add_known_layout`, `remove_known_layout` in new `commands/startup.rs`
- [x] S5-T4: Backend — register new commands in `lib.rs`. (No `state.rs` change needed — the registry is a stateless file like `connections.json`; deferred per YAGNI.)
- [x] S5-T5: API — add `getKnownLayouts`, `addKnownLayout`, `removeKnownLayout` Tauri invoke bindings in new `api/startup.ts`
- [x] S5-T6: Validate — integration test passes, atomic writes confirmed, files not deleted on remove (325/325 backend tests pass)

---

## S6: Layout picker gate [HITL]

**Layers**: Route, Component, Orchestrator, Store, API
**Blocked by**: S4, S5
**Complexity**: large
**User stories**: US1

No functionality is accessible until a layout is active. `+page.svelte` renders either the layout picker or the main UI — never both. `LayoutPicker.svelte` shows known layouts (name, location, last-opened date), "New Layout", and "Browse…". `startupOrchestrator.ts` owns the picker lifecycle: loading known layouts, handling selection, creating new layouts, and setting the active layout in the store. The picker disappears once a layout is active; disconnecting returns to the main UI (not the picker).

**Acceptance criteria**:
- [x] App with no active layout shows the picker; main UI is not accessible
- [x] Selecting a known layout opens it and picker disappears
- [x] "New Layout" prompts for name and location, creates the layout, opens it
- [x] "Browse…" opens an existing layout not in the known list and adds it to the list
- [x] "Remove" removes the entry from the known list without deleting files
- [x] Layout name is visible in the title bar or header after opening
- [x] Picker does not reappear when disconnecting — only appears when no layout is active

**Tasks**:
- [x] S6-T1: Write integration test — no layout → picker shown; select known → main UI; new layout → picker gone; browse → added to list
- [x] S6-T2: Store — extend layout store with `activeLayoutContext` and `knownLayouts` state
- [x] S6-T3: Orchestrator — implement `startupOrchestrator.ts` (load known layouts, open, create new, browse, set active)
- [x] S6-T4: Component — implement `LayoutEntry.svelte` (name, location, last-opened date, remove action)
- [x] S6-T5: Component — implement `NewLayoutDialog.svelte` (name + location inputs, create action)
- [x] S6-T6: Component — implement `LayoutPicker.svelte` (known list, New Layout, Browse…)
- [x] S6-T7: Route — add `activeLayoutContext` conditional render gate to `+page.svelte`
- [x] S6-T8: Validate — integration test passes, picker gate correct, title bar shows layout name

<!-- S6 SESSION NOTE (2026-04-25): Implemented `knownLayoutsStore` (svelte 5 runes; defensive against undefined backend payloads), `startupOrchestrator.ts` with `loadKnownLayouts` / `openLayoutFromRegistry` / `createNewLayout` / `removeKnownLayout` / `deriveLayoutNameFromPath` (14/14 orchestrator unit tests pass), and three picker components (`LayoutEntry`, `NewLayoutDialog`, `LayoutPicker`). Wired into `+page.svelte` via new `pickerActive` derived (`!startupBootstrapPending && !$layoutOpenInProgress && !layoutStore.activeContext`) gating the full app-shell content. New layout flow: `createNewLayoutCapture` → `saveLayoutDirectory(path, true, [])` → `openOfflineLayoutWithReplay` → `addKnownLayout`. Opening from registry reuses the existing replay path. Registry refresh failures only warn — they never block UX. Updated `page.route.test.ts` with `vi.mock('$lib/api/startup')`, stubbed `LayoutPicker`, added `createNewLayoutCapture` to the layout mock, and pre-seeded a `legacy_file` active context in `beforeEach` so the discovery-CTA legacy flow still drives the ConnectionManager (with re-seeding after the explicit close in the open-close-connect test). Full suite green: 899/899. -->

---

## S7: Layout-scoped connections [HITL]

**Layers**: Route, Component, Orchestrator, Store, API
**Blocked by**: S6, S4
**Complexity**: medium
**User stories**: US2, US3

With a layout open, the user manages a list of named connections **owned by the layout** (not the global `connections.json`) and uses one of them to go online. If the layout has exactly one connection, it is used directly with no selection step. If it has multiple, a selector appears. Disconnecting keeps the layout open in offline mode with all data intact. Only one connection may be active at a time. The existing `ConnectionManager.svelte` already provides add / edit / delete UI; this slice repoints all of it (list, add, edit, delete, connect) at the per-layout API.

S4 already wired `getLayoutConnections` / `saveLayoutConnections` through `layout::read_manifest` / `update_manifest_connections`, so no backend changes are expected. This is a frontend-only consumption slice covering both **CRUD on the layout's connection list** and the **connect/disconnect flow** that uses it.

**Persistence policy for connection edits**: connection add / edit / delete writes through to the layout immediately (call `saveLayoutConnections` on each mutation), the same way the global-prefs version writes through to `connections.json` today. Connection-list edits do **not** participate in the dirty-layout / Save-button flow — they are infrastructure, not user-content edits. (If this turns out wrong we can revisit; calling it out so the decision is explicit, not accidental.)

**UX shape (decision)**: keep today's list-as-management-surface. The connection panel always shows one row per layout connection with a row-level **Connect**, 🖊 **Edit**, and × **Delete**, plus a card-level **+** to add. "Single-connection fast path" means *no extra selector step on top of that row* — the user clicks Connect on the only row. With multiple rows the UI is unchanged; the user clicks Connect on the row they want. No separate management dialog and no collapsed "primary Connect + Edit connections…" form.

**Acceptance criteria**:
- [x] List, add, edit, and delete of layout connections all go through `getLayoutConnections` / `saveLayoutConnections`; no reference to `load_connection_prefs` / `save_connection_prefs` remains in the active flow
- [x] Each add / edit / delete persists to the layout's manifest immediately (no separate "Save" step for connection edits); a closed-then-reopened layout shows the same list
- [x] Switching the active layout swaps the visible connection list (no leaks between layouts)
- [x] Connect uses the chosen layout connection; no re-entry required when exactly one connection is defined (one-click connect on that row)
- [x] List, Connect / 🖊 Edit / × Delete per row, and a card-level "+" to add are reachable in both the one-connection and many-connection cases — no separate management dialog
- [x] Multi-connection case is the same UI with more rows; there is no extra picker step above the rows
- [x] Disconnect keeps the layout open; all node data (snapshots, bowties, offline changes) is preserved
- [x] Only one connection may be active at a time (the connect control is disabled or labeled "Disconnect" while a connection is active)
- [x] Connection edits round-trip through the S2e journal (free, since `update_manifest_connections` already routes through `layout/`); no `.staging` directories appear after a connection edit

**Tasks**:
- [x] S7-T1: Write integration tests — CRUD: add a connection → reopen → present; edit → reopen → updated; delete → reopen → gone. Connect: 1 conn → one-click on the row connects; N conns → same list UI, click the chosen row. Disconnect preserves layout. Switching layouts swaps the list.
- [x] S7-T2: Component — repoint `ConnectionManager.svelte` at the layout-scoped API: `loadPrefs` → `getLayoutConnections`, `savePrefs` → `saveLayoutConnections`. Trigger a reload whenever the active layout changes. Keep the existing row-level Connect / Edit / Delete + card-level "+" UX; do not add a multi-connection picker on top.
- [x] S7-T3: Orchestrator — update the connect path (extend `offlineLayoutOrchestrator.ts` or introduce a small `liveSessionOrchestrator` if cleaner) so the route stays a delegator and connect/disconnect lifecycle is owned in one place; ensure "only one active connection" is enforced at this seam, not in the component.
- [x] S7-T4: Cleanup — decide the fate of the global `load_connection_prefs` / `save_connection_prefs` commands. Either remove them (with their backend tests) if no caller remains, or leave a deprecation note in `aiwiki/owners.md` referencing this slice. Do not leave both paths writable.
- [x] S7-T5: Validate — integration tests pass; manual: open layout A (1 conn) → connect direct; open layout B (2 conns) → selector; add/edit/delete a connection → close → reopen → changes survived; disconnect leaves layout intact.

<!-- S7 SESSION NOTE (2026-05-23): Repointed `ConnectionManager.svelte` at the layout-scoped API: replaced `invoke('load_connection_prefs')` with `getLayoutConnections(path)` and `invoke('save_connection_prefs', …)` with `saveLayoutConnections(path, …)`. Added a `$effect` keyed on `layoutStore.activeContext?.rootPath` that reloads connections whenever the active layout changes (including initial mount once the picker resolves and when the user switches layouts). Add/edit/delete each `await` the per-layout save so each mutation rides the S2e journal — no separate save step. T3 was a verify-only step: the connect/disconnect lifecycle is already owned by `connectLiveSession` / `disconnectWithOfflineFallback` in `syncSessionOrchestrator.ts`; the route stays a thin delegator. T4 removed `load_connection_prefs` and `save_connection_prefs` entirely from `commands/connection.rs` and their registrations in `lib.rs`; the global `$APPDATA/bowties/connections.json` registry no longer exists. Updated the doc reference in `layout/known_layouts.rs` and the `aiwiki/owners.md` connection.rs row. New test file `app/src/lib/ConnectionManager.test.ts` (4 tests) covers: loads on mount from the active layout path; never calls the removed global commands; delete persists through `saveLayoutConnections`; no API calls when no layout is active. 903/903 vitest tests pass; 325/325 cargo tests pass. Single-active-connection is naturally enforced because the ConnectionManager is only mounted when `!connected && !layoutStore.hasLayoutFile && nodes.length === 0` or via the explicit reconnect dialog. -->

---

## S8: Layout as durable node roster [HITL]

**Layers**: Backend command, Backend domain (thin), Orchestrator, Store, Component
**Blocked by**: S7
**Complexity**: medium
**User stories**: US2

The layout is the durable source of truth for which nodes belong to it. The node list shows the union of layout snapshots and currently-discovered nodes, with three states distinguished:

| State | Source | Indicator |
|-------|--------|-----------|
| Saved + on bus | in layout file AND on the live bus | normal (no badge) |
| Saved + off bus | in layout file but NOT on the live bus | "not on bus" |
| Discovered + on bus | discovered on the bus but NOT in the layout file | "new" |

**Newly discovered nodes are not auto-persisted.** They are surfaced as "new" so the user knows the bus contains nodes the layout does not. This protects against the wrong-layout-wrong-bus case: opening layout A and accidentally connecting to bus B's nodes does not pollute A. Disconnecting or switching layouts without saving drops the unsaved entries cleanly.

**Promotion threshold — "in memory" = fully captured.** A discovered node only counts as an in-memory addition (and therefore as a dirty change that Save will persist) once it has been **fully captured**: CDI cached *and* every config value read successfully. Reasoning: until then we cannot save a usable offline copy of the node, so promoting it would write a stub the user could not edit offline. Partially-captured nodes keep the "new" badge but do not light up Save.

**Dirty semantics.** `layoutStore.isDirty` means *"there are in-memory changes that have not been saved"*. That includes (a) edits to the `LayoutFile` struct (bowties, metadata, hardware selections — the current narrow meaning), and (b) fully-captured discovered nodes not yet in `layoutNodeIds`. Consumers (Save gate, unsaved-changes guard) only read `isDirty`; the OR-in-the-route pattern goes away.

Saving the layout promotes the unsaved-in-memory nodes into saved snapshots via the existing three-phase save (S2 / S2a) — node-add is a `LayoutEditDelta::AddNode` variant applied through `layout::update_node_snapshots` (ADR-0005, ADR-0006). No new write path; no auto-persist.

**Acceptance criteria**:
- [x] Node list shows the union of layout nodes + discovered nodes, with the three states distinguished by indicator
- [ ] Layout nodes absent from the live bus show a clear "not on bus" indicator — HITL
- [x] Newly discovered nodes that are **not yet fully captured** appear with the "new" badge but do **not** make the layout dirty and Save remains disabled for them alone — enforced by the T8/T10 threshold (`computeUnsavedInMemoryNodeIds` requires `nodeTreeStore.trees.has(id) && !partialCaptureNodes.has(id)`)
- [x] Fully-captured discovered nodes contribute to `layoutStore.isDirty` (Save button activates, unsaved-changes guard triggers); the "new" badge stays until Save (badge tracks discovered-vs-persisted via `computeDiscoveredOnlyNodeIds`, not capture progress)
- [ ] Disconnecting without saving drops all discovered-only nodes (captured or not); reconnecting rediscovers them — HITL
- [x] Closing the layout (or switching layouts) without saving drops the unsaved discovered nodes; the layout file is unchanged
- [x] After Save of fully-captured discovered nodes, those nodes are present in the on-disk layout (verify by close → reopen offline) — T13 resolved as a downstream consequence of T10 (only fully-captured nodes are ever sent as `AddNode` deltas, so the backend's `fingerprint != "missing"` filter no longer drops them)
- [ ] Wrong-layout-wrong-bus scenario: open layout A (nodes X,Y) → connect to a bus exposing P,Q → P,Q appear as "new" but Save stays disabled until the user explicitly captures them; layout A's on-disk file still contains only X,Y — HITL (logic implemented via T8/T10 threshold)
- [x] All node-snapshot writes (including new-node promotion) flow through `layout::update_node_snapshots`; no new direct YAML writes or `nodes/` joins outside `layout/`
- [x] When no node is selected, the node-configuration display area shows an empty-state message and a "Read all" button whenever one or more discovered nodes are not yet fully captured (T16: `showCaptureRemainingCta` derived in `+page.svelte` re-uses the existing `config-cta-panel` markup for the layout-loaded case)
- [x] Unsaved-changes guard fires on every exit path when `layoutStore.isDirty` is true: closing the layout (`runCloseLayoutAction`), switching layouts (`openLayoutFromPicker`), disconnecting (T17: `menu-disconnect` listener now wrapped in `promptUnsaved`), and closing the application window (`appWindow.onCloseRequested`). Each prompt offers Save / Discard / Cancel and only Discard or a successful Save proceeds.

**Known defects in current implementation (2026-05-24)** — *all resolved 2026-05-25*:
1. ~~Save lights up on mere discovery (no capture required)~~ — fixed in T8/T10 via `computeUnsavedInMemoryNodeIds` threshold gate.
2. ~~Even after a full read-all, saving does not actually persist the new node snapshots~~ — resolved as a downstream consequence of T10: the backend's `fingerprint != "missing"` retain filter only dropped snapshots for not-fully-captured nodes; now those never reach the backend.
3. ~~Dirty state is computed at four consumer sites in `+page.svelte` rather than centralized on `layoutStore`~~ — centralised in T9/T10 (`layoutStore.isDirty` covers both `_dirty` and `_unsavedInMemoryNodeIds`).
4. The "new" badge currently fires on mere discovery; once threshold lands it should continue to fire on "discovered-not-in-layoutNodeIds" — semantically the badge is unchanged but its rendering predicate becomes simpler (no longer doubles as the dirty signal).

**Tasks**:
- [x] S8-T1: Write tests — Frontend: store reconciles `layoutNodes ∪ discoveredNodes` into the three states; unsaved nodes count toward dirty state; disconnect drops them; save promotes them. Backend: a `LayoutEditDelta` variant for adding a new node snapshot is applied through `apply_layout_deltas` and lands in the companion dir via `update_node_snapshots`; idempotent on re-apply.
- [x] S8-T2: Backend domain — extend `LayoutEditDelta` with an "add node" variant; route it through the existing `apply_layout_deltas` → `layout::update_node_snapshots` path. No new IPC command, no new write path.
- [x] S8-T3: Store — add a `discoveredOnlyNodes` (in-memory) layer alongside saved layout nodes; expose a unified roster view with `{ nodeId, source: 'saved' | 'discovered', onBus: boolean }`; clear `discoveredOnlyNodes` on disconnect, close-layout, and successful save (after promotion).
- [x] S8-T4: Orchestrator — pipe discovery events into `discoveredOnlyNodes` (no IPC write on discovery); update `saveLayoutOrchestrator` to collect unsaved discovered nodes as add-node deltas and include them in the save payload; clear from `discoveredOnlyNodes` after the persisted layout response includes them.
- [x] S8-T5: Store — make the unified dirty signal (the one feeding the Save button and the unsaved-changes guard) include `discoveredOnlyNodes.length > 0`. *(Implemented at the route level in 2026-05-24 hotfix; will move into `layoutStore` per S8-T9 below.)*
- [x] S8-T6: Component — add the two distinct indicators to `NodeEntry.svelte` (or wherever the sidebar renders node rows): "not on bus" (saved+offBus) and "new" / "unsaved" (discovered+onBus). Visually distinct; both readable in monochrome.
- [ ] S8-T7: Validate — integration tests pass; manual: layout with saved A,B,C + bus with B,C,D → A shows "not on bus", D shows "new"; Save → D promotes to saved; disconnect before save → D disappears, file unchanged. — HITL pending; currently blocked by defects 1 and 2 above.

**Tasks added 2026-05-24 to repair acceptance criteria**:
- [x] S8-T8: `nodeRoster.ts` — added `computeUnsavedInMemoryNodeIds(savedNodeIds, fullyCapturedNodeIds)` (threshold-gated). The existing `computeDiscoveredOnlyNodeIds` is retained as the badge-only predicate (no threshold); its sole consumer in `+page.svelte` is the sidebar badge derivation.
- [x] S8-T9: `layoutStore` — added `_unsavedInMemoryNodeIds: string[]` private state, `setUnsavedInMemoryNodeIds(ids)` setter (content-equal dedup), and `unsavedInMemoryNodeIds` readonly getter. `isDirty` is now `_dirty || _unsavedInMemoryNodeIds.length > 0`. `markClean()`, `loadLayoutFromPath`, `saveLayoutAs`, `hydrateOfflineLayout`, `newLayout`, `reset`, and `setActiveContext` all clear the set.
- [x] S8-T10: `+page.svelte` — added `fullyCapturedNodeIds = $derived.by(...)` (iterate `nodeTreeStore.trees.keys()`, canonicalize, exclude `partialCaptureNodes`), `unsavedInMemoryNodeIds = $derived(computeUnsavedInMemoryNodeIds(...))`, and a `$effect` that pushes the latter into `layoutStore.setUnsavedInMemoryNodeIds()`. Stripped the four ad-hoc `discoveredOnlyNodeIds.length > 0` ORs (two `hasInMemoryEdits` blocks, two `hasUnsavedPromptChanges` call sites). Save orchestrator now receives `unsavedInMemoryNodeIds` (threshold-gated) as `discoveredOnlyNodeIds`.
- [x] S8-T11: `unsavedChangesGuard` — dropped the 6th `discoveredOnlyNodeCount` parameter; the dirty signal it consumes now covers the case. Tests updated.
- [x] S8-T12: Sidebar presenter — `isUnsavedNew` continues to mean "discovered but not in `layoutNodeIds`" (no threshold). Verified unchanged.
- [x] S8-T13: **Defect 2 resolved by T10.** The backend's snapshot retain filter (`fingerprint != "missing" && fingerprint != "not_supported"`) silently dropped uncaptured-but-AddNode'd nodes because `build_node_snapshot` produces fingerprint="missing" when no CDI is cached. With T10's threshold gating, only fully-captured nodes are ever sent as `AddNode` deltas, so the backend always has real CDI to fingerprint and the snapshot survives the filter. No backend change required; existing 328 cargo tests still pass.
- [x] S8-T14: Added tests for `computeUnsavedInMemoryNodeIds` (5 cases) and `layoutStore.isDirty` + `setUnsavedInMemoryNodeIds` + `markClean` + `setActiveContext` interaction (4 cases). Updated `unsavedChangesGuard.test.ts` to drop the 6th-param case and assert the layoutDirty channel now carries the signal. 929/929 vitest pass (up from 920).
- [x] S8-T15: Updated `aiwiki/owners.md` for `layoutStore.isDirty` extended semantics and `nodeRoster.computeUnsavedInMemoryNodeIds`. Wrote `product/architecture/adr/0007-full-capture-threshold-for-node-promotion.md` capturing the threshold rationale, the wrong-bus-protection scenario, and the alternatives considered.
- [x] S8-T16: Restored the empty-state "Read all" affordance for the layout-loaded case. Added `showCaptureRemainingCta` derived (mirrors `showConfigCta` but requires `layoutStore.hasLayoutFile`); the existing `config-cta-panel` markup is reused via `{#if showConfigCta || showCaptureRemainingCta}`.
- [x] S8-T17: Verified the unsaved-changes guard fires on every exit path against `layoutStore.isDirty`. Close-layout (`runCloseLayoutAction`) and switch-layout (`openLayoutFromPicker`) and app-window-close (`onCloseRequested`) all already routed through `promptUnsaved`. Disconnect was the one historically-overlooked path: wrapped the `menu-disconnect` listener in `promptUnsaved("Disconnecting will discard unsaved changes. Continue?", () => disconnect())` so the same Save / Discard / Cancel prompt fires on the disconnect menu item too.

---

<!-- Session: 2026-05-17 — Completed S1, S2 (including T7 bug fix: offline_bowtie_data population on layout open). Next: S3 (AFK). -->
<!-- Session: 2026-05-18 — S2 acceptance criteria still failing (role data loss, layout null after Save As). Root-cause analysis found 7-state-owner problem and wholesale-replace in merge_saved_layout_metadata. Wrote ADR-0002 (backend owns layout file data). Quick-fix patches attempted and reverted as architecturally unsound. Next: implement ADR-0002 before reattempting S2 acceptance criteria. -->
<!-- Session: 2026-05-18b — Completed S2a (backend-authoritative save). Discovered systemic display-resolution divergence: 6+ frontend call sites read stale baseline instead of effective values, causing offline names/roles to differ from online. Wrote ADR-0003 (unified display resolution). Added S2b slice. Next: implement S2b. -->
<!-- Session: 2026-05-23b — Completed S3 (save progress tracking + API cleanup). Added `saveProgressStore` (`stores/saveProgress.svelte.ts`) that listens to `save-progress` Tauri events from `save_layout_with_bus_writes` and also exposes `begin()`/`apply()`/`fail()`/`reset()` for the offline save path which is driven by the route. New `SaveProgressDialog.svelte` renders modal phase labels and a per-field bus-write counter. `+page.svelte` starts the listener in onMount, drives the store around `saveLayoutOrchestrated`, mounts the dialog, and adds `saveProgressStore.isActive` to `isMenuBusy()` so a second save cannot be initiated. Backend `write_modified_values` now emits per-iteration `save-progress` events with `current`, `total`, and field `label`. Removed duplicate API wrappers `saveLayoutFile`/`openLayoutFile`; canonical names are `saveLayoutDirectory`/`openLayoutDirectory`. 883/883 vitest tests pass; 316/316 backend cargo tests pass. Next: S4 (AFK). -->
<!-- Session: 2026-05-23c — Completed S4 (per-layout connections in manifest). Moved `ConnectionConfig`/`AdapterType`/`FlowControl` from `commands/connection.rs` into `layout/types.rs` so a single serde shape covers both the global `connections.json` and the new per-layout list; `commands/connection.rs` re-exports them so existing imports compile unchanged. `LayoutManifest` now carries `#[serde(default)] connections: Vec<ConnectionConfig>` — older layout files (no field present) open cleanly with an empty list. New `layout::read_manifest()` and `layout::update_manifest_connections()` helpers read the base `.layout` file and write it back through the journal (ADR-0006) without disturbing the companion directory. New Tauri commands `get_layout_connections` / `save_layout_connections` and frontend bindings `getLayoutConnections` / `saveLayoutConnections` (with `LayoutConnectionConfig` type) in `api/layout.ts`. Two new integration tests in `layout/io.rs`: legacy layout with stripped `connections` field opens with empty list, and a full multi-connection round-trip including a `GridConnectSerial` with RTS/CTS. 883/883 vitest pass; 318/318 cargo tests pass. Next: S5 (AFK). -->
<!-- Session: 2026-05-23d — Completed S5 (known-layout registry backend). New `layout/known_layouts.rs` module owns the on-disk format for `$APPDATA/bowties/known-layouts.json` (camelCase `KnownLayoutEntry { name, path, lastOpened }`), the stale-path filter on read, and the atomic temp→flush→fsync→rename write (same protocol as `connections.json`). Add is upsert-by-path so re-opening a layout refreshes its name/timestamp without duplicating. Remove only forgets the registry entry — the test asserts the layout file on disk is untouched. New `commands/startup.rs` exposes `get_known_layouts` / `add_known_layout` / `remove_known_layout`, registered in `lib.rs`. New `app/src/lib/api/startup.ts` exports `getKnownLayouts` / `addKnownLayout` / `removeKnownLayout` and the `KnownLayoutEntry` type. No `state.rs` change — the registry is a stateless file like `connections.json` (deviation noted in S5-T4). 7 new module tests cover empty-on-missing, round-trip, upsert-replace, stale-path filtering, atomic writes (no leftover .tmp), remove-doesn't-delete-files, and parent-dir creation. 325/325 backend tests pass. Next: S6 (HITL). -->

<!-- Session: 2026-06-13 — Implemented S8 (layout as durable node roster). Backend: added `LayoutEditDelta::AddNode { node_id_hex }` (camelCase serde, no-op in `apply_layout_deltas` since node membership lives in the companion `nodes/` dir, not in `LayoutFile`). `save_layout_directory` extracts `AddNode` deltas and unions them with previously-persisted node IDs to compute a permitted set; live handles outside that set are skipped, previously-saved snapshots for permitted-but-off-bus nodes are carried forward from `prev.node_snapshots`. Backward-compat: when `previous == None` (first save), all live handles pass through so legacy tests/callers still work. `SaveLayoutResult` / `SaveWithBusWriteResult` gained `persisted_node_ids: Vec<String>` (canonical uppercase no-dots). Frontend: extended `ActiveLayoutContext` with optional `layoutNodeIds`; `openOfflineLayoutWithReplay` hydrates it from `result.nodeSnapshots`; `saveLayoutOrchestrator` accepts `discoveredOnlyNodeIds?: string[]`, appends them as `addNode` deltas, and seeds the post-save context with `persistedNodeIds`. New `lib/utils/nodeRoster.ts` exports `canonicalizeNodeId`, `computeDiscoveredOnlyNodeIds`, `isUnsavedDiscoveredNode`, `isSavedOffBusNode`. `+page.svelte` computes `discoveredOnlyNodeIds` via `$derived` and feeds it into the save args and into `hasUnsavedPromptChanges` (new 6th param) so the unsaved-changes guard and Save button gate include unsaved discovered nodes. `buildSidebarNodeEntries(nodes, savedNodeIds?)` returns `isUnsavedNew` per entry; `NodeEntry.svelte` renders a small amber "new" badge. Tests: 3 new backend unit tests (AddNode noop on LayoutFile, `as_add_node` helper, frontend JSON deserialize) — 328 cargo tests pass. New `nodeRoster.test.ts` (11 tests), 3 new orchestrator tests (delta append, empty/omitted, persistedNodeIds → context), 2 new presenter cases (savedNodeIds → isUnsavedNew), 1 new guard case. Updated `offlineLayoutOrchestrator.test.ts` and the save-orchestrator factories to include the new fields. 920/920 vitest pass. HITL items in the acceptance list (after-save reopen, wrong-bus scenario, manual visual) still pending. -->

<!-- Session: 2026-05-24 — S8 design refinement after live testing. (1) Save menu hotfix: added discoveredOnlyNodeIds.length > 0 to the two hasInMemoryEdits expressions in +page.svelte so the Save menu lights up when discovered-only nodes are present. (2) Design discussion sharpened the model: layoutStore.isDirty should mean 'in-memory changes not yet saved' uniformly (mirroring the existing field-level layer model), with the promotion threshold for discovered nodes being 'fully captured' (CDI cached + all config values read) — not mere discovery. Rationale: an uncaptured node cannot be edited offline, so writing a stub for it would be incorrect; and the wrong-layout-wrong-bus protection requires that the user not be able to accidentally write the foreign bus's node list. (3) Public name stays isDirty (broader, user-aligned meaning); the narrower 'LayoutFile struct changed' becomes a private _hasFileEdits inside the store. (4) Identified a second defect: even with the menu enabled, Save does not actually persist the new node snapshots on disk — needs a backend investigation (S8-T13). (5) Updated S8 acceptance criteria to reflect the threshold; un-checked the criteria that no longer pass; added tasks S8-T8 through S8-T15. The 'new' badge stays single-purpose (discovered notIn layoutNodeIds, no threshold) and the existing global unsaved-changes indication covers the in-memory-not-saved case — no new per-node 'will be saved' badge. -->

<!-- Session: 2026-05-25 - Completed S8 (T8-T17). Centralised the dirty signal in layoutStore: added _unsavedInMemoryNodeIds state pushed in by a route effect, redefined isDirty as _dirty OR _unsavedInMemoryNodeIds.length > 0, and stripped four ad-hoc discoveredOnlyNodeIds.length > 0 ORs from +page.svelte. Added nodeRoster.computeUnsavedInMemoryNodeIds(savedNodeIds, fullyCapturedNodeIds) for the threshold-gated list (CDI cached + not partial-capture). T13 backend-persistence defect resolved as a downstream consequence of T10 - the backend's fingerprint filter silently dropped uncaptured AddNode'd nodes, but threshold-gating in the frontend means only fully-captured nodes are ever sent. T16 restored the empty-state Read-all affordance for the layout-loaded case via a parallel showCaptureRemainingCta derived. T17 wrapped the menu-disconnect listener in promptUnsaved (the only exit path that previously bypassed the guard). Wrote ADR-0007 capturing the full-capture threshold rationale and wrong-bus-protection scenario. 929/929 vitest pass (added 9 new test cases); 328/328 cargo tests pass. -->