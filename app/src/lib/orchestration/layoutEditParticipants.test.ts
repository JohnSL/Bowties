import { describe, expect, it } from 'vitest';
import { effectiveNodeStore } from '$lib/layout/effectiveNodeStore.svelte';
import {
  layoutEditParticipants,
  discardSpecialPhaseBuckets,
  discardBucketExemptions,
} from './layoutEditParticipants';

function unclassifiedBuckets(buckets: object): string[] {
  const classified = new Set<string>([
    ...Object.values(layoutEditParticipants).map((participant) => participant.dirtyBucket),
    ...discardSpecialPhaseBuckets.pendingRevert,
    ...Object.keys(discardBucketExemptions),
  ]);
  return Object.keys(buckets).filter((bucket) => !classified.has(bucket));
}

describe('layout discard dirty-bucket coverage', () => {
  it('classifies every actual bucket and detects newly introduced buckets', () => {
    const breakdown = effectiveNodeStore.dirtyBreakdown;
    expect(unclassifiedBuckets(breakdown)).toEqual([]);
    expect(unclassifiedBuckets({ ...breakdown, newlyIntroducedBucket: 1 }))
      .toEqual(['newlyIntroducedBucket']);
    expect(discardSpecialPhaseBuckets.pendingRevert).toEqual(['pendingDrafts', 'revertedPendingChanges']);
    expect(Object.keys(discardBucketExemptions).sort()).toEqual([
      'configChanges', 'configNodes', 'unsavedNewNodes', 'unsavedRemovedNodes', 'waitingPendingChanges',
    ]);
    for (const reason of Object.values(discardBucketExemptions)) expect(reason.trim()).not.toBe('');
  });
});
