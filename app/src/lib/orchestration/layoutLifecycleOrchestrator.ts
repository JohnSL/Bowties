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
 * Dispatch is registry-driven: every layout-scoped store or orchestrator
 * implements `LayoutScopedParticipant` and is registered in the
 * `layoutScopedParticipants` array. Adding a new participant means
 * implementing the interface and appending to the array — the dispatch
 * loop handles both lifecycle events automatically.
 */

import type { LayoutEditDelta } from '$lib/types/bowtie';
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
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { eventStateStore } from '$lib/stores/eventState.svelte';
import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
import { saveProgressStore } from '$lib/stores/saveProgress.svelte';
import { syncPanelStore } from '$lib/stores/syncPanel.svelte';
import { cdiCacheStore } from '$lib/stores/cdiCache.svelte';
import { connectorSlotFocusStore } from '$lib/stores/connectorSlotFocus.svelte';
import { facilityCascadeOrchestrator } from '$lib/orchestration/facilityCascadeOrchestrator.svelte';
import { configDraftMirrorOrchestrator } from '$lib/orchestration/configDraftMirrorOrchestrator.svelte';

import type { CloseLayoutResult } from '$lib/api/layout';

// ─── Lifecycle participant interface ─────────────────────────────────────────

export interface LayoutScopedParticipant {
  resetForNewLayout?(): void;
  resetForFreshLiveSession?(): void;
  collectDeltas?(): LayoutEditDelta[];
}

const configReadStatusParticipant: LayoutScopedParticipant = {
  resetForNewLayout() { clearConfigReadStatus(); },
  resetForFreshLiveSession() { clearConfigReadStatus(); },
};

/**
 * Registry of all stores and orchestrators that participate in layout-scoped
 * lifecycle resets. The orchestrator dispatches to each participant via the
 * `LayoutScopedParticipant` interface instead of manual enumeration.
 */
export const layoutScopedParticipants: LayoutScopedParticipant[] = [
  partialCaptureNodesStore,
  nodeRoster,
  bowtieMetadataStore,
  offlineChangesStore,
  configChangesStore,
  layoutStore,
  connectorSelectionsStore,
  channelsStore,
  facilitiesStore,
  facilityCascadeOrchestrator,
  configDraftMirrorOrchestrator,
  configSidebarStore,
  nodeTreeStore,
  bowtieCatalogStore,
  saveProgressStore,
  syncPanelStore,
  cdiCacheStore,
  connectorSlotFocusStore,
  eventStateStore,
  configReadStatusParticipant,
];

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

    for (const p of layoutScopedParticipants) {
      p.resetForNewLayout?.();
    }

    afterReset?.();

    if (connected && reprobeLiveNodes && probeForNodes) {
      await probeForNodes();
    }
  }

  /**
   * Disconnect/reconnect path when no layout is loaded. Drops the live
   * roster but keeps placeholders (they are layout-scoped). Matches the
   * pre-refactor behavior of `resetFreshLiveSessionState`.
   *
   * Spec 016 / S2: also clears the event state store so PCER events from
   * a prior bus session never bleed into a fresh connect.
   */
  resetForFreshLiveSession(): void {
    if (layoutStore.hasLayoutFile) return;

    for (const p of layoutScopedParticipants) {
      p.resetForFreshLiveSession?.();
    }
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
