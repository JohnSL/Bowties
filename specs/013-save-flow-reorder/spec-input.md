# Spec 013: Three-Phase Save Flow Reorder

## Problem

The current online save flow writes config values to nodes first, then saves the layout file. This ordering causes two related defects:

1. **Blank bowties during and after save.** After `writeModifiedValues()`, the backend emits `node-tree-updated` events. The frontend's tree-refresh callback calls `pruneResolvedDraftsForNode()`, which removes drafts whose values now match baselines. When all drafts are pruned, `configIsDirty` becomes false and the bowtie preview switches from the slow path (tree scanning — current data) to the fast path (catalog-based — stale data). The old catalog was built before the user's config changes, so bowties show "No producers" / "No consumers."

2. **Cancel leaves blank bowties.** If the user cancels the Save dialog after config writes have already been sent to the bus, the catalog rebuild in `saveCurrentCaptureToFile` never executes. The bowties remain blank until something else triggers a catalog rebuild.

The root cause is that bus writes (irreversible, unreliable) happen before the layout file save (reversible, reliable), and bus writes disrupt the app's reactive state before the layout file captures the correct bowtie information.

## Solution

Reorder the online save into three phases:

| Phase | Action | Failure mode |
|-------|--------|-------------|
| 1 — Persist intent | Stage drafts as offline changes → show Save dialog → save layout file | File I/O — reliable, cancellable. Cancel here = nothing sent to bus. |
| 2 — Write to bus | Write the staged offline changes to nodes | May partially fail. Draft pruning fires but catalog is already correct from phase 1. |
| 3 — Reconcile | Clear succeeded offline changes → save layout file again → rebuild catalog | File I/O — reliable. |

This reuses the existing offline changes infrastructure. No new persistence mechanisms are needed.

## Why this ordering

- The layout file is the durable record of user intent. It should be saved when the data is cleanest — before bus writes disrupt reactive state.
- File saves are reliable and cancellable. Bus writes are irreversible and can fail.
- Cancel at the Save dialog (phase 1) means zero bus writes — clean abort.
- If the app crashes between phase 1 and phase 2, the layout file contains offline changes that can be replayed on next launch — existing behavior.
- The blank-bowties problem dissolves: phase 1 clears drafts via `stageDraftsForOfflineSave()` (existing), but no bus writes have happened yet, so CDI trees are unchanged and the catalog is still valid. Phase 2 triggers draft pruning, but there are no drafts left to prune. Phase 3 rebuilds the catalog from the now-updated CDI state.

## Current flow (for reference)

### Online save — `handleSave()` in SaveControls.svelte

```
1. writeModifiedValues()           ← sends config to bus, backend emits node-tree-updated
2. [if no layout edits] clearAllDrafts()
3. onOfflineSave()                 ← shows Save dialog, calls saveCurrentCaptureToFile()
   3a. saveLayoutFile()
   3b. buildBowtieCatalog() + setCatalog()
4. bowtieMetadataStore.clearAll(), layoutStore.markClean()
5. [deferred] clearAllDrafts()
```

**Bug:** Between steps 1 and 3b, pruneResolvedDraftsForNode (triggered by node-tree-updated) clears drafts → configIsDirty=false → preview uses stale catalog → blank bowties.

### Offline save — `handleSave()` in SaveControls.svelte

```
1. stageDraftsForOfflineSave()     ← stages to offline changes, clears drafts
2. onOfflineSave()                 ← shows Save dialog, calls saveCurrentCaptureToFile()
   2a. flushPendingToBackend()
   2b. saveLayoutFile()
   2c. buildBowtieCatalog() + setCatalog()
3. offlineChangesStore.reloadFromBackend()
4. configChangesStore.clearAllDrafts()
5. bowtieMetadataStore.clearAll(), layoutStore.markClean()
```

**No bug:** Drafts are cleared in step 1 before any disruptive side effects. The offline save path already has the correct ordering.

## New flow

### Online save — `handleSave()` rewritten

```
Phase 1 — Persist intent:
  1. stageDraftsForOfflineSave()     ← stages to offline changes, clears drafts (existing)
  2. Show Save dialog → saveLayoutFile()  (via saveCurrentCaptureToFile)
  3. If cancelled → unstage offline changes, restore drafts → return
  4. buildBowtieCatalog() + setCatalog()
  5. bowtieMetadataStore.clearAll(), layoutStore.markClean()

Phase 2 — Write to bus:
  6. Write staged offline changes to nodes (new: "replay offline changes online")
  7. Progress feedback during writes

Phase 3 — Reconcile:
  8. Clear succeeded offline changes from the store
  9. Save layout file again (removes offline changes from the persisted file)
  10. Rebuild catalog from post-write CDI state
  11. offlineChangesStore.reloadFromBackend()
```

### Offline save — unchanged

The offline save path already has the correct ordering and continues to work as-is.

## Scope boundaries

### In scope

