<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from 'svelte';
  import NodeList from '$lib/components/NodeList.svelte';
  import { discoverNodes as discoverNodesApi, querySnipBatch, refreshAllNodes } from '$lib/api/tauri';
  import type { DiscoveredNode } from '$lib/api/tauri';

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
  let showAdvanced = $state(false);

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
    } catch (e) {
      errorMessage = `Disconnect failed: ${e}`;
    }
  }

  async function discover() {
    errorMessage = "";
    
    // If we already have nodes, refresh them; otherwise discover new ones
    if (nodes.length > 0) {
      refreshing = true;
      try {
        const updated = await refreshAllNodes();
        nodes = updated;
      } catch (e) {
        console.error("Refresh failed:", e);
        errorMessage = `Refresh failed: ${e}`;
      } finally {
        refreshing = false;
      }
    } else {
      discovering = true;
      nodes = [];

      try {
        // Discover nodes
        const discovered = await discoverNodesApi(discoveryTimeout);
        nodes = discovered;
        
        // Query SNIP data for all discovered nodes
        if (discovered.length > 0) {
          queryingSnip = true;
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
      } catch (e) {
        console.error("Discovery failed:", e);
        errorMessage = `Discovery failed: ${e}`;
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

  function toggleAdvanced() {
    showAdvanced = !showAdvanced;
  }
</script>

<main>
  <h1>LCC Configuration Tool</h1>
  <p class="subtitle">Visual configuration tool for Layout Command Control (LCC/OpenLCB) networks</p>

  {#if !connected}
    <div class="card">
      <h2>Connection</h2>
      <form onsubmit={connect}>
        <label>
          Host: <input type="text" bind:value={host} disabled={connecting} />
        </label>
        <label>
          Port: <input type="number" bind:value={port} disabled={connecting} />
        </label>
        <button type="submit" disabled={connecting}>
          {connecting ? "Connecting..." : "Connect"}
        </button>
      </form>
      {#if errorMessage}
        <div class="error">{errorMessage}</div>
      {/if}
    </div>
  {:else}
    <div class="status-bar">
      <span class="connected">✓ Connected to {host}:{port}</span>
      <button class="disconnect-btn" onclick={disconnect}>Disconnect</button>
    </div>
  {/if}

  {#if connected}
    <div class="card discovery-card">
      <div class="discovery-toolbar">
        <button class="discover-btn" onclick={discover} disabled={discovering || queryingSnip || refreshing}>
          {discovering ? "Discovering..." : queryingSnip ? "Loading node info..." : refreshing ? "Refreshing..." : nodes.length > 0 ? "Refresh Nodes" : "Discover Nodes"}
        </button>
        {#if nodes.length > 0}
          <span class="node-count">{nodes.length} node{nodes.length === 1 ? '' : 's'} discovered</span>
        {/if}
        <button class="advanced-toggle" onclick={toggleAdvanced} title="Advanced options">
          ⚙️ {showAdvanced ? 'Hide' : ''} Advanced
        </button>
      </div>

      {#if showAdvanced}
        <div class="advanced-options">
          <label class="timeout-label">
            Discovery Timeout (ms): 
            <input class="timeout-input" type="number" bind:value={discoveryTimeout} min="50" max="1000" step="50" disabled={discovering} />
          </label>
        </div>
      {/if}

      {#if errorMessage}
        <div class="error">{errorMessage}</div>
      {/if}

      {#if nodes.length > 0}
        <div class="nodes-container">
          <NodeList nodes={nodes} isRefreshing={refreshing} />
        </div>
      {:else if discovering}
        <p class="status">Scanning network for nodes...</p>
      {:else if queryingSnip}
        <p class="status">Loading node information...</p>
      {:else}
        <p class="hint">Click "Discover Nodes" to scan the network</p>
      {/if}
    </div>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    min-height: 100vh;
  }

  main {
    max-width: 1200px;
    margin: 0 auto;
    padding: 1.5rem;
    color: #333;
  }

  h1 {
    color: white;
    text-align: center;
    font-size: 2rem;
    margin-bottom: 0.25rem;
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.2);
  }

  .subtitle {
    color: rgba(255, 255, 255, 0.9);
    text-align: center;
    margin-bottom: 1rem;
    font-size: 0.95rem;
  }

  .card {
    background: white;
    border-radius: 8px;
    padding: 1.25rem;
    margin-bottom: 1rem;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }

  .discovery-card {
    padding: 1rem;
  }

  h2 {
    margin-top: 0;
    color: #667eea;
    font-size: 1.25rem;
    margin-bottom: 1rem;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  label {
    display: flex;
    align-items: center;
    gap: 1rem;
    font-weight: 500;
  }

  input {
    flex: 1;
    padding: 0.75rem;
    border: 2px solid #e0e0e0;
    border-radius: 6px;
    font-size: 1rem;
  }

  input:focus {
    outline: none;
    border-color: #667eea;
  }

  button {
    padding: 0.6rem 1.25rem;
    border: none;
    border-radius: 6px;
    font-size: 0.95rem;
    font-weight: 500;
    cursor: pointer;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    transition: transform 0.2s;
  }

  button:hover:not(:disabled) {
    transform: translateY(-2px);
  }

  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .status-bar {
    background: white;
    border-radius: 6px;
    padding: 0.75rem 1.25rem;
    margin-bottom: 1rem;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .disconnect-btn {
    padding: 0.5rem 1rem;
    font-size: 0.9rem;
  }

  .connected {
    color: #10b981;
    font-weight: 500;
    font-size: 1.1rem;
  }

  .error {
    background: #fee2e2;
    border: 1px solid #fecaca;
    color: #dc2626;
    padding: 1rem;
    border-radius: 6px;
    margin-top: 1rem;
  }

  .nodes-container {
    margin-top: 0.75rem;
  }

  .discovery-toolbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .discover-btn {
    flex-shrink: 0;
  }

  .node-count {
    color: #667eea;
    font-weight: 500;
    font-size: 0.95rem;
  }

  .advanced-toggle {
    margin-left: auto;
    padding: 0.5rem 0.75rem;
    font-size: 0.85rem;
    background: #f3f4f6;
    color: #374151;
  }

  .advanced-toggle:hover:not(:disabled) {
    background: #e5e7eb;
  }

  .advanced-options {
    margin-top: 0.75rem;
    padding: 0.75rem;
    background: #f9fafb;
    border-radius: 6px;
    border: 1px solid #e5e7eb;
  }

  .timeout-label {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.9rem;
    color: #374151;
  }

  .timeout-input {
    width: 100px;
    padding: 0.4rem 0.6rem;
    font-size: 0.9rem;
  }

  .status {
    color: #667eea;
    font-style: italic;
    text-align: center;
    padding: 0.75rem;
    font-size: 0.9rem;
  }

  .hint {
    color: #999;
    font-style: italic;
    text-align: center;
    padding: 0.75rem;
    font-size: 0.9rem;
  }
</style>
