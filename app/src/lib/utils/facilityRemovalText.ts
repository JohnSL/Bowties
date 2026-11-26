/**
 * Per-board confirmation text for removing a facility: turns the removal
 * compose's claim list into the slots that will be reset, so the Remove dialog
 * lists exactly what is reset. Pure; board and pin naming are passed in.
 */
import type { FacilityRemovalClaim } from '$lib/api/facilities';

export interface RemovalResolvers {
  boardName: (nodeKey: string) => string;
  /** The pin label as the panel card shows it (e.g. `#9 H3-G`). */
  pinLabel: (nodeKey: string, lampSelection: number) => string;
}

export interface RemovalConfirmationGroup {
  board: string;
  nodeKey: string;
  items: string[];
}

function claimLabel(claim: FacilityRemovalClaim, resolvers: RemovalResolvers): string {
  switch (claim.kind) {
    case 'mast':
      return `Mast ${claim.index}`;
    case 'logic':
      return `Logic ${claim.index}`;
    case 'trackCircuit':
      return `Track Circuit ${claim.index}`;
    case 'directLamp':
      return claim.lampSelection === null
        ? `row ${claim.index} (no pin set)`
        : resolvers.pinLabel(claim.nodeKey, claim.lampSelection);
  }
}

/** Group claims by board in first-seen order, items in claim order. */
export function removalConfirmationGroups(
  claims: readonly FacilityRemovalClaim[],
  resolvers: RemovalResolvers,
): RemovalConfirmationGroup[] {
  const groups = new Map<string, RemovalConfirmationGroup>();
  for (const claim of claims) {
    let group = groups.get(claim.nodeKey);
    if (!group) {
      group = { board: resolvers.boardName(claim.nodeKey), nodeKey: claim.nodeKey, items: [] };
      groups.set(claim.nodeKey, group);
    }
    group.items.push(claimLabel(claim, resolvers));
  }
  return [...groups.values()];
}
