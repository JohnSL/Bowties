<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from 'svelte';

  // Connection state
  let host = $state("localhost");
  let port = $state(12021);
  let connected = $state(false);
  let connecting = $state(false);
  let errorMessage = $state("");

  // Discovery state
  let nodes = $state<any[]>([]);
  let discovering = $state(false);
  let discoveryTimeout = $state(250);

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

  async function discoverNodes() {
    errorMessage = "";
    discovering = true;
    nodes = [];

    try {
      const discovered = await invoke("discover_nodes", { timeoutMs: discoveryTimeout });
      nodes = discovered as any[];
    } catch (e) {
      errorMessage = `Discovery failed: ${e}`;
    } finally {
      discovering = false;
    }
  }

  function formatNodeId(nodeId: number[]): string {
    return nodeId.map(b => b.toString(16).toUpperCase().padStart(2, '0')).join('.');
  }

  function formatAlias(alias: number): string {
    return '0x' + alias.toString(16).toUpperCase().padStart(3, '0');
  }
</script>

<main>
  <h1>LCC Configuration Tool</h1>
  <p class="subtitle">Visual configuration tool for Layout Command Control (LCC/OpenLCB) networks</p>

  <div class="card">
    <h2>Connection</h2>
    
    {#if !connected}
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
    {:else}
      <div  class="connection-info">
        <p class="connected">✓ Connected to {host}:{port}</p>
        <button onclick={disconnect}>Disconnect</button>
      </div>
    {/if}

    {#if errorMessage}
      <div class="error">{errorMessage}</div>
    {/if}
  </div>

  {#if connected}
    <div class="card">
      <h2>Node Discovery</h2>
      
      <label>
        Timeout (ms): <input type="number" bind:value={discoveryTimeout} min="50" max="1000" step="50" disabled={discovering} />
      </label>
      <button onclick={discoverNodes} disabled={discovering}>
        {discovering ? "Discovering..." : "Discover Nodes"}
      </button>

      {#if nodes.length > 0}
        <table>
          <thead>
            <tr><th>#</th><th>Node ID</th><th>Alias</th></tr>
          </thead>
          <tbody>
            {#each nodes as node, index}
              <tr>
                <td>{index + 1}</td>
                <td class="mono">{formatNodeId(node.node_id)}</td>
                <td class="mono">{formatAlias(node.alias)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="count">Found {nodes.length} node{nodes.length !== 1 ? 's' : ''}</p>
      {:else if discovering}
        <p class="status">Scanning network for nodes...</p>
      {:else}
        <p class="hint">No nodes discovered yet. Click "Discover Nodes" to scan the network.</p>
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
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
    color: #333;
  }

  h1 {
    color: white;
    text-align: center;
    font-size: 2.5rem;
    margin-bottom: 0.5rem;
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.2);
  }

  .subtitle {
    color: rgba(255, 255, 255, 0.9);
    text-align: center;
    margin-bottom: 2rem;
  }

  .card {
    background: white;
    border-radius: 12px;
    padding: 2rem;
    margin-bottom: 1.5rem;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.2);
  }

  h2 {
    margin-top: 0;
    color: #667eea;
    font-size: 1.5rem;
    margin-bottom: 1.5rem;
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
    padding: 0.75rem 1.5rem;
    border: none;
    border-radius: 6px;
    font-size: 1rem;
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

  .connection-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
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

  table {
    width: 100%;
    margin-top: 1.5rem;
    border-collapse: collapse;
  }

  th {
    text-align: left;
    padding: 0.75rem;
    background: #f9fafb;
    font-weight: 600;
    border-bottom: 2px solid #e5e7eb;
  }

  td {
    padding: 0.75rem;
    border-bottom: 1px solid #e5e7eb;
  }

  tbody tr:hover {
    background: #f9fafb;
  }

  .mono {
    font-family: 'Courier New', monospace;
    color: #667eea;
  }

  .count {
    margin-top: 1rem;
    text-align: center;
    font-weight: 500;
  }

  .status {
    color: #667eea;
    font-style: italic;
    text-align: center;
    padding: 1rem;
  }

  .hint {
    color: #999;
    font-style: italic;
    text-align: center;
    padding: 1rem;
  }
</style>
