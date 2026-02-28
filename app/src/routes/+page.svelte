<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { WebviewWindow, getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import ConfigSidebar from '$lib/components/ConfigSidebar/ConfigSidebar.svelte';
  import SegmentView from '$lib/components/ElementCardDeck/SegmentView.svelte';
  import CdiXmlViewer from '$lib/components/CdiXmlViewer.svelte';
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { discoverNodes as discoverNodesApi, querySnipBatch, refreshAllNodes } from '$lib/api/tauri';
  import { readAllConfigValues, cancelConfigReading, getCdiXml, downloadCdi } from '$lib/api/cdi';
  import { getCdiErrorMessage, isCdiError } from '$lib/types/cdi';
  import type { ViewerStatus } from '$lib/types/cdi';
  import type { DiscoveredNode } from '$lib/api/tauri';
  import type { ReadProgressState } from '$lib/api/types';
  import { millerColumnsStore } from '$lib/stores/millerColumns';
  import { updateNodeInfo } from '$lib/stores/nodeInfo';
  import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { configReadNodesStore, markNodeConfigRead, clearConfigReadStatus } from '$lib/stores/configReadStatus';
  import BowtieCatalogPanel from '$lib/components/Bowtie/BowtieCatalogPanel.svelte';
  import DiscoveryProgressModal from '$lib/components/DiscoveryProgressModal.svelte';

  // Active tab state — 'config' (default) or 'bowties'
  let activeTab = $state<'config' | 'bowties'>('config');

  // Connection state
  let host = $state("localhost");
  let port = $state(12021);
  let connected = $state(false);
  let connecting = $state(false);
  let errorMessage = $state("");

  // Discovery state
  let nodes = $state<DiscoveredNode[]>([]);
  let discovering = $state(false);
  let discoveryTimeout = $state(250);
  let queryingSnip = $state(false);
  let refreshing = $state(false);
  let showDiscoveryOptions = $state(false);

  // Config reading progress state (T063-T067)
  let readProgress = $state<ReadProgressState | null>(null);
  let isCancelling = $state(false);

  // Discovery progress modal state
  let discoveryModalVisible = $state(false);
  let discoveryPhase = $state<'discovering' | 'querying' | 'refreshing' | 'reading' | 'complete' | 'cancelled'>('discovering');

  // Track whether a single-node or batch "read remaining" is in progress
  let readingRemaining = $state(false);

  // CDI XML viewer state
  let viewerVisible = $state(false);
  let viewerNodeId = $state<string | null>(null);
  let viewerXmlContent = $state<string | null>(null);
  let viewerStatus = $state<ViewerStatus>('idle');
  let viewerErrorMessage = $state<string | null>(null);

  // Check connection status on mount
  onMount(async () => {
    try {
      const status = await invoke("get_connection_status");
      connected = (status as any).connected;
      host = (status as any).host;
      port = (status as any).port;
    } catch (e) {
      console.error("Failed to get connection status:", e);
    }

    // Feature 006: Start bowties store listener so cdi-read-complete is captured
    // regardless of whether the user has visited the Bowties page.
    bowtieCatalogStore.startListening();

    // Spec 007: Start node-tree-updated listener so trees are refreshed
    // automatically as config values and event roles are merged server-side.
    nodeTreeStore.startListening();

    const unlistens: Array<() => void> = [];

    // T063: Setup config-read-progress event listener
    unlistens.push(await listen<ReadProgressState>('config-read-progress', (event) => {
      readProgress = event.payload;
      millerColumnsStore.setReadProgress(event.payload);
      discoveryPhase = 'reading';
      
      // Clear progress on completion or cancellation
      if (event.payload.status.type === 'Complete' || event.payload.status.type === 'Cancelled') {
        discoveryPhase = event.payload.status.type === 'Cancelled' ? 'cancelled' : 'complete';
        setTimeout(() => {
          readProgress = null;
          millerColumnsStore.setReadProgress(null);
          isCancelling = false;
          millerColumnsStore.setCancelling(false);
          discoveryModalVisible = false;
        }, 500); // Brief pause so user sees the final status
      }
    }));

    // Native menu event listeners — relay OS menu clicks to handler functions
    unlistens.push(await listen('menu-disconnect',     () => disconnect()));
    unlistens.push(await listen('menu-refresh',        () => { if (connected) discover(); }));
    unlistens.push(await listen('menu-traffic',        () => { if (connected) openTrafficMonitor(); }));
    unlistens.push(await listen('menu-view-cdi',       () => {
      const nodeId = get(configSidebarStore).selectedSegment?.nodeId;
      if (nodeId) openCdiViewer(nodeId, false);
    }));
    unlistens.push(await listen('menu-redownload-cdi', () => {
      const nodeId = get(configSidebarStore).selectedSegment?.nodeId;
      if (nodeId) openCdiViewer(nodeId, true);
    }));
    unlistens.push(await listen('menu-discovery-opts', () => { showDiscoveryOptions = !showDiscoveryOptions; }));

    // Cleanup all listeners on component unmount
    return () => {
      unlistens.forEach(u => u());
    };
  });

  async function connect(event: Event) {
    event.preventDefault();
    errorMessage = "";
    connecting = true;

    try {
      await invoke("connect_lcc", { host, port });
      connected = true;
    } catch (e) {
      errorMessage = `Connection failed: ${e}`;
      connected = false;
    } finally {
      connecting = false;
    }
  }

  async function disconnect() {
    errorMessage = "";
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

  async function discover() {
    errorMessage = "";
    discoveryModalVisible = true;
    
    // If we already have nodes, refresh them; otherwise discover new ones
    if (nodes.length > 0) {
      refreshing = true;
      discoveryPhase = 'refreshing';
      try {
        // T068: Clear config cache before refresh
        millerColumnsStore.clearConfigValues();
        // FR-018: reset sidebar state on node refresh
        configSidebarStore.reset();
        clearConfigReadStatus();
        
        const updated = await refreshAllNodes();
        nodes = updated;
        updateNodeInfo(nodes);

        // T064: Read all config values for nodes with SNIP data (likely to have CDI)
        // The backend will automatically load CDI from cache if available
        const cdiCandidatesRefresh = nodes.filter(n => n.snip_data !== null);
        let totalSuccessfulRefresh = 0;
        let totalFailedRefresh = 0;
        for (let nodeIdx = 0; nodeIdx < cdiCandidatesRefresh.length; nodeIdx++) {
          const node = cdiCandidatesRefresh[nodeIdx];
          try {
            // Format node_id from array to dotted hex string
            const nodeId = formatNodeId(node.node_id);
            const nodeName = node.snip_data?.user_name || nodeId;
            
            let hasCdi = false;
            try {
              const cdiCheck = await getCdiXml(nodeId);
              hasCdi = cdiCheck.xmlContent !== null;
            } catch {
              // CdiNotRetrieved or similar — CDI not available yet
            }

            if (!hasCdi) {
              console.log(`Skipping config read for ${nodeName} — CDI not yet downloaded`);
              continue;
            }

            console.log(`Reading config values from ${nodeName}...`);
            const response = await readAllConfigValues(nodeId, undefined, nodeIdx, cdiCandidatesRefresh.length);
            
            // Update store with batch values
            millerColumnsStore.setConfigValues(response.values);
            markNodeConfigRead(nodeId);
            totalSuccessfulRefresh += response.successfulReads;
            totalFailedRefresh += response.failedReads;
            
            console.log(`✓ Read ${response.successfulReads} of ${response.totalElements} config values from ${nodeName}`);
            if (response.failedReads > 0) {
              console.warn(`  ${response.failedReads} values failed to read`);
            }
          } catch (e) {
            const nodeName = node.snip_data?.user_name || 'unknown';
            console.warn(`Failed to read config values from node ${nodeName}:`, e);
            totalFailedRefresh++;
            // Continue with next node - don't fail entire refresh
          }
        }
        // Emit synthetic Complete so the progress strip auto-dismisses
        if (cdiCandidatesRefresh.length > 0) {
          const doneState: ReadProgressState = {
            totalNodes: cdiCandidatesRefresh.length,
            currentNodeIndex: cdiCandidatesRefresh.length - 1,
            currentNodeName: '',
            currentNodeId: '',
            totalElements: 0,
            elementsRead: totalSuccessfulRefresh,
            elementsFailed: totalFailedRefresh,
            percentage: 100,
            status: { type: 'Complete', success_count: totalSuccessfulRefresh, fail_count: totalFailedRefresh }
          };
          readProgress = doneState;
          millerColumnsStore.setReadProgress(doneState);
          discoveryPhase = 'complete';
          setTimeout(() => {
            readProgress = null;
            millerColumnsStore.setReadProgress(null);
            isCancelling = false;
            millerColumnsStore.setCancelling(false);
            discoveryModalVisible = false;
          }, 1500);
        } else {
          discoveryModalVisible = false;
        }
      } catch (e) {
        console.error("Refresh failed:", e);
        errorMessage = `Refresh failed: ${e}`;
        discoveryModalVisible = false;
      } finally {
        refreshing = false;
      }
    } else {
      discovering = true;
      discoveryPhase = 'discovering';
      nodes = [];

      try {
        // Discover nodes
        const discovered = await discoverNodesApi(discoveryTimeout);
        nodes = discovered;
        
        // Query SNIP data for all discovered nodes
        if (discovered.length > 0) {
          queryingSnip = true;
          discoveryPhase = 'querying';
          const aliases = discovered.map(n => n.alias);
          
          try {
            const results = await querySnipBatch(aliases);
            
            // Update each node with its SNIP data
            nodes = nodes.map(node => {
              const result = results.find(r => r.alias === node.alias);
              if (result) {
                return {
                  ...node,
                  snip_data: result.snip_data,
                  snip_status: result.status
                };
              }
              return node;
            });
          } catch (e) {
            console.error("Failed to query SNIP data:", e);
            errorMessage = `Failed to retrieve node information: ${e}`;
          } finally {
            queryingSnip = false;
          }
        }
        
        // Populate nodeInfo store for tooltips and display names
        updateNodeInfo(nodes);

        // T064: Read all config values for nodes with SNIP data (likely to have CDI)
        // The backend will automatically load CDI from cache if available
        const cdiCandidates = nodes.filter(n => n.snip_data !== null);
        let totalSuccessful = 0;
        let totalFailed = 0;
        for (let nodeIdx = 0; nodeIdx < cdiCandidates.length; nodeIdx++) {
          const node = cdiCandidates[nodeIdx];
          try {
            // Format node_id from array to dotted hex string
            const nodeId = formatNodeId(node.node_id);
            const nodeName = node.snip_data?.user_name || nodeId;
            
            let hasCdi = false;
            try {
              const cdiCheck = await getCdiXml(nodeId);
              hasCdi = cdiCheck.xmlContent !== null;
            } catch {
              // CdiNotRetrieved or similar — CDI not available yet
            }

            if (!hasCdi) {
              console.log(`Skipping config read for ${nodeName} — CDI not yet downloaded`);
              continue;
            }

            console.log(`Reading config values from ${nodeName}...`);
            const response = await readAllConfigValues(nodeId, undefined, nodeIdx, cdiCandidates.length);
            
            // Update store with batch values
            millerColumnsStore.setConfigValues(response.values);
            markNodeConfigRead(nodeId);
            totalSuccessful += response.successfulReads;
            totalFailed += response.failedReads;
            
            console.log(`✓ Read ${response.successfulReads} of ${response.totalElements} config values from ${nodeName}`);
            if (response.failedReads > 0) {
              console.warn(`  ${response.failedReads} values failed to read`);
            }
          } catch (e) {
            const nodeName = node.snip_data?.user_name || 'unknown';
            console.warn(`Failed to read config values from node ${nodeName}:`, e);
            totalFailed++;
            // Continue with next node - don't fail entire discovery
          }
        }
        // Emit synthetic Complete so the progress strip auto-dismisses
        if (cdiCandidates.length > 0) {
          const doneState: ReadProgressState = {
            totalNodes: cdiCandidates.length,
            currentNodeIndex: cdiCandidates.length - 1,
            currentNodeName: '',
            currentNodeId: '',
            totalElements: 0,
            elementsRead: totalSuccessful,
            elementsFailed: totalFailed,
            percentage: 100,
            status: { type: 'Complete', success_count: totalSuccessful, fail_count: totalFailed }
          };
          readProgress = doneState;
          millerColumnsStore.setReadProgress(doneState);
          discoveryPhase = 'complete';
          setTimeout(() => {
            readProgress = null;
            millerColumnsStore.setReadProgress(null);
            isCancelling = false;
            millerColumnsStore.setCancelling(false);
            discoveryModalVisible = false;
          }, 1500);
        } else {
          discoveryModalVisible = false;
        }
      } catch (e) {
        console.error("Discovery failed:", e);
        errorMessage = `Discovery failed: ${e}`;
        discoveryModalVisible = false;
      } finally {
        discovering = false;
      }
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
    millerColumnsStore.setCancelling(true);
    
    try {
      await cancelConfigReading();
      console.log('Config reading cancellation requested');
    } catch (e) {
      console.error('Failed to cancel config reading:', e);
      errorMessage = `Cancel failed: ${e}`;
      isCancelling = false;
      millerColumnsStore.setCancelling(false);
    }
  }

  // ─── CDI XML Viewer ───────────────────────────────────────────────────────

  /** Read config values for all unread nodes (batch) */
  async function readRemainingNodes() {
    const readNodes = get(configReadNodesStore);
    const unread = nodes.filter(n => {
      if (!n.snip_data) return false;
      const nodeId = formatNodeId(n.node_id);
      return !readNodes.has(nodeId);
    });
    if (unread.length === 0) return;

    readingRemaining = true;
    discoveryModalVisible = true;
    discoveryPhase = 'reading';
    errorMessage = '';

    let totalSuccessful = 0;
    let totalFailed = 0;
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
          } catch { /* CDI not available */ }
          if (!hasCdi) {
            console.log(`Skipping config read for ${nodeName} — CDI not yet downloaded`);
            continue;
          }
          console.log(`Reading config values from ${nodeName}...`);
          const response = await readAllConfigValues(nodeId, undefined, nodeIdx, unread.length);
          millerColumnsStore.setConfigValues(response.values);
          markNodeConfigRead(nodeId);
          // Force tree refresh so any visible SegmentView updates
          await nodeTreeStore.refreshTree(nodeId);
          totalSuccessful += response.successfulReads;
          totalFailed += response.failedReads;
        } catch (e) {
          console.warn(`Failed to read config values from ${nodeName}:`, e);
          totalFailed++;
        }
      }
      // Synthetic completion
      const doneState: ReadProgressState = {
        totalNodes: unread.length,
        currentNodeIndex: unread.length - 1,
        currentNodeName: '',
        currentNodeId: '',
        totalElements: 0,
        elementsRead: totalSuccessful,
        elementsFailed: totalFailed,
        percentage: 100,
        status: { type: 'Complete', success_count: totalSuccessful, fail_count: totalFailed }
      };
      readProgress = doneState;
      millerColumnsStore.setReadProgress(doneState);
      discoveryPhase = 'complete';
      setTimeout(() => {
        readProgress = null;
        millerColumnsStore.setReadProgress(null);
        isCancelling = false;
        millerColumnsStore.setCancelling(false);
        discoveryModalVisible = false;
      }, 1500);
    } catch (e) {
      errorMessage = `Read remaining failed: ${e}`;
      discoveryModalVisible = false;
    } finally {
      readingRemaining = false;
    }
  }

  /** Read config values for a single node (triggered from sidebar indicator) */
  async function readSingleNodeConfig(nodeId: string) {
    const node = nodes.find(n => formatNodeId(n.node_id) === nodeId);
    if (!node?.snip_data) return;

    readingRemaining = true;
    discoveryModalVisible = true;
    discoveryPhase = 'reading';
    errorMessage = '';

    const nodeName = node.snip_data.user_name || nodeId;
    try {
      let hasCdi = false;
      try {
        const cdiCheck = await getCdiXml(nodeId);
        hasCdi = cdiCheck.xmlContent !== null;
      } catch { /* CDI not available */ }
      if (!hasCdi) {
        errorMessage = `CDI not available for ${nodeName}`;
        discoveryModalVisible = false;
        readingRemaining = false;
        return;
      }
      const response = await readAllConfigValues(nodeId, undefined, 0, 1);
      millerColumnsStore.setConfigValues(response.values);
      markNodeConfigRead(nodeId);
      // Force tree refresh so the currently visible SegmentView updates
      await nodeTreeStore.refreshTree(nodeId);
      // Synthetic completion
      const doneState: ReadProgressState = {
        totalNodes: 1,
        currentNodeIndex: 0,
        currentNodeName: nodeName,
        currentNodeId: nodeId,
        totalElements: response.totalElements,
        elementsRead: response.successfulReads,
        elementsFailed: response.failedReads,
        percentage: 100,
        status: { type: 'Complete', success_count: response.successfulReads, fail_count: response.failedReads }
      };
      readProgress = doneState;
      millerColumnsStore.setReadProgress(doneState);
      discoveryPhase = 'complete';
      setTimeout(() => {
        readProgress = null;
        millerColumnsStore.setReadProgress(null);
        isCancelling = false;
        millerColumnsStore.setCancelling(false);
        discoveryModalVisible = false;
      }, 1500);
    } catch (e) {
      errorMessage = `Failed to read config for ${nodeName}: ${e}`;
      discoveryModalVisible = false;
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

  // Sync native menu item enable/disable state with current app state.
  // Tauri v2 has no "menu will open" event, so we push state eagerly whenever
  // any of the tracked reactive values change.
  async function syncMenuState(conn: boolean, busy: boolean, sel: boolean) {
    try {
      await invoke("update_menu_state", { connected: conn, isBusy: busy, hasSelection: sel });
    } catch (e) {
      console.warn("Failed to update menu state:", e);
    }
  }

  $effect(() => {
    const conn = connected;
    const busy = discovering || queryingSnip || refreshing || readingRemaining;
    const sel  = !!$configSidebarStore.selectedSegment;
    syncMenuState(conn, busy, sel);
  });
</script>


<div class="app-shell">

  <!-- ═══ TOOLBAR (connected only) ═══ -->
  {#if connected}
    <div class="toolbar" role="toolbar" aria-label="Main toolbar">
      <div class="toolbar-left">
        <button
          class="toolbar-btn"
          onclick={discover}
          disabled={discovering || queryingSnip || refreshing || readingRemaining}
          title={nodes.length > 0 ? 'Refresh nodes on the network' : 'Discover nodes on the network'}
        >
          <span class="tb-icon" class:tb-spin={discovering || refreshing}>⟳</span>
          <span>{discovering ? 'Discovering…' : queryingSnip ? 'Querying…' : refreshing ? 'Refreshing…' : nodes.length > 0 ? 'Refresh Nodes' : 'Discover Nodes'}</span>
        </button>
        <span class="toolbar-sep" aria-hidden="true"></span>
        <button
          class="toolbar-btn"
          onclick={readRemainingNodes}
          disabled={discovering || queryingSnip || refreshing || readingRemaining || nodes.length === 0}
          title="Read configuration values for nodes not yet read"
        >
          <span class="tb-icon" class:tb-spin={readingRemaining}>⟳</span>
          <span>{readingRemaining ? 'Reading…' : 'Read Remaining'}</span>
        </button>
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
        </button>
      </div>
      <div class="toolbar-right">
        <div class="toolbar-status" aria-live="polite">
          <span class="status-dot status-connected" title="Connected to {host}:{port}"></span>
          <span class="status-text">{host}:{port}</span>
        </div>
        <button class="btn-danger tb-disconnect-btn" onclick={disconnect}>Disconnect</button>
      </div>
    </div>
  {/if}

  <!-- ═══ DISCOVERY PROGRESS MODAL ═══ -->
  <DiscoveryProgressModal
    visible={discoveryModalVisible}
    phase={discoveryPhase}
    {readProgress}
    {isCancelling}
    onCancel={handleCancelConfigReading}
  />

  <!-- ═══ ERROR BANNER ═══ -->
  {#if errorMessage}
    <div class="error-banner" role="alert">
      <span class="error-banner-text">⚠ {errorMessage}</span>
      <button class="error-banner-close" onclick={() => errorMessage = ''} aria-label="Dismiss error">✕</button>
    </div>
  {/if}

  <!-- ═══ DISCOVERY OPTIONS BAR ═══ -->
  {#if showDiscoveryOptions}
    <div class="discovery-options-bar">
      <label class="dob-label">
        Discovery Timeout (ms):
        <input class="dob-input" type="number" bind:value={discoveryTimeout} min="50" max="1000" step="50" disabled={discovering} />
      </label>
      <button class="btn-secondary !text-xs !px-2 !py-1" onclick={() => showDiscoveryOptions = false}>Close</button>
    </div>
  {/if}

  <!-- ═══ MAIN CONTENT ═══ -->
  <div class="main-content">
    {#if !connected}
      <div class="connect-area">
        <div class="connect-card">
          <h2>Connect to LCC Network</h2>
          <form onsubmit={connect}>
            <label>Host: <input type="text" bind:value={host} disabled={connecting} /></label>
            <label>Port: <input type="number" bind:value={port} disabled={connecting} /></label>
            <button type="submit" class="btn-primary" disabled={connecting}>
              {connecting ? "Connecting..." : "Connect"}
            </button>
          </form>
        </div>
      </div>

    {:else if nodes.length === 0}
      <div class="empty-area">
        {#if discovering}
          <p class="empty-status">Scanning network for nodes…</p>
        {:else if queryingSnip}
          <p class="empty-status">Retrieving node information…</p>
        {:else}
          <p class="empty-status">No nodes found.</p>
          <p class="empty-hint">Click <strong>Discover Nodes</strong> in the toolbar or View menu.</p>
        {/if}
      </div>

    {:else if activeTab === 'bowties'}
      <!-- Feature 006: Bowties catalog in-page tab (no navigation) -->
      <BowtieCatalogPanel />

    {:else}
      <!-- FR-001: two-panel layout — fixed sidebar + scrollable main area -->
      <div class="config-layout">
        <ConfigSidebar on:readNodeConfig={(e) => readSingleNodeConfig(e.detail.nodeId)} />
        <div class="config-main">
          <SegmentView />
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

  .toolbar-status {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    font-size: 12px;
    color: #555;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .status-connected    { background: #10b981; }
  .status-connecting   { background: #f59e0b; animation: status-pulse 1.2s infinite; }
  .status-disconnected { background: #9ca3af; }

  @keyframes status-pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.4; }
  }

  /* ─── Toolbar ───────────────────────────────────────── */

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #fff;
    border-bottom: 1px solid #e5e7eb;
    padding: 0 8px;
    height: 40px;
    flex-shrink: 0;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 4px;
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
    background: none;
    border: 1px solid transparent;
    border-radius: 4px;
    font-size: 13px;
    color: #374151;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, border-color 0.12s;
  }

  .toolbar-btn:hover:not(:disabled) {
    background: #f0f4ff;
    border-color: #c7d2fe;
  }

  .toolbar-btn:disabled {
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
    background: #e5e7eb;
    margin: 0 4px;
  }

  .tb-disconnect-btn {
    padding: 4px 12px !important;
    font-size: 12px !important;
  }

  /* Active (pressed) state for toggle toolbar buttons */
  .toolbar-btn-active {
    background: #eff6ff;
    border-color: #6366f1 !important;
    color: #4338ca !important;
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

  /* ─── Discovery Options Bar ─────────────────────────── */

  .discovery-options-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 12px;
    background: #f9fafb;
    border-bottom: 1px solid #e5e7eb;
    flex-shrink: 0;
    font-size: 13px;
  }

  .dob-label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #374151;
  }

  .dob-input {
    width: 90px;
    padding: 4px 8px;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    font-size: 13px;
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

  .connect-card {
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    padding: 2rem;
    width: 340px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.08);
  }

  .connect-card h2 {
    margin-top: 0;
    margin-bottom: 1.5rem;
    color: #2563eb;
    font-size: 1.1rem;
  }

  .connect-card form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .connect-card label {
    display: flex;
    align-items: center;
    gap: 1rem;
    font-size: 14px;
    font-weight: 500;
    color: #374151;
  }

  .connect-card input {
    flex: 1;
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 14px;
  }

  .connect-card input:focus {
    outline: none;
    border-color: #2563eb;
    box-shadow: 0 0 0 3px rgba(37,99,235,0.12);
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

  .main-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary, #999);
    font-size: 14px;
  }
</style>
