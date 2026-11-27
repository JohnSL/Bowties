import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PanelWiringFacts, PinOwner } from '$lib/api/panels';

const panelWiringFactsMock = vi.fn<(nodeKeys?: string[], release?: unknown[]) => Promise<PanelWiringFacts>>();
vi.mock('$lib/api/panels', () => ({ panelWiringFacts: panelWiringFactsMock }));

const createPanelFacilityMock = vi.fn();
const updatePanelFacilityMock = vi.fn();
const panelCardEntriesMock = vi.fn();
vi.mock('$lib/layout', () => ({ effectiveLayoutStore: { panelCardEntries: panelCardEntriesMock } }));
const syncDraftsMock = vi.fn(async () => undefined);
vi.mock('$lib/orchestration/facilityOrchestrator', () => ({
  createPanelFacility: createPanelFacilityMock,
  updatePanelFacility: updatePanelFacilityMock,
  syncDraftsForComposition: syncDraftsMock,
}));

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { panelEditorStore: store } = await import('$lib/stores/panelEditor.svelte');
const { panelEditorActions: actions, panelEditorOrchestrator } = await import('$lib/orchestration/panelEditorOrchestrator');
const panelEditorOrchestratorStartEdit = (id: string) => panelEditorOrchestrator.startEdit(id);

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
    releaseIssues: [],
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

