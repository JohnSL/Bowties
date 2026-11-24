import { describe, it, expect, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

const { composeBlockSignals, composeBlockSignalsRemoval, previewBlockSignals } = await import('$lib/api/signaling');
import type {
  BlockSignalsPreview,
  BlockSignalsPreviewRequest,
  ComposeBlockSignalsRequest,
} from '$lib/api/signaling';

describe('composeBlockSignalsRemoval', () => {
  it('asks the compose command to release the facility mast and Logic', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({ edits: [], signalResources: [], logicPlacement: [], issues: [] });
    await composeBlockSignalsRemoval('abs:main');
    expect(invokeMock).toHaveBeenCalledWith('compose_block_signals', {
      request: { facilityId: 'abs:main', remove: true },
    });
  });
});

describe('released owners', () => {
  const line: BlockSignalsPreviewRequest = {
    facilityId: 'abs:main',
    facilityName: 'Main',
    direction: 'eastward',
    movements: 'oneDirection',
    blocks: [],
    entranceEnd: { kind: 'noPlant' },
    exitEnd: { kind: 'nothingBeyond' },
    settings: { aspects: 3, turnoutNotLined: 'stop', approachLighting: 'alwaysLit' },
    signalOverrides: [],
    signalNames: {},
    release: [{ nodeKey: '050101010700', owner: 'Mast 3' }],
  };

  it('sends released owners with the compose request and accepts fresh event IDs in its edits', async () => {
    invokeMock.mockReset();
    const edit = { nodeKey: '050101010700', space: 253, address: 1, path: 'p', value: { type: 'freshEventId' } };
    invokeMock.mockResolvedValue({ edits: [edit], signalResources: [], logicPlacement: [], issues: [] });
    const request: ComposeBlockSignalsRequest = {
      line,
      wiring: [],
      release: [{ nodeKey: '050101010700', owner: 'Direct lamp 2' }],
    };
    const response = await composeBlockSignals(request);
    expect(invokeMock).toHaveBeenCalledWith('compose_block_signals', { request });
    expect(response.edits[0].value).toEqual({ type: 'freshEventId' });
  });

  it('reports the matched and kept Logic of each outside mast in the preview', async () => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({
      outsideOwners: [{ nodeKey: '050101010700', owner: 'Mast 3', matchedLogic: [2, 5], keptLogic: [5] }],
    });
    const preview: BlockSignalsPreview = await previewBlockSignals(line);
    expect(preview.outsideOwners?.[0].keptLogic).toEqual([5]);
    expect(invokeMock).toHaveBeenCalledWith('preview_block_signals', { request: line });
  });
});
