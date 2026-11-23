import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  BlockSignalsPreview,
  BlockSignalsPreviewRequest,
  ComposeBlockSignalsRequest,
  ComposeBlockSignalsResponse,
  SignalOutputHost,
} from '$lib/api/signaling';

const previewBlockSignalsMock = vi.fn<(request: BlockSignalsPreviewRequest) => Promise<BlockSignalsPreview>>();

vi.mock('$lib/api/behaviorTemplates', () => ({
  listBehaviorTemplates: async () => [
    {
      templateId: 'automatic-block-signals',
      displayName: 'Automatic Block Signals',
      slots: [{ label: 'signals', displayLabel: 'signals', kind: 'consumer', requiredRole: 'signal-indication', minChannels: 0, maxChannels: null }],
      mapping: [],
    },
  ],
}));
vi.mock('$lib/api/facilities', () => ({ listFacilities: async () => [] }));
vi.mock('$lib/api/channels', () => ({ listChannels: async () => [] }));
vi.mock('$lib/api/facilityBowties', () => ({ composeFacilityBowties: vi.fn() }));
vi.mock('$lib/api/layout', () => ({
  syncLayoutDrafts: vi.fn(async () => undefined),
  clearLayoutDrafts: vi.fn(async () => undefined),
}));

const composeBlockSignalsMock = vi.fn<(request: ComposeBlockSignalsRequest) => Promise<ComposeBlockSignalsResponse>>();

vi.mock('$lib/api/signaling', () => ({
  previewBlockSignals: previewBlockSignalsMock,
  composeBlockSignals: composeBlockSignalsMock,
}));

const { blockSignalsEditorStore: store } = await import('$lib/stores/blockSignalsEditor.svelte');
const {
  addSignaledBlock,
  addTurnout,
  cancelBlockSignalsEditor,
  chooseGreenOutput,
  chooseRedOutput,
  chooseWiringNode,
  finishBlockSignalsEditor,
  moveTurnout,
  navigateBlockSignalsEditor,
  removeSignaledBlock,
  resetBlockSignalsEditorForNewLayout,
  setExitEnd,
  setBlockSignalsMovements,
  setFacilityName,
  setFacilitySignalSettings,
  setJoinedToPrevious,
  setSignalName,
  setSignalOverride,
  setTurnoutShape,
  startBlockSignalsEditor,
  startEditBlockSignalsEditor,
} = await import('./blockSignalsEditorOrchestrator');

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function preview(facilityName = 'Canonical'): BlockSignalsPreview {
  return {
    facilityName,
    direction: 'eastward',
    movements: 'oneDirection',
    blocks: [],
    entranceEnd: { kind: 'noPlant' },
    exitEnd: { kind: 'nothingBeyond' },
    signals: [],
    capability: { kind: 'incompletePreview', stage: 'unavailable', save: 'unavailable', apply: 'unavailable' },
    issues: [],
  };
}

const CEDAR = { blockId: 'block:cedar', name: 'Cedar Hill' };
const MILL = { blockId: 'block:mill', name: 'Mill curve' };

