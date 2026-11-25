import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import type { ListedBlock, SignalPreview } from '$lib/api/signaling';
import type { BlockOccupancyMap } from './occupancy';
import TrackDiagram from './TrackDiagram.svelte';
import type { TrackInput } from './trackGeometry';

const blocks: ListedBlock[] = [
  { blockId: 'a', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [{ turnoutId: 't1', name: 'Switch 1' }] },
  { blockId: 'b', name: 'Mill curve', joinedToPrevious: true, turnouts: [] },
] as ListedBlock[];

const signal = {
  signalId: 's1', name: 'Cedar East', governs: 'eastward', run: 'withList', entranceBlockId: 'a',
  protectedBlockIds: ['a'], turnoutIds: [], settings: { aspects: 3, approachLighting: 'alwaysLit' },
  overridden: false, next: { kind: 'nothingBeyond' }, rules: [], observation: 'unavailable',
} as SignalPreview;

const input: TrackInput = {
  mode: 'summary', blocks, direction: 'eastward', movements: 'oneDirection',
  signals: [signal], shapes: {}, endLabels: { start: 'Plant', end: null },
};

describe('TrackDiagram summary mode', () => {
  it('draws the line read-only: names and occupancy, nothing to click or focus', () => {
    const { container } = render(TrackDiagram, {
      input,
      occupancy: new Map([['a', 'occupied'], ['b', 'clear']]) as BlockOccupancyMap,
    });

    expect(screen.getByLabelText('Track and signals for eastward travel')).toBeTruthy();
    expect(screen.getByText('Cedar East')).toBeTruthy();
    expect(screen.getByText('Plant')).toBeTruthy();
    expect(container.querySelector('[role="button"]')).toBeNull();
    expect(container.querySelector('[tabindex]')).toBeNull();
    expect(container.querySelector('circle.occupancy.occupied')).not.toBeNull();
    expect(container.querySelector('circle.occupancy.clear')).not.toBeNull();
  });
});
