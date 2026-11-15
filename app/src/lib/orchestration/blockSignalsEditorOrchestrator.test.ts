import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  BlockSignalsPreview,
  BlockSignalsPreviewRequest,
} from '$lib/api/signaling';

const previewBlockSignalsMock = vi.fn<(request: BlockSignalsPreviewRequest) => Promise<BlockSignalsPreview>>();

vi.mock('$lib/api/signaling', () => ({
  previewBlockSignals: previewBlockSignalsMock,
}));

const { blockSignalsEditorStore: store } = await import('$lib/stores/blockSignalsEditor.svelte');
const {
  addSignaledBlock,
  addTurnout,
  cancelBlockSignalsEditor,
  moveTurnout,
  navigateBlockSignalsEditor,
  removeSignaledBlock,
  resetBlockSignalsEditorForNewLayout,
  setExitEnd,
  setFacilitySignalSettings,
  setJoinedToPrevious,
  setSignalName,
  setSignalOverride,
  setTurnoutShape,
  startBlockSignalsEditor,
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
