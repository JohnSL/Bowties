<script lang="ts">
  import { channelsStore } from '$lib/stores/channels.svelte';
  import { eventStateStore } from '$lib/stores/eventState.svelte';
  import { deriveChannelState, type OccupancyState } from '$lib/utils/channelState';
  import type { InformationChannel } from '$lib/api/channels';
  import ChannelRow from './ChannelRow.svelte';

  let {
    nodeName,
    resolvedEventIds,
    daughterboardName,
    usedBy,
  }: {
    nodeName: (nodeKey: string) => string;
    /** Map from channelId to { occupied, clear } event IDs. Supplied by parent/orchestrator. */
    resolvedEventIds?: ReadonlyMap<string, { occupied?: string; clear?: string }>;
    /**
     * Resolves the daughterboard display name for a (nodeKey, connector) pair —
     * used in connectorInput group headers (e.g. "TowerLCC-1 · Connector A · BOD-8").
     * Optional: when absent, the group header omits the daughter-board segment.
     */
    daughterboardName?: (nodeKey: string, connector: string) => string | undefined;
    /**
     * Resolves the facility-slot consumers of a channel for the "Used by"
     * column (Spec 018 / S4 — supplied by the route from
     * `effectiveLayoutStore.channelUsageMap`). Optional: when absent, every
     * row renders em-dash.
     */
    usedBy?: (channelId: string) => ReadonlyArray<{ facilityName: string; slotLabel: string }>;
  } = $props();

  /** Derive occupancy states for all channels from event store + resolved IDs. */
  let channelStates = $derived.by(() => {
    const states = new Map<string, OccupancyState>();
    if (!resolvedEventIds) return states;
    const events = eventStateStore.events;
    for (const [channelId, ids] of resolvedEventIds) {
      states.set(channelId, deriveChannelState(events, ids.occupied, ids.clear));
    }
    return states;
  });

  function handleRename(id: string, newName: string) {
    channelsStore.renameChannel(id, newName);
  }

  function formatConnectorLabel(connectorId: string): string {
    const match = connectorId.match(/^connector-([a-z])$/i);
    if (match) return `Connector ${match[1].toUpperCase()}`;
    return connectorId;
  }

  function groupLabel(groupKey: string, sample: InformationChannel): string {
    const node = nodeName(sample.binding.kind === 'connectorInput' ? sample.binding.nodeKey : sample.binding.nodeKey);
    if (sample.binding.kind === 'connectorInput') {
      const connector = formatConnectorLabel(sample.binding.connector);
      const board = daughterboardName?.(sample.binding.nodeKey, sample.binding.connector);
      return board ? `${node} · ${connector} · ${board}` : `${node} · ${connector}`;
    }
    if (sample.binding.kind === 'lampRow') {
      return `${node} · Direct Lamp Control`;
    }
    return groupKey;
  }
</script>

<section class="channels-panel" data-testid="channels-panel">
  <h3 class="section-heading">Channels</h3>
  {#if channelsStore.isEmpty}
    <div class="empty-state" data-testid="channels-empty-state">
      <p class="empty-title">No channels yet</p>
      <p class="empty-hint">Select a daughter board in the Config tab to create channels.</p>
    </div>
  {:else}
    <div class="channels-scroll">
      <table class="channels-table">
        <thead>
          <tr>
            <th aria-label="State"></th>
            <th>Name</th>
            <th>Role / Style</th>
            <th>Location</th>
            <th>State</th>
            <th>Used by</th>
          </tr>
        </thead>
        <tbody>
          {#each [...channelsStore.groupedByHardware.entries()] as [groupKey, channels] (groupKey)}
            <tr class="group-header">
              <td colspan="6">{groupLabel(groupKey, channels[0])}</td>
            </tr>
            {#each channels as channel (channel.id)}
              <ChannelRow
                {channel}
                occupancyState={channelStates.get(channel.id) ?? 'unknown'}
                usedBy={usedBy?.(channel.id)}
                onRename={handleRename}
              />
            {/each}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .channels-panel {
    margin-top: 1.25rem;
  }
  .section-heading {
    font-size: 0.95rem;
    font-weight: 600;
    margin: 0 0 0.5rem 0;
    color: var(--text-primary, #222);
  }
  .empty-state {
    padding: 2rem;
    text-align: center;
    color: var(--text-muted, #666);
    background: var(--bg-subtle, #fafafa);
    border: 1px dashed var(--border, #e2e2e2);
    border-radius: 5px;
  }
  .empty-title {
    font-size: 1rem;
    font-weight: 500;
    margin-bottom: 0.25rem;
  }
  .empty-hint {
    font-size: 0.85rem;
    margin: 0;
  }
  .channels-scroll {
    overflow-x: auto;
    border: 1px solid var(--border-subtle, #e2e2e2);
    border-radius: 5px;
  }
  .channels-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }
  .channels-table thead th {
    text-align: left;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary, #555);
    padding: 0.5rem 0.6rem;
    border-bottom: 1px solid var(--border, #d1d1d1);
    background: var(--bg-table-header, #f7f7f7);
  }
  .group-header td {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-secondary, #444);
    background: var(--bg-table-group, #f0f3f7);
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--border-subtle, #e2e2e2);
  }
</style>
