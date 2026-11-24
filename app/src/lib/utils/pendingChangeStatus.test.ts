import { describe, expect, it } from 'vitest';
import type { PendingChangeStatus } from '$lib/api/sync';
import { isWaitingPendingChange } from '$lib/utils/pendingChangeStatus';

describe('isWaitingPendingChange', () => {
  it.each<[PendingChangeStatus, boolean]>([
    ['pending', true],
    ['failed', true],
    ['conflict', true],
    ['skipped', true],
    ['applied', false],
    ['alreadyApplied', false],
    ['clean', false],
  ])('%s waiting=%s', (status, expected) => {
    expect(isWaitingPendingChange({ status })).toBe(expected);
  });
});
