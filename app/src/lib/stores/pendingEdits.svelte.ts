/**
 * Svelte 5 reactive store for tracking pending (unsaved) configuration edits.
 *
 * Maps a compound key `"${nodeId}:${space}:${address}"` to a `PendingEdit`
 * describing the original value, the user's in-progress value, its validation
 * state, and its write lifecycle state.
 *
 * Spec: 007-edit-node-config, data-model.md, research.md R4.
 */

import { writable } from 'svelte/store';
import type {
  PendingEdit,
  WriteState,
  ValidationState,
} from '$lib/types/nodeTree';

// ─── Svelte 4 reactivity bridge ───────────────────────────────────────────────
//
// ConfigSidebar.svelte uses Svelte 4 legacy syntax ($:, $store).
// It cannot automatically track mutations to the Svelte 5 $state Map inside
// PendingEditsStore. This writable increments each time any edit changes,
// giving legacy components a subscription point via `$pendingEditsVersion`.
//
const _versionStore = writable(0);

/** Subscribe to this in Svelte 4 legacy components to react to any edit change. */
export const pendingEditsVersion = { subscribe: _versionStore.subscribe };

// ─── Key helpers ──────────────────────────────────────────────────────────────

/**
 * Build the compound key used to identify a single configurable field.
 */
export function makePendingEditKey(nodeId: string, space: number, address: number): string {
  return `${nodeId}:${space}:${address}`;
}

// ─── Store class ──────────────────────────────────────────────────────────────

class PendingEditsStore {
  /** Map of compound key → PendingEdit. */
  private _edits = $state<Map<string, PendingEdit>>(new Map());

  // ── Reactive getters ───────────────────────────────────────────────────────

  /** Total number of edits currently in the store. */
  get dirtyCount(): number {
    return this._edits.size;
  }

  /** True if any edit currently has `validationState === 'invalid'`. */
  get hasInvalid(): boolean {
    for (const edit of this._edits.values()) {
      if (edit.validationState === 'invalid') return true;
    }
    return false;
  }

  /** True if there is at least one pending edit. */
  get hasPendingEdits(): boolean {
    return this._edits.size > 0;
  }

  // ── Mutation methods ───────────────────────────────────────────────────────

  /**
   * Add or replace a pending edit.
   *
   * If the new `pendingValue` equals `originalValue` (by JSON comparison),
   * the edit is removed automatically (reverted to clean state).
   */
  setEdit(key: string, edit: PendingEdit): void {
    // Auto-remove when the value is reverted to the original —
    // but only if the edit is valid (invalid edits must stay to show the error).
    if (edit.validationState !== 'invalid' && valuesEqual(edit.pendingValue, edit.originalValue)) {
      this._edits.delete(key);
      _versionStore.update(v => v + 1);
      return;
    }
    this._edits.set(key, { ...edit });
    _versionStore.update(v => v + 1);
  }

  /** Remove a specific pending edit (user discarded or write succeeded). */
  removeEdit(key: string): void {
    this._edits.delete(key);
    _versionStore.update(v => v + 1);
  }

  /** Remove all pending edits. */
  clearAll(): void {
    this._edits.clear();
    _versionStore.update(v => v + 1);
  }

  /** Remove all pending edits for a specific node. */
  clearForNode(nodeId: string): void {
    for (const key of this._edits.keys()) {
      if (key.startsWith(`${nodeId}:`)) {
        this._edits.delete(key);
      }
    }
    _versionStore.update(v => v + 1);
  }

  /**
   * Transition an edit to `writing` state.
   * No-op if the key is not present.
   */
  markWriting(key: string): void {
    const edit = this._edits.get(key);
    if (!edit) return;
    this._edits.set(key, { ...edit, writeState: 'writing' });
    _versionStore.update(v => v + 1);
  }

  /**
   * Transition an edit to `error` state with an error message.
   * No-op if the key is not present.
   */
  markError(key: string, message: string): void {
    const edit = this._edits.get(key);
    if (!edit) return;
    this._edits.set(key, { ...edit, writeState: 'error', writeError: message });
    _versionStore.update(v => v + 1);
  }

