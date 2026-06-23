import { describe, expect, it, vi } from 'vitest';
import {
  CdiInspectionOrchestrator,
  loadCdiViewerState,
  type CdiInspectionDeps,
} from './cdiInspectionOrchestrator.svelte';

function makeDeps(overrides: Partial<CdiInspectionDeps> = {}): CdiInspectionDeps {
  return {
    getCdiXml: vi.fn(async () => ({ xmlContent: '<cdi/>', sizeBytes: 5, retrievedAt: null })),
    downloadCdi: vi.fn(async () => ({ xmlContent: '<downloaded/>', sizeBytes: 13, retrievedAt: null })),
    resolveNodeName: (nodeId: string) => {
      if (nodeId === '02.01.57.00.00.01') return 'East Panel';
      return nodeId;
    },
    ...overrides,
  };
}

describe('loadCdiViewerState', () => {
  it('loads from cache when available', async () => {
    const getCdiXml = vi.fn(async () => ({ xmlContent: '<cdi />', sizeBytes: 7, retrievedAt: null }));
    const downloadCdi = vi.fn();
    await expect(loadCdiViewerState('n', getCdiXml, downloadCdi)).resolves.toEqual({
      errorMessage: null,
      status: 'success',
      xmlContent: '<cdi />',
    });
    expect(downloadCdi).not.toHaveBeenCalled();
  });

  it('downloads on a CdiNotRetrieved cache miss', async () => {
    const getCdiXml = vi.fn(async () => { throw 'CdiNotRetrieved: cache miss'; });
    const downloadCdi = vi.fn(async () => ({ xmlContent: '<downloaded />', sizeBytes: 13, retrievedAt: null }));
    await expect(loadCdiViewerState('n', getCdiXml, downloadCdi)).resolves.toEqual({
      errorMessage: null,
      status: 'success',
      xmlContent: '<downloaded />',
    });
    expect(downloadCdi).toHaveBeenCalledWith('n');
  });

  it('surfaces missing XML and errors as error states', async () => {
    await expect(loadCdiViewerState(
      'n',
      async () => ({ xmlContent: null, sizeBytes: null, retrievedAt: null }),
      vi.fn(),
    )).resolves.toEqual({
      errorMessage: 'No CDI data available for this node.',
      status: 'error',
      xmlContent: null,
    });

    await expect(loadCdiViewerState(
      'n',
      async () => { throw 'RetrievalFailed: timed out'; },
      vi.fn(),
    )).resolves.toEqual({
      errorMessage: 'CDI retrieval failed. Check node connection and try again.',
      status: 'error',
      xmlContent: null,
    });
  });
});

describe('CdiInspectionOrchestrator', () => {
  it('opens the viewer, shows a loading state, then loads content', async () => {
    const orch = new CdiInspectionOrchestrator(makeDeps());
    const pending = orch.openViewer('02.01.57.00.00.01');
    expect(orch.viewerVisible).toBe(true);
    expect(orch.viewerNodeId).toBe('02.01.57.00.00.01');
    expect(orch.viewerStatus).toBe('loading');

    await pending;
    expect(orch.viewerStatus).toBe('success');
    expect(orch.viewerXmlContent).toBe('<cdi/>');
    expect(orch.viewerErrorMessage).toBeNull();
  });

  it('closes the viewer and resets its state', async () => {
    const orch = new CdiInspectionOrchestrator(makeDeps());
    await orch.openViewer('02.01.57.00.00.01');
    orch.closeViewer();
    expect(orch.viewerVisible).toBe(false);
    expect(orch.viewerNodeId).toBeNull();
    expect(orch.viewerXmlContent).toBeNull();
    expect(orch.viewerStatus).toBe('idle');
    expect(orch.viewerErrorMessage).toBeNull();
  });

  it('opens the re-download dialog with a resolved name, falling back to the id', () => {
    const orch = new CdiInspectionOrchestrator(makeDeps());
    orch.openRedownload('02.01.57.00.00.01');
    expect(orch.redownloadVisible).toBe(true);
    expect(orch.redownloadNodeId).toBe('02.01.57.00.00.01');
    expect(orch.redownloadNodeName).toBe('East Panel');

    orch.openRedownload('02.01.57.00.00.09');
    expect(orch.redownloadNodeName).toBe('02.01.57.00.00.09');
  });

  it('resolves name for canonical (no-dot) nodeId format', () => {
    const orch = new CdiInspectionOrchestrator(makeDeps({
      resolveNodeName: (nodeId: string) => {
        if (nodeId === '020157000001' || nodeId === '02.01.57.00.00.01') return 'East Panel';
        return nodeId;
      },
    }));
    // Canonical format (as stored in configSidebarStore)
    orch.openRedownload('020157000001');
    expect(orch.redownloadNodeName).toBe('East Panel');
  });

  it('closes the re-download dialog and resets its state', () => {
    const orch = new CdiInspectionOrchestrator(makeDeps());
    orch.openRedownload('02.01.57.00.00.01');
    orch.closeRedownload();
    expect(orch.redownloadVisible).toBe(false);
    expect(orch.redownloadNodeId).toBeNull();
    expect(orch.redownloadNodeName).toBeNull();
  });
});