- Reorder online save flow in `SaveControls.svelte` to use three phases
- Add "replay offline changes to bus" capability (write staged offline changes to live nodes)
- Cancel handling: unstage offline changes if Save dialog is cancelled in phase 1
- Phase 3 reconciliation: clear succeeded changes, re-save layout, rebuild catalog
- Partial failure handling: keep failed offline changes staged for retry
- Progress feedback across all three phases
- Persist all resolved event role classifications in the layout file during save (not just user overrides for ambiguous slots)
- Update `saveLayoutOrchestrator.ts` if the orchestrated sequence changes
- Update tests in `SaveControls.test.ts` and `saveControlsPresenter.test.ts`

### Out of scope (separate work)

- Pending-writes marker in layout file for crash recovery (can be added later; YAGNI for now)
- Changes to the offline save path (already correct)
- Changes to `writeModifiedValues` backend command itself

## Key modules affected

| Module | Change |
|--------|--------|
| `SaveControls.svelte` | Reorder `handleSave()` for online path to use three phases |
| `configDraftOrchestrator.ts` | Add `unstageDraftsFromOfflineSave()` for cancel recovery; add `replayOfflineChangesToBus()` for phase 2 |
| `offlineChangesStore.svelte.ts` | May need method to identify which changes were just staged (for cancel rollback and selective clearing) |
| `saveLayoutOrchestrator.ts` | Update orchestrated sequence if phase 1/3 split requires it |
| `+page.svelte` | `saveCurrentCaptureToFile` may need adjustment for the new call pattern |
| `saveControlsPresenter.ts` | Update if progress state derivation changes |
| `commands/bowties.rs` or `layout_capture.rs` | Capture all resolved roles from catalog into `layout.role_classifications` before save |
| `layout/types.rs` | No schema change needed — `role_classifications` map already exists |
| Tests | Update for new flow ordering, new cancel behavior, and role persistence |

## Design details

### Phase 2: Replay offline changes to bus

This is the main new capability. The app needs to take staged offline changes and write them to live nodes. This is conceptually similar to the sync-apply flow but scoped to "changes staged in this save session."

Options:
- **A. Reuse `writeModifiedValues()`:** Stage offline changes back into the backend tree as `modified_value` entries, then call `writeModifiedValues()`. Pro: reuses existing write path. Con: round-trips through the backend tree state.
- **B. New backend command:** A `write_offline_changes_to_bus()` command that reads offline change rows and writes them to nodes directly. Pro: clean separation. Con: new IPC surface.
- **C. Use `setModifiedValue` per change + `writeModifiedValues`:** Loop through staged offline changes, call `setModifiedValue` for each, then batch-write with `writeModifiedValues`. Pro: reuses existing APIs entirely. Con: N+1 IPC calls.

**Recommended: Option C.** It reuses existing APIs without new backend commands. The `setModifiedValue` calls set `modified_value` on each tree leaf, then `writeModifiedValues` batch-writes them. This is the same path the online edit flow uses today, just sourced from offline changes instead of UI edits. The N+1 IPC overhead is acceptable because N is typically small (a handful of config fields per save).

### Cancel recovery (phase 1)

If the user cancels the Save dialog after `stageDraftsForOfflineSave()` has run:

1. The offline changes store has new draft rows that were just staged.
2. Config drafts have been cleared by `stageDraftsForOfflineSave()`.

Recovery:
- Revert the offline change drafts: `offlineChangesStore.revertAllPending()` restores to the last saved snapshot.
- Restore config drafts: this requires either (a) re-reading them from the offline change rows before reverting, or (b) keeping a snapshot of the draft entries before staging.

**Recommended:** Before calling `stageDraftsForOfflineSave()`, snapshot the draft entries. If cancelled, restore them to `configChangesStore` and revert the offline changes. A helper `unstageDraftsFromOfflineSave(snapshot)` in `configDraftOrchestrator.ts` can own this.

### Progress feedback

Three-phase progress display:

| Phase | Progress label | Count source |
|-------|---------------|-------------|
| 1 | "Saving layout file…" | 1 step |
| 2 | "Writing configuration…" | Per-field from `writeModifiedValues` result |
| 3 | "Updating layout…" | 1 step |

The `SaveProgress` type already supports `state`, `total`, `completed`, `failed`, and `currentFieldLabel`. No type changes needed.

### Partial failure in phase 2

If some writes fail:
- Failed offline changes remain in the offline changes store.
- Phase 3 saves the layout file with the remaining offline changes.
- The UI shows the failure count and leaves Save/Discard visible.
- The user can retry (clicking Save again will attempt to write the remaining changes).

This matches the existing partial-failure behavior.

### Event role persistence during save

The layout file already has a `role_classifications` map (`BTreeMap<String, RoleClassification>` in `layout/types.rs`), but it currently only stores user-classified overrides for ambiguous slots. Roles resolved automatically via protocol exchange (`query_event_roles`, Tier 0) are not persisted. When reopening offline, protocol exchange is unavailable, so those slots fall back to profile heuristics or CDI keyword matching — which may fail, producing "Unknown role."

**Fix:** During save, populate `role_classifications` with ALL resolved roles from the current catalog, not just user overrides. Specifically:

