/**
 * Unified node-roster facade (Spec 014 / S8.7, S8.12).
 *
 * Single source of truth for "the set of nodes the user sees" in the active
 * layout. Collapses what were four parallel surfaces into one typed view:
 *
 *   - `nodeInfoStore`              — identity + SNIP        (`Map<NodeKey, DiscoveredNode>`)
 *   - `nodeTreeStore`              — CDI configuration tree (`Map<NodeKey, NodeConfigTree>`)
 *   - `configReadNodesStore`       — "config read complete" (`Set<NodeKey>`)
 *   - internal `_profileStems`     — placeholder profile stem (`Map<NodeKey, string>`)
 *
 * Post-S8.12 the roster internalizes the placeholder profile-stem tracking
 * that was previously in `inMemoryPlaceholdersStore` (deleted S8.12).
 *
 * Every new write goes through the roster mutators, which fan out
 * deterministically. Consumers that need "all nodes" / "live only" /
 * "placeholders only" read typed views off the roster instead of filtering
 * with `isPlaceholderInput` at every call site.
 *
 * See ADR-0008 (Unified NodeKey) for the identity model. The roster does
 * not branch on `NodeID` vs `placeholder:<uuid>` — both flow through the
 * same code paths, and the `kind` discriminator on each entry is computed
 * from the key's prefix.
 */

import { get } from 'svelte/store';
import { nodeInfoStore } from './nodeInfo';
import { nodeTreeStore } from './nodeTree.svelte';
import {
  configReadNodesStore,
  markNodeConfigRead,
  clearConfigReadStatus,
  removeNodesConfigRead,
} from './configReadStatus';
import {
  isPlaceholderInput,
  nodeKey,
  nodeKeyToString,
  toCanonicalNodeKey,
  type NodeKeyInput,
} from '$lib/utils/nodeKey';
import { formatNodeId } from '$lib/utils/nodeId';
import { layoutStore } from './layout.svelte';
import type { DiscoveredNode } from '$lib/api/tauri';
import type { NodeConfigTree } from '$lib/types/nodeTree';

/**
 * Accepted identifier form for roster methods.
 *
 * Spec 014 Step 6b widens public signatures to accept either a legacy
 * string (dotted or canonical live form, or `placeholder:<id>`) or a
 * `NodeKey`. The roster canonicalizes at the boundary via
 * {@link toCanonical} so internal map keys are always the canonical wire
 * form. Step 7 (Wave 2) tightens this to branded `NodeKey` only.
 */
export type { NodeKeyInput };

function toCanonical(input: NodeKeyInput): string {
  return toCanonicalNodeKey(input);
}

/** Canonical wire-form key for a live node from its 6-byte NodeID. */
function liveKeyFromBytes(bytes: number[]): string {
  return nodeKeyToString(nodeKey(formatNodeId(bytes)));
}

export type NodeRosterKind = 'live' | 'placeholder';

export interface NodeRosterEntry {
  /** Stable NodeKey — `NodeID` for live nodes, `placeholder:<uuidv4>` for placeholders. */
  nodeKey: string;
  /** Typed discriminator derived from the key prefix. */
  kind: NodeRosterKind;
  /** Identity + SNIP shape. Synthetic for placeholders (manufacturer/model from bundled profile). */
  info: DiscoveredNode;
  /** CDI tree, when loaded. Always present for placeholders (built at add-time). */
  tree?: NodeConfigTree;
  /** Whether config values have been read from the node (always `read` for placeholders). */
  readStatus: 'unread' | 'read';
  /** Bundled-profile stem (placeholders only). */
  profileStem?: string;
}

class NodeRoster {
  // ─── Reactive mirrors of the underlying stores ──────────────────────────
  // Subscribing in the constructor lets us expose reactive getters from a
  // plain class (Svelte 5 `$state` cannot live in module scope when other
  // modules' tests reset the underlying stores; mirroring keeps the roster
  // in sync without forcing every consumer to import the writables).

