<script lang="ts">
  /**
   * SelectChannelPicker — modal for binding a channel into a facility
   * slot (Spec 018 / S4). Mockup §5.
   *
   * Modelled on `AddFacilityDialog` (Dialog shell + native <form> for
   * Enter-to-submit). Lists unbound role-compatible channels; Confirm
   * enables as soon as a row is selected. Rebind was retired in
   * S6 D4 — changing a slot's channel is now a two-step Remove +
   * Select flow.
   */
  import type { InformationChannel } from '$lib/api/channels';
  import {
    channelStateClass,
    channelStateLabel,
    type ChannelState,
  } from '$lib/utils/channelState';
  import SearchableSingleSelectDialog from '$lib/components/SingleSelectList/SearchableSingleSelectDialog.svelte';

  let {
    slotLabel,
    requiredRole: _requiredRole,
    candidateChannels,
    channelState,
    onConfirm,
    onCancel,
  }: {
    slotLabel: string;
    /**
     * Role the slot requires. Present for caller documentation /
     * future filter hooks; the candidate list is already pre-filtered
     * by the route via `effectiveLayoutStore.unboundChannelsForRole`.
     */
    requiredRole: string;
    candidateChannels: InformationChannel[];
    channelState: (channelId: string) => ChannelState;
    onConfirm: (channelId: string) => void;
    onCancel: () => void;
  } = $props();

  const dialogTitle = $derived(`Select channel for '${slotLabel}'`);

  const listItems = $derived(candidateChannels.map((ch) => ({ key: ch.id })));

  function describeLocation(ch: InformationChannel): string {
    if (ch.binding.kind === 'connectorInput') {
      const match = ch.binding.connector.match(/^connector-([a-z])$/i);
      const connector = match ? `Connector ${match[1].toUpperCase()}` : ch.binding.connector;
      return `${connector} · Input ${ch.binding.input}`;
    }
    if (ch.binding.kind === 'lampRow') {
      return `Row ${ch.binding.rowOrdinal}`;
    }
    return '';
  }

  function stateLabel(state: ChannelState): string {
    return channelStateLabel(state);
  }

</script>

<SearchableSingleSelectDialog
  title={dialogTitle}
  filterLabel="Filter channels"
  items={listItems}
  searchTextFor={(id) => {
    const channel = candidateChannels.find((candidate) => candidate.id === id);
    return channel ? `${channel.name} ${describeLocation(channel)}` : id;
  }}
  ariaLabel="Channel candidates"
  name="select-channel"
  onConfirm={onConfirm}
  {onCancel}
>
  {#snippet row(key)}
        {@const ch = candidateChannels.find((c) => c.id === key)}
        {#if ch}
          {@const state = channelState(ch.id)}
          {@const stateClass = channelStateClass(state)}
          <span
            class="scp-state-dot"
            class:occupied={stateClass === 'occupied'}
            class:clear={stateClass === 'clear'}
            class:lit={stateClass === 'lit'}
            class:unlit={stateClass === 'unlit'}
            class:unknown={stateClass === 'unknown'}
            class:no-config={stateClass === 'no-config'}
            aria-hidden="true"
          ></span>
          <span class="scp-name">{ch.name}</span>
          <span class="scp-meta">{describeLocation(ch)} · {stateLabel(state)}</span>
        {/if}
  {/snippet}
</SearchableSingleSelectDialog>

<style>
  .scp-state-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-muted, #999);
    background: transparent;
  }
  .scp-state-dot.occupied {
    width: 10px;
    height: 10px;
    border: none;
    background: #d55e00;
  }
  .scp-state-dot.clear {
    border: none;
    background: #009e73;
  }
  .scp-state-dot.no-config {
    border-style: dashed;
    border-color: var(--text-muted, #999);
    opacity: 0.6;
  }
  .scp-name {
    font-weight: 500;
    color: var(--fluent-neutralForeground1);
  }
  .scp-meta {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground2);
  }
</style>