1. After rebuilding the catalog (end of phase 1, and again at end of phase 3), extract the role for every `EventSlotEntry` in every `BowtieCard`'s `producers`, `consumers`, and `ambiguous_entries` lists.
2. Write these into `layout.role_classifications` using the existing key format `"{nodeId}:{element_path.join("/")}"` and value `{ role: "Producer" | "Consumer" }`.
3. Skip entries with `role == Ambiguous` (no resolved role to persist).

This means the layout file captures the full role knowledge at save time. When reopening offline, `merge_layout_metadata` already applies `role_classifications` to reclassify ambiguous entries — so the existing merge logic handles the read side with no changes.

**Where to put this logic:** A helper function in `commands/bowties.rs` (or `layout_capture.rs`) that takes the rebuilt `BowtieCatalog` and returns the full `role_classifications` map. Called by the save orchestration before `saveLayoutFile`.

**What changes in `layoutStore` / `+page.svelte`:** Before saving, call the helper to update `layout.role_classifications` from the current catalog. This ensures the layout file always contains the most complete role information available.

## Implementation order

### Task 1: Add cancel-recovery helpers to configDraftOrchestrator

Add `snapshotDraftEntries()` and `unstageDraftsFromOfflineSave(snapshot)` to `configDraftOrchestrator.ts`. These allow staging to be undone if the Save dialog is cancelled.

**Test:** Unit test that stages drafts, then unstages them, verifying configChangesStore and offlineChangesStore return to pre-stage state.

### Task 2: Add replay-offline-changes-to-bus orchestration

Add `replayOfflineChangesToBus()` to `configDraftOrchestrator.ts` (or a new `saveOrchestrator` module). This reads staged offline changes, calls `setModifiedValue` for each, then calls `writeModifiedValues`.

**Test:** Unit test with mocked IPC that verifies the correct `setModifiedValue` calls are made from offline change rows, followed by `writeModifiedValues`.

### Task 3: Rewrite online handleSave() in SaveControls.svelte

Replace the current online save flow with the three-phase sequence:
- Phase 1: snapshot drafts → stage → save dialog → save file → rebuild catalog. On cancel: unstage + restore.
- Phase 2: replay offline changes to bus → progress feedback.
- Phase 3: clear succeeded offline changes → re-save layout → rebuild catalog.

**Test:** Update `SaveControls.test.ts` for new flow ordering, cancel behavior, and partial failure.

### Task 4: Update saveCurrentCaptureToFile in +page.svelte

Adjust `saveCurrentCaptureToFile` for the new call pattern. It may need to skip `flushPendingToBackend()` when called from the online path (the offline changes are written to bus in phase 2, not flushed to backend persistence in phase 1).

**Test:** Verify existing layout save tests still pass.

### Task 5: Update saveLayoutOrchestrator

If the phase 1 and phase 3 save steps diverge from the current `saveLayoutOrchestrated` sequence, update the orchestrator or create phase-specific helpers.

**Test:** Update `saveLayoutOrchestrator.test.ts`.

### Task 6: Persist all resolved event roles during save

Add a helper (backend or frontend) that extracts all resolved roles from a `BowtieCatalog` into a `role_classifications` map. Call it before each `saveLayoutFile` to populate `layout.role_classifications` with every producer/consumer classification from the catalog.

**Test:** Unit test that given a catalog with producers, consumers, and ambiguous entries, produces the correct `role_classifications` map (producers and consumers included, ambiguous entries excluded). Integration test: save a layout file online, reload offline, verify roles are preserved.

### Task 7: Verify full test suite and manual test

- Run full test suite (790+ tests).
- Manual test: create connections → Save → verify bowties stay correct during dialog → confirm save → verify bowties correct after.
- Manual test: create connections → Save → cancel dialog → verify bowties still correct → Save again → confirm → verify file saved correctly.
- Manual test: partial write failure → verify failed changes remain staged → retry.
- Manual test: save with same-board producer/consumer → disconnect → close layout → reopen layout → verify roles are correct (not "Unknown role").

## Risks

- **Timing of node-tree-updated during phase 2:** After `writeModifiedValues` in phase 2, `node-tree-updated` events fire and `pruneResolvedDraftsForNode` runs. But config drafts are already empty (cleared in phase 1), so pruning is a no-op. The catalog was rebuilt at the end of phase 1, so the fast path uses correct data. Phase 3 rebuilds the catalog again with post-write CDI state. This should be safe.

- **Double file save:** Phase 1 and phase 3 each save the layout file. This is two file writes per save. File writes are fast and atomic (temp → flush → rename), so this is acceptable.

- **Interaction with `triggerSaveAs`:** The `triggerSaveAs()` method in SaveControls also saves. It should use the same three-phase flow. Verify during implementation.

## Relationship to other work

- **Edit layer refactor (backlog):** The edit layer refactor replaces `leaf.modifiedValue` with the changes module. The three-phase save flow is compatible with that refactor — it uses the changes module's `draftEntries()` and `clearAllDrafts()` which already exist.
- **Offline role persistence:** The "Unknown role" issue when reopening a layout file offline is addressed by Task 6 — persisting all resolved roles in `layout.role_classifications` during save.
- **Spec 009 T019:** The original "unified save flow" task specified "write to nodes first, then save layout." This spec supersedes that ordering decision.
