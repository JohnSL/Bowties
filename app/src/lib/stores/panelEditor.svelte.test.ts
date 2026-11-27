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

  it.each(['panel', 'entry'] as const)('clears a finish error when %s lit-when changes', (target) => {
    store.start('P');
    store.addBlock(A);
    store.setFinishError('Cannot finish');
    if (target === 'panel') store.setLitWhen('clear');
    else store.setEntryLitWhen(store.entries[0].entryId, 'clear');
    expect(store.finishError).toBeNull();
  });

  it('addBlocks keeps first-seen order and snapshots block names while skipping duplicates', () => {
    store.start('P');
    const first = { ...A };
    store.addBlocks([first, B, { ...A, name: 'Duplicate' }]);
    first.name = 'Changed later';
    store.addBlocks([B]);
    expect(store.entries.map(({ blockId, blockName }) => [blockId, blockName])).toEqual([
      ['block:a', 'Block A'], ['block:b', 'Block B'],
    ]);
  });

  it('moves whole entries and leaves them unchanged at bounds or for an unknown id', () => {
    store.start('P');
    store.addBlocks([A, B]);
    const [a, b] = store.entries;
    store.seatLamp(a.entryId, 'node1', 3);
    store.setEntryLitWhen(a.entryId, 'clear');
    store.moveEntry(a.entryId, -1);
    store.moveEntry(b.entryId, 1);
    store.moveEntry('missing', 1);
    expect(store.entries.map((e) => e.entryId)).toEqual([a.entryId, b.entryId]);
    store.moveEntry(a.entryId, 1);
    expect(store.entries.map((e) => e.entryId)).toEqual([b.entryId, a.entryId]);
    expect(store.entries[1]).toMatchObject({ blockId: A.blockId, blockName: A.name, lamp: { nodeKey: 'node1', output: 3 }, litWhen: 'clear' });
    store.moveEntry(a.entryId, -1);
    expect(store.entries.map((e) => e.entryId)).toEqual([a.entryId, b.entryId]);
  });

  it('moves an entry to an arbitrary valid index without changing composition, and ignores invalid moves', () => {
    const entries = [
      { entryId: 'a', blockId: 'block:a', blockName: 'Block A', lamp: { nodeKey: 'node1', output: 3 }, litWhen: 'clear' as const },
      { entryId: 'b', blockId: 'block:b', blockName: 'Block B' },
      { entryId: 'c', blockId: 'block:c', blockName: 'Block C', lamp: { nodeKey: 'node2', output: 4 } },
      { entryId: 'd', blockId: 'block:d', blockName: 'Block D' },
    ];
    store.startEdit({ facilityId: 'f', name: 'Panel', litWhen: 'occupied', entries });
    store.selectEntry('a');

    store.moveEntryTo('a', 3);
    expect(store.entries).toEqual([entries[1], entries[2], entries[3], entries[0]]);
    expect(store.selectedEntryId).toBe('a');
    store.moveEntryTo('a', 1);
    expect(store.entries).toEqual([entries[1], entries[0], entries[2], entries[3]]);

    for (const [id, index] of [['missing', 0], ['a', -1], ['a', 4], ['a', 1.5], ['a', NaN], ['a', Infinity], ['a', 1]] as const) {
      const before = store.entries;
      store.moveEntryTo(id, index);
      expect(store.entries).toBe(before);
      expect(store.entries).toEqual([entries[1], entries[0], entries[2], entries[3]]);
    }
  });

  it('removes an entry override when restoring the panel default', () => {
    store.start('P');
    store.addBlock(A);
    const id = store.entries[0].entryId;
    store.setEntryLitWhen(id, 'clear');
    store.setEntryLitWhen(id, undefined);
    expect(store.entries[0]).not.toHaveProperty('litWhen');
    expect(store.litWhen).toBe('occupied');
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

  it('restores a wired entry’s board on selection without changing assignments or releases', () => {
    store.startEdit({
      facilityId: 'f', name: 'Panel', litWhen: 'occupied',
      entries: [
        { entryId: 'a', blockId: 'block:a', blockName: 'A' },
        { entryId: 'b', blockId: 'block:b', blockName: 'B', lamp: { nodeKey: 'node2', output: 4 } },
        { entryId: 'c', blockId: 'block:c', blockName: 'C' },
      ],
    });
    const owner = { nodeKey: 'node1', owner: 'Direct lamp 4' };
    store.releaseAndSeat('a', owner, 'node1', 3);
    const entries = JSON.parse(JSON.stringify(store.entries));
    store.setBoard('node3');
    store.setMessage({ kind: 'warn', text: 'Warning' });
    store.setPendingRelease({ owner, nodeKey: 'node1', output: 3, description: 'Lamp' });

    store.selectEntry('a');
    expect(store.boardNodeKey).toBe('node1');
    expect(store.message).toBeUndefined();
    expect(store.pendingRelease).toBeNull();
    store.selectEntry('b');
    expect(store.boardNodeKey).toBe('node2');
    store.selectEntry('a');
    expect(store.boardNodeKey).toBe('node1');
    store.setBoard('node3');
    store.selectEntry('c');
    expect(store.boardNodeKey).toBe('node3');
    store.selectEntry(undefined);
    expect(store.boardNodeKey).toBe('node3');
    store.selectEntry('a');
    expect(store.boardNodeKey).toBe('node1');
    expect(store.entries).toEqual(entries);
    expect(store.releases).toEqual([owner]);
  });

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

  it('copies edit overrides and lamps without retaining the seed entry', () => {
    const entry = { entryId: 'e', blockId: 'b', blockName: 'B', litWhen: 'clear' as const, lamp: { nodeKey: 'node1', output: 3 } };
    store.startEdit({ facilityId: 'f', name: 'n', litWhen: 'occupied', entries: [entry] });
    entry.lamp.output = 7;
    expect(store.entries[0]).toMatchObject({ litWhen: 'clear', lamp: { nodeKey: 'node1', output: 3 } });
    store.setEntryLitWhen('e', 'occupied');
    expect(entry.litWhen).toBe('clear');
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
