/**
 * partialCaptureNodesStore — canonical NodeKey set of live nodes whose
 * config read finished with at least one missing/failed leaf (S8 partial
 * capture warnings).
 *
 * Promoted from a `+page.svelte` `$state<Set>` so the layout facade
 * (`effectiveNodeStore`) and the lifecycle owner
 * (`layoutLifecycleOrchestrator`) share a single reactive source for
 * the partial-capture half of the full-capture threshold (ADR-0007,
 * ADR-0011).
 */

import { toCanonicalNodeKey, type NodeKeyInput } from '$lib/utils/nodeKey';

class PartialCaptureNodesStore {
  private _set = $state<Set<string>>(new Set());

  /** Reactive set of canonical NodeKeys with partial-capture warnings. */
  get nodes(): ReadonlySet<string> {
    return this._set;
  }

  /** True iff `key` is currently flagged partial-capture. */
  has(key: NodeKeyInput): boolean {
    return this._set.has(toCanonicalNodeKey(key));
  }

  /** Replace the set wholesale from a list of warnings (canonical or dotted). */
  replace(keys: Iterable<NodeKeyInput>): void {
    this._set = new Set(Array.from(keys, toCanonicalNodeKey));
  }

  /** Clear all entries (layout-close, fresh live session). */
  clear(): void {
    if (this._set.size === 0) return;
    this._set = new Set();
  }
}

export const partialCaptureNodesStore = new PartialCaptureNodesStore();
