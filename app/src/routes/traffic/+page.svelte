<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import TrafficMonitor from '$lib/components/TrafficMonitor.svelte';

  let isConnected = $state(false);

  onMount(async () => {
    try {
      const status = await invoke('get_connection_status');
      isConnected = (status as any).connected;
    } catch (e) {
      console.error('Traffic window: failed to get connection status:', e);
    }
  });
</script>

<div class="h-screen overflow-hidden">
  <TrafficMonitor {isConnected} standalone />
</div>