describe('blockSignalsEditorOrchestrator', () => {
  beforeEach(() => {
    previewBlockSignalsMock.mockReset();
    previewBlockSignalsMock.mockImplementation(async () => preview());
    resetBlockSignalsEditorForNewLayout();
  });

  it('accepts only the newest preview and discards responses after cancel', async () => {
    const obsolete = deferred<BlockSignalsPreview>();
    const current = deferred<BlockSignalsPreview>();
    const cancelled = deferred<BlockSignalsPreview>();
    previewBlockSignalsMock
      .mockReturnValueOnce(obsolete.promise)
      .mockReturnValueOnce(current.promise)
      .mockReturnValueOnce(cancelled.promise);

    startBlockSignalsEditor('Main');
    const first = addSignaledBlock(CEDAR);
    const second = addSignaledBlock(MILL);
    current.resolve(preview('current'));
    await second;
    obsolete.resolve(preview('obsolete'));
    await first;
    expect(store.preview?.facilityName).toBe('current');

    const late = setFacilitySignalSettings({ aspects: 4 });
    cancelBlockSignalsEditor();
    cancelled.resolve(preview('late'));
    await late;
    expect(store.open).toBe(false);
    expect(store.preview).toBeNull();
  });

  it('sends the travel-order list with joins, turnouts, ends, settings, overrides and names', async () => {
    startBlockSignalsEditor('Cedar Hill — Eastward');
    await addSignaledBlock(CEDAR);
    await addSignaledBlock(MILL);
    await setJoinedToPrevious(MILL.blockId, true);
    const turnoutId = await addTurnout(MILL.blockId);
    await setExitEnd({ kind: 'treatAsStop' });
    await setFacilitySignalSettings({ turnoutNotLined: 'restricting' });
    await setSignalOverride('signal:x', { aspects: 2, approachLighting: 'whenApproached' });
    await setSignalName('signal:x', 'Cedar home');

    const request = previewBlockSignalsMock.mock.lastCall![0];
    expect(request).toMatchObject({
      facilityName: 'Cedar Hill — Eastward',
      direction: 'eastward',
      movements: 'oneDirection',
      blocks: [
        { blockId: 'block:cedar', joinedToPrevious: false, turnouts: [] },
        { blockId: 'block:mill', joinedToPrevious: true, turnouts: [{ turnoutId, name: 'Turnout 1' }] },
      ],
      entranceEnd: { kind: 'noPlant' },
      exitEnd: { kind: 'treatAsStop' },
      settings: { aspects: 3, turnoutNotLined: 'restricting', approachLighting: 'alwaysLit' },
      signalOverrides: [{ signalId: 'signal:x', aspects: 2, approachLighting: 'whenApproached' }],
      signalNames: { 'signal:x': 'Cedar home' },
    });
    expect(request.facilityId).toMatch(/^[0-9a-f-]{36}$/i);
  });

  it('keeps the user input when a preview arrives', async () => {
    previewBlockSignalsMock.mockImplementation(async () => ({ ...preview(), blocks: [{ ...CEDAR, name: 'Canonical', joinedToPrevious: false, turnouts: [] }] }));
    startBlockSignalsEditor('Main');
    await addSignaledBlock(CEDAR);

    expect(store.blocks[0].name).toBe('Cedar Hill');
  });

  it('moves a turnout within its block and on into the neighbouring block', async () => {
    startBlockSignalsEditor('Main');
    await addSignaledBlock(CEDAR);
    await addSignaledBlock(MILL);
    const first = await addTurnout(CEDAR.blockId);
    const second = await addTurnout(CEDAR.blockId, first);

    expect(store.canMoveTurnout(first, -1)).toBe(false);
    await moveTurnout(first, 1);
    expect(store.blocks[0].turnouts.map((t) => t.turnoutId)).toEqual([second, first]);
    await moveTurnout(first, 1);
    expect(store.blocks[0].turnouts.map((t) => t.turnoutId)).toEqual([second]);
    expect(store.blocks[1].turnouts.map((t) => t.turnoutId)).toEqual([first]);
    expect(store.canMoveTurnout(first, 1)).toBe(false);
  });

  it('clears the preview when the last block is removed, without a request', async () => {
    startBlockSignalsEditor('Main');
    await addSignaledBlock(CEDAR);
    expect(store.preview).not.toBeNull();
    const calls = previewBlockSignalsMock.mock.calls.length;

    await removeSignaledBlock(CEDAR.blockId);
    expect(store.preview).toBeNull();
    expect(previewBlockSignalsMock.mock.calls.length).toBe(calls);
  });

  it('changes steps and drawing shapes without asking for a new preview', async () => {
    startBlockSignalsEditor('Main');
    await addSignaledBlock(CEDAR);
    const turnoutId = await addTurnout(CEDAR.blockId);
    const calls = previewBlockSignalsMock.mock.calls.length;

    navigateBlockSignalsEditor('aspects');
    setTurnoutShape(turnoutId, { face: 'east', side: 'above' });
    expect(store.step).toBe('aspects');
    expect(store.turnoutShapes[turnoutId]).toEqual({ face: 'east', side: 'above' });
    expect(previewBlockSignalsMock.mock.calls.length).toBe(calls);
  });
});

// ── Spec 020 / E1a B11a — steps and wiring state ─────────────────────────

const HOST: SignalOutputHost = {
  nodeKey: '050101018800',
  nodeName: 'Signal LCC',
  style: { styleId: 'two-driver-rg-signal', bindingKind: 'signalLccRgOutputs', appearances: { stop: ['red'], approach: ['red', 'green'], clear: ['green'] } },
  evidence: 'complete',
  outputs: [
    { ordinal: 1, label: 'Output 1', owner: null },
    { ordinal: 2, label: 'Output 2', owner: 'Other signal' },
    { ordinal: 3, label: 'Output 3', owner: null },
  ],
};

function signalPreview(): BlockSignalsPreview {
  return {
    ...preview(),
    outputHosts: [HOST],
    signals: [{ signalId: 's1', name: 'Signal 1', protectedBlockIds: ['block:cedar'] } as never],
  };
}

