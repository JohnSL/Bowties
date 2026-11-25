/**
 * E5-T4 — `effectiveLayoutStore.facilityWriteOutcome(facilityId)`: per-board
 * write outcome of a facility, read from the Pending Change rows the layout
 * loaded (origin survives reopening because the rows persist).
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
import { pendingChangesStore } from '$lib/stores/pendingChanges.svelte';

beforeEach(() => pendingChangesStore.clear());

describe('effectiveLayoutStore.facilityWriteOutcome', () => {
  it('reports each board of a facility with a waiting row from rows loaded as on layout open', () => {
    pendingChangesStore.setRows([
      { changeId: 'a', kind: 'config', nodeId: '05.02.01.02.03.00', space: 253, offset: '0x1', baselineValue: '0', plannedValue: '1', status: 'applied', origin: { facilityId: 'f-1' } },
      { changeId: 'b', kind: 'config', nodeId: '05.02.01.02.03.01', space: 253, offset: '0x2', baselineValue: '0', plannedValue: '1', status: 'failed', error: 'Timed out', origin: { facilityId: 'f-1' } },
    ]);
    const outcome = effectiveLayoutStore.facilityWriteOutcome('f-1');
    expect(outcome.hasWaiting).toBe(true);
    expect(outcome.boards.map((b) => [b.nodeKey, b.waitingReasons])).toEqual([['050201020301', ['Timed out']]]);
    expect(outcome.boards[0].nodeName).toBeTruthy();
    expect(effectiveLayoutStore.facilityWriteOutcome('other').hasWaiting).toBe(false);
  });
});
