/**
 * layoutLifecycleOrchestrator — single owner of the in-memory reset
 * sequences the layout lifecycle depends on (ADR-0011, extends ADR-0004).
 *
 * Two named entry points:
 *
 *   - `resetForNewLayout()`    : full teardown — used by close-layout,
 *                                  create-new-layout, and the "no layout"
 *                                  recovery path. Clears live AND
 *                                  placeholder roster, trees, config-read
 *                                  status, drafts, metadata, offline
 *                                  changes, and the LayoutFile.
 *
 *   - `resetForFreshLiveSession()` : disconnect/reconnect teardown when
 *                                  no layout is loaded. Clears the live
 *                                  roster but preserves placeholders
 *                                  (placeholders are layout-scoped, not
 *                                  bus-scoped).
 *
 * Imports stores directly rather than taking callback bags so the next
 * facade input cannot be added without also extending the reset path —
 * the orchestrator is the single resetter `effectiveNodeStore` depends on.
 */

import { nodeRoster } from '$lib/stores/nodeRoster.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { clearConfigReadStatus } from '$lib/stores/configReadStatus';
import { partialCaptureNodesStore } from '$lib/stores/partialCaptureNodes.svelte';
import { layoutStore, type ActiveLayoutMode } from '$lib/stores/layout.svelte';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import { configSidebarStore } from '$lib/stores/configSidebar';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';

import type { CloseLayoutResult } from '$lib/api/layout';

export interface ResetForNewLayoutOptions {
  /** Whether the bus is currently connected (drives `probeForNodes`). */
  connected: boolean;
  /**
   * When `true` and `connected`, re-probe live nodes after the reset to
   * repopulate the empty roster. Default `true`.
   */
  reprobeLiveNodes?: boolean;
  /** Probe callback, supplied by the route since IPC lives there. */
  probeForNodes?: () => Promise<void>;
  /** Optional post-reset side-effects scoped to the route (snapshots, etc.). */
  afterReset?: () => void;
}

export interface CloseLayoutOptions extends ResetForNewLayoutOptions {
  /** Active layout's mode at the time of close. Selects backend close vs. legacy-recent clear. */
  activeMode: ActiveLayoutMode | null | undefined;
  /** Backend `close_layout` IPC. */
  closeLayoutIpc: (decision: 'discard') => Promise<CloseLayoutResult>;
  /** Backend "forget the recent-layout path" IPC, used for the legacy_file path. */
  clearRecentLayout: () => Promise<void>;
  /** Reporter invoked when `clearRecentLayout` throws on the legacy_file path. */
  onRecentLayoutClearError?: (error: unknown) => void;
  /**
   * Called after the backend confirms the close but before frontend stores are
   * wiped. Required when `connected` is true so the bus is torn down and the
   * connection indicator is cleared as part of the close lifecycle.
   * Skipped when `connected` is false or when the backend refuses to close.
   */
  disconnectBeforeClose?: () => Promise<void>;
}

class LayoutLifecycleOrchestrator {
  /**
   * R7 fix: also clears placeholders so a previously-loaded placeholder
   * roster does not bleed into the next layout. Calls `clearLayoutScope`
   * (full roster teardown) rather than `replaceLiveRoster([])`
   * (placeholder-preserving).
   */
  async resetForNewLayout(opts: ResetForNewLayoutOptions): Promise<void> {
    const { connected, reprobeLiveNodes = true, probeForNodes, afterReset } = opts;

    partialCaptureNodesStore.clear();
    nodeRoster.clearLayoutScope(); // clears nodeInfoStore + nodeTreeStore + configReadStatus + profile stems
    clearConfigReadStatus();         // belt-and-braces; clearLayoutScope already does this
    bowtieMetadataStore.clearAll();
    offlineChangesStore.clear();
    configChangesStore.clearAllDrafts();
    layoutStore.reset();
    connectorSelectionsStore.reset();
    configSidebarStore.reset();
    nodeTreeStore.reset();

    afterReset?.();

    if (connected && reprobeLiveNodes && probeForNodes) {
      await probeForNodes();
    }
  }

  /**
   * Disconnect/reconnect path when no layout is loaded. Drops the live
   * roster but keeps placeholders (they are layout-scoped). Matches the
   * pre-refactor behavior of `resetFreshLiveSessionState`.
   */
  resetForFreshLiveSession(): void {
    if (layoutStore.hasLayoutFile) return;

    nodeRoster.replaceLiveRoster([]);
    clearConfigReadStatus();
    configSidebarStore.reset();
    nodeTreeStore.reset();
    connectorSelectionsStore.reset();
  }

  /**
   * Full layout close — owns both sides of the IPC boundary.
   *
   * Order pinned by ADR-0011 (2026-05-31 extension):
   *   1. Backend `close_layout('discard')` for offline layouts, otherwise
   *      `clear_recent_layout` for the legacy_file path. The backend clears
   *      its `node_registry` placeholder proxies inside `close_layout`.
   *   2. Frontend `resetForNewLayout` to wipe every store the facade reads.
   *
   * Returns `false` when the backend refused to close (e.g. cancellation),
   * leaving frontend state untouched.
   */
  async closeLayout(opts: CloseLayoutOptions): Promise<boolean> {
    const {
      activeMode,
      closeLayoutIpc,
      clearRecentLayout,
      onRecentLayoutClearError,
      disconnectBeforeClose,
      ...resetOpts
    } = opts;

    if (activeMode === 'offline_file') {
      const result = await closeLayoutIpc('discard');
      if (!result.closed) return false;
    } else {
      try {
        await clearRecentLayout();
      } catch (error) {
        onRecentLayoutClearError?.(error);
      }
    }

    if (resetOpts.connected && disconnectBeforeClose) {
      await disconnectBeforeClose();
    }

    await this.resetForNewLayout(resetOpts);
    return true;
  }
}

export const layoutLifecycleOrchestrator = new LayoutLifecycleOrchestrator();