  private _info = $state<Map<string, DiscoveredNode>>(new Map());
  private _read = $state<Set<string>>(new Set());
  // `nodeTreeStore` is already $state-backed and exposes `.trees` reactively —
  // we read from it directly, no mirror needed.

  /** Profile stems for placeholders, keyed by NodeKey. */
  private _profileStems = $state<Map<string, string>>(new Map());

  /**
   * NodeKeys that were persisted in the open layout but have been removed
   * in-memory and not yet saved. Drives the layout dirty signal and is
   * emitted as `RemoveNode` deltas at save time. Cleared after a
   * successful save.
   */
  private _persistedRemovals = $state<Set<string>>(new Set());

  constructor() {
    nodeInfoStore.subscribe((m) => {
      this._info = m;
    });
    configReadNodesStore.subscribe((s) => {
      this._read = s;
    });
  }

  // ─── Derived views ──────────────────────────────────────────────────────

  /** Every node in the active layout, in NodeKey-iteration order. */
  get allEntries(): NodeRosterEntry[] {
    const trees = nodeTreeStore.trees;
    const out: NodeRosterEntry[] = [];
    for (const [nodeKey, info] of this._info.entries()) {
      const placeholder = isPlaceholderInput(nodeKey);
      out.push({
        nodeKey,
        kind: placeholder ? 'placeholder' : 'live',
        info,
        tree: trees.get(nodeKey),
        readStatus: this._read.has(nodeKey) ? 'read' : 'unread',
        profileStem: placeholder
          ? this._profileStems.get(nodeKey)
          : undefined,
      });
    }
    return out;
  }

  /** Entries whose NodeKey is a live `NodeID`. */
  get liveEntries(): NodeRosterEntry[] {
    return this.allEntries.filter((e) => e.kind === 'live');
  }

  /** Entries whose NodeKey is `placeholder:<uuid>`. */
  get placeholderEntries(): NodeRosterEntry[] {
    return this.allEntries.filter((e) => e.kind === 'placeholder');
  }

  /**
   * Live discovered nodes as `DiscoveredNode[]`. Back-compat surface for
   * `+page.svelte` — the page-local `nodes = $state<DiscoveredNode[]>([])`
   * was replaced with a `$derived` over this getter so every visibility
   * gate that read from it now reflects the unified roster.
   *
   * NOTE: this excludes placeholders by design — call sites that need
   * placeholders too should read `allEntries`. The `nodes.length === 0`
   * misfire that motivated S8.7 was specifically about main-content
   * visibility, which is fixed by reading `allEntries.length` (or any
   * derived view that includes placeholders) instead.
   */
  get liveNodes(): DiscoveredNode[] {
    return this.liveEntries.map((e) => e.info);
  }

  /** True when the roster has at least one live or placeholder entry. */
  get hasAnyEntries(): boolean {
    return this._info.size > 0;
  }

  /** True iff `nodeKey` is currently in the roster. */
  has(nodeKey: NodeKeyInput): boolean {
    return this._info.has(toCanonical(nodeKey));
  }

  // ─── Mutators ───────────────────────────────────────────────────────────
  //
  // Each mutator owns the fan-out across the four backing stores. Consumers
  // call exactly one method per workflow step; the roster guarantees the
  // four stores stay coherent. No call site outside this module should
  // mutate the underlying stores directly for placeholder / live-roster
  // operations — go through the mutators.

  /** Insert or replace a live node by NodeID. */
  upsertLive(node: DiscoveredNode): void {
    const key = liveKeyFromBytes(node.node_id);
    nodeInfoStore.update((m) => {
      const next = new Map(m);
      next.set(key, node);
      return next;
    });
  }

  /**
   * Replace the live-node set in one shot, preserving every placeholder.
   *
   * Used by the discovery / refresh / replay paths in `+page.svelte` where
   * the live roster is rebuilt from a fresh `DiscoveredNode[]` (refresh
   * result, offline replay, post-disconnect cleanup). Placeholders must
   * survive because they are layout-scoped, not bus-scoped.
   */
  replaceLiveRoster(nodes: DiscoveredNode[]): void {
    const current = get(nodeInfoStore);
    const next = new Map<string, DiscoveredNode>();
    // Preserve placeholders.
    for (const [key, info] of current.entries()) {
      if (isPlaceholderInput(key)) next.set(key, info);
    }
    // Belt-and-braces: skip entries with empty node_id (placeholders that
    // leak in when the caller builds from allEntries instead of liveNodes).
    for (const n of nodes) {
      if (n.node_id.length === 0) continue;
      next.set(liveKeyFromBytes(n.node_id), n);
    }
    nodeInfoStore.set(next);
  }

