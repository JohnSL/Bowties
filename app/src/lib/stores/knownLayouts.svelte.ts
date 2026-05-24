/**
 * Svelte 5 reactive store for the known-layout registry (Spec 013 / S6).
 *
 * Mirrors the backend `known-layouts.json` registry exposed via
 * `$lib/api/startup`. The picker reads from this store; the
 * `startupOrchestrator` writes through it.
 */

import type { KnownLayoutEntry } from '$lib/api/startup';

class KnownLayoutsStore {
  private _entries = $state<KnownLayoutEntry[]>([]);
  private _loaded = $state<boolean>(false);
  private _busy = $state<boolean>(false);

  /** Current registry entries (most-recent first as returned by backend). */
  get entries(): KnownLayoutEntry[] {
    return this._entries;
  }

  /** True once the registry has been loaded at least once. */
  get loaded(): boolean {
    return this._loaded;
  }

  /** True while a backend registry call is in flight. */
  get busy(): boolean {
    return this._busy;
  }

  /** Replace the in-memory entries (called by the orchestrator after each backend call). */
  setEntries(entries: KnownLayoutEntry[]): void {
    this._entries = Array.isArray(entries) ? entries : [];
    this._loaded = true;
  }

  setBusy(value: boolean): void {
    this._busy = value;
  }

  reset(): void {
    this._entries = [];
    this._loaded = false;
    this._busy = false;
  }
}

export const knownLayoutsStore = new KnownLayoutsStore();
