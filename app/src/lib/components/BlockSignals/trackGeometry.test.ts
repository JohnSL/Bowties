import { describe, expect, it } from 'vitest';
import type { ListedBlock, SignalPreview } from '$lib/api/signaling';
import { buildTrackModel, mirrorX, signalBlocks, type TrackInput } from './trackGeometry';
import { conditionText, endChipLabel, nextSignalText, rulesNote } from './aspectText';

const block = (blockId: string, name: string, extra: Partial<ListedBlock> = {}): ListedBlock => ({
  blockId, name, joinedToPrevious: false, turnouts: [], ...extra,
});

function signal(overrides: Partial<SignalPreview>): SignalPreview {
  return {
    signalId: 's', name: 'S', governs: 'eastward', run: 'withList', entranceBlockId: 'a',
    protectedBlockIds: ['a'], turnoutIds: [], settings: { aspects: 3, approachLighting: 'alwaysLit' },
    overridden: false, next: { kind: 'nothingBeyond' }, rules: [], observation: 'unavailable',
    ...overrides,
  };
}

function input(overrides: Partial<TrackInput>): TrackInput {
  return {
    mode: 'blocks',
    blocks: [block('a', 'Cedar Hill'), block('b', 'Mill curve')],
    direction: 'eastward',
    movements: 'oneDirection',
    signals: [],
    shapes: {},
    endLabels: { start: null, end: null },
    ...overrides,
  };
}

describe('trackGeometry', () => {
  it('groups joined blocks into signal blocks, ignoring a joined flag on the first block', () => {
    expect(signalBlocks([
      block('a', 'A', { joinedToPrevious: true }),
      block('b', 'B', { joinedToPrevious: true }),
      block('c', 'C'),
    ])).toEqual([[0, 1], [2]]);
  });

  it('places each block centre in list order and mirrors the drawing for a westward list', () => {
    const east = buildTrackModel(input({}));
    const west = buildTrackModel(input({ direction: 'westward' }));

    expect(east.blocks[0].cx).toBeLessThan(east.blocks[1].cx);
    expect(west.flip).toBe(true);
    expect(mirrorX(west, west.blocks[0].cx)).toBeGreaterThan(mirrorX(west, west.blocks[1].cx));
  });

  it('puts with-list signals at the start of their signal block and against-list signals at its far end', () => {
    const model = buildTrackModel(input({
      movements: 'bothDirections',
      signals: [
        signal({ signalId: 'e', entranceBlockId: 'a' }),
        signal({ signalId: 'w', entranceBlockId: 'b', run: 'againstList', governs: 'westward' }),
      ],
    }));

    const [east, west] = model.signals;
    expect(east.jx).toBe(model.joints[0]);
    expect(east.k).toBe(1);
    expect(east.sy).toBe(1);
    expect(west.jx).toBe(model.joints[2]);
    expect(west.k).toBe(-1);
    expect(west.sy).toBe(-1);
    expect(model.arrows).toEqual({ east: true, west: true });
  });

  it('shades the whole signal block of a selected turnout and draws a ghost at a joined boundary', () => {
    const blocks = [
      block('a', 'Cedar Hill'),
      block('b', 'Mill curve', { joinedToPrevious: true, turnouts: [{ turnoutId: 't', name: 'Mill siding' }] }),
    ];
    const model = buildTrackModel(input({ blocks, selection: { kind: 'turnout', turnoutId: 't' } }));

    expect(model.shade).toEqual({ x0: model.joints[0] + model.gap, x1: model.joints[2] - model.gap });
    expect(model.turnouts[0]).toMatchObject({ turnoutId: 't', selected: true, blockName: 'Mill curve' });
    expect(model.ghosts).toEqual([{
      blockId: 'b',
      between: 'Cedar Hill and Mill curve',
      signals: [expect.objectContaining({ name: 'Mill curve East', jx: model.joints[1] })],
    }]);
  });

  it('makes room below the rail for a signal box that carries a to-resolve count', () => {
    const signals = [signal({ signalId: 's1' })];
    const plain = buildTrackModel(input({ mode: 'signals', signals }));
    const flagged = buildTrackModel(input({ mode: 'signals', signals, issueCounts: { s1: 1 } }));
    const boxBottom = flagged.railY + 16 + 50;
    expect(flagged.height).toBeGreaterThan(boxBottom + 1);
    expect(plain.height).toBeLessThan(flagged.height);
  });

  it('keeps turnouts clear of the block centre', () => {
    const blocks = [block('a', 'A', { turnouts: [{ turnoutId: 't1', name: '1' }, { turnoutId: 't2', name: '2' }] })];
    const model = buildTrackModel(input({ blocks }));

    for (const turnout of model.turnouts) expect(turnout.x).not.toBe(model.blocks[0].cx);
  });
});

