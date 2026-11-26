import type { ReleasedOwner } from '$lib/api/signaling';
import { canonicalizeNodeId } from '$lib/utils/nodeRoster';

/** Add a released owner; the same owner (canonical node key + owner identity) is held once. */
export function withReleasedOwner(released: readonly ReleasedOwner[], owner: ReleasedOwner): ReleasedOwner[] {
  const key = canonicalizeNodeId(owner.nodeKey);
  if (released.some((r) => canonicalizeNodeId(r.nodeKey) === key && r.owner === owner.owner)) return [...released];
  return [...released, { ...owner }];
}
