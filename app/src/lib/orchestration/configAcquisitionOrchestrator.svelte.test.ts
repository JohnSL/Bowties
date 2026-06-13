import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DiscoveredNode, ProtocolFlags, SNIPData } from '$lib/api/tauri';
import type { NodeReadState, ReadProgressState } from '$lib/api/types';
import { cdiCacheStore } from '$lib/stores/cdiCache.svelte';
import {
  ConfigAcquisitionOrchestrator,
  computeConfigReadProgressUpdate,
  createWaitingCdiDownloadNodes,
  resolvePostDownloadReadNodes,
  updateCdiDownloadNodeStatus,
  type ConfigAcquisitionDeps,
} from './configAcquisitionOrchestrator.svelte';

function makeSnipData(overrides: Partial<SNIPData> = {}): SNIPData {
  return {
    manufacturer: 'RR-CirKits',
    model: 'Tower-LCC',
    hardware_version: '1',
    software_version: '1.0',
    user_name: 'East Panel',
    user_description: 'Panel note',
    ...overrides,
  };
}

function makePipFlags(overrides: Partial<ProtocolFlags> = {}): ProtocolFlags {
  return {
    simple_protocol: true,
    datagram: true,
    stream: false,
    memory_configuration: true,
    reservation: false,
    event_exchange: true,
    identification: true,
    teach_learn: false,
    remote_button: false,
    acdi: true,
    display: false,
    snip: true,
    cdi: true,
    traction_control: false,
    function_description_information: false,
    dcc_command_station: false,
    simple_train_node: false,
    function_configuration: false,
    firmware_upgrade: false,
    firmware_upgrade_active: false,
    ...overrides,
  };
}

function makeNode(overrides: Partial<DiscoveredNode> = {}): DiscoveredNode {
  return {
    node_id: [0x02, 0x01, 0x57, 0x00, 0x00, 0x01],
    alias: 0x123,
    snip_data: makeSnipData(),
    snip_status: 'Complete',
    connection_status: 'Connected',
    last_verified: null,
    last_seen: '2026-04-25T00:00:00.000Z',
    cdi: null,
    pip_flags: makePipFlags(),
    pip_status: 'Complete',
    ...overrides,
  };
}

const NODE_ID = '02.01.57.00.00.01';

function makeDeps(overrides: Partial<ConfigAcquisitionDeps> = {}): ConfigAcquisitionDeps {
  return {
    getNodes: () => [makeNode()],
    getReadNodeIds: () => new Set(),
    getCdiXml: vi.fn(async () => ({ xmlContent: '<cdi/>', sizeBytes: 5, retrievedAt: null })),
    downloadCdi: vi.fn(async () => ({ xmlContent: '<cdi/>', sizeBytes: 5, retrievedAt: null })),
    readAllConfigValues: vi.fn(async (nodeId: string) => ({
      nodeId,
      values: {},
      totalElements: 1,
      successfulReads: 1,
      failedReads: 0,
      durationMs: 1,
    })),
    cancelConfigReading: vi.fn(async () => {}),
    markNodeConfigRead: vi.fn(),
    refreshTree: vi.fn(async () => {}),
    loadTree: vi.fn(async () => {}),
    recomputeConnectorCompatibility: vi.fn(),
    setErrorMessage: vi.fn(),
    warn: vi.fn(),
    ...overrides,
  };
}

function makeReadProgress(status: ReadProgressState['status']): ReadProgressState {
  return {
    currentNodeId: NODE_ID,
    currentNodeIndex: 1,
    currentNodeName: 'West Panel',
    elementsFailed: 0,
    elementsRead: 5,
    percentage: 42,
    status,
    totalElements: 12,
    totalNodes: 2,
  };
}

function makeNodeReadStates(): NodeReadState[] {
  return [
    { nodeId: '02.01.57.00.00.01', name: 'East Panel', percentage: 0, status: 'waiting' },
    { nodeId: '02.01.57.00.00.02', name: 'West Panel', percentage: 0, status: 'waiting' },
  ];
}

