/**
 * Dependency-aware config edit application.
 *
 * ConfigEditor is the single public entry point for all user-initiated config
 * changes. It calls configChangesStore.set() for the user's edit, then
 * (when cascade rules are available) checks and corrects dependent fields.
 *
 * PR 1 implementation: thin pass-through — applyEdit delegates directly to
 * configChangesStore.set() with no cascade logic. Cascade behavior is added
 * in a future PR when profile cascade rules are authored.
 *
 * Design constraints (from refactor plan):
 * - Purely synchronous. No IPC, no async.
 * - Writes only to configChangesStore, never to the Rust backend.
 * - Drafts reach the backend only at Save, as Pending Change rows staged by
 *   `stageConfigDraftsAsPendingChanges` (configDraftOrchestrator). Callers of
 *   `applyEdit` do nothing else.
 * - Lives in stores/ because its primary job is coordinating writes into a store.
 */

import { configChangesStore } from '$lib/stores/configChanges.svelte';
import type { TreeConfigValue } from '$lib/types/nodeTree';
import type { PendingChangeOrigin } from '$lib/api/sync';

// ─── ConfigEditor class ───────────────────────────────────────────────────────

class ConfigEditor {
  /**
   * Apply a user-initiated edit to a config field.
   *
   * All components call this method — never configChangesStore.set() directly.
   *
   * @param key - Canonical edit key from editKeyForLeaf()
   * @param value - New value for the field
   * @param origin - The facility staging the edit; omit for a hand edit
   */
  applyEdit(key: string, value: TreeConfigValue, origin?: PendingChangeOrigin): void {
    configChangesStore.set(key, value, origin);
    // Cascade dependency resolution goes here in a future PR
    // when profile cascade rules are authored.
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const configEditor = new ConfigEditor();
