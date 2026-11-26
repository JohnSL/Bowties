import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PanelWiringFacts, PinOwner } from '$lib/api/panels';

const panelWiringFactsMock = vi.fn<(nodeKeys?: string[]) => Promise<PanelWiringFacts>>();
vi.mock('$lib/api/panels', () => ({ panelWiringFacts: panelWiringFactsMock }));

const createPanelFacilityMock = vi.fn();
const syncDraftsMock = vi.fn(async () => undefined);
vi.mock('$lib/orchestration/facilityOrchestrator', () => ({
  createPanelFacility: createPanelFacilityMock,
  syncDraftsForComposition: syncDraftsMock,
}));

const { panelEditorStore: store } = await import('$lib/stores/panelEditor.svelte');
const { panelEditorActions: actions } = await import('$lib/orchestration/panelEditorOrchestrator');

const free: PinOwner = { kind: 'free' };
const NODE = '050101010101';

function facts(owners: Record<number, PinOwner> = {}): PanelWiringFacts {
  return {
    boards: [{
      nodeKey: NODE,
      name: 'Board 1',
      pins: [8, 9, 10].map((output) => ({
        output,
        label: `#${output} H3-G`,
        group: 'H3',
        owner: owners[output] ?? free,
      })),
    }],
    issues: [],
  };
}

async function openOnWiring(owners: Record<number, PinOwner> = {}) {
  panelWiringFactsMock.mockResolvedValue(facts(owners));
  actions.startPanelEditor('Panel');
  await actions.addBlock({ blockId: 'block:a', name: 'Block A' });
  await actions.addBlock({ blockId: 'block:b', name: 'Block B' });
  await actions.goTo('wiring');
  const [a, b] = store.entries;
  return { a, b };
}

describe('panelEditorOrchestrator wiring step', () => {
  beforeEach(() => {
    store.reset();
    panelWiringFactsMock.mockReset();
    createPanelFacilityMock.mockReset();
    syncDraftsMock.mockClear();
  });

  it('loads wiring facts once after syncing drafts and defaults to the first board', async () => {
    await openOnWiring();
    await actions.goTo('blocks');
    await actions.goTo('wiring');
    expect(panelWiringFactsMock).toHaveBeenCalledTimes(1);
    expect(syncDraftsMock).toHaveBeenCalled();
    expect(store.boardNodeKey).toBe(NODE);
  });

  it('a free pin seats the selected entry; its own pin clears it', async () => {
    const { a } = await openOnWiring();
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 8);
    expect(store.lampFor(a.entryId)).toEqual({ nodeKey: NODE, output: 8 });
    actions.pinClick(NODE, 8);
    expect(store.lampFor(a.entryId)).toBeNull();
  });

  it('choosing a pin another entry holds moves it and says who needs a lamp', async () => {
    const { a, b } = await openOnWiring();
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 9);
    actions.selectEntry(b.entryId);
    actions.pinClick(NODE, 9);
    expect(store.lampFor(b.entryId)).toEqual({ nodeKey: NODE, output: 9 });
    expect(store.lampFor(a.entryId)).toBeNull();
    expect(store.message).toEqual({
      kind: 'warn',
      text: 'Moved #9 H3-G from Block A to Block B. Block A now needs a lamp.',
    });
  });

  it('a pin owned by another facility or panel is not choosable', async () => {
    const { a } = await openOnWiring({
      8: { kind: 'facility', facilityId: 'f1', name: 'Signals' },
      9: { kind: 'panel', facilityId: 'other', name: 'Other panel', row: 2 },
    });
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 8);
    actions.pinClick(NODE, 9);
    expect(store.lampFor(a.entryId)).toBeNull();
  });

  it('an outside-owned pin is not choosable and the message names its Lamp Description', async () => {
    const { a } = await openOnWiring({ 10: { kind: 'outside', row: 4, description: 'Yard lamp' } });
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 10);
    expect(store.lampFor(a.entryId)).toBeNull();
    expect(store.message?.kind).toBe('warn');
    expect(store.message?.text).toContain('Yard lamp');
  });

  it('changing board leaves the existing lamp until a pin is clicked', async () => {
    const { a } = await openOnWiring();
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 8);
    actions.chooseBoard('050101010102');
    expect(store.boardNodeKey).toBe('050101010102');
    expect(store.lampFor(a.entryId)).toEqual({ nodeKey: NODE, output: 8 });
  });

  it('pinClick without a selected entry does nothing', async () => {
    await openOnWiring();
    actions.pinClick(NODE, 8);
    expect(store.entries.every((e) => !e.lamp)).toBe(true);
  });
});

