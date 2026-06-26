import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { get } from 'svelte/store';
import type { CloseLayoutResult, OpenLayoutResult } from '$lib/api/layout';
import { clearConfigReadStatus, configReadNodesStore, markNodeConfigRead } from '$lib/stores/configReadStatus';
import { configSidebarStore } from '$lib/stores/configSidebar';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import { channelsStore } from '$lib/stores/channels.svelte';
import { eventStateStore } from '$lib/stores/eventState.svelte';
import { resetLayoutOpenPhase } from '$lib/stores/layoutOpenLifecycle';

const {
  eventHandlers,
  getRecentLayoutRef,
  invokeRef,
  dialogOpenRef,
  openLayoutFileRef,
  closeLayoutRef,
  probeNodesRef,
  refreshAllNodesRef,
  registerNodeRef,
  querySnipRef,
  queryPipRef,
  getCdiXmlRef,
  readAllConfigValuesRef,
  startListeningRef,
  listChannelsRef,
  closeRequestedHandler,
  appWindowMock,
} = vi.hoisted(() => ({
  closeRequestedHandler: { current: null as ((event: any) => unknown) | null },
  appWindowMock: {
    onCloseRequested: vi.fn(async (handler: (event: any) => unknown) => {
      closeRequestedHandler.current = handler;
      return () => { closeRequestedHandler.current = null; };
    }),
    setTitle: vi.fn(async () => {}),
    close: vi.fn(),
  },
  eventHandlers: new Map<string, (event: any) => unknown>(),
  getRecentLayoutRef: vi.fn<() => Promise<{ path?: string | null } | null>>(async () => null),
  invokeRef: vi.fn(),
  dialogOpenRef: vi.fn<() => Promise<string | null>>(async () => null),
  openLayoutFileRef: vi.fn<(path: string) => Promise<OpenLayoutResult>>(async () => ({
    layoutId: 'restored-layout',
    capturedAt: '2026-04-25T00:00:00.000Z',
    offlineMode: true,
    nodeCount: 0,
    pendingOfflineChangeCount: 0,
    partialNodes: [],
    nodeSnapshots: [],
  })),
  closeLayoutRef: vi.fn<(decision: 'discard') => Promise<CloseLayoutResult>>(async () => ({ closed: true })),
  probeNodesRef: vi.fn(async () => {}),
  refreshAllNodesRef: vi.fn<() => Promise<string[]>>(async () => []),
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
  listChannelsRef: vi.fn<() => Promise<unknown[]>>(async () => []),
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
  open: dialogOpenRef,
  save: vi.fn(async () => null),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: class {},
  getCurrentWebviewWindow: () => appWindowMock,
}));

vi.mock('$lib/api/tauri', () => ({
  probeNodes: probeNodesRef,
  registerNode: registerNodeRef,
  querySnip: querySnipRef,
  queryPip: queryPipRef,
  refreshAllNodes: refreshAllNodesRef,
}));

vi.mock('$lib/api/bowties', () => ({
  getRecentLayout: getRecentLayoutRef,
  clearRecentLayout: vi.fn(async () => {}),
  buildBowtieCatalog: vi.fn(async () => ({ bowties: [], built_at: '', source_node_count: 0, total_slots_scanned: 0 })),
}));

vi.mock('$lib/api/channels', () => ({
  listChannels: listChannelsRef,
}));

vi.mock('$lib/api/layout', () => ({
  closeLayout: closeLayoutRef,
  saveLayoutDirectory: vi.fn(async () => ({ warnings: [] })),
  openLayoutDirectory: openLayoutFileRef,
  buildOfflineNodeTree: vi.fn(async () => {
    throw new Error('not needed in route discovery test');
  }),
  createNewLayoutCapture: vi.fn(async () => ({
    layoutId: 'created-layout',
    capturedAt: '2026-04-25T00:00:00.000Z',
    layout: { schemaVersion: '1.0', bowties: {}, roleClassifications: {} },
    offlineMode: true,
    nodeCount: 0,
    pendingOfflineChangeCount: 0,
    partialNodes: [],
    nodeSnapshots: [],
  })),
}));

