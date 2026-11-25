import { describe, expect, it } from 'vitest';
import type { ListedBlock } from '$lib/api/signaling';
import { trackInputFor, type TrackLine } from './trackInput';

const blocks: ListedBlock[] = [
  { blockId: 'a', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] },
];

const line: TrackLine = {
  blocks, direction: 'westward', movements: 'bothDirections',
  entranceEnd: { kind: 'plant', plantId: 'p1' }, exitEnd: { kind: 'treatAsStop' }, signals: [],
};

describe('trackInputFor', () => {
  it('labels each end from the line and carries the mode and line facts through', () => {
    const input = trackInputFor(line, 'summary');

    expect(input.mode).toBe('summary');
    expect(input.blocks).toBe(blocks);
    expect(input.direction).toBe('westward');
    expect(input.movements).toBe('bothDirections');
    expect(input.shapes).toEqual({});
    expect(input.endLabels).toEqual({ start: 'Plant', end: 'End: Stop' });
  });

  it('passes shapes and mode-specific extras through', () => {
    const shapes = { t1: { face: 'east', side: 'above' } } as const;
    const input = trackInputFor({ ...line, shapes }, 'signals', { selectedSignalId: 's1', issueCounts: { s1: 2 } });

    expect(input.shapes).toBe(shapes);
    expect(input.selectedSignalId).toBe('s1');
    expect(input.issueCounts).toEqual({ s1: 2 });
  });
});