  /**
   * Synthesize and seed a placeholder entry (post-S8.5 in-memory pattern).
   * The orchestrator builds `info` and `tree`; the roster owns the fan-out.
   */
  addPlaceholder(args: {
    nodeKey: NodeKeyInput;
    profileStem: string;
    info: DiscoveredNode;
    tree: NodeConfigTree;
  }): void {
    const key = toCanonical(args.nodeKey);
    nodeInfoStore.update((m) => {
      const next = new Map(m);
      next.set(key, args.info);
      return next;
    });
    nodeTreeStore.setTree(key, args.tree);
    markNodeConfigRead(key);
    this._profileStems = new Map(this._profileStems);
    this._profileStems.set(key, args.profileStem);
  }

  /** Remove a placeholder from every backing store. No-op for live keys. */
  removePlaceholder(nodeKey: NodeKeyInput): void {
    const key = toCanonical(nodeKey);
    if (!isPlaceholderInput(key)) return;
    const wasPersisted =
      layoutStore.activeContext?.layoutNodeIds?.includes(key) ?? false;
    nodeInfoStore.update((m) => {
      if (!m.has(key)) return m;
      const next = new Map(m);
      next.delete(key);
      return next;
    });
    nodeTreeStore.removeTree(key);
    removeNodesConfigRead([key]);
    if (this._profileStems.has(key)) {
      this._profileStems = new Map(this._profileStems);
      this._profileStems.delete(key);
    }
    if (wasPersisted && !this._persistedRemovals.has(key)) {
      const next = new Set(this._persistedRemovals);
      next.add(key);
      this._persistedRemovals = next;
    }
  }

  /** NodeKeys removed from the open layout that have not yet been saved. */
  get persistedRemovals(): ReadonlySet<string> {
    return this._persistedRemovals;
  }

  /** Drop the persisted-removals set after a successful save. */
  clearPersistedRemovals(): void {
    if (this._persistedRemovals.size > 0) {
      this._persistedRemovals = new Set();
    }
  }

  /**
   * Mark placeholders as persisted: drop their profile-stem tracking so
   * `placeholderEntries` no longer lists them, but leave nodeInfoStore /
   * tree / readStatus intact (the node is now a saved entry).
   */
  markPlaceholdersPersisted(nodeKeys: NodeKeyInput[]): void {
    let changed = false;
    const next = new Map(this._profileStems);
    for (const raw of nodeKeys) {
      const key = toCanonical(raw);
      if (next.has(key)) {
        next.delete(key);
        changed = true;
      }
    }
    if (changed) this._profileStems = next;
  }

  /** Store a tree directly (e.g. from `get_node_tree` post-CDI-scan). */
  setTree(nodeKey: NodeKeyInput, tree: NodeConfigTree): void {
    nodeTreeStore.setTree(toCanonical(nodeKey), tree);
  }

  /** Mark a node as having had its config read. */
  markRead(nodeKey: NodeKeyInput): void {
    markNodeConfigRead(toCanonical(nodeKey));
  }

  /**
   * Layout-scope teardown: drop every roster entry across all backing stores.
   *
   * Used by the layout-close / disconnect-and-clear / new-layout paths in
   * `+page.svelte`, collapsing what was a ~30-line fan-out into one call.
   */
  clearLayoutScope(): void {
    nodeInfoStore.set(new Map());
    nodeTreeStore.reset();
    clearConfigReadStatus();
    this._profileStems = new Map();
    this._persistedRemovals = new Set();
  }
}

/** Singleton roster. */
export const nodeRoster = new NodeRoster();