describe('blockSignalsEditor steps and wiring (E1a B11a)', () => {
  beforeEach(() => {
    previewBlockSignalsMock.mockReset();
    previewBlockSignalsMock.mockImplementation(async () => signalPreview());
    composeBlockSignalsMock.mockReset();
    composeBlockSignalsMock.mockResolvedValue({ edits: [], claimedSlots: [], issues: [] });
    resetBlockSignalsEditorForNewLayout();
  });

  it('orders steps blocks, aspects, appearance, wiring and keeps review unavailable', () => {
    startBlockSignalsEditor('Main');
    expect(store.steps.map((s) => s.id)).toEqual(['blocks', 'aspects', 'appearance', 'wiring', 'review']);
    store.addBlock({ blockId: 'block:a', name: 'A' });
    navigateBlockSignalsEditor('wiring');
    expect(store.step).toBe('wiring');
    navigateBlockSignalsEditor('review');
    expect(store.step).toBe('wiring');
  });

  it('exposes preview hosts and outputs, and keeps node, red and green choices consistent', async () => {
    startBlockSignalsEditor('Main');
    await addSignaledBlock(CEDAR);
    expect(store.outputHosts).toEqual([HOST]);

    await chooseWiringNode('s1', HOST.nodeKey);
    await chooseRedOutput('s1', 1);
    await chooseGreenOutput('s1', 1);
    expect(store.wiringFor('s1')).toEqual({ nodeKey: HOST.nodeKey, redOutput: 1, greenOutput: null });
    await chooseGreenOutput('s1', 3);
    expect(store.wiringFor('s1')).toEqual({ nodeKey: HOST.nodeKey, redOutput: 1, greenOutput: 3 });
    expect(store.wiringComplete).toBe(true);

    await chooseWiringNode('s1', '050101018899');
    expect(store.wiringFor('s1')).toEqual({ nodeKey: '050101018899', redOutput: null, greenOutput: null });
    expect(store.wiringComplete).toBe(false);
  });

  it('flags out-of-scope settings and clears wiring with the layout', async () => {
    startBlockSignalsEditor('Main');
    await addSignaledBlock(CEDAR);
    await chooseWiringNode('s1', HOST.nodeKey);
    await chooseRedOutput('s1', 1);
    await chooseGreenOutput('s1', 3);
    expect(store.outOfScopeSettings).toEqual([]);

    await setFacilitySignalSettings({ aspects: 4 });
    await setBlockSignalsMovements('bothDirections');
    expect(store.outOfScopeSettings).toEqual(['aspects', 'movements']);
    resetBlockSignalsEditorForNewLayout();
    expect(store.wiringFor('s1')).toBeNull();
  });
});

// ── Spec 020 / E1a B11b — compose check and Done ─────────────────────────

const { facilitiesStore } = await import('$lib/stores/facilities.svelte');
const { channelsStore } = await import('$lib/stores/channels.svelte');
const { behaviorTemplatesStore } = await import('$lib/stores/behaviorTemplates.svelte');
const { configChangesStore } = await import('$lib/stores/configChanges.svelte');
const { configEditor } = await import('$lib/stores/configEditor.svelte');
const { effectiveNodeStore } = await import('$lib/layout/effectiveNodeStore.svelte');
const { editKeyForLeaf } = await import('$lib/utils/editKey');

const OTHER_NODE = '050101018899';

async function wireSignal(): Promise<void> {
  startBlockSignalsEditor('Main line');
  await addSignaledBlock(CEDAR);
  navigateBlockSignalsEditor('wiring');
  await chooseWiringNode('s1', HOST.nodeKey);
  await chooseRedOutput('s1', 1);
  await chooseGreenOutput('s1', 3);
}

