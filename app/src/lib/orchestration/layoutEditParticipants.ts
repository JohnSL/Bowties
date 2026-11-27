import type { DirtyBreakdown } from '$lib/layout/effectiveNodeStore.svelte';
import { discardAllConfigDrafts } from '$lib/orchestration/configDraftOrchestrator';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { facilitiesStore } from '$lib/stores/facilities.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';

/** An edit layer with a baseline-relative discard operation. */
export interface LayoutEditParticipant {
  readonly dirtyBucket: keyof DirtyBreakdown;
  discard(): void;
}

/** Distinct from layout-scope lifecycle reset participants. */
export const layoutEditParticipants = {
  config: { dirtyBucket: 'config', discard: () => discardAllConfigDrafts() },
  metadata: { dirtyBucket: 'metadata', discard: () => bowtieMetadataStore.clearAll() },
  connectors: { dirtyBucket: 'connectorSelections', discard: () => connectorSelectionsStore.discard() },
  channels: { dirtyBucket: 'channels', discard: () => channelsStore.discard() },
  facilities: { dirtyBucket: 'facilities', discard: () => facilitiesStore.discard() },
  layout: { dirtyBucket: 'layoutStruct', discard: () => layoutStore.revertToSaved() },
} satisfies Record<string, LayoutEditParticipant>;

/** Dirty buckets handled by the explicitly ordered pending-change phase. */
export const discardSpecialPhaseBuckets = {
  pendingRevert: ['pendingDrafts', 'revertedPendingChanges'],
} as const satisfies Record<string, readonly (keyof DirtyBreakdown)[]>;

/** Buckets that are derived, not edits, or lack a discardable baseline (S7). */
export const discardBucketExemptions = {
  configNodes: 'Derived distinct-node count of config drafts.',
  configChanges: 'Derived distinct targets across config and pending changes.',
  waitingPendingChanges: 'Already saved rows awaiting a board; not unsaved edits.',
  unsavedNewNodes: 'S7: node membership has no saved-baseline discard.',
  unsavedRemovedNodes: 'S7: node membership has no saved-baseline discard.',
} as const satisfies Partial<Record<keyof DirtyBreakdown, string>>;
