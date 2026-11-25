import { beforeEach, describe, expect, it } from 'vitest';
import { blockSignalsEditorStore as store } from './blockSignalsEditor.svelte';
import type { BlockSignalsEditSeed } from './blockSignalsEditor.svelte';
import type { BlockSignalsPreview } from '$lib/api/signaling';
import { previewWith } from '$lib/components/BlockSignals/editorTestSupport';

const statuses = () => store.steps.map((s) => s.status);

function seed(): BlockSignalsEditSeed {
  return {
    facilityId: 'abs-1',
    name: 'Cedar Hill',
    wiring: { 'sig-1': { nodeKey: 'node-a', redOutput: 1, greenOutput: 2 } },
    record: {
      direction: 'eastward',
      movements: 'oneDirection',
      entranceEnd: { kind: 'noPlant' },
      exitEnd: { kind: 'nothingBeyond' },
      blocks: [{ blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] }],
      settings: { aspects: 3, turnoutNotLined: 'stop', approachLighting: 'alwaysLit' },
      signals: [{
        signalId: 'sig-1', name: 'Cedar Hill East', protectedBlock: 'block:cedar', channelId: 'ch-1',
        resources: { nodeKey: '020157000001', mast: 1, trackCircuits: [] },
      }],
    },
  };
}

const preview = (): BlockSignalsPreview => previewWith();

describe('blockSignalsEditorStore step statuses', () => {
  beforeEach(() => store.reset());

  it('keeps every step but Blocks unavailable until a block is added, and Review always unavailable', () => {
    store.start('Main');
    expect(statuses()).toEqual(['todo', 'unavailable', 'unavailable', 'unavailable', 'unavailable']);
    expect(store.steps[1].unavailableReason).toBe('Add a block first');
    expect(store.steps[4].unavailableReason).toBe('Not in this release');
    store.navigate('aspects');
    expect(store.step).toBe('blocks');
    store.addBlock({ blockId: 'block:cedar', name: 'Cedar Hill' });
    expect(store.steps[4].unavailableReason).toBe('Not in this release');
    store.navigate('review');
    expect(store.step).toBe('blocks');
  });

  it('marks steps later than the furthest one reached as todo while creating', () => {
    store.start('Main');
    store.addBlock({ blockId: 'block:cedar', name: 'Cedar Hill' });
    expect(statuses()).toEqual(['complete', 'todo', 'todo', 'todo', 'unavailable']);
    store.navigate('appearance');
    expect(statuses()).toEqual(['complete', 'complete', 'complete', 'todo', 'unavailable']);
    store.navigate('blocks');
    expect(statuses().slice(0, 4)).toEqual(['complete', 'complete', 'complete', 'todo']);
    store.start('Again');
    expect(statuses()).toEqual(['todo', 'unavailable', 'unavailable', 'unavailable', 'unavailable']);
  });

  it('flags out-of-scope settings as needing work on the step that owns them', () => {
    store.startEdit(seed());
    store.setMovements('bothDirections');
    store.setSettings({ aspects: 2 });
    expect(statuses().slice(0, 3)).toEqual(['needsWork', 'needsWork', 'complete']);
    store.setMovements('oneDirection');
    store.setSettings({ aspects: 3, approachLighting: 'whenApproached' });
    expect(statuses().slice(0, 3)).toEqual(['complete', 'needsWork', 'complete']);
  });

  it('shows every step of a saved facility from its data, with Wiring todo until the preview loads', () => {
    store.startEdit(seed());
    expect(statuses()).toEqual(['complete', 'complete', 'complete', 'todo', 'unavailable']);
    store.acceptPreview(preview());
    expect(statuses()).toEqual(['complete', 'complete', 'complete', 'complete', 'unavailable']);
  });

  it('marks Wiring as needing work when the preview has signals that are not fully wired', () => {
    store.startEdit({ ...seed(), wiring: {} });
    store.acceptPreview(preview());
    expect(store.steps[3].status).toBe('needsWork');
  });
});

describe('blockSignalsEditorStore seating a lamp', () => {
  beforeEach(() => {
    store.reset();
    store.start('Main');
    store.chooseWiringNode('s1', 'node-a');
  });

  it('places a lamp on a free pin', () => {
    store.seatLamp('s1', 'red', 1);
    store.seatLamp('s1', 'green', 2);
    expect(store.wiringFor('s1')).toEqual({ nodeKey: 'node-a', redOutput: 1, greenOutput: 2 });
  });

  it('clears the lamp when its own pin is chosen again, so wiring is no longer complete', () => {
    store.seatLamp('s1', 'red', 1);
    store.seatLamp('s1', 'green', 2);
    store.seatLamp('s1', 'red', 1);
    expect(store.wiringFor('s1')).toEqual({ nodeKey: 'node-a', redOutput: null, greenOutput: 2 });
    expect(store.wiringComplete).toBe(false);
  });

  it("swaps the lamps when the other lamp's pin is chosen", () => {
    store.seatLamp('s1', 'red', 1);
    store.seatLamp('s1', 'green', 2);
    store.seatLamp('s1', 'red', 2);
    expect(store.wiringFor('s1')).toEqual({ nodeKey: 'node-a', redOutput: 2, greenOutput: 1 });
  });

  it('moves the other lamp off the pin when the selected lamp had none', () => {
    store.seatLamp('s1', 'green', 2);
    store.seatLamp('s1', 'red', 2);
    expect(store.wiringFor('s1')).toEqual({ nodeKey: 'node-a', redOutput: 2, greenOutput: null });
  });
});

describe('blockSignalsEditorStore released owners (E2c B1)', () => {
  beforeEach(() => store.reset());

  const owner = { nodeKey: '05.01.01.01.88.00', owner: 'Mast 2' };

  it('holds each released owner once by canonical node key, and clears on reset, create and edit', () => {
    store.start('Main');
    store.releaseOwner(owner);
    store.releaseOwner({ nodeKey: '050101018800', owner: 'Mast 2' });
    store.releaseOwner({ nodeKey: '050101018800', owner: 'Direct lamp 3' });
    expect(store.released).toHaveLength(2);
    expect(store.previewRequest().release).toEqual(store.released);

    store.start('Again');
    expect(store.released).toEqual([]);

    store.releaseOwner(owner);
    store.startEdit({ ...seed(), wiring: {} });
    expect(store.released).toEqual([]);

    store.releaseOwner(owner);
    store.reset();
    expect(store.released).toEqual([]);
  });

  it('releases an owner and places the clicked pin as one intent', () => {
    store.start('Main');
    store.chooseWiringNode('s1', '050101018800');
    store.releaseAndPlace('s1', 'red', owner, 4);
    expect(store.released).toEqual([owner]);
    expect(store.wiringFor('s1')).toMatchObject({ redOutput: 4, greenOutput: null });
    store.releaseAndPlace('s1', 'green', { nodeKey: '050101018800', owner: 'Mast 3' }, 5);
    expect(store.wiringFor('s1')).toMatchObject({ redOutput: 4, greenOutput: 5 });
    expect(store.released).toHaveLength(2);
  });

  it('keeps a lamp already on the released pin instead of clearing it', () => {
    store.start('Main');
    store.chooseWiringNode('s1', '050101018800');
    store.seatLamp('s1', 'red', 4);
    store.releaseAndPlace('s1', 'red', owner, 4);
    expect(store.wiringFor('s1')).toMatchObject({ redOutput: 4, greenOutput: null });
  });
});