vi.mock('$lib/api/startup', () => ({
  getKnownLayouts: vi.fn(async () => []),
  addKnownLayout: vi.fn(async () => []),
  removeKnownLayout: vi.fn(async () => []),
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
      setCatalog: vi.fn(),
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

vi.mock('$lib/ConnectionManager.svelte', async () => await import('$lib/test/ConnectionManagerStub.svelte'));

vi.mock('$lib/components/Sync/SyncPanel.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/Layout/OfflineBanner.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/LayoutPicker/LayoutPicker.svelte', async () => await import('$lib/test/StubComponent.svelte'));

vi.mock('$lib/components/Railroad/RailroadPanel.svelte', async () => await import('$lib/test/StubComponent.svelte'));

import Page from './+page.svelte';

function makeOfflineLayout() {
  return {
    schemaVersion: '1.0',
    bowties: {},
    roleClassifications: {},
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  eventHandlers.clear();
  closeRequestedHandler.current = null;

  invokeRef.mockImplementation(async (command: string) => {
    if (command === 'get_connection_status') {
      return {
        connected: true,
        config: { name: 'Bench Bus' },
      };
    }
    if (command === 'list_offline_changes') {
      return [];
    }
    return null;
  });

  clearConfigReadStatus();
  configSidebarStore.reset();
  nodeInfoStore.set(new Map());
  nodeTreeStore.reset();
  connectorSelectionsStore.reset();
  channelsStore.reset();
  layoutStore.reset();
  bowtieMetadataStore.clearAll();
  offlineChangesStore.clear();
  resetLayoutOpenPhase();
  getCdiXmlRef.mockReset();
  getCdiXmlRef.mockImplementation(async () => ({ xmlContent: '<cdi />' }));
  dialogOpenRef.mockReset();
  dialogOpenRef.mockImplementation(async () => null);
  closeLayoutRef.mockReset();
  closeLayoutRef.mockImplementation(async () => ({ closed: true }));
  readAllConfigValuesRef.mockReset();
  readAllConfigValuesRef.mockImplementation(async () => ({ failedReads: 0, totalElements: 0 }));
  refreshAllNodesRef.mockReset();
  refreshAllNodesRef.mockImplementation(async () => []);
  getRecentLayoutRef.mockReset();
  getRecentLayoutRef.mockImplementation(async () => null);
  listChannelsRef.mockReset();
  listChannelsRef.mockImplementation(async () => []);
  openLayoutFileRef.mockReset();
  openLayoutFileRef.mockImplementation(async () => ({
    layoutId: 'restored-layout',
    capturedAt: '2026-04-25T00:00:00.000Z',
    layout: makeOfflineLayout(),
    offlineMode: true,
    nodeCount: 0,
    pendingOfflineChangeCount: 0,
    partialNodes: [],
    nodeSnapshots: [],
  }));

  // Spec 013 / S6: pre-seed an active layout context so the picker gate
  // (which now blocks the main UI when no layout is active) does not hide
  // the toolbar and ConnectionManager that these legacy discovery tests
  // exercise. Tests that explicitly close the layout re-seed as needed.
  layoutStore.setActiveContext({
    layoutId: 'test-pre-seeded',
    rootPath: '',
    mode: 'legacy_file',
    pendingOfflineChangeCount: 0,
  });
});

function makeJmriSnipData() {
  return {
    user_name: 'JMRI',
    user_description: '',
    manufacturer: 'JMRI',
    model: 'LccPro',
    hardware_version: '',
    software_version: '5.11.6',
  };
}

async function discoverJmriNode(): Promise<void> {
  const discovered = eventHandlers.get('lcc-node-discovered');
  expect(discovered).toBeTypeOf('function');

  await discovered?.({
    payload: {
      nodeId: '09.00.99.05.01.C1',
      alias: 0x345,
      timestamp: '2026-04-25T12:00:00.000Z',
    },
  });
}

describe('+page route discovery CTA', () => {
  it('keeps a new JMRI node CDI-less after open, close, and connect', async () => {
    invokeRef.mockImplementation(async (command: string) => {
      if (command === 'get_connection_status') {
        return {
          connected: false,
          config: null,
        };
      }
      if (command === 'list_offline_changes') {
        return [];
      }
      return null;
    });
    querySnipRef.mockResolvedValueOnce({
      status: 'Complete',
      snip_data: makeJmriSnipData(),
    });
    queryPipRef.mockResolvedValueOnce({
      status: 'Complete',
      pip_flags: {
        cdi: false,
        memory_configuration: false,
      },
    });

    render(Page);

    await waitFor(() => {
      expect(screen.getByTestId('connect-manager-button')).toBeInTheDocument();
    });

    // Close the pre-seeded layout to simulate the open/close cycle that
    // resets node/CDI state. (Since Spec 013 / S7+, menu-open-layout no
    // longer loads a file directly — it surfaces the picker, which is
    // stubbed in this suite. The discovery-CTA scenario only needs the
    // post-close re-seed for the connect+discover phase below.)
    const closeLayout = eventHandlers.get('menu-close-layout');
    expect(closeLayout).toBeTypeOf('function');
    await closeLayout?.({});

    await waitFor(() => {
      expect(layoutStore.activeContext).toBe(null);
    });

    // Spec 013 / S6: the picker is stubbed in this suite, so re-seed an
    // active layout context so the ConnectionManager continues to render
    // for the remainder of this discovery-CTA scenario.
    layoutStore.setActiveContext({
      layoutId: 'test-pre-seeded',
      rootPath: '',
      mode: 'legacy_file',
      pendingOfflineChangeCount: 0,
    });

    await waitFor(() => {
      expect(screen.getByTestId('connect-manager-button')).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByTestId('connect-manager-button'));

    await waitFor(() => {
      expect(probeNodesRef).toHaveBeenCalledTimes(1);
    });

    await discoverJmriNode();

    await waitFor(() => {
      expect(screen.getByText('JMRI')).toBeInTheDocument();
    });

    expect(screen.queryByText(/1 unread/i)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /read node configuration/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /read configuration/i })).not.toBeInTheDocument();
  });

  it('keeps a direct-connect JMRI node CDI-less', async () => {
    invokeRef.mockImplementation(async (command: string) => {
      if (command === 'get_connection_status') {
        return {
          connected: false,
          config: null,
        };
      }
      if (command === 'list_offline_changes') {
        return [];
      }
      return null;
    });
    querySnipRef.mockResolvedValueOnce({
      status: 'Complete',
      snip_data: makeJmriSnipData(),
    });
    queryPipRef.mockResolvedValueOnce({
      status: 'Complete',
      pip_flags: {
        cdi: false,
        memory_configuration: false,
      },
    });

    render(Page);

    await waitFor(() => {
      expect(screen.getByTestId('connect-manager-button')).toBeInTheDocument();
    });

    await fireEvent.click(screen.getByTestId('connect-manager-button'));

    await waitFor(() => {
      expect(probeNodesRef).toHaveBeenCalledTimes(1);
    });

    await discoverJmriNode();

    await waitFor(() => {
      expect(screen.getByText('JMRI')).toBeInTheDocument();
    });

    expect(screen.queryByText(/1 unread/i)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /read node configuration/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /read configuration/i })).not.toBeInTheDocument();
  });

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

  it('restores the recent layout during startup before probing the live bus', async () => {
    getRecentLayoutRef.mockResolvedValueOnce({ path: 'D:/Layouts/yard.layout.yaml' });
    openLayoutFileRef.mockResolvedValueOnce({
      layoutId: 'yard-layout',
      capturedAt: '2026-04-25T00:00:00.000Z',
      layout: makeOfflineLayout(),
      offlineMode: true,
      nodeCount: 0,
      pendingOfflineChangeCount: 0,
      partialNodes: [],
      nodeSnapshots: [],
    });

    render(Page);

    await waitFor(() => {
      expect(layoutStore.activeContext?.rootPath).toBe('D:/Layouts/yard.layout.yaml');
      expect(probeNodesRef).toHaveBeenCalledTimes(1);
    });
  });

  it('clears stale node state on refresh when the selected node disappears', async () => {
    refreshAllNodesRef.mockResolvedValueOnce(['02.01.57.00.00.01']);

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
    });

    markNodeConfigRead('02.01.57.00.00.01');
    configSidebarStore.setSelectedNode('02.01.57.00.00.01');

    const refresh = eventHandlers.get('menu-refresh');
    expect(refresh).toBeTypeOf('function');
    await refresh?.({});

    await waitFor(() => {
      expect(screen.queryByText('East Panel')).not.toBeInTheDocument();
    });

    const sidebarState = get(configSidebarStore);
    expect(sidebarState.selectedNodeId).toBe(null);
    expect(sidebarState.selectedSegment).toBe(null);
    expect(get(configReadNodesStore).has('02.01.57.00.00.01')).toBe(false);
  });

  it('clears stale connector state when the selected node has no connector profile', async () => {
    const nodeId = '02.01.57.00.00.01';
    const connectorProfile = {
      nodeId,
      carrierKey: 'rr-cirkits::tower-lcc',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD4-CP'],
          affectedPaths: ['Port I/O/Line'],
          resolvedAffectedPaths: [['seg:0', 'elem:0']],
          baseBehaviorWhenEmpty: null,
          supportedDaughterboardConstraints: [],
        },
      ],
      supportedDaughterboards: [
        {
          daughterboardId: 'BOD4-CP',
          displayName: 'BOD4-CP',
          kind: 'detection',
          description: 'Detector board',
        },
      ],
    };

    await connectorSelectionsStore.loadNode(nodeId, connectorProfile as any);
    connectorSelectionsStore.setCompatibilityWarnings(nodeId, ['Connector A requires a supported daughterboard.']);

    expect(connectorSelectionsStore.getProfile(nodeId)?.carrierKey).toBe('rr-cirkits::tower-lcc');
    expect(connectorSelectionsStore.getDocument(nodeId)).not.toBe(null);
    expect(connectorSelectionsStore.getWarnings(nodeId)).toEqual(['Connector A requires a supported daughterboard.']);

    render(Page);

    nodeTreeStore.setTree(nodeId, {
      nodeId,
      identity: null,
      connectorProfile: null,
      segments: [],
    });
    configSidebarStore.setSelectedNode(nodeId);

    await waitFor(() => {
      expect(connectorSelectionsStore.getProfile(nodeId)).toBe(null);
      expect(connectorSelectionsStore.getDocument(nodeId)).toBe(null);
      expect(connectorSelectionsStore.getWarnings(nodeId)).toEqual([]);
    });
  });
});

