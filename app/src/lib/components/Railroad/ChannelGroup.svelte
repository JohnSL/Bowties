<script lang="ts">
  import type { InformationChannel } from '$lib/api/channels';
  import type { OccupancyState } from '$lib/utils/channelState';
  import ChannelCard from './ChannelCard.svelte';

  let {
    typeName,
    channels,
    nodeName,
    channelStates,
    onRename,
  }: {
    typeName: string;
    channels: InformationChannel[];
    nodeName: (nodeKey: string) => string;
    channelStates?: ReadonlyMap<string, OccupancyState>;
    onRename?: (id: string, newName: string) => void;
  } = $props();
</script>

<section class="channel-group">
  <h3 class="group-header">
    <span class="group-dot"></span>
    <span class="group-name">{typeName}</span>
    <span class="group-count">({channels.length} channel{channels.length === 1 ? '' : 's'})</span>
  </h3>
  <div class="group-grid" role="list">
    {#each channels as channel (channel.id)}
      <ChannelCard {channel} {nodeName} occupancyState={channelStates?.get(channel.id) ?? 'unknown'} {onRename} />
    {/each}
  </div>
</section>

<style>
  .channel-group {
    margin-bottom: 1.5rem;
  }
  .group-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0 0 0.5rem 0;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary, #555);
  }
  .group-dot {
    width: 0.65rem;
    height: 0.65rem;
    border-radius: 50%;
    background: var(--channel-group-dot-bg, #a7f3d0);
    border: 1px solid var(--channel-group-dot-border, #34d399);
    flex-shrink: 0;
  }
  .group-name {
    font-weight: 600;
  }
  .group-count {
    font-weight: 400;
    color: var(--text-muted, #888);
  }
  .group-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.5rem;
  }

  @media (max-width: 640px) {
    .group-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
