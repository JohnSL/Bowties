/**
 * Config Draft Backend Mirror — reactive owner of the sync-config-drafts-to-backend seam.
 *
 * ADR-0012 2026-07-03 extension. Closes the gap between `configEditor.applyEdit`
 * (synchronous, writes only to `configChangesStore`) and the Rust backend's
 * `NodeProxy.modified_value` map that `save_layout_with_bus_writes` Phase 2
 * scans to emit bus writes.
 *
 * Before this orchestrator existed, `flushDraftToBackend` was called at ONLY
 * two callsites (leaf-row edit commit + one bowtie-catalog panel path), so
 * every other draft producer (facility composition, teardown resets,
 * load-time repair, cascade side effects) staged config drafts that the
 * connected save never wrote — a Save felt like it succeeded but the bus
 * saw nothing, the catalog rebuild found empty consumers, and the composed
 * bowties silently disappeared. The mirror runs reactively over
 * `configChangesStore.draftEntries()` and emits `setModifiedValue` for every
 * new or changed draft so no draft producer has to remember to flush.
 *
 * Ownership contract:
 *   - `ConfigEditor.applyEdit` stays sync + no-IPC. Draft producers write
 *     to the store and are done.
 *   - This orchestrator is the SOLE owner of the config-draft → backend IPC
 *     path (once Commit 2 retires the per-callsite `flushDraftToBackend`).
 *   - Offline mode: this orchestrator is a silent no-op. Offline persistence
 *     is owned by `stageDraftsForOfflineSave` in `configDraftOrchestrator`.
 *   - Placeholder NodeKeys are skipped — they have no bus identity. Their
 *     edits persist through `stageDraftsForOfflineSave` at save time.
 *
 * Lifecycle: mount in `+page.svelte`'s layout-open path alongside
 * `facilityCascadeOrchestrator.startCascade()`; tear down in
 * `layoutLifecycleOrchestrator.resetForNewLayout()`.
 */

import { setModifiedValue } from '$lib/api/config';
import {
  configChangesStore,
  type ConfigDraftEntry,
} from '$lib/stores/configChanges.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { parseEditKey } from '$lib/utils/editKey';
import { normalizeNodeId } from '$lib/utils/nodeId';
import { isPlaceholderInput } from '$lib/utils/nodeKey';
import type { TreeConfigValue } from '$lib/types/nodeTree';

class ConfigDraftMirrorOrchestrator {
  /** Draft state observed at the last reconciliation. */
  private _lastSeen = new Map<string, TreeConfigValue>();
  /** Disposer returned by `$effect.root`, or `null` when stopped. */
  private _dispose: (() => void) | null = null;

  /**
   * Start the mirror subscription. Idempotent: repeated calls without an
   * intervening `stopMirror()` are no-ops. Seeds the last-seen map from the
   * current draft snapshot so a mount over pre-existing drafts (e.g. an
   * offline-then-connect handoff) does not re-flush the backlog.
   */
  startMirror(): void {
    if (this._dispose !== null) return;
    for (const { key, value } of configChangesStore.draftEntries()) {
      this._lastSeen.set(key, value);
    }
    this._dispose = $effect.root(() => {
      $effect(() => {
        this._reconcile(configChangesStore.draftEntries());
      });
    });
  }

  /**
   * Tear down the mirror. Called by
   * `layoutLifecycleOrchestrator.resetForNewLayout()` on layout close.
   */
  stopMirror(): void {
    this._dispose?.();
    this._dispose = null;
    this._lastSeen = new Map();
  }

  resetForNewLayout(): void {
    this.stopMirror();
  }

  /**
   * Test seam: force a diff pass against a caller-supplied entries array.
   * Used by the unit tests instead of driving the `$effect.root` reactive
   * path (which requires a mounted Svelte harness). Also usable by future
   * integration flows that need to flush drafts on demand.
   */
  reconcile(entries: readonly ConfigDraftEntry[]): void {
    this._reconcile(entries);
  }

  private _reconcile(entries: readonly ConfigDraftEntry[]): void {
    const current = new Map<string, TreeConfigValue>();
    for (const { key, value } of entries) current.set(key, value);

    // Connection state is checked inside the body (not read reactively as a
    // dependency of the outer `$effect`): the plan explicitly rules out
    // re-emitting every pending draft on connect/disconnect. Advancing
    // `_lastSeen` below covers offline drafts so they are not retried on
    // reconnect either. A future "flush pending drafts on connect" feature
    // is a separate follow-up.
    const connected = layoutStore.isConnected;

    for (const [key, value] of current) {
      const prev = this._lastSeen.get(key);
      if (prev !== undefined && valuesEqual(prev, value)) continue;
      if (!connected) continue;
      const { normalizedNodeId, space, address } = parseEditKey(key);
      if (isPlaceholderInput(normalizedNodeId)) continue;
      const dottedNodeId = findDottedNodeId(normalizedNodeId) ?? normalizedNodeId;
      setModifiedValue(dottedNodeId, address, space, value).catch((err) => {
        console.error(
          `[configDraftMirrorOrchestrator] setModifiedValue failed for ${key}:`,
          err,
        );
      });
    }

    // Advance last-seen to the current snapshot — this both prunes removed
    // keys (no IPC needed; baseline update / draft prune already reconciled
    // the backend) and acknowledges drafts skipped because we were offline.
    this._lastSeen = current;
  }
}

// ─── Internal helpers ──────────────────────────────────────────────────────

function findDottedNodeId(normalizedId: string): string | null {
  for (const key of nodeTreeStore.trees.keys()) {
    if (normalizeNodeId(key) === normalizedId) return key;
  }
  return null;
}

function valuesEqual(left: TreeConfigValue, right: TreeConfigValue): boolean {
  if (left.type !== right.type) return false;
  if (left.type === 'int' && right.type === 'int') return left.value === right.value;
  if (left.type === 'float' && right.type === 'float') return left.value === right.value;
  if (left.type === 'string' && right.type === 'string') return left.value === right.value;
  if (left.type === 'eventId' && right.type === 'eventId') return left.hex === right.hex;
  return false;
}

export const configDraftMirrorOrchestrator = new ConfigDraftMirrorOrchestrator();
