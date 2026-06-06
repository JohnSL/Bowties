/**
 * Save-layout orchestrator — owns the full save-then-rebuild lifecycle.
 *
 * ADR-0002: the backend is the sole owner of layout file data. The frontend
 * sends structured edit deltas (not a full LayoutFile) and hydrates its
 * layout store from the backend's response.
 *
 * Supports two paths:
 * - **Online path** (`saveWithBusWrites` provided): delegates the full three-phase
 *   save (layout → bus writes → reconcile → catalog rebuild) to the backend.
 * - **Offline path** (`saveFile` + `rebuildCatalog` + `setCatalog` provided): the
 *   original two-step flow used for offline-only saves.
 *
 * Callers do not need to remember individual cleanup steps; this function owns:
 * flush → save → hydrate layout → rebuild catalog → set context → update partial
 * nodes → clear metadata → mark clean.
 */

import type { LayoutFile, LayoutEditDelta } from '$lib/types/bowtie';
import type { SaveLayoutResult, SaveWithBusWriteResult, OfflineNodeSnapshot } from '$lib/api/layout';
import type { ActiveLayoutContext } from '$lib/stores/layout.svelte';
import { normalizeLayoutTitle } from '$lib/utils/layoutPath';

export interface SaveLayoutOrchestratedArgs {
  // ── Online path ─────────────────────────────────────────────────────────
  /**
   * Three-phase save command (wraps `save_layout_with_bus_writes` IPC).
   * When provided, `saveFile`, `rebuildCatalog`, and `setCatalog` are ignored.
   */
  saveWithBusWrites?: (path: string, deltas: LayoutEditDelta[]) => Promise<SaveWithBusWriteResult>;

  // ── Offline path ─────────────────────────────────────────────────────────
  /** Persist the layout file to disk (wraps saveLayoutDirectory IPC). */
  saveFile?: (path: string, deltas: LayoutEditDelta[]) => Promise<SaveLayoutResult>;
  /** Rebuild the bowtie catalog with layout metadata merged in. */
  rebuildCatalog?: (layout: LayoutFile | null) => Promise<import('$lib/api/tauri').BowtieCatalog>;
  /** Apply the rebuilt catalog to the store. */
  setCatalog?: (catalog: import('$lib/api/tauri').BowtieCatalog) => void;

  // ── Shared ───────────────────────────────────────────────────────────────
  /** Clear all pending bowtie metadata edits. */
  clearMetadata: () => void;
  /** Mark the layout store as clean (no unsaved changes). */
  markClean: () => void;
  /** Hydrate the layout store from the backend's persisted copy. */
  hydrateLayout: (layout: LayoutFile) => void;
  /** The filesystem path to save to. */
  path: string;
  /** Edit deltas to send to the backend (ADR-0002). */
  deltas: LayoutEditDelta[];
  /**
   * Node keys (real or placeholder) that are visible to the frontend but
   * not yet persisted in the layout. The orchestrator promotes them by
   * appending `{ type: 'addNode', nodeKey }` deltas before sending the
   * save to the backend.
   *
   * Replaces the S8 `discoveredOnlyNodeIds` + S8.5 `unsavedPlaceholders`
   * with a single unified set (S8.11).
   */
  inMemorySnapshotKeys?: string[];
  /**
   * Node keys that were persisted in the open layout but have been
   * removed in-memory and not yet saved. The orchestrator appends a
   * `{ type: 'removeNode', nodeKey }` delta for each before sending.
   * Symmetric to `inMemorySnapshotKeys` (S8 / Bug 3).
   */
  inMemoryRemovedKeys?: string[];
  /** Optional: flush pending offline changes to backend before saving. */
  flushPending?: () => Promise<void>;
  /** Update the active layout context in the store after save. */
  setActiveContext: (context: ActiveLayoutContext) => void;
  /** Update the set of partially-captured node IDs from save warnings. */
  updatePartialCaptureNodes: (warnings: string[]) => void;
  /** Returns the current pending offline change count for the context update. */
  getPendingChangeCount: () => number;
  /**
   * Clear `configChangesStore` drafts that have now been persisted (ADR-0004).
   *
   * Called after a successful save once the rebuilt catalog has been applied,
   * so the effective read model never observes a window where the catalog
   * has been swapped but stale drafts still mask it.
   *
   * Optional for backwards compatibility — callers that do not own a config
   * draft layer simply omit it.
   */
  clearPersistedDrafts?: () => void;
  /**
   * Drop in-memory placeholder roster entries that have been persisted by
   * this save (S8.11). Receives the list of placeholder `nodeKey`s that
   * were among the `inMemorySnapshotKeys`. Called after the rebuilt catalog
   * / layout have been applied, mirroring `clearPersistedDrafts`.
   *
   * Optional for backwards compatibility.
   */
  clearPersistedPlaceholders?: (nodeKeys: string[]) => void;
  /**
   * Drop the persisted-removals set after a successful save. Symmetric to
   * `clearPersistedPlaceholders`.
   */
  clearPersistedRemovals?: () => void;
  /**
   * Reload offline-change rows from the backend after a successful save.
   * Only supplied in offline mode — the backend rewrites pending changes
   * during the save, so the frontend mirror must be re-pulled. Online
   * mode omits this callback (no offline-change state to reload).
   */
  reloadOfflineChanges?: () => Promise<void>;
}

export interface SaveLayoutOrchestratedResult {
  /** Warnings from the save (e.g. partial capture nodes). */
  warnings: string[];
  /** Bus write result (only present when saveWithBusWrites was used). */
  busWriteResult?: SaveWithBusWriteResult['busWrites'];
  /** Node snapshots persisted by this save. The page caches these so
   *  disconnect can rehydrate the offline view (Bug 2b fix). */
  nodeSnapshots: OfflineNodeSnapshot[];
}