describe('aspectText', () => {
  const blocks = [block('a', 'A', { turnouts: [{ turnoutId: 't', name: 'Depot track' }] })];

  it('describes conditions for one block or a joined signal block', () => {
    expect(conditionText({ kind: 'blockOccupied' }, blocks, 1)).toBe('Protected block occupied');
    expect(conditionText({ kind: 'blockOccupied' }, blocks, 2)).toBe('A protected block occupied');
    expect(conditionText({ kind: 'turnoutNotLined', turnoutId: 't' }, blocks, 1))
      .toBe('Depot track not lined for the main, or position unknown');
    expect(conditionText({ kind: 'clearNothingBeyondTreatedAsStop' }, blocks, 1))
      .toBe('Protected block clear; nothing beyond, treated as Stop');
  });

  it('names the next signal and explains fallbacks at the end of the track', () => {
    const next = signal({ signalId: 'n', name: 'Mill curve East' });
    expect(nextSignalText(signal({ next: { kind: 'signal', signalId: 'n' } }), [next])).toBe('Mill curve East');
    expect(rulesNote(signal({ next: { kind: 'treatedAsStop' } }))).toMatch(/Approach at best/);
    expect(rulesNote(signal({ next: { kind: 'nothingBeyond' } }))).toMatch(/Stop \/ Clear only/);
    expect(rulesNote(signal({ next: { kind: 'signal', signalId: 'n' } }))).toBeNull();
  });

  it('labels the end chips', () => {
    expect(endChipLabel('end', { kind: 'noPlant' }, { kind: 'treatAsStop' })).toBe('End: Stop');
    expect(endChipLabel('end', { kind: 'noPlant' }, { kind: 'nothingBeyond' })).toBeNull();
    expect(endChipLabel('start', { kind: 'noPlant' }, { kind: 'treatAsStop' })).toBeNull();
  });

  it('summary mode draws plain signals as in signals mode, with no ghosts, shade, selection room or issue room', () => {
    const blocks = [block('a', 'Cedar Hill'), block('b', 'Mill curve', { joinedToPrevious: true })];
    const sig = signal({ signalId: 's1', name: 'Cedar East', entranceBlockId: 'a' });
    const base = {
      blocks, signals: [sig], selection: { kind: 'block', blockId: 'b' } as const,
      selectedSignalId: 's1', issueCounts: { s1: 2 },
    };
    const summary = buildTrackModel(input({ ...base, mode: 'summary' }));
    const signalsMode = buildTrackModel(input({ ...base, mode: 'signals' }));

    expect(summary.ghosts).toEqual([]);
    expect(summary.shade).toBeNull();
    expect(summary.signals[0].selected).toBe(false);
    expect(summary.signals[0].issueCount).toBe(0);
    expect(summary.signals[0].jx).toBe(signalsMode.signals[0].jx);
    expect(summary.height).toBe(buildTrackModel(input({ blocks, mode: 'blocks' })).height);
  });
});