describe('panelEditorOrchestrator direct editing intents', () => {
  beforeEach(() => store.reset());

  it('exposes arbitrary entry reorder through orchestrator and action intents', () => {
    actions.startPanelEditor('Panel');
    actions.addBlocks([
      { blockId: 'block:a', name: 'Block A' },
      { blockId: 'block:b', name: 'Block B' },
      { blockId: 'block:c', name: 'Block C' },
    ]);
    const [a, b, c] = store.entries;
    actions.moveEntryTo(a.entryId, 2);
    expect(store.entries.map((entry) => entry.entryId)).toEqual([b.entryId, c.entryId, a.entryId]);
    panelEditorOrchestrator.moveEntryTo(a.entryId, 0);
    expect(store.entries.map((entry) => entry.entryId)).toEqual([a.entryId, b.entryId, c.entryId]);
  });

  it('adds unique blocks, moves them within bounds, and sets or clears lit-when overrides', () => {
    actions.startPanelEditor('Panel');
    const a = { blockId: 'block:a', name: 'Block A' };
    const b = { blockId: 'block:b', name: 'Block B' };
    actions.addBlock(a);
    actions.addBlocks([a, b, b]);
    const [first, second] = store.entries;
    expect(store.entries.map((entry) => entry.blockId)).toEqual(['block:a', 'block:b']);
    actions.moveEntry(first.entryId, -1);
    actions.moveEntry(second.entryId, 1);
    expect(store.entries.map((entry) => entry.entryId)).toEqual([first.entryId, second.entryId]);
    actions.moveEntry(second.entryId, -1);
    expect(store.entries.map((entry) => entry.entryId)).toEqual([second.entryId, first.entryId]);
    actions.setLitWhen('clear');
    actions.setEntryLitWhen(first.entryId, 'occupied');
    expect(store.litWhen).toBe('clear');
    expect(store.entries.find((entry) => entry.entryId === first.entryId)?.litWhen).toBe('occupied');
    actions.setEntryLitWhen(first.entryId, undefined);
    expect(store.entries.find((entry) => entry.entryId === first.entryId)).not.toHaveProperty('litWhen');
  });
});

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

  it('a pin another entry holds is refused: nothing moves and there is no message', async () => {
    const { a, b } = await openOnWiring();
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 9);
    actions.selectEntry(b.entryId);
    actions.pinClick(NODE, 9);
    expect(store.lampFor(b.entryId)).toBeNull();
    expect(store.lampFor(a.entryId)).toEqual({ nodeKey: NODE, output: 9 });
    expect(store.message).toBeUndefined();
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

  it('an outside-owned pin does not seat; it offers the release with the Lamp Description', async () => {
    const { a } = await openOnWiring({ 10: { kind: 'outside', row: 4, description: 'Yard lamp' } });
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 10);
    expect(store.lampFor(a.entryId)).toBeNull();
    expect(store.pendingRelease).toEqual({
      owner: { nodeKey: NODE, owner: 'Direct lamp 4' },
      nodeKey: NODE,
      output: 10,
      description: 'Yard lamp',
    });
    actions.pinClick(NODE, 8);
    expect(store.pendingRelease).toBeNull();
  });

  it('confirming a release seats the lamp, re-reads facts with the release, and Done passes it on', async () => {
    const { a } = await openOnWiring({ 10: { kind: 'outside', row: 4, description: 'Yard lamp' } });
    actions.selectEntry(a.entryId);
    actions.pinClick(NODE, 10);
    panelWiringFactsMock.mockResolvedValue({ ...facts(), releaseIssues: [{ nodeKey: NODE, detail: 'gone' }] });
    await panelEditorOrchestrator.confirmRelease();
    const released = [{ nodeKey: NODE, owner: 'Direct lamp 4' }];
    expect(store.lampFor(a.entryId)).toEqual({ nodeKey: NODE, output: 10 });
    expect(store.pendingRelease).toBeNull();
    expect(panelWiringFactsMock).toHaveBeenLastCalledWith(undefined, released);
    expect(store.boards[0].pins.find((p) => p.output === 10)?.owner).toEqual({ kind: 'free' });
    expect(store.releaseIssues).toEqual([{ nodeKey: NODE, detail: 'gone' }]);
    createPanelFacilityMock.mockResolvedValue({ ok: true });
    await actions.finish();
    expect(createPanelFacilityMock.mock.calls[0][0].release).toEqual(released);
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

  it('Done on create sends the panel setting and only explicit entry overrides', async () => {
    const { a, b } = await openOnWiring();
    actions.setLitWhen('clear');
    actions.setEntryLitWhen(a.entryId, 'occupied');
    createPanelFacilityMock.mockResolvedValue({ ok: true });
    await actions.finish();
    expect(createPanelFacilityMock.mock.calls[0][0].record).toEqual({
      litWhen: 'clear',
      entries: [
        { entryId: a.entryId, blockId: 'block:a', litWhen: 'occupied' },
        { entryId: b.entryId, blockId: 'block:b' },
      ],
    });
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

describe('panelEditorOrchestrator edit', () => {
  const SAVED = 'fac-saved';

  beforeEach(() => {
    store.reset();
    panelWiringFactsMock.mockReset();
    createPanelFacilityMock.mockReset();
    updatePanelFacilityMock.mockReset();
    facilitiesStore.hydrateBaseline([
      {
        facilityId: SAVED,
        templateId: 'panel',
        name: 'Saved panel',
        slotBindings: {},
        panel: {
          litWhen: 'occupied',
          entries: [
            { entryId: 'e1', blockId: 'block:a', lampChannelId: 'l1' },
            { entryId: 'e2', blockId: 'block:b', lampChannelId: 'l2' },
          ],
        },
      },
    ]);
    panelCardEntriesMock.mockReturnValue([
      {
        entryId: 'e1', blockId: 'block:a', blockName: 'Block A', blockMissing: false,
        lamp: { channelId: 'l1', nodeKey: NODE, boardName: 'Board 1', boardMissing: false, pin: 9, pinLabel: '#9 H3-G' },
      },
      {
        entryId: 'e2', blockId: 'block:b', blockName: 'Block B', blockMissing: false,
        lamp: { channelId: 'l2', nodeKey: NODE, boardName: 'Board 1', boardMissing: false, pin: null, pinLabel: '' },
      },
    ]);
  });

  it('Edit opens with both steps filled; a row with no pin opens with no lamp', () => {
    panelEditorOrchestratorStartEdit(SAVED);
    expect(store.mode).toBe('edit');
    expect(store.facilityId).toBe(SAVED);
    expect(store.name).toBe('Saved panel');
    expect(store.step).toBe('blocks');
    expect(store.entries.map((e) => [e.entryId, e.blockName])).toEqual([['e1', 'Block A'], ['e2', 'Block B']]);
    expect(store.lampFor('e1')).toEqual({ nodeKey: NODE, output: 9 });
    expect(store.lampFor('e2')).toBeNull();
  });

  it('follows saved entries to their boards before and after wiring facts arrive without staging changes', async () => {
    const otherNode = '050101010102';
    panelCardEntriesMock.mockReturnValue([
      {
        entryId: 'e1', blockId: 'block:a', blockName: 'Block A', blockMissing: false,
        lamp: { channelId: 'l1', nodeKey: NODE, boardName: 'Board 1', boardMissing: false, pin: 9, pinLabel: '#9 H3-G' },
      },
      {
        entryId: 'e2', blockId: 'block:b', blockName: 'Block B', blockMissing: false,
        lamp: { channelId: 'l2', nodeKey: otherNode, boardName: 'Board 2', boardMissing: false, pin: 10, pinLabel: '#10 H3-G' },
      },
    ]);
    let resolveFacts!: (value: PanelWiringFacts) => void;
    panelWiringFactsMock.mockReturnValue(new Promise((resolve) => { resolveFacts = resolve; }));
    actions.startPanelEdit(SAVED);
    const entries = JSON.parse(JSON.stringify(store.entries));
    const deltas = JSON.parse(JSON.stringify(facilitiesStore.collectDeltas()));
    const loading = actions.goTo('wiring');
    await vi.waitFor(() => expect(panelWiringFactsMock).toHaveBeenCalledTimes(1));

    actions.selectEntry('e2');
    expect(store.boardNodeKey).toBe(otherNode);
    expect(store.factsLoaded).toBe(false);
    resolveFacts({
      ...facts(),
      boards: [...facts().boards, { ...facts().boards[0], nodeKey: otherNode, name: 'Board 2' }],
    });
    await loading;
    expect(store.boardNodeKey).toBe(otherNode);
    actions.selectEntry('e1');
    expect(store.boardNodeKey).toBe(NODE);
    actions.selectEntry('e2');
    expect(store.boardNodeKey).toBe(otherNode);
    await actions.goTo('blocks');
    await actions.goTo('wiring');
    expect(panelWiringFactsMock).toHaveBeenCalledTimes(1);
    expect(store.entries).toEqual(entries);
    expect(facilitiesStore.collectDeltas()).toEqual(deltas);
    expect(store.releases).toEqual([]);
    expect(createPanelFacilityMock).not.toHaveBeenCalled();
    expect(updatePanelFacilityMock).not.toHaveBeenCalled();
  });

  it('matches saved entry overrides by entryId even when effective entries are reordered', async () => {
    facilitiesStore.hydrateBaseline([{
      facilityId: SAVED, templateId: 'panel', name: 'Saved panel', slotBindings: {},
      panel: { litWhen: 'clear', entries: [
        { entryId: 'e1', blockId: 'block:a', litWhen: 'occupied' },
        { entryId: 'e2', blockId: 'block:b' },
      ] },
    }]);
    panelCardEntriesMock.mockReturnValue([
      { entryId: 'e2', blockId: 'block:b', blockName: 'Block B', blockMissing: false, lamp: null },
      { entryId: 'e1', blockId: 'block:a', blockName: 'Block A', blockMissing: false, lamp: null },
    ]);
    actions.startPanelEdit(SAVED);
    expect(store.entries.map((entry) => entry.entryId)).toEqual(['e2', 'e1']);
    expect(store.entries[0]).not.toHaveProperty('litWhen');
    expect(store.entries[1].litWhen).toBe('occupied');
    updatePanelFacilityMock.mockResolvedValue({ ok: true });
    await actions.finish();
    expect(updatePanelFacilityMock.mock.calls[0][0].record).toEqual({
      litWhen: 'clear', entries: [
        { entryId: 'e2', blockId: 'block:b' },
        { entryId: 'e1', blockId: 'block:a', litWhen: 'occupied' },
      ],
    });
    expect(createPanelFacilityMock).not.toHaveBeenCalled();
  });

  it('Done in edit mode updates the facility with the same id and never creates', async () => {
    panelEditorOrchestratorStartEdit(SAVED);
    updatePanelFacilityMock.mockResolvedValue({ ok: true });
    await actions.finish();
    expect(createPanelFacilityMock).not.toHaveBeenCalled();
    const args = updatePanelFacilityMock.mock.calls[0][0];
    expect(args.facilityId).toBe(SAVED);
    expect(args.name).toBe('Saved panel');
    expect(args.record).toEqual({
      litWhen: 'occupied',
      entries: [{ entryId: 'e1', blockId: 'block:a' }, { entryId: 'e2', blockId: 'block:b' }],
    });
    expect(args.lamps).toEqual([{ entryId: 'e1', nodeKey: NODE, output: 9, name: 'Block A' }]);
    expect(store.open).toBe(false);
  });

  it('Cancel in edit mode calls no facility function and leaves the saved panel unchanged', () => {
    panelEditorOrchestratorStartEdit(SAVED);
    actions.cancel();
    expect(store.open).toBe(false);
    expect(createPanelFacilityMock).not.toHaveBeenCalled();
    expect(updatePanelFacilityMock).not.toHaveBeenCalled();
    expect(facilitiesStore.facilities.find((f) => f.facilityId === SAVED)?.panel?.entries).toHaveLength(2);
  });
});
