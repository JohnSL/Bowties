import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { clearConfigReadStatus, markNodeConfigRead } from '$lib/stores/configReadStatus';
import { configSidebarStore } from '$lib/stores/configSidebar';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import { resetLayoutOpenPhase } from '$lib/stores/layoutOpenLifecycle';

const {
  eventHandlers,
  invokeRef,
  probeNodesRef,
  registerNodeRef,
  querySnipRef,
  queryPipRef,
  getCdiXmlRef,
  readAllConfigValuesRef,
  startListeningRef,
} = vi.hoisted(() => ({
  eventHandlers: new Map<string, (event: any) => unknown>(),
  invokeRef: vi.fn(),
  probeNodesRef: vi.fn(async () => {}),
  registerNodeRef: vi.fn(async () => {}),
  querySnipRef: vi.fn(async () => ({
    status: 'Complete',
    snip_data: {
      user_name: 'East Panel',
      user_description: '',
      manufacturer: 'ACME',
      model: 'Node-8',
      hardware_version: '1.0',
      software_version: '1.0',
    },
  })),
  queryPipRef: vi.fn(async () => ({
    status: 'Complete',
    pip_flags: {
      cdi: true,
      memory_configuration: true,
    },
  })),
  getCdiXmlRef: vi.fn(async () => ({ xmlContent: '<cdi />' })),
  readAllConfigValuesRef: vi.fn(async () => ({ failedReads: 0, totalElements: 0 })),
  startListeningRef: vi.fn(async () => {}),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeRef,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (eventName: string, handler: (event: any) => unknown) => {
    eventHandlers.set(eventName, handler);
    return () => eventHandlers.delete(eventName);
  }),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
  save: vi.fn(async () => null),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: class {},
  getCurrentWebviewWindow: () => ({
    onCloseRequested: vi.fn(async () => () => {}),
    setTitle: vi.fn(async () => {}),
    close: vi.fn(),
  }),
}));

vi.mock('$lib/api/tauri', () => ({
  probeNodes: probeNodesRef,
  registerNode: registerNodeRef,
  querySnip: querySnipRef,
  queryPip: queryPipRef,
  refreshAllNodes: vi.fn(async () => []),
}));

vi.mock('$lib/api/bowties', () => ({
  getRecentLayout: vi.fn(async () => null),
  clearRecentLayout: vi.fn(async () => {}),
}));

vi.mock('$lib/api/layout', () => ({
  closeLayout: vi.fn(async () => {}),
  saveLayoutFile: vi.fn(async () => ({ warnings: [] })),
  openLayoutFile: vi.fn(async () => ({
    partialNodes: [],
    nodeSnapshots: [],
  })),
  buildOfflineNodeTree: vi.fn(async () => {
    throw new Error('not needed in route discovery test');
  }),
}));

vi.mock('$lib/api/cdi', () => ({
  readAllConfigValues: readAllConfigValuesRef,
  cancelConfigReading: vi.fn(async () => {}),
  getCdiXml: getCdiXmlRef,
  downloadCdi: vi.fn(async () => ({ success: true })),
}));

vi.mock('../lib/keyboard/menuShortcuts', () => ({
  installMenuShortcuts: vi.fn(() => () => {}),
}));

vi.mock('$lib/stores/bowties.svelte', async () => {
  const actual = await vi.importActual<object>('$lib/stores/bowties.svelte');
  return {
    ...actual,
    bowtieCatalogStore: {
      startListening: startListeningRef,
    },
  };
});

vi.mock('$lib/components/SegmentView.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/CdiXmlViewer.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/Bowtie/BowtieCatalogPanel.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/DiscoveryProgressModal.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/ElementCardDeck/SaveControls.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/CdiDownloadDialog.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/CdiRedownloadDialog.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/ErrorDialog.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/Layout/MissingCaptureBadge.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/ConnectionManager.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/Sync/SyncPanel.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/Layout/OfflineBanner.svelte', async () => await import('$lib/test/StubComponent.svelte'));

import Page from './+page.svelte';