describe('blockSignalsEditor compose check and Done (E1a B11b)', () => {
  beforeEach(async () => {
    previewBlockSignalsMock.mockReset();
    previewBlockSignalsMock.mockImplementation(async () => signalPreview());
    composeBlockSignalsMock.mockReset();
    composeBlockSignalsMock.mockResolvedValue({ edits: [], claimedSlots: [], issues: [] });
    resetBlockSignalsEditorForNewLayout();
    configChangesStore.clearAllDrafts();
    facilitiesStore.hydrateBaseline([]);
    channelsStore.hydrateBaseline([]);
    behaviorTemplatesStore.reset();
    await behaviorTemplatesStore.loadBehaviorTemplates();
  });

  it('checks complete wiring with the wiring node\'s unsaved edits, and edits make the check stale', async () => {
    configEditor.applyEdit(editKeyForLeaf(HOST.nodeKey, 253, 99), { type: 'int', value: 7 });
    configEditor.applyEdit(editKeyForLeaf(OTHER_NODE, 253, 98), { type: 'int', value: 8 });
    expect(store.canFinish).toBe(false);

    await wireSignal();

    const request = composeBlockSignalsMock.mock.lastCall![0];
    expect(request.facilityId).toBe(store.facilityId);
    expect(request.wiring).toEqual([{ signalId: 's1', nodeKey: HOST.nodeKey, redOutput: 1, greenOutput: 3 }]);
    expect(request.pendingEdits).toEqual([
      { nodeKey: HOST.nodeKey, space: 253, address: 99, value: { type: 'int', value: 7 } },
    ]);
    expect(store.canFinish).toBe(true);

    const slow = deferred<BlockSignalsPreview>();
    previewBlockSignalsMock.mockReturnValueOnce(slow.promise);
    const pending = setSignalName('s1', 'Renamed');
    expect(store.canFinish).toBe(false);
    slow.resolve(signalPreview());
    await pending;
    expect(store.canFinish).toBe(true);
  });

  it('ignores a stale compose response and shows issues by signal and node', async () => {
    const stale = deferred<ComposeBlockSignalsResponse>();
    composeBlockSignalsMock.mockReturnValueOnce(stale.promise);
    startBlockSignalsEditor('Main line');
    await addSignaledBlock(CEDAR);
    navigateBlockSignalsEditor('wiring');
    await chooseWiringNode('s1', HOST.nodeKey);
    await chooseRedOutput('s1', 1);
    const first = chooseGreenOutput('s1', 3);
    const issue = { code: 'outputInUse' as const, signalId: 's1', nodeKey: HOST.nodeKey, detail: 'busy' };
    composeBlockSignalsMock.mockResolvedValueOnce({ edits: [], claimedSlots: [], issues: [issue] });
    await chooseGreenOutput('s1', 2);
    stale.resolve({ edits: [], claimedSlots: [], issues: [] });
    await first;

    expect(store.issuesForSignal('s1')).toEqual([issue]);
    expect(store.issuesForNode(HOST.nodeKey)).toEqual([issue]);
    expect(store.canFinish).toBe(false);
  });

  it('on Done creates the facility, closes the editor and dirties the Save toolbar', async () => {
    composeBlockSignalsMock.mockResolvedValue({
      edits: [{ nodeKey: HOST.nodeKey, space: 253, address: 10, path: 'a', value: { type: 'int', value: 1 } }],
      claimedSlots: [{ nodeKey: HOST.nodeKey, kind: 'mast', index: 1 }],
      issues: [],
    });
    await wireSignal();
    const facilityId = store.facilityId;
    expect(effectiveNodeStore.isDirty).toBe(false);

    await finishBlockSignalsEditor();

    expect(store.open).toBe(false);
    const facility = facilitiesStore.facilities.find((f) => f.facilityId === facilityId)!;
    expect(facility.name).toBe('Main line');
    expect(facility.blockSignals?.signals).toMatchObject([{ signalId: 's1', name: 'Signal 1', protectedBlock: 'block:cedar' }]);
    expect(effectiveNodeStore.isDirty).toBe(true);
  });

  it('on Done with issues keeps the editor open and shows them', async () => {
    await wireSignal();
    const issue = { code: 'outputInUse' as const, signalId: 's1', nodeKey: HOST.nodeKey, detail: 'taken meanwhile' };
    composeBlockSignalsMock.mockResolvedValueOnce({ edits: [], claimedSlots: [], issues: [issue] });

    await finishBlockSignalsEditor();

    expect(store.open).toBe(true);
    expect(store.issuesForSignal('s1')).toEqual([issue]);
    expect(store.canFinish).toBe(false);
    expect(facilitiesStore.pendingCreations).toHaveLength(0);
  });

  it('on Done with an error keeps the editor open with a message', async () => {
    await wireSignal();
    composeBlockSignalsMock.mockRejectedValueOnce(new Error('ipc down'));

    await finishBlockSignalsEditor();

    expect(store.open).toBe(true);
    expect(store.finishError).toBe('ipc down');
    expect(facilitiesStore.pendingCreations).toHaveLength(0);
  });

  it('never carries an out-of-scope setting into Done', async () => {
    await wireSignal();
    await setFacilitySignalSettings({ aspects: 4 });
    await chooseGreenOutput('s1', 3);
    expect(store.canFinish).toBe(false);

    await finishBlockSignalsEditor();

    expect(store.open).toBe(true);
    expect(facilitiesStore.pendingCreations).toHaveLength(0);
  });
});

