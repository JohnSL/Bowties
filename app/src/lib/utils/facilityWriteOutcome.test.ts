import { describe, it, expect } from 'vitest';
import type { PendingChangeRow } from '$lib/api/sync';
import { facilityWriteOutcome } from './facilityWriteOutcome';

function row(over: Partial<PendingChangeRow>): PendingChangeRow {
  return {
    changeId: Math.random().toString(),
    kind: 'config',
    nodeId: '05.02.01.02.03.00',
    space: 253,
    offset: '0x00000010',
    baselineValue: '0',
    plannedValue: '1',
    status: 'pending',
    origin: { facilityId: 'f-1' },
    ...over,
  };
}

describe('facilityWriteOutcome', () => {
  it('splits one facility\'s rows per board into waiting with reason and not attempted, leaving written rows out', () => {
    const outcome = facilityWriteOutcome('f-1', [
      row({ nodeId: '05.02.01.02.03.00', status: 'applied' }),
      row({ nodeId: '05.02.01.02.03.00', status: 'failed', error: 'Timed out' }),
      row({ nodeId: '05.02.01.02.03.01', status: 'pending' }),
      row({ nodeId: '05.02.01.02.03.01', status: 'conflict', error: 'Value changed on the board' }),
      row({ nodeId: '05.02.01.02.03.09', origin: { facilityId: 'other' } }),
      row({ nodeId: '05.02.01.02.03.09', origin: undefined }),
    ]);
    expect(outcome.boards).toHaveLength(2);
    const east = outcome.boards.find((b) => b.nodeKey === '050201020300')!;
    expect(east).toMatchObject({ notAttempted: 0, waitingReasons: ['Timed out'] });
    const west = outcome.boards.find((b) => b.nodeKey === '050201020301')!;
    expect(west).toMatchObject({ notAttempted: 1, waitingReasons: ['Value changed on the board'] });
    expect(outcome.hasWaiting).toBe(true);
  });

  it('reports nothing waiting when every row of the facility was written', () => {
    const outcome = facilityWriteOutcome('f-1', [row({ status: 'applied' }), row({ status: 'alreadyApplied' })]);
    expect(outcome.hasWaiting).toBe(false);
  });
});