describe('window close unsaved-changes guard', () => {
  it('shows unsaved-changes dialog and prevents close when dirty', async () => {
    render(Page);
    await waitFor(() => expect(closeRequestedHandler.current).toBeTypeOf('function'));

    // Make the layout dirty
    bowtieMetadataStore.createBowtie('00.00.00.00.00.00.00.01');

    const event = { preventDefault: vi.fn() };
    await closeRequestedHandler.current!(event);

    expect(event.preventDefault).toHaveBeenCalled();
    // The unsaved-changes dialog should be visible
    await waitFor(() => {
      expect(screen.getByText('Unsaved Changes')).toBeInTheDocument();
    });
  });

  it('disconnects gracefully and closes when no unsaved changes', async () => {
    render(Page);
    await waitFor(() => expect(closeRequestedHandler.current).toBeTypeOf('function'));

    const event = { preventDefault: vi.fn() };
    await closeRequestedHandler.current!(event);

    // Should still prevent close (to do async disconnect first)
    expect(event.preventDefault).toHaveBeenCalled();
    // Should invoke disconnect then close
    await waitFor(() => {
      expect(invokeRef).toHaveBeenCalledWith('disconnect_lcc');
      expect(appWindowMock.close).toHaveBeenCalled();
    });
  });
});

describe('+page Railroad tab (Spec 015 / S1)', () => {
  it('shows Railroad tab button in the toolbar', async () => {
    render(Page);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /railroad/i })).toBeInTheDocument();
    });
  });

  it('Railroad tab button appears after Bowties', async () => {
    render(Page);
    await waitFor(() => {
      const buttons = screen.getAllByRole('button', { pressed: false })
        .concat(screen.getAllByRole('button', { pressed: true }));
      const bowties = buttons.find(b => b.textContent?.includes('Bowties'));
      const railroad = buttons.find(b => b.textContent?.includes('Railroad'));
      expect(bowties).toBeTruthy();
      expect(railroad).toBeTruthy();
    });
  });

  it('clicking Railroad tab switches active tab', async () => {
    render(Page);
    const railroadBtn = await waitFor(() => screen.getByRole('button', { name: /railroad/i }));
    await fireEvent.click(railroadBtn);
    expect(railroadBtn.getAttribute('aria-pressed')).toBe('true');
  });
});