describe('blockSignalsEditor edit mode (E2a B5b)', () => {
  const SAVED = {
    facilityId: 'abs-1',
    templateId: 'automatic-block-signals',
    name: 'Saved line',
    slotBindings: { signals: ['ch-sig-1'] },
    blockSignals: {
      direction: 'westward',
      movements: 'oneDirection',
      entranceEnd: { kind: 'noPlant' },
      exitEnd: { kind: 'treatAsStop' },
      blocks: [{ blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] }],
      settings: { aspects: 3, turnoutNotLined: 'restricting', approachLighting: 'alwaysLit' },
      signals: [{ signalId: 's1', name: 'Saved signal', protectedBlock: 'block:cedar', channelId: 'ch-sig-1' }],
      claimedSlots: [{ nodeKey: HOST.nodeKey, kind: 'mast' as const, index: 9 }],
    },
  };

  beforeEach(async () => {
    previewBlockSignalsMock.mockReset();
    previewBlockSignalsMock.mockImplementation(async () => signalPreview());
    composeBlockSignalsMock.mockReset();
    composeBlockSignalsMock.mockResolvedValue({ edits: [], claimedSlots: SAVED.blockSignals.claimedSlots, issues: [] });
    resetBlockSignalsEditorForNewLayout();
    configChangesStore.clearAllDrafts();
    behaviorTemplatesStore.reset();
    await behaviorTemplatesStore.loadBehaviorTemplates();
    channelsStore.hydrateBaseline([{
      id: 'ch-sig-1',
      name: 'Saved signal',
      role: 'signal-indication',
      style: 'two-driver-rg-signal',
      ownership: 'user-owned',
      binding: { kind: 'signalLccRgOutputs', nodeKey: HOST.nodeKey, redOutput: 1, greenOutput: 3 },
    } as never]);
    facilitiesStore.hydrateBaseline([SAVED as never]);
  });

  it('starts in create mode for a new facility', () => {
    startBlockSignalsEditor('Main');
    expect(store.mode).toBe('create');
  });

  it('seeds every input from the saved facility and its channel wiring', async () => {
    await startEditBlockSignalsEditor('abs-1');

    expect(store.open).toBe(true);
    expect(store.mode).toBe('edit');
    expect(store.facilityId).toBe('abs-1');
    expect(store.facilityName).toBe('Saved line');
    expect(store.direction).toBe('westward');
    expect(store.exitEnd).toEqual({ kind: 'treatAsStop' });
    expect(store.blocks.map((b) => b.blockId)).toEqual(['block:cedar']);
    expect(store.settings.turnoutNotLined).toBe('restricting');
    expect(store.signalNames).toEqual({ s1: 'Saved signal' });
    expect(store.wiringFor('s1')).toEqual({ nodeKey: HOST.nodeKey, redOutput: 1, greenOutput: 3 });
    const request = previewBlockSignalsMock.mock.lastCall![0];
    expect(request).toMatchObject({ facilityId: 'abs-1', facilityName: 'Saved line', signalNames: { s1: 'Saved signal' } });
    expect(store.preview).not.toBeNull();
  });

  it('on Done updates the saved facility and its channel instead of creating', async () => {
    await startEditBlockSignalsEditor('abs-1');
    await navigateBlockSignalsEditor('wiring');
    await setFacilityName('Renamed line');
    await chooseGreenOutput('s1', 2);
    expect(store.canFinish).toBe(true);

    await finishBlockSignalsEditor();

    expect(store.open).toBe(false);
    expect(facilitiesStore.pendingCreations).toHaveLength(0);
    expect(facilitiesStore.facilities).toHaveLength(1);
    expect(facilitiesStore.facilities[0].name).toBe('Renamed line');
    expect(channelsStore.channels).toHaveLength(1);
    expect(channelsStore.channels[0].binding).toMatchObject({ redOutput: 1, greenOutput: 2 });
    expect(facilitiesStore.facilities[0].blockSignals?.signals[0].channelId).toBe('ch-sig-1');
    expect(effectiveNodeStore.isDirty).toBe(true);
  });

  it('cancel in edit mode stages nothing', async () => {
    await startEditBlockSignalsEditor('abs-1');
    await setFacilityName('Renamed line');

    cancelBlockSignalsEditor();

    expect(store.open).toBe(false);
    expect(effectiveNodeStore.isDirty).toBe(false);
    expect(facilitiesStore.facilities[0].name).toBe('Saved line');
  });
});
