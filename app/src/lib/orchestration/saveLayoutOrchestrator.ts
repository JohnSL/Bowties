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
import type { SaveLayoutResult, SaveWithBusWriteResult } from '$lib/api/layout';
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
   * Discovered node IDs (canonical, uppercase, no dots) that are visible to
   * the frontend but not yet in the saved layout roster (S8). The orchestrator
   * promotes them by appending `{ type: 'addNode', nodeIdHex }` deltas before
   * sending the save to the backend.
   *
   * Optional for backwards compatibility — when omitted, no AddNode deltas
   * are added and the save behaves exactly as before.
   */
  discoveredOnlyNodeIds?: string[];
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
}

export interface SaveLayoutOrchestratedResult {
  /** Warnings from the save (e.g. partial capture nodes). */
  warnings: string[];
  /** Bus write result (only present when saveWithBusWrites was used). */
  busWriteResult?: SaveWithBusWriteResult['busWrites'];
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
  discoveredOnlyNodeIds,
  flushPending,
  setActiveContext,
  updatePartialCaptureNodes,
  getPendingChangeCount,
  clearPersistedDrafts,
}: SaveLayoutOrchestratedArgs): Promise<SaveLayoutOrchestratedResult> {
  // 1. Flush pending offline changes (if applicable)
  if (flushPending) {
    await flushPending();
  }

  // S8: promote any unsaved discovered nodes into the layout roster by
  // appending AddNode deltas. The backend uses these (plus the previously
  // persisted snapshots) to compute the set of nodes that are permitted to
  // be written into the companion `nodes/` directory.
  const effectiveDeltas: LayoutEditDelta[] = discoveredOnlyNodeIds && discoveredOnlyNodeIds.length > 0
    ? [
        ...deltas,
        ...discoveredOnlyNodeIds.map((nodeIdHex) => ({
          type: 'addNode' as const,
          nodeIdHex,
        })),
      ]
    : deltas;

  let warnings: string[];
  let busWriteResult: SaveWithBusWriteResult['busWrites'] | undefined;
  let persistedLayout: LayoutFile;
  let persistedNodeIds: string[];

  if (saveWithBusWrites) {
    // ── Online path: backend owns all three phases + catalog rebuild ──────
    const result = await saveWithBusWrites(path, effectiveDeltas);
    warnings = result.warnings;
    busWriteResult = result.busWrites ?? undefined;
    persistedLayout = result.layout;
    persistedNodeIds = result.persistedNodeIds;
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

  return { warnings, busWriteResult };
}
