/**
 * Node-roster helpers (S8).
 *
 * The layout is the durable source of truth for which nodes belong to it.
 * The set of "saved" node IDs comes from the active layout context
 * (`layoutNodeIds`), populated on layout open and on save success.
 *
 * "Discovered-only" nodes are nodes the bus exposed during this session
 * that are NOT yet in the saved roster. They are in-memory drafts —
 * exactly like config edits or bowtie metadata edits — and exist only
 * until the user either:
 *   - saves the layout (promotes them into `layoutNodeIds`), or
 *   - disconnects / closes the layout (drops them cleanly).
 *
 * Keeping this as a pure derivation (rather than a separate writable
 * store) makes the "drop on disconnect" behaviour automatic: when
 * `nodes` is cleared on disconnect and rehydrated from saved snapshots,
 * the derivation re-evaluates to an empty set without any cleanup hooks.
 */

/** Canonical node-ID form (uppercase, no dots), used in `layoutNodeIds`. */
import { normalizeNodeId } from './nodeId';

export function canonicalizeNodeId(nodeId: string): string {
  return normalizeNodeId(nodeId);
}

/**
 * Compute the set of node IDs that are currently visible on the bus / in the
 * frontend but are NOT in the saved layout roster.
 *
 * @param savedNodeIds Canonical node IDs from the active layout context. When
 *   `undefined` (legacy contexts) every current node is treated as saved —
 *   this preserves pre-S8 behaviour for layouts that never carried the field.
 * @param currentNodeIds Node IDs visible to the frontend in any form
 *   (dotted-hex, canonical, mixed case). Each is normalized before
 *   comparison.
 */
export function computeDiscoveredOnlyNodeIds(
  savedNodeIds: string[] | undefined,
  currentNodeIds: Iterable<string>,
): string[] {
  if (savedNodeIds === undefined) {
    return [];
  }
  const saved = new Set(savedNodeIds.map(canonicalizeNodeId));
  const out: string[] = [];
  const seen = new Set<string>();
  for (const id of currentNodeIds) {
    const canonical = canonicalizeNodeId(id);
    if (seen.has(canonical)) continue;
    seen.add(canonical);
    if (!saved.has(canonical)) {
      out.push(canonical);
    }
  }
  return out;
}

/**
 * True when the given node ID is in the current frontend roster but NOT in
 * the saved layout roster (i.e. it is an unsaved discovered node).
 */
export function isUnsavedDiscoveredNode(
  nodeId: string,
  savedNodeIds: string[] | undefined,
): boolean {
  if (savedNodeIds === undefined) return false;
  const canonical = canonicalizeNodeId(nodeId);
  return !savedNodeIds.some((saved) => canonicalizeNodeId(saved) === canonical);
}

/**
 * True when the given node ID is in the saved layout roster but NOT in the
 * current frontend roster (i.e. a saved node that is not on the bus).
 *
 * Used to render the "not on bus" indicator for layout nodes that did not
 * answer to the most recent discovery.
 */
export function isSavedOffBusNode(
  nodeId: string,
  savedNodeIds: string[] | undefined,
  currentNodeIds: Iterable<string>,
): boolean {
  if (savedNodeIds === undefined) return false;
  const canonical = canonicalizeNodeId(nodeId);
  if (!savedNodeIds.some((saved) => canonicalizeNodeId(saved) === canonical)) {
    return false;
  }
  for (const id of currentNodeIds) {
    if (canonicalizeNodeId(id) === canonical) return false;
  }
  return true;
}

/**
 * Compute the set of "unsaved in-memory" node IDs (S8 promotion threshold).
 *
 * A discovered node only counts as an in-memory addition — and therefore
 * a dirty-layout change that Save will persist — once it has been **fully
 * captured**: CDI cached and every config value read successfully. Until
 * then, the node carries the "new" badge but Save remains a no-op for it,
 * because we cannot save a usable offline copy of a node we have not
 * finished reading.
 *
 * The badge predicate (mere "discovered, not in saved roster") is
 * `computeDiscoveredOnlyNodeIds`; this helper is its companion for the
 * dirty signal and for the save orchestrator's AddNode delta payload.
 *
 * @param savedNodeIds Canonical node IDs from the active layout context.
 *   When `undefined`, returns `[]` to preserve pre-S8 behaviour.
 * @param fullyCapturedNodeIds Node IDs (any form) that are confirmed
 *   fully captured. Each is normalized before comparison.
 */
export function computeUnsavedInMemoryNodeIds(
  savedNodeIds: string[] | undefined,
  fullyCapturedNodeIds: Iterable<string>,
): string[] {
  if (savedNodeIds === undefined) return [];
  const saved = new Set(savedNodeIds.map(canonicalizeNodeId));
  const out: string[] = [];
  const seen = new Set<string>();
  for (const id of fullyCapturedNodeIds) {
    const canonical = canonicalizeNodeId(id);
    if (seen.has(canonical)) continue;
    seen.add(canonical);
    if (!saved.has(canonical)) {
      out.push(canonical);
    }
  }
  return out;
}
