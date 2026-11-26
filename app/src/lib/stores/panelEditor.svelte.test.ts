import { beforeEach, describe, expect, it } from 'vitest';
import { panelEditorStore as store } from '$lib/stores/panelEditor.svelte';

const A = { blockId: 'block:a', name: 'Block A' };
const B = { blockId: 'block:b', name: 'Block B' };

describe('panelEditorStore blocks step', () => {
  beforeEach(() => store.reset());

  it('start opens on the Blocks step with no entries and a minted facility id', () => {
    store.start('Dispatcher panel');
    expect(store.open).toBe(true);
    expect(store.name).toBe('Dispatcher panel');
    expect(store.step).toBe('blocks');
    expect(store.entries).toEqual([]);
    expect(store.facilityId).not.toBe('');
  });

  it('addBlock appends an entry and refuses a block already on the panel', () => {
    store.start('P');
    store.addBlock(A);
    store.addBlock(B);
    store.addBlock(A);
    expect(store.entries.map((e) => e.blockId)).toEqual(['block:a', 'block:b']);
    expect(store.entries[0].entryId).not.toBe(store.entries[1].entryId);
  });

  it('removeEntry drops the entry and its lamp', () => {
    store.start('P');
    store.addBlock(A);
    store.addBlock(B);
    const [a, b] = store.entries;
    store.seatLamp(a.entryId, 'node1', 3);
    store.removeEntry(a.entryId);
    expect(store.entries.map((e) => e.entryId)).toEqual([b.entryId]);
    expect(store.lampFor(a.entryId)).toBeNull();
  });

  it('seatLamp refuses a pin another entry holds, and the owner can still clear it', () => {
    store.start('P');
    store.addBlock(A);
    store.addBlock(B);
    const [a, b] = store.entries;
    store.seatLamp(a.entryId, 'node1', 3);
    store.seatLamp(b.entryId, 'node1', 3);
    expect(store.lampFor(b.entryId)).toBeNull();
    expect(store.lampFor(a.entryId)).toEqual({ nodeKey: 'node1', output: 3 });
    store.seatLamp(a.entryId, 'node1', 3);
    expect(store.lampFor(a.entryId)).toBeNull();
  });

  it('step bar reports block count and wiring need', () => {
    store.start('P');
    expect(store.stepBar).toEqual({ blocksCount: 0, wiring: 'none' });
    store.addBlock(A);
    store.addBlock(B);
    expect(store.stepBar).toEqual({ blocksCount: 2, wiring: { needLamp: 2 } });
    store.seatLamp(store.entries[0].entryId, 'node1', 3);
    expect(store.stepBar.wiring).toEqual({ needLamp: 1 });
    store.seatLamp(store.entries[1].entryId, 'node1', 4);
    expect(store.stepBar.wiring).toBe('allWired');
  });

  it('goTo switches steps, including Wiring with no blocks', () => {
    store.start('P');
    store.goTo('wiring');
    expect(store.step).toBe('wiring');
    store.goTo('blocks');
    expect(store.step).toBe('blocks');
  });

  it('reset discards everything', () => {
    store.start('P');
    store.addBlock(A);
    store.reset();
    expect(store.open).toBe(false);
    expect(store.entries).toEqual([]);
    expect(store.name).toBe('');
  });
});

describe('panelEditorStore edit mode', () => {
  beforeEach(() => store.reset());

  it('startEdit keeps the facility id, opens in edit mode on Blocks with entries and lamps filled', () => {
    store.startEdit({
      facilityId: 'fac-1',
      name: 'Saved panel',
      litWhen: 'occupied',
      entries: [
        { entryId: 'e1', blockId: 'block:a', blockName: 'Block A', lamp: { nodeKey: 'node1', output: 9 } },
        { entryId: 'e2', blockId: 'block:b', blockName: 'Block B' },
      ],
    });
    expect(store.open).toBe(true);
    expect(store.mode).toBe('edit');
    expect(store.facilityId).toBe('fac-1');
    expect(store.name).toBe('Saved panel');
    expect(store.step).toBe('blocks');
    expect(store.lampFor('e1')).toEqual({ nodeKey: 'node1', output: 9 });
    expect(store.lampFor('e2')).toBeNull();
    expect(store.entries.map((e) => e.entryId)).toEqual(['e1', 'e2']);
  });

  it('start opens in create mode', () => {
    store.start('P');
    expect(store.mode).toBe('create');
  });
});

describe('panelEditorStore release', () => {
  beforeEach(() => store.reset());
  const owner = { nodeKey: 'node1', owner: 'Direct lamp 4' };

  it('releaseAndSeat records the release once and seats the lamp; start, startEdit and reset clear releases', () => {
    store.start('P');
    store.addBlock(A);
    const [a] = store.entries;
    store.releaseAndSeat(a.entryId, owner, 'node1', 10);
    expect(store.releases).toEqual([owner]);
    expect(store.lampFor(a.entryId)).toEqual({ nodeKey: 'node1', output: 10 });

    store.start('Q');
    expect(store.releases).toEqual([]);
    store.addBlock(A);
    store.releaseAndSeat(store.entries[0].entryId, owner, 'node1', 10);
    store.startEdit({ facilityId: 'f', name: 'n', litWhen: 'occupied', entries: [{ entryId: 'e', blockId: 'b', blockName: 'B' }] });
    expect(store.releases).toEqual([]);
    store.releaseAndSeat('e', owner, 'node1', 10);
    store.reset();
    expect(store.releases).toEqual([]);
  });
});