describe('computeConfigReadProgressUpdate', () => {
  it('advances per-node states for an active read', () => {
    expect(computeConfigReadProgressUpdate(
      makeNodeReadStates(),
      makeReadProgress({ type: 'ReadingNode', node_name: 'West Panel' }),
    )).toEqual({
      type: 'reading',
      nodeReadStates: [
        { nodeId: '02.01.57.00.00.01', name: 'East Panel', percentage: 100, status: 'complete' },
        { nodeId: '02.01.57.00.00.02', name: 'West Panel', percentage: 42, status: 'reading' },
      ],
      readProgress: makeReadProgress({ type: 'ReadingNode', node_name: 'West Panel' }),
    });
  });

  it('switches to building-catalog phase', () => {
    const progress = makeReadProgress({ type: 'BuildingCatalog' });
    expect(computeConfigReadProgressUpdate(makeNodeReadStates(), progress)).toEqual({
      type: 'building-catalog',
      readProgress: progress,
    });
  });

  it('closes the progress UI on terminal statuses', () => {
    expect(computeConfigReadProgressUpdate(makeNodeReadStates(), makeReadProgress({ type: 'Cancelled' })))
      .toEqual({ type: 'close' });
    expect(computeConfigReadProgressUpdate(
      makeNodeReadStates(),
      makeReadProgress({ type: 'Complete', fail_count: 0, success_count: 2 }),
    )).toEqual({ type: 'close' });
  });
});

describe('CDI download helpers', () => {
  it('marks queued nodes waiting and updates a single node by index', () => {
    const nodes = createWaitingCdiDownloadNodes([
      { nodeId: 'a', nodeName: 'A' },
      { nodeId: 'b', nodeName: 'B' },
    ]);
    expect(nodes.map((n) => n.downloadStatus)).toEqual(['waiting', 'waiting']);
    expect(updateCdiDownloadNodeStatus(nodes, 1, 'downloading')[1].downloadStatus).toBe('downloading');
    expect(updateCdiDownloadNodeStatus(nodes, 0, 'done')[0].downloadStatus).toBe('done');
  });

  it('resolves post-download read nodes to those now cached', () => {
    expect(resolvePostDownloadReadNodes({
      nodesToDownload: [{ nodeId: 'a', nodeName: 'A' }],
      nodesWithCdi: new Set(['b']),
      pendingConfigNodes: [
        { nodeId: 'a', nodeName: 'A' },
        { nodeId: 'b', nodeName: 'B' },
      ],
    })).toEqual([{ nodeId: 'b', nodeName: 'B' }]);
  });
});