beforeEach(() => {
  vi.clearAllMocks();
  eventHandlers.clear();

  invokeRef.mockImplementation(async (command: string) => {
    if (command === 'get_connection_status') {
      return {
        connected: true,
        config: { name: 'Bench Bus' },
      };
    }
    return null;
  });

  clearConfigReadStatus();
  configSidebarStore.reset();
  nodeInfoStore.set(new Map());
  nodeTreeStore.reset();
  layoutStore.reset();
  bowtieMetadataStore.clearAll();
  offlineChangesStore.clear();
  resetLayoutOpenPhase();
  getCdiXmlRef.mockReset();
  getCdiXmlRef.mockImplementation(async () => ({ xmlContent: '<cdi />' }));
  readAllConfigValuesRef.mockReset();
  readAllConfigValuesRef.mockImplementation(async () => ({ failedReads: 0, totalElements: 0 }));
});

describe('+page route discovery CTA', () => {
  it('shows the fresh-session config CTA and friendly name after live discovery', async () => {
    render(Page);

    await waitFor(() => {
      expect(probeNodesRef).toHaveBeenCalledTimes(1);
    });

    const discovered = eventHandlers.get('lcc-node-discovered');
    expect(discovered).toBeTypeOf('function');

    await discovered?.({
      payload: {
        nodeId: '02.01.57.00.00.01',
        alias: 0x123,
        timestamp: '2026-04-25T12:00:00.000Z',
      },
    });

    await waitFor(() => {
      expect(screen.getByText('East Panel')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /read node configuration/i })).toBeInTheDocument();
      expect(screen.getByText(/1 unread/i)).toBeInTheDocument();
    });

    expect(screen.queryByText('02.01.57.00.00.01')).not.toBeInTheDocument();
  });

  it('clears stale config-read status before a fresh live discovery session', async () => {
    markNodeConfigRead('02.01.57.00.00.01');

    render(Page);

    await waitFor(() => {
      expect(probeNodesRef).toHaveBeenCalledTimes(1);
    });

    const discovered = eventHandlers.get('lcc-node-discovered');
    expect(discovered).toBeTypeOf('function');

    await discovered?.({
      payload: {
        nodeId: '02.01.57.00.00.01',
        alias: 0x123,
        timestamp: '2026-04-25T12:00:00.000Z',
      },
    });

    await waitFor(() => {
      expect(screen.getByText('East Panel')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /read node configuration/i })).toBeInTheDocument();
      expect(screen.getByText(/1 unread/i)).toBeInTheDocument();
    });
  });

  it('clears stale sidebar selection before a fresh live discovery session', async () => {
    configSidebarStore.setSelectedNode('AA.BB.CC.DD.EE.FF');

    render(Page);

    await waitFor(() => {
      expect(probeNodesRef).toHaveBeenCalledTimes(1);
    });

    const discovered = eventHandlers.get('lcc-node-discovered');
    expect(discovered).toBeTypeOf('function');

    await discovered?.({
      payload: {
        nodeId: '02.01.57.00.00.01',
        alias: 0x123,
        timestamp: '2026-04-25T12:00:00.000Z',
      },
    });

    await waitFor(() => {
      expect(screen.getByText('East Panel')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: /read node configuration/i })).toBeInTheDocument();
      expect(screen.getByText(/1 unread/i)).toBeInTheDocument();
    });
  });

  it('shows a CDI error banner instead of the download prompt when preflight fails for another reason', async () => {
    getCdiXmlRef.mockRejectedValueOnce('RetrievalFailed: timed out');

    render(Page);

    await waitFor(() => {
      expect(probeNodesRef).toHaveBeenCalledTimes(1);
    });

    const discovered = eventHandlers.get('lcc-node-discovered');
    expect(discovered).toBeTypeOf('function');

    await discovered?.({
      payload: {
        nodeId: '02.01.57.00.00.01',
        alias: 0x123,
        timestamp: '2026-04-25T12:00:00.000Z',
      },
    });

    const readButton = await screen.findByRole('button', { name: /read node configuration/i });
    await fireEvent.click(readButton);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(
        'Cannot read configuration for East Panel: CDI retrieval failed. Check node connection and try again.',
      );
    });

    expect(readAllConfigValuesRef).not.toHaveBeenCalled();
  });
});