# Sync Panel Workflow

## Purpose

This document describes the current sync panel workflow: how a sync session is built, how rows are classified, how the user resolves conflicts, and how apply semantics work. It is the authoritative reference for these behaviors.

The sync panel is the mechanism by which offline edits captured in an offline layout file are pushed back to the physical LCC bus when the user reconnects.

---

## When The Sync Panel Appears

The sync panel appears automatically when all of the following are true on reconnect:

1. An offline layout is open with pending offline changes (`pendingCount > 0`).
2. Discovery has settled (all SNIP/PIP enrichment complete).
3. Layout node match classification is `likely_same` (≥ 80% overlap with discovered nodes).
4. The sync panel has not already been dismissed this session (`isDismissed === false`).

If any condition is not met, the sync panel does not appear. The user can open it manually later from the toolbar.

**The sync panel does not re-open automatically after being dismissed.** `isDismissed` is only reset by `resetSyncSessionAutoTrigger()`, which runs during `connect_fresh_live` or `startup_fresh_live` transitions (i.e., when no offline layout is loaded).

---

## Sync Mode Choice

Before presenting rows, the user must choose a sync mode:

| Mode | Meaning |
|---|---|
| `target_layout_bus` | The connected bus is the same physical layout the file was captured from. Apply changes to these nodes. |
| `bench_other_bus` | The connected bus is a different setup (e.g., a test bench). Dismiss the panel immediately without applying. |

**Owner:** `syncPanelViewOrchestrator.ts` — `applySyncModeChoice()`

If `bench_other_bus` is selected, the panel dismisses immediately and no session rows are loaded.

If `target_layout_bus` is selected, the backend loads and classifies the session rows.

---

## Session Building

Session rows are built by the backend command `build_sync_session` in `app/src-tauri/src/commands/sync_panel.rs`.

The backend:

1. Reads `offline-changes.yaml` from the offline layout directory.
2. For each pending change, fetches the current live bus value for the corresponding CDI address.
3. Classifies each row into one of four buckets.
4. Prunes already-applied rows from the offline-changes file.

**Row classification:**

| Classification | Condition |
|---|---|
| `clean` | Bus value matches the planned value — nothing to apply |
| `conflict` | Bus value differs from both the baseline and the planned value |
| `alreadyApplied` | Bus value already equals the planned value — pruned from offline-changes.yaml |
| `nodeMissing` | The node holding this change is not present on the live bus |

The session delivered to the frontend (`SyncSession`) contains:

- `conflictRows` — changes with bus values that differ from plan (user must choose apply or skip)
- `cleanRows` — changes where bus already equals plan or baseline applies cleanly
- `alreadyAppliedCount` — pruned rows, reported as a count only
- `nodeMissingRows` — rows targeting nodes not present on the bus

---

## Row Resolution And The Apply Button

### Conflict rows

For each conflict row, the user must choose:

- **Apply** — write the planned value to the bus, overwriting the current bus value
- **Skip** — leave the bus value unchanged

The Apply button is disabled until all conflict rows have a resolution.

### Clean rows

Clean rows are selected by default. The user may deselect individual clean rows to exclude them from apply.

### Node-missing rows

Rows for missing nodes are shown for awareness but cannot be applied. They remain in `offline-changes.yaml` until the user reconnects with matching nodes present.

### Apply count

The Apply button shows the count of rows that will be written (resolved-apply conflicts + selected clean rows). The button is disabled when the apply count is zero or when `isApplying === true`.

**Owner:** `syncPanel.svelte.ts` — `canApply`, `applyCount`, `allConflictsResolved`

---

## Apply Execution

**Owner:** `syncPanelViewOrchestrator.ts` — `applySyncSelectionAndReconcile()`

Steps:

1. `store.applySelected()` calls the backend `apply_sync_changes` command.
2. The backend writes selected rows to the bus and returns an `ApplySyncResult`.
3. `syncApplyOrchestrator.ts` — `reconcileOfflineTreesAfterSyncApply()` runs post-apply cleanup:
   - Reloads `offlineChangesStore` from the backend (already-applied rows removed).
   - Rebuilds node trees for all affected nodes by fetching fresh offline tree snapshots.
   - Restamps offline pending values onto the rebuilt trees.
4. If no rows failed, the sync panel dismisses automatically.
5. If some rows failed, the panel remains open showing failure details.

### Apply Result Shape

```ts
interface ApplySyncResult {
  applied: string[];          // changeIds successfully written
  skipped: string[];          // changeIds the user chose to skip
  failed: Array<{ changeId: string; reason: string }>; // write failures
  readOnlyCleared: string[];  // rows cleared without writing (already-applied, node-missing cleanup)
}
```

---

## Dismiss Without Applying

The user can dismiss the sync panel at any time without applying. `isDismissed` is set to `true`. The offline changes remain in `offline-changes.yaml` and are not modified.

The panel can be manually re-opened from the toolbar as long as the layout is open and the bus is connected.

---

## Layout Match Classification

Before the sync panel auto-triggers, `computeLayoutMatchStatus()` is called with the discovered node IDs. This compares them against the node IDs in the offline layout file.

| Classification | Threshold | Meaning |
|---|---|---|
| `likely_same` | ≥ 80% overlap | Auto-trigger the sync panel |
| `uncertain` | 40–79% overlap | Do not auto-trigger; user may open manually |
| `likely_different` | < 40% overlap | Do not auto-trigger; likely a different layout or bus |

**Owner:** `syncPanel.svelte.ts` — `computeMatch()`, fed by `syncSessionOrchestrator.ts`

---

## Partial Apply Semantics

Apply is partial by design: the user can skip conflicts, deselect clean rows, and leave node-missing rows unresolved. After a partial apply:

- Applied rows are removed from `offline-changes.yaml` by the backend.
- Skipped rows remain in `offline-changes.yaml` with `status: skipped`.
- Node-missing rows remain unchanged.
- The pending change count updates after the reload.
- The user can re-open the sync panel and attempt another partial apply.

---

## Sources

- `app/src/lib/stores/syncPanel.svelte.ts`
- `app/src/lib/orchestration/syncPanelViewOrchestrator.ts`
- `app/src/lib/orchestration/syncSessionOrchestrator.ts`
- `app/src/lib/orchestration/syncApplyOrchestrator.ts`
- `app/src/lib/api/sync.ts`
- `app/src-tauri/src/commands/sync_panel.rs`
- `specs/010-offline-layout-editing/spec.md` (Phases 5–6)
