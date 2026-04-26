import { describe, expect, it, vi } from 'vitest';
import {
  createCancelledCdiDownloadState,
  createClosedCdiRedownloadState,
  createClosedCdiViewerState,
  createOpenCdiRedownloadState,
  createOpeningCdiViewerState,
  createWaitingCdiDownloadNodes,
  loadCdiViewerState,
  resolvePostDownloadReadNodes,
  updateCdiDownloadNodeStatus,
} from './cdiDialogOrchestrator';

describe('cdiDialogOrchestrator', () => {
  it('creates opening and closed viewer states', () => {
    expect(createOpeningCdiViewerState('02.01.57.00.00.01')).toEqual({
      errorMessage: 'Checking cache…',
      nodeId: '02.01.57.00.00.01',
      status: 'loading',
      visible: true,
      xmlContent: null,
    });

    expect(createClosedCdiViewerState()).toEqual({
      errorMessage: null,
      nodeId: null,
      status: 'idle',
      visible: false,
      xmlContent: null,
    });
  });

  it('loads viewer content from cache when available', async () => {
    const getCdiXml = vi.fn(async () => ({ xmlContent: '<cdi />', sizeBytes: 7, retrievedAt: null }));
    const downloadCdi = vi.fn();

    await expect(loadCdiViewerState('02.01.57.00.00.01', getCdiXml, downloadCdi)).resolves.toEqual({
      errorMessage: null,
      status: 'success',
      xmlContent: '<cdi />',
    });
    expect(downloadCdi).not.toHaveBeenCalled();
  });

  it('downloads viewer content when cache misses with CdiNotRetrieved', async () => {
    const getCdiXml = vi.fn(async () => {
      throw 'CdiNotRetrieved: cache miss';
    });
    const downloadCdi = vi.fn(async () => ({ xmlContent: '<downloaded />', sizeBytes: 13, retrievedAt: null }));

    await expect(loadCdiViewerState('02.01.57.00.00.01', getCdiXml, downloadCdi)).resolves.toEqual({
      errorMessage: null,
      status: 'success',
      xmlContent: '<downloaded />',
    });
    expect(downloadCdi).toHaveBeenCalledWith('02.01.57.00.00.01');
  });

  it('surfaces viewer errors and missing XML as error states', async () => {
    await expect(loadCdiViewerState(
      '02.01.57.00.00.01',
      async () => ({ xmlContent: null, sizeBytes: null, retrievedAt: null }),
      vi.fn(),
    )).resolves.toEqual({
      errorMessage: 'No CDI data available for this node.',
      status: 'error',
      xmlContent: null,
    });

    await expect(loadCdiViewerState(
      '02.01.57.00.00.01',
      async () => {
        throw 'RetrievalFailed: timed out';
      },
      vi.fn(),
    )).resolves.toEqual({
      errorMessage: 'CDI retrieval failed. Check node connection and try again.',
      status: 'error',
      xmlContent: null,
    });
  });

  it('builds redownload dialog state with node-name fallback and resets it cleanly', () => {
    expect(createOpenCdiRedownloadState('02.01.57.00.00.01', [
      { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
    ])).toEqual({
      nodeId: '02.01.57.00.00.01',
      nodeName: 'East Panel',
      visible: true,
    });

    expect(createOpenCdiRedownloadState('02.01.57.00.00.02', [])).toEqual({
      nodeId: '02.01.57.00.00.02',
      nodeName: '02.01.57.00.00.02',
      visible: true,
    });

    expect(createClosedCdiRedownloadState()).toEqual({
      nodeId: null,
      nodeName: null,
      visible: false,
    });
  });

  it('tracks CDI download dialog status transitions and post-download read candidates', () => {
    const nodes = createWaitingCdiDownloadNodes([
      { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
      { nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' },
    ]);

    expect(nodes.map((node) => node.downloadStatus)).toEqual(['waiting', 'waiting']);
    expect(updateCdiDownloadNodeStatus(nodes, 1, 'downloading')[1].downloadStatus).toBe('downloading');
    expect(updateCdiDownloadNodeStatus(nodes, 0, 'done')[0].downloadStatus).toBe('done');

    expect(resolvePostDownloadReadNodes({
      nodesToDownload: nodes,
      nodesWithCdi: new Set(['02.01.57.00.00.02']),
      pendingConfigNodes: [
        { nodeId: '02.01.57.00.00.01', nodeName: 'East Panel' },
        { nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' },
        { nodeId: '02.01.57.00.00.03', nodeName: 'South Panel' },
      ],
    })).toEqual([
      { nodeId: '02.01.57.00.00.02', nodeName: 'West Panel' },
    ]);

    expect(createCancelledCdiDownloadState()).toEqual({
      cdiDownloadDialogVisible: false,
      cdiMissingNodes: [],
      pendingConfigNodes: [],
    });
  });
});