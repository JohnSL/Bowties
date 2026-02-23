/**
 * Svelte 5 reactive stores for the Bowties tab — Feature 006.
 *
 * bowtieCatalogStore  — holds the latest BowtieCatalog or null (before first CDI read)
 * cdiReadCompleteStore — true once the first cdi-read-complete event has been received
 *
 * The stores are populated by registering a persistent Tauri event listener for
 * the `cdi-read-complete` event emitted by the backend after CDI reads + the
 * Identify Events exchange both complete.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { type BowtieCatalog, type BowtieCard, type CdiReadCompletePayload } from '../api/tauri';

// ─── Store class ─────────────────────────────────────────────────────────────

class BowtieCatalogStore {
  /** The latest built catalog, or null if CDI reads have not yet completed. */
  private _catalog = $state<BowtieCatalog | null>(null);

  /** True once the first `cdi-read-complete` event has been received. */
  private _readComplete = $state<boolean>(false);

  /** Active listener handle (stored so it can be cleaned up). */
  private _unlisten: UnlistenFn | null = null;

  // ── Reactive getters ──────────────────────────────────────────────────────

  get catalog(): BowtieCatalog | null {
    return this._catalog;
  }

  get readComplete(): boolean {
    return this._readComplete;
  }

  /**
   * Derived map: event_id_hex → BowtieCard.
   * O(1) lookup used by cross-reference navigation (FR-008, research.md RQ-10).
   * Because `_catalog` is `$state`, this getter is re-evaluated reactively
   * whenever the catalog changes.
   */
  get usedInMap(): Map<string, BowtieCard> {
    const map = new Map<string, BowtieCard>();
    if (!this._catalog) return map;
    for (const card of this._catalog.bowties) {
      map.set(card.event_id_hex, card);
    }
    return map;
  }

  /**
   * Derived map: `${nodeId}:${elementPath.join('/')}` → BowtieCard.
   * Used by ElementCard to look up cross-reference by structural identity
   * (node + CDI path) rather than current byte value, which may not yet be
   * fetched at render time (FR-008, SC-005).
   */
  get nodeSlotMap(): Map<string, BowtieCard> {
    const map = new Map<string, BowtieCard>();
    if (!this._catalog) return map;
    for (const card of this._catalog.bowties) {
      for (const entry of [...card.producers, ...card.consumers, ...card.ambiguous_entries]) {
        const key = `${entry.node_id}:${entry.element_path.join('/')}`;
        if (!map.has(key)) {
          map.set(key, card);
        }
      }
    }
    return map;
  }

  // ── Mutations ─────────────────────────────────────────────────────────────

  /** Set the catalog directly (used by the event listener and in tests). */
  setCatalog(catalog: BowtieCatalog): void {
    this._catalog = catalog;
    this._readComplete = true;
  }

  /** Reset store to initial empty state (useful on disconnect). */
  reset(): void {
    this._catalog = null;
    this._readComplete = false;
  }

  // ── Listener lifecycle ────────────────────────────────────────────────────

  /**
   * Register a persistent Tauri event listener for `cdi-read-complete`.
   * Safe to call multiple times — subsequent calls are no-ops until destroyed.
   */
  async startListening(): Promise<void> {
    if (this._unlisten) return; // already registered

    this._unlisten = await listen<CdiReadCompletePayload>(
      'cdi-read-complete',
      (event) => {
        this.setCatalog(event.payload.catalog);
      }
    );
  }

  /**
   * Remove the Tauri event listener.
   * Called on component teardown (onDestroy) or page navigation away.
   */
  stopListening(): void {
    if (this._unlisten) {
      this._unlisten();
      this._unlisten = null;
    }
  }
}

// ─── Singleton exports ────────────────────────────────────────────────────────

/**
 * Singleton store holding the BowtieCatalog and read-complete flag.
 *
 * Usage in a Svelte component:
 * ```svelte
 * <script>
 *   import { bowtieCatalogStore } from '$lib/stores/bowties';
 *   import { onMount, onDestroy } from 'svelte';
 *
 *   onMount(() => bowtieCatalogStore.startListening());
 *   onDestroy(() => bowtieCatalogStore.stopListening());
 *
 *   let catalog = $derived(bowtieCatalogStore.catalog);
 *   let enabled = $derived(bowtieCatalogStore.readComplete);
 * </script>
 * ```
 */
export const bowtieCatalogStore = new BowtieCatalogStore();
