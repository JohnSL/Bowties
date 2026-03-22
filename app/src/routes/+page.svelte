<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from '@tauri-apps/api/event';
  import { onMount, untrack } from 'svelte';
  import { get } from 'svelte/store';
  import { WebviewWindow, getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import ConfigSidebar from '$lib/components/ConfigSidebar/ConfigSidebar.svelte';
  import SegmentView from '$lib/components/ElementCardDeck/SegmentView.svelte';
  import CdiXmlViewer from '$lib/components/CdiXmlViewer.svelte';
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { probeNodes as probeNodesApi, querySnip, queryPip, registerNode, refreshAllNodes } from '$lib/api/tauri';
  import { readAllConfigValues, cancelConfigReading, getCdiXml, downloadCdi } from '$lib/api/cdi';
  import { getCdiErrorMessage, isCdiError } from '$lib/types/cdi';
  import type { ViewerStatus } from '$lib/types/cdi';
  import type { DiscoveredNode } from '$lib/api/tauri';
  import type { ReadProgressState, NodeReadState } from '$lib/api/types';
  import { updateNodeInfo } from '$lib/stores/nodeInfo';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { hasModifiedLeaves, resolvePillSelectionsForPath } from '$lib/types/nodeTree';
  import { configReadNodesStore, markNodeConfigRead, clearConfigReadStatus, removeNodesConfigRead } from '$lib/stores/configReadStatus';
  import BowtieCatalogPanel from '$lib/components/Bowtie/BowtieCatalogPanel.svelte';
  import DiscoveryProgressModal from '$lib/components/DiscoveryProgressModal.svelte';
  import SaveControls from '$lib/components/ElementCardDeck/SaveControls.svelte';
  import CdiDownloadDialog from '$lib/components/CdiDownloadDialog.svelte';
  import ConnectionManager from '$lib/ConnectionManager.svelte';
  import { connectionRequestStore } from '$lib/stores/connectionRequest.svelte';
  import { bowtieFocusStore } from '$lib/stores/bowtieFocus.svelte';
  import { configFocusStore } from '$lib/stores/configFocus.svelte';
  import { setPillSelection } from '$lib/stores/pillSelection';
  import type { MissingCdiNode } from '$lib/components/CdiDownloadDialog.svelte';

  // Active tab state — 'config' (default) or 'bowties'
  let activeTab = $state<'config' | 'bowties'>('config');

  // T050: prompt-to-save guard state
  let unsavedDialog = $state<{ message: string; proceed: () => void; confirmLabel: string } | null>(null);
  let isForceClosing = false;

  function promptUnsaved(message: string, proceed: () => void, confirmLabel = 'Discard & Continue'): void {
    const hasUnsaved = [...nodeTreeStore.trees.values()].some(t => hasModifiedLeaves(t)) || bowtieMetadataStore.isDirty;
    if (hasUnsaved) {
      unsavedDialog = { message, proceed, confirmLabel };
    } else {
      proceed();
    }
  }

  // T041: Switch to bowties tab when a config-first connection request is pending
  $effect(() => {
    if (connectionRequestStore.pendingRequest) {
      activeTab = 'bowties';
    }
  });

  // Switch to bowties tab when a "Used in" link is clicked on the config page
  $effect(() => {
    if (bowtieFocusStore.highlightedEventIdHex) {
      activeTab = 'bowties';
    }
  });

  // Switch to config tab and navigate to the target field when a bowtie entry link is clicked
  $effect(() => {
    const focus = configFocusStore.navigationRequest;
    if (!focus) return;

    // Consume immediately — TreeLeafRow handles its own leafFocusRequest.
    untrack(() => configFocusStore.clearNavigation());

    activeTab = 'config';

    const tree = untrack(() => nodeTreeStore.getTree(focus.nodeId));
    if (!tree) return;

    const segMatch = focus.elementPath[0]?.match(/^seg:(\d+)$/);
    if (!segMatch) return;
    const segIdx = parseInt(segMatch[1], 10);
    const seg = tree.segments[segIdx];
    if (!seg) return;

    // Compute and apply pill selections (pure utility — no tree-structure
    // knowledge required here).
    const pillEntries = resolvePillSelectionsForPath(focus.nodeId, seg, focus.elementPath);
    for (const [key, idx] of pillEntries) {
      setPillSelection(key, idx);
    }

    // Expand node in sidebar if needed
    const sidebarState = get(configSidebarStore);
    if (!sidebarState.expandedNodeIds.includes(focus.nodeId)) {
      configSidebarStore.toggleNodeExpanded(focus.nodeId);
    }

    // Select the segment → triggers card deck render → TreeLeafRow mounts →
    // leafFocusRequest scrolls + focuses the input.
    configSidebarStore.selectSegment(focus.nodeId, `seg:${segIdx}`, seg.name);
  });

  // Connection state
  let connectionLabel = $state("");
  let connected = $state(false);
  let errorMessage = $state("");

  // Discovery state
  let nodes = $state<DiscoveredNode[]>([]);
  let probing = $state(false);

  // Config reading progress state (T063-T067)
  let readProgress = $state<ReadProgressState | null>(null);
  let isCancelling = $state(false);

  // Discovery progress modal state
  let discoveryModalVisible = $state(false);
  let discoveryPhase = $state<'reading' | 'complete' | 'cancelled'>('reading');

  // Track whether a single-node or batch "read remaining" is in progress
  let readingRemaining = $state(false);

  // Per-node progress state for the redesigned progress modal
  let nodeReadStates = $state<NodeReadState[]>([]);

  // Returns true when PIP has confirmed the node does not support CDI or Memory Configuration
  function pipConfirmsNoCdi(n: DiscoveredNode): boolean {
    if (n.pip_status !== 'Complete') return false;
    if (!n.pip_flags) return false;
    return !n.pip_flags.cdi && !n.pip_flags.memory_configuration;
  }

  // Reactive count of nodes with SNIP data not yet config-read — drives "Read Remaining" visibility
  let unreadCount = $derived(
    nodes.filter(n => {
      if (!n.snip_data) return false;
      if (pipConfirmsNoCdi(n)) return false;
      return !$configReadNodesStore.has(formatNodeId(n.node_id));
    }).length
  );

  // Show CTA panel when nodes discovered but not yet read and nothing selected
  let showConfigCta = $derived(
    nodes.length > 0 &&
    unreadCount > 0 &&
    !$configSidebarStore.selectedSegment &&
    !$configSidebarStore.selectedNodeId
  );

  // CDI XML viewer state
  let viewerVisible = $state(false);
  let viewerNodeId = $state<string | null>(null);
  let viewerXmlContent = $state<string | null>(null);
  let viewerStatus = $state<ViewerStatus>('idle');
  let viewerErrorMessage = $state<string | null>(null);

  // CDI download dialog state
  let cdiDownloadDialogVisible = $state(false);
  let cdiMissingNodes = $state<MissingCdiNode[]>([]);
  let cdiDownloading = $state(false);
  let cdiDownloadedCount = $state(0);

  // Track which nodes have CDI available in cache (populated during discovery/refresh)
  let nodesWithCdi = $state(new Set<string>());

  // Check connection status on mount
  onMount(() => {
    const unlistens: Array<() => void> = [];

    (async () => {
      try {
        const status = await invoke("get_connection_status");
        connected = (status as any).connected;
        if (connected && (status as any).config) {
          const cfg = (status as any).config;
          connectionLabel = cfg.name ?? (cfg.host ? `${cfg.host}:${cfg.port}` : cfg.serialPort ?? 'LCC');
        }
      } catch (e) {
        console.error("Failed to get connection status:", e);
      }

      // Feature 006: Start bowties store listener so cdi-read-complete is captured
      // regardless of whether the user has visited the Bowties page.
      // Must be awaited so the listener is registered before checkAndReopenRecent()
      // triggers buildBowtieCatalog which immediately emits cdi-read-complete.
      await bowtieCatalogStore.startListening();

      // Spec 009 T015: Auto-reopen the most recent layout file on startup
      layoutStore.checkAndReopenRecent();

      // Spec 007: Start node-tree-updated listener so trees are refreshed
      // automatically as config values and event roles are merged server-side.
      nodeTreeStore.startListening();

      // T050: Prompt-to-save guard on app close (FR-024)
      const appWindow = getCurrentWebviewWindow();
      unlistens.push(await appWindow.onCloseRequested((event) => {
        if (isForceClosing) return;
        const hasUnsaved = [...nodeTreeStore.trees.values()].some(t => hasModifiedLeaves(t)) || bowtieMetadataStore.isDirty;
        if (hasUnsaved) {
          event.preventDefault();
          unsavedDialog = {
            message: 'You have unsaved changes. Exit without saving?',
            confirmLabel: 'Exit Without Saving',
            proceed: () => { isForceClosing = true; bowtieMetadataStore.clearAll(); appWindow.close(); },
          };
        }
      }));

      // T063: Setup config-read-progress event listener
      unlistens.push(await listen<ReadProgressState>('config-read-progress', (event) => {
        readProgress = event.payload;
        discoveryPhase = 'reading';

        const payload = event.payload;
        const idx = payload.currentNodeIndex;

        // Update per-node progress bar states
        if (nodeReadStates.length > 0) {
          nodeReadStates = nodeReadStates.map((s, i) => {
            if (i < idx) return { ...s, status: 'complete' as const, percentage: 100 };
            if (i === idx) {
              if (payload.status.type === 'NodeComplete') {
                return { ...s, status: 'complete' as const, percentage: 100 };
              }
              // payload.percentage is per-node local progress (0-100)
              return { ...s, status: 'reading' as const, percentage: payload.percentage };
            }
            return s;
          });
        }

        // Close modal immediately on cancellation
        if (payload.status.type === 'Cancelled') {
          readProgress = null;
          isCancelling = false;
          discoveryModalVisible = false;
          nodeReadStates = [];
        }
      }));

      // Reactive node discovery: nodes appear one-by-one as VerifiedNode replies arrive.
      // Register in backend cache, add skeleton to local list, then fetch SNIP+PIP per node.
      unlistens.push(await listen<{ nodeId: string; alias: number; timestamp: string }>('lcc-node-discovered', async (event) => {
        if (!connected) return; // ignore stray events after disconnect
        const { nodeId, alias } = event.payload;
        if (nodes.some(n => formatNodeId(n.node_id) === nodeId)) return; // dedup

        // Parse dotted-hex nodeId back to number[]
        const nodeIdBytes = nodeId.split('.').map(b => parseInt(b, 16));

        // Add skeleton immediately so the UI shows the node without waiting for SNIP
        const skeleton: DiscoveredNode = {
          node_id: nodeIdBytes,
          alias,
          snip_data: null,
          snip_status: 'Unknown',
          connection_status: 'Connected',
          last_verified: null,
          last_seen: new Date().toISOString(),
          cdi: null,
          pip_flags: null,
          pip_status: 'Unknown',
        };
        nodes = [...nodes, skeleton];
        updateNodeInfo(nodes);

        try {
          // Register in backend state first so SNIP/CDI cache updates work correctly
          await registerNode(nodeId, alias);

          // Fetch SNIP + PIP concurrently
          const [snipResult, pipResult] = await Promise.all([
            querySnip(alias),
            queryPip(alias),
          ]);

          nodes = nodes.map(n => {
            if (n.alias !== alias) return n;
            return {
              ...n,
              snip_data: snipResult.snip_data,
              snip_status: snipResult.status,
              pip_flags: pipResult.pip_flags,
              pip_status: pipResult.status,
            };
          });
          updateNodeInfo(nodes);
        } catch (e) {
          console.warn(`Failed to query node ${nodeId}:`, e);
        }
      }));

      // Now that the lcc-node-discovered listener is registered, probe for existing nodes.
      // Doing this after listener setup avoids a race where VerifiedNode replies arrive
      // before the frontend listener is ready and get silently dropped.
      if (connected) probeForNodes();

      // Native menu event listeners — relay OS menu clicks to handler functions
      unlistens.push(await listen('menu-disconnect',     () => disconnect()));
      unlistens.push(await listen('menu-refresh',        () => { if (connected) handleRefresh(); }));
      unlistens.push(await listen('menu-traffic',        () => { if (connected) openTrafficMonitor(); }));
      unlistens.push(await listen('menu-view-cdi',       () => {
        const state = get(configSidebarStore);
        const nodeId = state.selectedSegment?.nodeId ?? state.selectedNodeId;
        if (nodeId) openCdiViewer(nodeId, false);
      }));
      unlistens.push(await listen('menu-redownload-cdi', () => {
        const state = get(configSidebarStore);
        const nodeId = state.selectedSegment?.nodeId ?? state.selectedNodeId;
        if (nodeId) openCdiViewer(nodeId, true);
      }));
      unlistens.push(await listen('menu-exit', () => {
        const win = getCurrentWebviewWindow();
        promptUnsaved('You have unsaved changes. Exit without saving?', () => {
          isForceClosing = true;
          bowtieMetadataStore.clearAll();
          win.close();
        }, 'Exit Without Saving');
      }));
    })();

    // Cleanup all listeners on component unmount
    return () => {
      unlistens.forEach(u => u());
    };
  });

  function handleConnected(e: CustomEvent<{ config: any }>) {
    const cfg = e.detail.config;
    connectionLabel = cfg.name ?? (cfg.host ? `${cfg.host}:${cfg.port}` : cfg.serialPort ?? 'LCC');
    connected = true;
    probeForNodes();
  }

  async function disconnect() {
    errorMessage = "";
    connectionLabel = "";
    try {
      await invoke("disconnect_lcc");
      connected = false;
      nodes = [];
      updateNodeInfo([]);
      nodeTreeStore.reset();
      clearConfigReadStatus();
    } catch (e) {
      errorMessage = `Disconnect failed: ${e}`;
    }
  }

  /** Fire-and-forget probe — nodes appear via lcc-node-discovered events */
  async function probeForNodes() {
    try {
      await probeNodesApi();
    } catch (e) {
      console.error("Probe failed:", e);
    }
  }

  /**
   * Re-probe the network. Culls stale nodes (those that don't reply) from the
   * UI; new or returning nodes appear automatically via lcc-node-discovered events.
   */
  async function handleRefresh() {
    if (probing) return;
    errorMessage = "";
    probing = true;
    try {
      const staleIds = await refreshAllNodes();
      if (staleIds.length > 0) {
        nodes = nodes.filter(n => !staleIds.includes(formatNodeId(n.node_id)));
        updateNodeInfo(nodes);
        // Only clean up state for nodes that actually left — preserve config read
        // status and CDI data for nodes that are still present.
        removeNodesConfigRead(staleIds);
        nodesWithCdi = new Set([...nodesWithCdi].filter(id => !staleIds.includes(id)));
        // Reset the sidebar only if the currently selected node was removed.
        const sidebarState = get(configSidebarStore);
        const selectedId = sidebarState.selectedSegment?.nodeId ?? sidebarState.selectedNodeId;
        if (selectedId && staleIds.includes(selectedId)) {
          configSidebarStore.reset();
        }
      }
    } catch (e) {
      console.error("Refresh failed:", e);
      errorMessage = `Refresh failed: ${e}`;
    } finally {
      probing = false;
    }
  }

  function formatNodeId(nodeId: number[]): string {
    return nodeId.map(b => b.toString(16).toUpperCase().padStart(2, '0')).join('.');
  }

  function formatAlias(alias: number): string {
    return '0x' + alias.toString(16).toUpperCase().padStart(3, '0');
  }

  async function openTrafficMonitor() {
    const win = new WebviewWindow('traffic', {
      url: '/traffic',
      title: 'LCC Traffic Monitor',
      width: 960,
      height: 640,
      parent: getCurrentWebviewWindow(),
    });
    // If a window with this label already exists Tauri emits tauri://error
    // instead of creating a duplicate — just focus the existing one.
    win.once('tauri://error', async () => {
      const existing = await WebviewWindow.getByLabel('traffic');
      if (existing) await existing.setFocus();
    });
  }

  // T066: Cancel button handler for config reading
  async function handleCancelConfigReading() {
    if (isCancelling) return; // Already cancelling
    
    isCancelling = true;
    
    try {
      await cancelConfigReading();
      console.log('Config reading cancellation requested');
    } catch (e) {
      console.error('Failed to cancel config reading:', e);
      errorMessage = `Cancel failed: ${e}`;
      isCancelling = false;
    }
  }

  // ─── CDI XML Viewer ───────────────────────────────────────────────────────

  /** Read config values for all unread nodes (batch) */
  async function readRemainingNodes() {
    const readNodes = get(configReadNodesStore);
    const unread = nodes.filter(n => {
      if (!n.snip_data) return false;
      if (pipConfirmsNoCdi(n)) return false;
      const nodeId = formatNodeId(n.node_id);
      return !readNodes.has(nodeId);
    });
    if (unread.length === 0) return;

    // Init per-node progress states (all waiting)
    nodeReadStates = unread.map(n => ({
      nodeId: formatNodeId(n.node_id),
      name: n.snip_data?.user_name || formatNodeId(n.node_id),
      percentage: 0,
      status: 'waiting' as const,
    }));

    readingRemaining = true;
    discoveryModalVisible = true;
    discoveryPhase = 'reading';
    errorMessage = '';

    const localCdiMissing: MissingCdiNode[] = [];
    const newNodesWithCdi = new Set<string>();
    try {
      for (let nodeIdx = 0; nodeIdx < unread.length; nodeIdx++) {
        const node = unread[nodeIdx];
        const nodeId = formatNodeId(node.node_id);
        const nodeName = node.snip_data?.user_name || nodeId;
        try {
          let hasCdi = false;
          try {
            const cdiCheck = await getCdiXml(nodeId);
            hasCdi = cdiCheck.xmlContent !== null;
            if (hasCdi) newNodesWithCdi.add(nodeId);
          } catch { /* CDI not available */ }
          if (!hasCdi) {
            localCdiMissing.push({ nodeId, nodeName });
            nodeReadStates = nodeReadStates.map((s, i) =>
              i === nodeIdx ? { ...s, status: 'no-cdi' as const } : s
            );
            continue;
          }
          console.log(`Reading config values from ${nodeName}...`);
          await readAllConfigValues(nodeId, undefined, nodeIdx, unread.length);
          markNodeConfigRead(nodeId);
          await nodeTreeStore.refreshTree(nodeId);
          // Ensure slot marked complete (listener may have done it already)
          nodeReadStates = nodeReadStates.map((s, i) =>
            i === nodeIdx ? { ...s, status: 'complete' as const, percentage: 100 } : s
          );
        } catch (e) {
          console.warn(`Failed to read config values from ${nodeName}:`, e);
          nodeReadStates = nodeReadStates.map((s, i) =>
            i === nodeIdx ? { ...s, status: 'failed' as const } : s
          );
        }
      }
      nodesWithCdi = newNodesWithCdi;
      cdiMissingNodes = localCdiMissing;
      // Close immediately — no delay
      discoveryModalVisible = false;
      readProgress = null;
      isCancelling = false;
      nodeReadStates = [];
      if (cdiMissingNodes.length > 0) {
        cdiDownloadDialogVisible = true;
      }
    } catch (e) {
      errorMessage = `Read remaining failed: ${e}`;
      discoveryModalVisible = false;
      nodeReadStates = [];
    } finally {
      readingRemaining = false;
    }
  }

  /** Read config values for a single node (triggered from sidebar indicator) */
  async function readSingleNodeConfig(nodeId: string) {
    const node = nodes.find(n => formatNodeId(n.node_id) === nodeId);
    if (!node?.snip_data) return;

    const nodeName = node.snip_data.user_name || nodeId;
    // Init single-node progress state
    nodeReadStates = [{ nodeId, name: nodeName, percentage: 0, status: 'waiting' }];

    readingRemaining = true;
    discoveryModalVisible = true;
    discoveryPhase = 'reading';
    errorMessage = '';

    try {
      let hasCdi = false;
      try {
        const cdiCheck = await getCdiXml(nodeId);
        hasCdi = cdiCheck.xmlContent !== null;
        if (hasCdi) nodesWithCdi = new Set([...nodesWithCdi, nodeId]);
      } catch { /* CDI not available */ }
      if (!hasCdi) {
        errorMessage = `CDI not available for ${nodeName}`;
        discoveryModalVisible = false;
        nodeReadStates = [];
        readingRemaining = false;
        return;
      }
      await readAllConfigValues(nodeId, undefined, 0, 1);
      markNodeConfigRead(nodeId);
      // Force tree refresh so the currently visible SegmentView updates
      await nodeTreeStore.refreshTree(nodeId);
      // Close immediately — no delay
      discoveryModalVisible = false;
      readProgress = null;
      isCancelling = false;
      nodeReadStates = [];
    } catch (e) {
      errorMessage = `Failed to read config for ${nodeName}: ${e}`;
      discoveryModalVisible = false;
      nodeReadStates = [];
    } finally {
      readingRemaining = false;
    }
  }

  async function openCdiViewer(nodeId: string, forceDownload: boolean) {
    viewerVisible = true;
    viewerNodeId = nodeId;
    viewerXmlContent = null;
    viewerStatus = 'loading';
    viewerErrorMessage = forceDownload ? 'Downloading CDI from node…' : 'Checking cache…';

    try {
      let response;
      if (forceDownload) {
        response = await downloadCdi(nodeId);
      } else {
        try {
          response = await getCdiXml(nodeId);
        } catch (cacheError: any) {
          if (isCdiError(cacheError, 'CdiNotRetrieved')) {
            viewerErrorMessage = 'Downloading CDI from node…';
            response = await downloadCdi(nodeId);
          } else {
            throw cacheError;
          }
        }
      }

      if (response.xmlContent) {
        viewerXmlContent = response.xmlContent;
        viewerStatus = 'success';
        viewerErrorMessage = null;
      } else {
        viewerStatus = 'error';
        viewerErrorMessage = 'No CDI data available for this node.';
      }
    } catch (err) {
      viewerStatus = 'error';
      viewerErrorMessage = getCdiErrorMessage(err);
    }
  }

  function closeCdiViewer() {
    viewerVisible = false;
    viewerNodeId = null;
    viewerXmlContent = null;
    viewerStatus = 'idle';
    viewerErrorMessage = null;
  }

  function handleCdiDownloadCancel() {
    cdiDownloadDialogVisible = false;
    cdiMissingNodes = [];
  }

  async function handleCdiDownload() {
    cdiDownloading = true;
    cdiDownloadedCount = 0;
    const nodesToDownload = [...cdiMissingNodes];

    for (let i = 0; i < nodesToDownload.length; i++) {
      const { nodeId, nodeName } = nodesToDownload[i];
      try {
        await downloadCdi(nodeId);
        console.log(`Downloaded CDI for ${nodeName}`);
        nodesWithCdi = new Set([...nodesWithCdi, nodeId]);
      } catch (e) {
        console.warn(`Failed to download CDI for ${nodeName}:`, e);
      }
      cdiDownloadedCount = i + 1;
    }

    cdiDownloadDialogVisible = false;
    cdiDownloading = false;
    cdiMissingNodes = [];

    // Read config values for nodes that now have CDI
    for (let i = 0; i < nodesToDownload.length; i++) {
      const { nodeId, nodeName } = nodesToDownload[i];
      try {
        const cdiCheck = await getCdiXml(nodeId);
        if (cdiCheck.xmlContent !== null) {
          const response = await readAllConfigValues(nodeId, undefined, i, nodesToDownload.length);
          markNodeConfigRead(nodeId);
          await nodeTreeStore.loadTree(nodeId);
          console.log(`✓ Read config for ${nodeName}`);
        }
      } catch (e) {
        console.warn(`Failed to read config for ${nodeId}:`, e);
      }
    }
  }

  // Sync native menu item enable/disable state with current app state.
  // Tauri v2 has no "menu will open" event, so we push state eagerly whenever
  // any of the tracked reactive values change.
  async function syncMenuState(
    conn: boolean,
    busy: boolean,
    canViewCdi: boolean,
    canRedownloadCdi: boolean
  ) {
    try {
      await invoke("update_menu_state", {
        connected: conn,
        isBusy: busy,
        canViewCdi,
        canRedownloadCdi,
      });
    } catch (e) {
      console.warn("Failed to update menu state:", e);
    }
  }

  $effect(() => {
    const conn = connected;
    const busy = probing || readingRemaining;
    const store = $configSidebarStore;

    // Determine which node is selected
    const selectedNodeId = store.selectedSegment?.nodeId ?? store.selectedNodeId;

    // Re-download CDI is available if any node is selected
    const canRedownloadCdi = conn && !busy && !!selectedNodeId;

    // View CDI is available if:
    // - A segment is selected (segment exists → CDI exists), OR
    // - A node is selected and has cached CDI
    const canViewCdi =
      conn &&
      !busy &&
      (!!store.selectedSegment || (!!selectedNodeId && nodesWithCdi.has(selectedNodeId)));

    syncMenuState(conn, busy, canViewCdi, canRedownloadCdi);
  });
</script>


<div class="app-shell">

  <!-- ═══ TOOLBAR (connected only) ═══ -->
  {#if connected}
    <div class="toolbar" role="toolbar" aria-label="Main toolbar">
      <div class="toolbar-left">
        <button
          class="toolbar-btn"
          onclick={handleRefresh}
          disabled={probing || readingRemaining}
          title={nodes.length > 0 ? 'Refresh nodes on the network' : 'Discover nodes on the network'}
        >
          <span class="tb-icon" class:tb-spin={probing}>⟳</span>
          <span>{probing ? 'Refreshing…' : nodes.length > 0 ? 'Refresh Nodes' : 'Discover Nodes'}</span>
        </button>
        {#if readingRemaining || unreadCount > 0}
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button
            class="toolbar-btn"
            onclick={readRemainingNodes}
            disabled={probing || readingRemaining}
            title="Read configuration values for nodes not yet read"
          >
            <span class="tb-icon" class:tb-spin={readingRemaining}>⟳</span>
            <span>{readingRemaining ? 'Reading…' : `Read Remaining (${unreadCount})`}</span>
          </button>
        {/if}
        <span class="toolbar-sep" aria-hidden="true"></span>
        <!-- FR-013: Bowties tab — disabled until cdi-read-complete fires -->
        <button
          class="toolbar-btn toolbar-btn-bowties"
          class:toolbar-btn-active={activeTab === 'bowties'}
          onclick={() => { activeTab = activeTab === 'bowties' ? 'config' : 'bowties'; }}
          disabled={!bowtieCatalogStore.readComplete}
          aria-disabled={!bowtieCatalogStore.readComplete}
          aria-pressed={activeTab === 'bowties'}
          title={bowtieCatalogStore.readComplete
            ? 'View discovered bowtie connections'
            : 'Bowties available after reading all node configurations'}
        >
          <span class="tb-icon">🎀</span>
          <span>Bowties</span>
          <!-- T022: Global unsaved indicator — show dot when trees have modified values or metadata is dirty -->
          {#if [...nodeTreeStore.trees.values()].some(t => hasModifiedLeaves(t)) || bowtieMetadataStore.isDirty}
            <span class="global-dirty-dot" title="Unsaved changes" aria-label="Unsaved changes">●</span>
          {/if}
        </button>
        <SaveControls toolbar={true} />
        <!-- Feature 009: Layout file controls -->
        {#if bowtieCatalogStore.readComplete}
          <span class="toolbar-sep" aria-hidden="true"></span>
          <button
            class="toolbar-btn"
            onclick={() => promptUnsaved('Opening a new layout will discard unsaved changes. Continue?', () => layoutStore.openLayout(), 'Discard & Open')}
            disabled={layoutStore.isBusy}
            title="Open a layout file (.bowties.yaml)"
          >
            <span class="tb-icon">📂</span>
            <span>Open Layout</span>
          </button>
          {#if layoutStore.isLoaded}
            <button
              class="toolbar-btn"
              onclick={async () => { await layoutStore.saveCurrentLayout(); bowtieMetadataStore.clearAll(); }}
              disabled={layoutStore.isBusy || !layoutStore.isDirty}
              title={layoutStore.isDirty ? `Save changes to ${layoutStore.displayName}` : 'No unsaved changes'}
            >
              <span class="tb-icon">💾</span>
              <span>Save{layoutStore.isDirty ? '*' : ''}</span>
            </button>
            <button
              class="toolbar-btn"
              onclick={async () => { await layoutStore.saveLayoutAs(); bowtieMetadataStore.clearAll(); }}
              disabled={layoutStore.isBusy}
              title="Save layout to a new file"
            >
              <span class="tb-icon">💾</span>
              <span>Save As</span>
            </button>
          {/if}
        {/if}
      </div>
      <div class="toolbar-right">
        <button
          class="toolbar-status-btn"
          onclick={disconnect}
          title="Disconnect from {connectionLabel}"
          aria-label="Disconnect from {connectionLabel}"
        >
          <span class="status-dot status-connected" aria-hidden="true"></span>
          <span class="status-text">{connectionLabel}</span>
          <span class="status-disconnect-hint" aria-hidden="true">Disconnect</span>
        </button>
      </div>
    </div>
  {/if}

  <!-- ═══ DISCOVERY PROGRESS MODAL ═══ -->
  <DiscoveryProgressModal
    visible={discoveryModalVisible}
    phase={discoveryPhase}
    {readProgress}
    {isCancelling}
    {nodeReadStates}
    onCancel={handleCancelConfigReading}
  />

  <!-- ═══ ERROR BANNER ═══ -->
  {#if errorMessage}
    <div class="error-banner" role="alert">
      <span class="error-banner-text">⚠ {errorMessage}</span>
      <button class="error-banner-close" onclick={() => errorMessage = ''} aria-label="Dismiss error">✕</button>
    </div>
  {/if}

  <!-- ═══ MAIN CONTENT ═══ -->
  <div class="main-content">
    {#if !connected}
      <div class="connect-area">
        <ConnectionManager on:connected={handleConnected} />
      </div>

    {:else if nodes.length === 0}
      <div class="empty-area">
        <p class="empty-status">No nodes found.</p>
        <p class="empty-hint">Click <strong>Refresh Nodes</strong> in the toolbar to scan the network again.</p>
      </div>

    {:else if activeTab === 'bowties'}
      <!-- Feature 006: Bowties catalog in-page tab (no navigation) -->
      <BowtieCatalogPanel
        highlightedEventIdHex={bowtieFocusStore.highlightedEventIdHex}
        onReadConfig={readRemainingNodes}
        hasUnreadNodes={showConfigCta}
        readingConfig={readingRemaining}
        {unreadCount}
        nodesCount={nodes.length}
      />

    {:else}
      <!-- FR-001: two-panel layout — fixed sidebar + scrollable main area -->
      <div class="config-layout">
        <ConfigSidebar on:readNodeConfig={(e) => readSingleNodeConfig(e.detail.nodeId)} />
        <div class="config-main">
          {#if showConfigCta}
            <div class="config-cta-panel">
              <h2 class="cta-title">Node Configuration</h2>
              <p class="cta-desc">
                {nodes.length} {nodes.length === 1 ? 'node' : 'nodes'} discovered.
                Click below to read their configuration.
              </p>
              <button
                class="cta-btn"
                onclick={readRemainingNodes}
                disabled={readingRemaining}
              >
                Read Node Configuration
              </button>
              {#if unreadCount > 0}
                <span class="cta-badge">{unreadCount} unread</span>
              {/if}
            </div>
          {:else}
            <SegmentView />
          {/if}
        </div>
      </div>
    {/if}
  </div>

</div>

<!-- CDI XML Viewer Modal -->
<CdiXmlViewer
  visible={viewerVisible}
  nodeId={viewerNodeId}
  xmlContent={viewerXmlContent}
  status={viewerStatus}
  errorMessage={viewerErrorMessage}
  onClose={closeCdiViewer}
/>

<!-- CDI Download Dialog — shown when nodes lack a cached CDI after discovery -->
{#if cdiDownloadDialogVisible}
  <CdiDownloadDialog
    nodes={cdiMissingNodes}
    downloading={cdiDownloading}
    downloadedCount={cdiDownloadedCount}
    onDownload={handleCdiDownload}
    onCancel={handleCdiDownloadCancel}
  />
{/if}

<!-- T050: Prompt-to-save guard dialog (FR-024) -->
{#if unsavedDialog}
  <div
    class="unsaved-overlay"
    role="dialog"
    aria-modal="true"
    aria-label="Unsaved changes warning"
  >
    <div class="unsaved-dialog">
      <h3 class="unsaved-title">Unsaved Changes</h3>
      <p class="unsaved-body">{unsavedDialog.message}</p>
      <div class="unsaved-actions">
        <button
          class="unsaved-btn unsaved-btn-secondary"
          onclick={() => { unsavedDialog = null; }}
        >Cancel</button>
        <button
          class="unsaved-btn unsaved-btn-danger"
          onclick={() => { const proceed = unsavedDialog!.proceed; unsavedDialog = null; proceed(); }}
        >{unsavedDialog.confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    height: 100%;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: white;
    overflow: hidden;
  }

  /* ─── App Shell ─────────────────────────────────────── */

  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  /* ─── Status indicator (toolbar) ───────────────────── */

  .toolbar-status-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    font-size: 12px;
    color: #555;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, border-color 0.12s, color 0.12s, box-shadow 0.12s;
  }

  .toolbar-status-btn:hover {
    background: #fee2e2;
    border-color: #fca5a5;
    color: #b91c1c;
  }

  .toolbar-status-btn:hover .status-text {
    display: none;
  }

  .status-disconnect-hint {
    display: none;
  }

  .toolbar-status-btn:hover .status-disconnect-hint {
    display: inline;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .status-connected    { background: #10b981; }

  @keyframes status-pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.4; }
  }

  /* ─── Toolbar ───────────────────────────────────────── */

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #f3f4f6;
    border-bottom: 1px solid #d1d5db;
    padding: 0 8px;
    height: 40px;
    flex-shrink: 0;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
  }

  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    font-size: 13px;
    color: #374151;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, border-color 0.12s, box-shadow 0.12s;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: #f0f4ff;
    border-color: #c7d2fe;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .toolbar-btn:disabled {
    background: #fafafa;
    border-color: #ebebeb;
    box-shadow: none;
    color: #bbb;
    cursor: not-allowed;
    pointer-events: none;
  }

  .tb-icon {
    font-size: 15px;
  }

  .tb-spin {
    display: inline-block;
    animation: tb-rotate 1s linear infinite;
  }

  @keyframes tb-rotate {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  .toolbar-sep {
    width: 1px;
    height: 20px;
    background: #d1d5db;
    margin: 0 4px;
  }

  /* Active (pressed) state for toggle toolbar buttons */
  .toolbar-btn-active {
    background: #eff6ff;
    border-color: #6366f1 !important;
    color: #4338ca !important;
  }

  /* T022: Global unsaved-changes indicator dot */
  .global-dirty-dot {
    color: #ca5010;
    font-size: 0.55rem;
    vertical-align: super;
    margin-left: 2px;
    line-height: 1;
  }



  /* ─── Error Banner ──────────────────────────────────── */

  .error-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    height: 32px;
    background: #fee2e2;
    border-bottom: 1px solid #fecaca;
    flex-shrink: 0;
  }

  .error-banner-text {
    flex: 1;
    font-size: 12px;
    color: #dc2626;
  }

  .error-banner-close {
    background: none;
    border: none;
    color: #dc2626;
    cursor: pointer;
    font-size: 14px;
    padding: 2px 4px;
    border-radius: 3px;
  }

  .error-banner-close:hover {
    background: #fecaca;
  }

  /* ─── Main Content ──────────────────────────────────── */

  .main-content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ─── Connect form ──────────────────────────────────── */

  .connect-area {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
  }

  /* ─── Empty / loading state ─────────────────────────── */

  .empty-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: #6b7280;
    gap: 4px;
  }

  .empty-status {
    margin: 0;
    font-size: 14px;
  }

  .empty-hint {
    margin: 4px 0 0 0;
    font-size: 13px;
  }

  /* ─── Config Layout (two-panel: sidebar + main) ─────── */

  .config-layout {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
    overflow: hidden;
  }

  .config-main {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* T050: Unsaved changes dialog (FR-024) */
  .unsaved-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .unsaved-dialog {
    background: #fff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
    padding: 20px 24px;
    width: 380px;
    max-width: 95vw;
  }

  .unsaved-title {
    margin: 0 0 10px;
    font-size: 0.95rem;
    font-weight: 600;
    color: #1f2937;
  }

  .unsaved-body {
    margin: 0 0 16px;
    font-size: 0.85rem;
    color: #6b7280;
    line-height: 1.5;
  }

  .unsaved-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .unsaved-btn {
    padding: 6px 14px;
    font-size: 0.82rem;
    font-weight: 500;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.15s;
  }

  .unsaved-btn-secondary {
    color: #374151;
    background: #fff;
    border-color: #d1d5db;
  }

  .unsaved-btn-secondary:hover {
    background: #f9fafb;
  }

  .unsaved-btn-danger {
    color: #fff;
    background: #dc2626;
    border-color: #dc2626;
  }

  .unsaved-btn-danger:hover {
    background: #b91c1c;
  }

  /* ─── Read Configuration CTA Panel ─────────────────── */

  .config-cta-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    height: 100%;
    padding: 48px 32px;
    text-align: center;
  }

  .cta-title {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: #1e293b;
  }

  .cta-desc {
    margin: 0;
    font-size: 14px;
    color: #64748b;
    max-width: 360px;
    line-height: 1.6;
  }

  .cta-btn {
    padding: 10px 24px;
    font-size: 14px;
    font-weight: 500;
    background: #2563eb;
    color: #fff;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .cta-btn:hover:not(:disabled) {
    background: #1d4ed8;
  }

  .cta-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .cta-badge {
    font-size: 12px;
    color: #94a3b8;
  }
</style>
