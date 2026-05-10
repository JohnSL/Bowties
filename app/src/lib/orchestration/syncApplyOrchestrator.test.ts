import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import { reconcileOfflineTreesAfterSyncApply } from './syncApplyOrchestrator';

const {
  buildOfflineNodeTreeRef,
  markNodeConfigReadRef,
  nodeTreeStoreRef,
  offlineChangesStoreRef,
} = vi.hoisted(() => ({
  buildOfflineNodeTreeRef: vi.fn(),
  markNodeConfigReadRef: vi.fn(),
  nodeTreeStoreRef: {
    setTree: vi.fn(),
  },
  offlineChangesStoreRef: {
    persistedRows: [] as Array<{ changeId: string; nodeId?: string; plannedValue: string; baselineValue: string; status: string; kind: string; space?: number; offset?: string }>,
    get effectiveRows() { return this.persistedRows; },
    reloadFromBackend: vi.fn(async () => {}),
  },
}));

vi.mock('$lib/api/layout', () => ({
  buildOfflineNodeTree: buildOfflineNodeTreeRef,
}));

vi.mock('$lib/stores/configReadStatus', () => ({
  markNodeConfigRead: markNodeConfigReadRef,
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: nodeTreeStoreRef,
}));

vi.mock('$lib/stores/offlineChanges.svelte', () => ({
  offlineChangesStore: offlineChangesStoreRef,
}));

function makeTree(nodeId: string): NodeConfigTree {
  return {
    nodeId,
    identity: null,
    segments: [],
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  offlineChangesStoreRef.persistedRows = [
    {
      changeId: 'row-2',
      kind: 'config',
      nodeId: '05.02.01.02.09.00',
      space: 253,
      offset: '0x00000022',
      baselineValue: '7',
      plannedValue: '9',
      status: 'pending',
    },
  ];
});

describe('reconcileOfflineTreesAfterSyncApply', () => {
  it('rebuilds only affected nodes after partial apply (no restamping needed)', async () => {
    buildOfflineNodeTreeRef.mockImplementation(async (nodeId: string) => makeTree(nodeId.match(/.{1,2}/g)?.join('.') ?? nodeId));

    await reconcileOfflineTreesAfterSyncApply(
      {
        applied: ['row-1'],
        readOnlyCleared: [],
        skipped: [],
        failed: [],
      },
      {
        conflictRows: [],
        cleanRows: [
          {
            changeId: 'row-1',
            nodeId: '05.02.01.02.03.00',
            baselineValue: '10',
            plannedValue: '20',
            resolution: 'apply',
          },
          {
            changeId: 'row-2',
            nodeId: '05.02.01.02.09.00',
            baselineValue: '7',
            plannedValue: '9',
            resolution: 'skip',
          },
        ],
        alreadyAppliedCount: 0,
        nodeMissingRows: [],
      },
    );

    expect(offlineChangesStoreRef.reloadFromBackend).toHaveBeenCalledTimes(1);
    expect(buildOfflineNodeTreeRef).toHaveBeenCalledTimes(1);
    expect(buildOfflineNodeTreeRef).toHaveBeenCalledWith('050201020300');
    expect(nodeTreeStoreRef.setTree).toHaveBeenCalledWith('05.02.01.02.03.00', makeTree('05.02.01.02.03.00'));
    expect(markNodeConfigReadRef).toHaveBeenCalledWith('05.02.01.02.03.00');
  });

  it('continues even if rebuilding one affected node fails', async () => {
    buildOfflineNodeTreeRef.mockRejectedValueOnce(new Error('tree rebuild failed'));
    const warnRef = vi.spyOn(console, 'warn').mockImplementation(() => {});

    await reconcileOfflineTreesAfterSyncApply(
      {
        applied: [],
        readOnlyCleared: ['row-1'],
        skipped: [],
        failed: [],
      },
      {
        conflictRows: [],
        cleanRows: [],
        alreadyAppliedCount: 0,
        nodeMissingRows: [
          {
            changeId: 'row-1',
            nodeId: '05.02.01.02.03.00',
            baselineValue: '10',
            plannedValue: '20',
            resolution: 'skip',
          },
        ],
      },
    );

    expect(nodeTreeStoreRef.setTree).not.toHaveBeenCalled();
    expect(markNodeConfigReadRef).not.toHaveBeenCalled();
    expect(warnRef).toHaveBeenCalled();

    warnRef.mockRestore();
  });
});