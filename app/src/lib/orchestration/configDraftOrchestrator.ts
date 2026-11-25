/**
 * Config draft orchestrator — draft staging at Save time.
 *
 * Owns the boundary between synchronous config edits (via ConfigEditor →
 * ConfigChangesStore) and Pending Changes (E3b D1/D3):
 *
 * - Save (offline or connected): stages current config drafts into
 *   pendingChangesStore; the Save orchestrator records them in the backend and
 *   clears the drafts once they are in the layout file.
 * - Discard: clears config drafts.
 * - Tree refresh: prunes drafts whose refreshed baseline now matches.
 *
 * Drafts are never mirrored to the backend while editing. Components never call
 * IPC or pendingChangesStore for edit operations directly.
 */

import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';
import {
  parseEditKey,
  addressToOffsetHex,
  configValueToOfflineString,
} from '$lib/utils/editKey';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { findLeafByAddress } from '$lib/types/nodeTree';
import { normalizeNodeId } from '$lib/utils/nodeId';

/**
 * Stage all current config drafts into pendingChangesStore before a Save.
 *
 * For each draft entry, finds the baseline value (from the persisted row
 * or the tree leaf) and upserts a pending change row. The config drafts are
 * left in place: the Save clears them only after the rows are recorded in the
 * layout file, so a Save that fails before recording loses nothing.
 *
 * Post-S8.11 all node species (real and placeholder) flow through the same
 * pending change channel. Placeholder NodeKeys are accepted verbatim by
 * the backend when the Save records the rows (S8.11 root-cause fix).
 */
export function stageConfigDraftsAsPendingChanges(): void {
  const entries = configChangesStore.draftEntries();
  for (const { key, value, origin } of entries) {
    const { normalizedNodeId, space, address } = parseEditKey(key);
    const offset = addressToOffsetHex(address);
    const dottedNodeId = findDottedNodeId(normalizedNodeId) ?? normalizedNodeId;

    // Resolve baseline: prefer existing offline row baseline, fall back to tree leaf
    const existingDraft = pendingChangesStore.findDraftConfigChange(normalizedNodeId, space, offset);
    const existingPersisted = pendingChangesStore.findPersistedConfigChange(normalizedNodeId, space, offset);
    let baselineValue = existingDraft?.baselineValue ?? existingPersisted?.baselineValue;
    if (!baselineValue) {
      const leaf = findLeafInTree(normalizedNodeId, address);
      baselineValue = leaf?.value ? configValueToOfflineString(leaf.value) : '';
    }

    pendingChangesStore.upsertConfigChange({
      nodeId: dottedNodeId,
      space,
      offset,
      baselineValue,
      plannedValue: configValueToOfflineString(value),
      origin,
    });
  }
}

/**
 * Discard all config drafts.
 * Restores display to persisted pending values via layer resolution.
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