describe('panelEditorOrchestrator finish and cancel', () => {
  beforeEach(() => {
    store.reset();
    panelWiringFactsMock.mockReset();
    createPanelFacilityMock.mockReset();
  });

  it('Done creates the facility with entries in order and lamps only for wired entries, then closes', async () => {
    const { a } = await openOnWiring();
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 8);
    const facilityId = store.facilityId;
    createPanelFacilityMock.mockResolvedValue({ ok: true });
    await actions.finish();
    const args = createPanelFacilityMock.mock.calls[0][0];
    expect(args.facilityId).toBe(facilityId);
    expect(args.name).toBe('Panel');
    expect(args.record).toEqual({
      litWhen: 'occupied',
      entries: [
        { entryId: store.entries[0]?.entryId ?? a.entryId, blockId: 'block:a' },
        { entryId: expect.any(String), blockId: 'block:b' },
      ],
    });
    expect(args.lamps).toEqual([{ entryId: a.entryId, nodeKey: NODE, output: 8, name: 'Block A' }]);
    expect(store.open).toBe(false);
  });

  it('compose issues keep the editor open with the error text', async () => {
    await openOnWiring();
    createPanelFacilityMock.mockResolvedValue({
      ok: false,
      issues: [{ code: 'pinOwnedElsewhere', entryId: 'x', nodeKey: NODE, output: 8, detail: 'Pin 8 is taken.' }],
    });
    await actions.finish();
    expect(store.open).toBe(true);
    expect(store.finishError).toBe('Pin 8 is taken.');
  });

  it('Cancel stages nothing and a late Done result is discarded', async () => {
    await openOnWiring();
    let resolve!: (v: unknown) => void;
    createPanelFacilityMock.mockReturnValue(new Promise((r) => { resolve = r; }));
    const pending = actions.finish();
    actions.cancel();
    resolve({ ok: false, issues: [{ code: 'pinOwnedElsewhere', entryId: 'x', nodeKey: NODE, output: 8, detail: 'late' }] });
    await pending;
    expect(store.open).toBe(false);
    expect(store.finishError).toBeNull();
    expect(createPanelFacilityMock).toHaveBeenCalledTimes(1);
  });
});

describe('panelEditorOrchestrator empty panel', () => {
  beforeEach(() => {
    store.reset();
    panelWiringFactsMock.mockReset();
    createPanelFacilityMock.mockReset();
  });

  it('Done with no blocks creates an empty panel with no lamps', async () => {
    actions.startPanelEditor('Empty');
    createPanelFacilityMock.mockResolvedValue({ ok: true });
    await actions.finish();
    const args = createPanelFacilityMock.mock.calls[0][0];
    expect(args.record).toEqual({ litWhen: 'occupied', entries: [] });
    expect(args.lamps).toEqual([]);
    expect(store.open).toBe(false);
  });

  it('Wiring is reachable with no blocks and loads its facts', async () => {
    panelWiringFactsMock.mockResolvedValue(facts());
    actions.startPanelEditor('Empty');
    await actions.goTo('wiring');
    expect(store.step).toBe('wiring');
    expect(store.boards).toHaveLength(1);
  });
});
