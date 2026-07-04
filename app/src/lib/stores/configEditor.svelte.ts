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
 * - Online IPC flush is handled by `configDraftMirrorOrchestrator`, mounted
 *   in the layout-open lifecycle (see `+page.svelte`, ADR-0012 2026-07-03
 *   extension). Callers of `applyEdit` do nothing else — the mirror
 *   observes the draft and forwards it to the backend.
 * - Lives in stores/ because its primary job is coordinating writes into a store.
 */

import { configChangesStore } from '$lib/stores/configChanges.svelte';
import type { TreeConfigValue } from '$lib/types/nodeTree';

// ─── ConfigEditor class ───────────────────────────────────────────────────────

class ConfigEditor {
  /**
   * Apply a user-initiated edit to a config field.
   *
   * All components call this method — never configChangesStore.set() directly.
   *
   * @param key - Canonical edit key from editKeyForLeaf()
   * @param value - New value for the field
   */
  applyEdit(key: string, value: TreeConfigValue): void {
    configChangesStore.set(key, value);
    // Cascade dependency resolution goes here in a future PR
    // when profile cascade rules are authored.
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const configEditor = new ConfigEditor();
