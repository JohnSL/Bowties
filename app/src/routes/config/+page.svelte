<script lang="ts">
  import ConfigSidebar from '$lib/components/ConfigSidebar/ConfigSidebar.svelte';
  import SegmentView from '$lib/components/ElementCardDeck/SegmentView.svelte';
  import { configSidebarStore } from '$lib/stores/configSidebar';
  import { nodeInfoStore, updateNodeInfo } from '$lib/stores/nodeInfo';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, onDestroy } from 'svelte';

  const pageTitle = 'Configuration';

  let unlisten: (() => void) | null = null;

  /** Refresh node list and reset sidebar state (FR-018) */
  async function handleRefreshNodes() {
    configSidebarStore.reset();
    nodeTreeStore.reset();
    nodeInfoStore.set(new Map());
    try {
      const nodes = await invoke<any[]>('refresh_all_nodes', { timeout_ms: 3000 });
      updateNodeInfo(nodes ?? []);
    } catch (err) {
      console.error('[Config] Failed to refresh nodes:', err);
    }
  }

  onMount(async () => {
    // Spec 007: start listening for incremental tree updates
    await nodeTreeStore.startListening();

    // Populate nodeInfoStore from current backend state
    try {
      const nodes = await invoke<any[]>('discover_nodes', { timeout_ms: 100 });
      updateNodeInfo(nodes ?? []);
    } catch (_) {
      // No nodes yet — expected; store remains empty
    }

    // FR-018: clear sidebar when menu-triggered refresh fires
    unlisten = await listen('menu-refresh', () => {
      handleRefreshNodes();
    });
  });

  onDestroy(() => {
    unlisten?.();
    nodeTreeStore.stopListening();
  });

</script>

<svelte:head>
  <title>{pageTitle} - LCC Bowties</title>
</svelte:head>

<!-- FR-001: fixed-width sidebar + scrollable main area -->
<div class="config-page">
  <ConfigSidebar />

  <main class="main-content">
    <SegmentView />
  </main>
</div>

<style>
  .config-page {
    display: flex;
    flex-direction: row;
    height: 100vh;
    width: 100%;
    overflow: hidden;
  }

  /* Main content takes the remaining width (FR-001) */
  .main-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background-color: var(--main-bg, #f8f9fa);
  }

</style>
