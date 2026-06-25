<script lang="ts">
  import { channelsStore } from '$lib/stores/channels.svelte';
  import ChannelGroup from './ChannelGroup.svelte';

  let {
    nodeName,
  }: {
    nodeName: (nodeKey: string) => string;
  } = $props();

  const CHANNEL_TYPE_LABELS: Record<string, string> = {
    'block-occupancy': 'Block Occupancy',
  };

  function labelForType(type: string): string {
    return CHANNEL_TYPE_LABELS[type] ?? type;
  }

  function handleRename(id: string, newName: string) {
    channelsStore.renameChannel(id, newName);
  }
</script>

<div class="railroad-panel" data-testid="railroad-panel">
  {#if channelsStore.isEmpty}
    <div class="empty-state" data-testid="railroad-empty-state">
      <p class="empty-title">No channels yet</p>
      <p class="empty-hint">Select a daughter board in the Config tab to create channels.</p>
    </div>
  {:else}
    <h2 class="panel-title">All Channels</h2>
    {#each [...channelsStore.grouped.entries()] as [type, channels] (type)}
      <ChannelGroup typeName={labelForType(type)} {channels} {nodeName} onRename={handleRename} />
    {/each}
  {/if}
</div>

<style>
  .railroad-panel {
    padding: 1.25rem;
    overflow-y: auto;
    height: 100%;
  }
  .panel-title {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 1rem 0;
    color: var(--text-primary, #222);
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem;
    text-align: center;
    color: var(--text-muted, #666);
  }
  .empty-title {
    font-size: 1.1rem;
    font-weight: 500;
    margin-bottom: 0.5rem;
  }
  .empty-hint {
    font-size: 0.9rem;
  }
</style>