describe('ConfigAcquisitionOrchestrator', () => {
  beforeEach(() => {
    cdiCacheStore.reset();
  });

  it('does nothing when there are no unread nodes', async () => {
    const deps = makeDeps({ getNodes: () => [] });
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.readRemaining();
    expect(orch.discoveryModalVisible).toBe(false);
    expect(deps.readAllConfigValues).not.toHaveBeenCalled();
  });

  it('reads all nodes when CDI is cached, then closes the session', async () => {
    const deps = makeDeps();
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.readRemaining();
    expect(deps.readAllConfigValues).toHaveBeenCalledWith(NODE_ID, 0, 1);
    expect(deps.markNodeConfigRead).toHaveBeenCalledWith(NODE_ID);
    expect(deps.refreshTree).toHaveBeenCalledWith(NODE_ID);
    expect(orch.discoveryModalVisible).toBe(false);
    expect(orch.readingRemaining).toBe(false);
    expect(orch.nodeReadStates).toEqual([]);
    expect(cdiCacheStore.has(NODE_ID)).toBe(true);
  });

  it('diverts to the download dialog when CDI is missing', async () => {
    const deps = makeDeps({
      getCdiXml: vi.fn(async () => ({ xmlContent: null, sizeBytes: null, retrievedAt: null })),
    });
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.readRemaining();
    expect(orch.cdiDownloadDialogVisible).toBe(true);
    expect(orch.cdiMissingNodes).toHaveLength(1);
    expect(orch.cdiMissingNodes[0].nodeId).toBe(NODE_ID);
    expect(orch.pendingConfigNodes).toHaveLength(1);
    expect(orch.discoveryModalVisible).toBe(false);
    expect(orch.readingRemaining).toBe(false);
    expect(deps.readAllConfigValues).not.toHaveBeenCalled();
  });

  it('fails the session and reports an error when preflight errors', async () => {
    const deps = makeDeps({
      getCdiXml: vi.fn(async () => { throw 'RetrievalFailed: timed out'; }),
    });
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.readRemaining();
    expect(deps.setErrorMessage).toHaveBeenCalledWith(expect.stringContaining('Cannot read configuration'));
    expect(orch.discoveryModalVisible).toBe(false);
    expect(deps.readAllConfigValues).not.toHaveBeenCalled();
  });

  it('reads a single node by id', async () => {
    const deps = makeDeps();
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.readSingleNode(NODE_ID);
    expect(deps.readAllConfigValues).toHaveBeenCalledWith(NODE_ID, 0, 1);
    expect(deps.markNodeConfigRead).toHaveBeenCalledWith(NODE_ID);
    expect(orch.discoveryModalVisible).toBe(false);
  });

  it('downloads missing CDI then reads the now-cached nodes', async () => {
    const downloaded = new Set<string>();
    const deps = makeDeps({
      getCdiXml: vi.fn(async (id: string) => (downloaded.has(id)
        ? { xmlContent: '<cdi/>', sizeBytes: 5, retrievedAt: null }
        : { xmlContent: null, sizeBytes: null, retrievedAt: null })),
      downloadCdi: vi.fn(async (id: string) => {
        downloaded.add(id);
        return { xmlContent: '<cdi/>', sizeBytes: 5, retrievedAt: null };
      }),
    });
    const orch = new ConfigAcquisitionOrchestrator(deps);

    await orch.readRemaining();
    expect(orch.cdiDownloadDialogVisible).toBe(true);

    await orch.downloadMissingCdi();
    expect(deps.downloadCdi).toHaveBeenCalledWith(NODE_ID);
    expect(deps.loadTree).toHaveBeenCalledWith(NODE_ID);
    expect(deps.markNodeConfigRead).toHaveBeenCalledWith(NODE_ID);
    expect(orch.cdiDownloadDialogVisible).toBe(false);
    expect(orch.cdiDownloading).toBe(false);
    expect(orch.cdiMissingNodes).toEqual([]);
    expect(orch.pendingConfigNodes).toEqual([]);
  });

  it('cancels the download dialog without downloading', async () => {
    const deps = makeDeps({
      getCdiXml: vi.fn(async () => ({ xmlContent: null, sizeBytes: null, retrievedAt: null })),
    });
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.readRemaining();
    expect(orch.cdiDownloadDialogVisible).toBe(true);

    orch.cancelDownload();
    expect(orch.cdiDownloadDialogVisible).toBe(false);
    expect(orch.cdiMissingNodes).toEqual([]);
    expect(orch.pendingConfigNodes).toEqual([]);
    expect(deps.downloadCdi).not.toHaveBeenCalled();
  });

  it('requests cancellation and ignores re-entrant cancel calls', async () => {
    const deps = makeDeps();
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.cancel();
    expect(deps.cancelConfigReading).toHaveBeenCalledTimes(1);
    expect(orch.isCancelling).toBe(true);
    await orch.cancel();
    expect(deps.cancelConfigReading).toHaveBeenCalledTimes(1);
  });

  it('surfaces an error when cancellation fails', async () => {
    const deps = makeDeps({ cancelConfigReading: vi.fn(async () => { throw 'boom'; }) });
    const orch = new ConfigAcquisitionOrchestrator(deps);
    await orch.cancel();
    expect(deps.setErrorMessage).toHaveBeenCalledWith('Cancel failed: boom');
    expect(orch.isCancelling).toBe(false);
  });

  it('applies progress events to the modal state', () => {
    const orch = new ConfigAcquisitionOrchestrator(makeDeps());

    orch.applyProgressEvent(makeReadProgress({ type: 'BuildingCatalog' }));
    expect(orch.discoveryPhase).toBe('building-catalog');
    expect(orch.readProgress).not.toBeNull();

    orch.applyProgressEvent(makeReadProgress({ type: 'ReadingNode', node_name: 'West Panel' }));
    expect(orch.discoveryPhase).toBe('reading');

    orch.applyProgressEvent(makeReadProgress({ type: 'Cancelled' }));
    expect(orch.discoveryModalVisible).toBe(false);
    expect(orch.readProgress).toBeNull();
  });
});
