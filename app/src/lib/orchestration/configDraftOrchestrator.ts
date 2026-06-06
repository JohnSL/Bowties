/**
 * Config draft orchestrator — async draft mirroring and staging.
 *
 * Owns the async boundary between synchronous config edits (via ConfigEditor →
 * ConfigChangesStore) and backend/persistence side effects:
 *
 * - Online: mirrors each draft to the Rust backend via setModifiedValue IPC.
 * - Offline save: stages current config drafts into offlineChangesStore before flush.
 * - Offline discard: clears config drafts and restores persisted rows.
 * - Tree refresh: prunes drafts whose refreshed baseline now matches.
 *
 * Components never call IPC or offlineChangesStore for edit operations directly.
 */

import { setModifiedValue } from '$lib/api/config';
import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import {
  parseEditKey,
  addressToOffsetHex,
  configValueToOfflineString,
} from '$lib/utils/editKey';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { findLeafByAddress } from '$lib/types/nodeTree';
import { normalizeNodeId } from '$lib/utils/nodeId';
import { isPlaceholderInput } from '$lib/utils/nodeKey';

/**
 * Mirror a single config draft to the Rust backend via IPC.
 * Called by the edit path when the app is online.
 *
 * Placeholder NodeKeys are skipped: placeholders have no bus identity, so
 * there is nothing to mirror to. Their edits stay in `configChangesStore`
 * and are persisted into the on-disk `NodeSnapshot` at Save time.
 */
export function flushDraftToBackend(key: string): void {
  const { normalizedNodeId, space, address } = parseEditKey(key);
  if (isPlaceholderInput(normalizedNodeId)) return;
  const value = configChangesStore.visibleValue(key);
  if (value === null) return;

  // Find the dotted nodeId used by the tree store (IPC expects it)
  const dottedNodeId = findDottedNodeId(normalizedNodeId) ?? normalizedNodeId;

  setModifiedValue(dottedNodeId, address, space, value).catch((err) => {
    console.error(`[configDraftOrchestrator] setModifiedValue failed for ${key}:`, err);
  });
}

/**
 * Stage all current config drafts into offlineChangesStore before an offline save.
 *
 * For each draft entry, finds the baseline value (from the persisted offline row
 * or the tree leaf) and upserts an offline change row. After staging, clears the
 * staged config drafts so the display falls back to the newly-persisted layer.
 *
 * Post-S8.11 all node species (real and placeholder) flow through the same
 * offline-change channel. Placeholder NodeKeys are accepted verbatim by
 * the `replace_offline_changes` IPC (S8.11 root-cause fix).
 */
export function stageDraftsForOfflineSave(): void {
  const entries = configChangesStore.draftEntries();
  const stagedKeys: string[] = [];
  for (const { key, value } of entries) {
    const { normalizedNodeId, space, address } = parseEditKey(key);
    const offset = addressToOffsetHex(address);
    const dottedNodeId = findDottedNodeId(normalizedNodeId) ?? normalizedNodeId;

    // Resolve baseline: prefer existing offline row baseline, fall back to tree leaf
    const existingDraft = offlineChangesStore.findDraftConfigChange(normalizedNodeId, space, offset);
    const existingPersisted = offlineChangesStore.findPersistedConfigChange(normalizedNodeId, space, offset);
    let baselineValue = existingDraft?.baselineValue ?? existingPersisted?.baselineValue;
    if (!baselineValue) {
      const leaf = findLeafInTree(normalizedNodeId, address);
      baselineValue = leaf?.value ? configValueToOfflineString(leaf.value) : '';
    }

    offlineChangesStore.upsertConfigChange({
      nodeId: dottedNodeId,
      space,
      offset,
      baselineValue,
      plannedValue: configValueToOfflineString(value),
    });
    stagedKeys.push(key);
  }

  for (const key of stagedKeys) configChangesStore.revert(key);
}

/**
 * Discard all config drafts (offline mode).
 * Restores display to persisted offline pending values via layer resolution.
 */
export function discardAllConfigDrafts(): void {
  configChangesStore.clearAllDrafts();
}

/**
 * After a tree refresh (node-tree-updated), prune drafts whose baseline
 * now matches the draft value. This handles the partial-failure case where
 * some writes succeeded and some didn't.
 */
export function reconcileDraftsAfterTreeRefresh(nodeId: string): void {
  configChangesStore.pruneResolvedDraftsForNode(nodeId);
}

// ─── Internal helpers ──────────────────────────────────────────────────────

function findDottedNodeId(normalizedId: string): string | null {
  for (const key of nodeTreeStore.trees.keys()) {
    if (normalizeNodeId(key) === normalizedId) return key;
  }
  return null;
}

function findLeafInTree(normalizedNodeId: string, address: number) {
  for (const [treeKey, tree] of nodeTreeStore.trees.entries()) {
    if (normalizeNodeId(treeKey) !== normalizedNodeId) continue;
    return findLeafByAddress(tree, address);
  }
  return null;
}