/**
 * Save the layout and rebuild the bowtie catalog in a single orchestrated
 * sequence, ensuring the catalog always reflects the persisted metadata after
 * every save.
 *
 * Throws on any step failure — callers should handle errors.
 */
export async function saveLayoutOrchestrated({
  saveWithBusWrites,
  saveFile,
  rebuildCatalog,
  setCatalog,
  clearMetadata,
  markClean,
  hydrateLayout,
  path,
  deltas,
  inMemorySnapshotKeys,
  inMemoryRemovedKeys,
  flushPending,
  setActiveContext,
  updatePartialCaptureNodes,
  getPendingChangeCount,
  clearPersistedDrafts,
  clearPersistedPlaceholders,
  clearPersistedRemovals,
  reloadOfflineChanges,
}: SaveLayoutOrchestratedArgs): Promise<SaveLayoutOrchestratedResult> {
  // 1. Flush pending offline changes (if applicable)
  if (flushPending) {
    await flushPending();
  }

  // S8.11: promote any in-memory nodes (real or placeholder) into the
  // layout roster by appending unified `AddNode { nodeKey }` deltas.
  // The backend uses these (plus the previously persisted snapshots) to
  // compute the set of nodes permitted in the companion `nodes/` directory.
  const addNodeDeltas: LayoutEditDelta[] = inMemorySnapshotKeys
    ? inMemorySnapshotKeys.map((nodeKey) => ({ type: 'addNode' as const, nodeKey }))
    : [];
  const removeNodeDeltas: LayoutEditDelta[] = inMemoryRemovedKeys
    ? inMemoryRemovedKeys.map((nodeKey) => ({ type: 'removeNode' as const, nodeKey }))
    : [];
  const effectiveDeltas: LayoutEditDelta[] =
    addNodeDeltas.length === 0 && removeNodeDeltas.length === 0
      ? deltas
      : [...deltas, ...addNodeDeltas, ...removeNodeDeltas];

  let warnings: string[];
  let busWriteResult: SaveWithBusWriteResult['busWrites'] | undefined;
  let persistedLayout: LayoutFile;
  let persistedNodeIds: string[];
  let nodeSnapshots: OfflineNodeSnapshot[];

  if (saveWithBusWrites) {
    // ── Online path: backend owns all three phases + catalog rebuild ──────
    const result = await saveWithBusWrites(path, effectiveDeltas);
    warnings = result.warnings;
    busWriteResult = result.busWrites ?? undefined;
    persistedLayout = result.layout;
    persistedNodeIds = result.persistedNodeIds;
    nodeSnapshots = result.nodeSnapshots;
  } else {
    // ── Offline path: explicit save → rebuild → set catalog ───────────────
    if (!saveFile || !rebuildCatalog || !setCatalog) {
      throw new Error(
        'saveFile, rebuildCatalog, and setCatalog are required when saveWithBusWrites is not provided',
      );
    }
    const result = await saveFile(path, effectiveDeltas);
    warnings = result.warnings;
    persistedLayout = result.layout;
    persistedNodeIds = result.persistedNodeIds;
    nodeSnapshots = result.nodeSnapshots;

    const catalog = await rebuildCatalog(persistedLayout);
    setCatalog(catalog);
  }

  // 2. ADR-0002: hydrate layout store from backend's authoritative copy.
  hydrateLayout(persistedLayout);

  // 3. Update partial capture nodes
  updatePartialCaptureNodes(warnings);

  // 4. Update the active layout context (S8: include the post-save node roster
  //    so the dirty signal recomputes — any previously-unsaved discovered
  //    nodes that were promoted in this save are now in `layoutNodeIds`).
  const layoutId = normalizeLayoutTitle(path) ?? 'layout';
  setActiveContext({
    layoutId,
    rootPath: path,
    mode: 'offline_file',
    capturedAt: new Date().toISOString(),
    pendingOfflineChangeCount: getPendingChangeCount(),
    layoutNodeIds: persistedNodeIds,
  });

  // 5. Clear pending state
  clearMetadata();
  markClean();

  // 6. ADR-0004: drop config drafts whose values are now persisted on disk,
  //    so the effective read model never reads a stale draft after the
  //    catalog has been rebuilt from the saved layout.
  if (clearPersistedDrafts) {
    clearPersistedDrafts();
  }

  // 7. S8.5 / T8: drop in-memory placeholder entries whose snapshots are
  //    now persisted on disk. The frontend stores (`nodeInfoStore` etc.)
  //    keep the placeholder under the same NodeKey — it just stops being
  //    "in-memory only" from the save-flush composer's perspective.
  if (clearPersistedPlaceholders && inMemorySnapshotKeys && inMemorySnapshotKeys.length > 0) {
    // Only placeholder keys need clearing — real nodes don't have in-memory
    // roster entries that need cleanup.
    const placeholderKeys = inMemorySnapshotKeys.filter((k) => k.startsWith('placeholder:'));
    if (placeholderKeys.length > 0) {
      clearPersistedPlaceholders(placeholderKeys);
    }
  }

  // 8. Drop the persisted-removals set — the backend has now applied the
  //    `removeNode` deltas and pruned the corresponding node files.
  if (clearPersistedRemovals) {
    clearPersistedRemovals();
  }

  // 9. Offline mode only: reload offline-change rows from the backend so
  //    the frontend mirror matches the rewritten pending set. Online mode
  //    omits this callback.
  if (reloadOfflineChanges) {
    await reloadOfflineChanges();
  }

  return { warnings, busWriteResult, nodeSnapshots };
}
