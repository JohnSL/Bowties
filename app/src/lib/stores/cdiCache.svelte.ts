/**
 * cdiCacheStore — canonical node-ID set of nodes whose CDI is present in the
 * local cache.
 *
 * Promoted from a `+page.svelte` `$state<Set>` (spec S6) so the single piece
 * of shared "which nodes have cached CDI" state has one owner. It is written
 * by the config-acquisition workflow (preflight + post-download merge) and the
 * refresh reconciler, and read by the native-menu enable effect to gate the
 * "View CDI" item. Co-locating it here removes the DRY/SRP smell of a
 * route-local Set passed by parameter into the orchestrator.
 */

class CdiCacheStore {
  private _set = $state<Set<string>>(new Set());

  /** Reactive set of node IDs with cached CDI. */
  get nodes(): ReadonlySet<string> {
    return this._set;
  }

  /** True iff `nodeId` currently has cached CDI. */
  has(nodeId: string): boolean {
    return this._set.has(nodeId);
  }

  /** Union `ids` into the cache (preflight + post-download merge). */
  add(ids: Iterable<string>): void {
    const additions = [...ids];
    if (additions.length === 0) return;
    this._set = new Set([...this._set, ...additions]);
  }

  /** Replace the cache wholesale (refresh reconciliation drops stale nodes). */
  replace(ids: Iterable<string>): void {
    this._set = new Set(ids);
  }

  /** Clear all entries (fresh live session). */
  reset(): void {
    if (this._set.size === 0) return;
    this._set = new Set();
  }
}

export const cdiCacheStore = new CdiCacheStore();