  /**
   * Transition an edit to `clean` state and remove it from the store.
   * Called on successful write.
   */
  markClean(key: string): void {
    this._edits.delete(key);
    _versionStore.update(v => v + 1);
  }

  // ── Query methods ──────────────────────────────────────────────────────────

  /** Get a specific edit, or undefined if not tracked. */
  getEdit(key: string): PendingEdit | undefined {
    return this._edits.get(key);
  }

  /**
   * Return all dirty edits for a specific node.
   *
   * "Dirty" means `writeState === 'dirty'` — i.e., modified but not yet
   * being written.
   */
  getDirtyForNode(nodeId: string): PendingEdit[] {
    const result: PendingEdit[] = [];
    for (const edit of this._edits.values()) {
      if (edit.nodeId === nodeId && edit.writeState === 'dirty') {
        result.push(edit);
      }
    }
    return result;
  }

  /**
   * Return all dirty edits within a specific segment (identified by origin
   * address) for a specific node.
   */
  getDirtyForSegment(nodeId: string, segmentOrigin: number): PendingEdit[] {
    const result: PendingEdit[] = [];
    for (const edit of this._edits.values()) {
      if (
        edit.nodeId === nodeId &&
        edit.segmentOrigin === segmentOrigin &&
        edit.writeState === 'dirty'
      ) {
        result.push(edit);
      }
    }
    return result;
  }

  /**
   * Return all edits that should be (re)attempted on a Save: those with
   * writeState 'dirty' or 'error', within the given segment.
   * This supports T046: retry-only-failed behavior — subsequent save clicks
   * skip already-clean fields and only re-attempt dirty/error ones.
   */
  getRetryableForSegment(nodeId: string, segmentOrigin: number): PendingEdit[] {
    const result: PendingEdit[] = [];
    for (const edit of this._edits.values()) {
      if (
        edit.nodeId === nodeId &&
        edit.segmentOrigin === segmentOrigin &&
        (edit.writeState === 'dirty' || edit.writeState === 'error')
      ) {
        result.push(edit);
      }
    }
    return result;
  }

  /**
   * Return all edits for a specific node regardless of write state.
   */
  getAllForNode(nodeId: string): PendingEdit[] {
    const result: PendingEdit[] = [];
    for (const edit of this._edits.values()) {
      if (edit.nodeId === nodeId) {
        result.push(edit);
      }
    }
    return result;
  }

  /**
   * Return the count of edits in a specific write state for a node.
   */
  countByWriteState(nodeId: string, writeState: WriteState): number {
    let count = 0;
    for (const edit of this._edits.values()) {
      if (edit.nodeId === nodeId && edit.writeState === writeState) {
        count++;
      }
    }
    return count;
  }

  /**
   * Return whether any edit for a node has a given validation state.
   */
  hasValidationState(nodeId: string, validationState: ValidationState): boolean {
    for (const edit of this._edits.values()) {
      if (edit.nodeId === nodeId && edit.validationState === validationState) {
        return true;
      }
    }
    return false;
  }

  /**
   * Return all edits that should be (re)attempted on a Save, across ALL
   * nodes and segments. Includes those with writeState 'dirty' or 'error'.
   * Used by the global Save button to save everything in one pass.
   */
  getRetryableAll(): PendingEdit[] {
    const result: PendingEdit[] = [];
    for (const edit of this._edits.values()) {
      if (edit.writeState === 'dirty' || edit.writeState === 'error') {
        result.push(edit);
      }
    }
    return result;
  }

  /**
   * Snapshot of all edits as a plain array (for iteration in save logic).
   */
  get allEdits(): PendingEdit[] {
    return Array.from(this._edits.values());
  }
}

// ─── Value equality helper ────────────────────────────────────────────────────

/**
 * Deep-equal comparison for `TreeConfigValue` variants.
 * Uses JSON serialization as a cheap but reliable approach for these
 * simple value union types.
 */
function valuesEqual(a: PendingEdit['pendingValue'], b: PendingEdit['originalValue']): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

// ─── Singleton export ─────────────────────────────────────────────────────────

/** Global singleton — import this in components and the save handler. */
export const pendingEditsStore = new PendingEditsStore();