describe('+page disconnect lifecycle (Spec 016 / S2)', () => {
  it('clears the event state store when disconnecting via the menu', async () => {
    // Layout context seeded in beforeEach uses mode='legacy_file' →
    // hasLayoutFile=false, so disconnect takes the no-layout branch which
    // calls the `clearLiveState` lambda. This test exercises that wiring.
    render(Page);
    await waitFor(() => expect(eventHandlers.has('menu-disconnect')).toBe(true));

    // Seed PCER events as if the bus had been active during this session.
    eventStateStore.record('0501010101000001', 1000);
    eventStateStore.record('0501010101000002', 2000);
    expect(eventStateStore.size).toBe(2);

    invokeRef.mockImplementation(async (command: string) => {
      if (command === 'disconnect_lcc') return null;
      if (command === 'list_offline_changes') return [];
      return null;
    });

    await eventHandlers.get('menu-disconnect')!({ payload: null });

    await waitFor(() => {
      expect(eventStateStore.size).toBe(0);
    });
  });
});

describe('+page channel resolution timing (Spec 017 / S1)', () => {
  it('re-resolves channel event IDs after live node discovery without a CDI read', async () => {
    // Reproduces the Spec 016 timing gap: on connect with a saved layout, the
    // resolve $effect ran once with empty backend state, then never re-ran
    // because nodeTreeStore.trees.size did not change on discovery. Channels
    // stayed at 'unknown' until the user forced a CDI read.
    //
    // S1 fix: depend on `nodeRoster.liveEntries.length` so the effect re-runs
    // each time a live proxy is registered — backend proxies are seeded with
    // the saved tree at register time (node_registry.rs L100), so resolution
    // succeeds with no CDI read in the path.

    invokeRef.mockImplementation(async (command: string) => {
      if (command === 'get_connection_status') {
        return { connected: true, config: { name: 'Bench Bus' } };
      }
      if (command === 'list_offline_changes') return [];
      if (command === 'resolve_channel_event_ids') return [];
      if (command === 'register_node') return null;
      return null;
    });

    // Seed a channel via the listChannels IPC so it survives the route's
    // automatic `channelsStore.loadChannels()` on layout mount.
    listChannelsRef.mockImplementation(async () => [{
      id: 'ch-1',
      name: 'East Block',
      channelType: 'block-occupancy',
      hardwareRef: { nodeKey: '09.00.99.05.01.C1', connector: 'connector-a', input: 1 },
    }]);

    render(Page);
    await waitFor(() => expect(eventHandlers.has('lcc-node-discovered')).toBe(true));
    // Wait for the channels store to hydrate from the mocked IPC so the
    // effect's `channels.length > 0` precondition holds before discovery.
    await waitFor(() => expect(channelsStore.channels.length).toBeGreaterThan(0));

    // Clear the invoke history so we only see calls triggered by discovery.
    invokeRef.mockClear();

    await discoverJmriNode();

    // After discovery, the resolve effect must re-run and call the IPC.
    await waitFor(() => {
      const calls = invokeRef.mock.calls.map((c) => c[0]);
      expect(calls).toContain('resolve_channel_event_ids');
    });

    // And no CDI read should be needed to make that happen.
    const calls = invokeRef.mock.calls.map((c) => c[0]);
    expect(calls).not.toContain('read_all_config_values');
  });
});