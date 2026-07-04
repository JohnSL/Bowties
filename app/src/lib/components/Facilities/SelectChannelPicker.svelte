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
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';

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

  let selectedId = $state<string | undefined>(undefined);
  let searchText = $state('');

  const filteredChannels = $derived.by(() => {
    const q = searchText.trim().toLowerCase();
    if (q.length === 0) return candidateChannels;
    return candidateChannels.filter((ch) => {
      if (ch.name.toLowerCase().includes(q)) return true;
      const location = describeLocation(ch).toLowerCase();
      return location.includes(q);
    });
  });

  const confirmDisabled = $derived(selectedId === undefined);

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

  function handleConfirm() {
    if (selectedId === undefined) return;
    onConfirm(selectedId);
  }
</script>

<Dialog open width="md" ariaLabel={dialogTitle} initialFocus="first" onCancel={onCancel}>
  {#snippet title()}
    <DialogTitle>{dialogTitle}</DialogTitle>
  {/snippet}

  <form
    class="scp-form"
    onsubmit={(e) => {
      e.preventDefault();
      handleConfirm();
    }}
  >
    <input
      class="scp-search"
      type="search"
      placeholder="Search by name or location…"
      bind:value={searchText}
      aria-label="Filter channels"
    />

    {#if filteredChannels.length === 0}
      <p class="scp-empty">No matching channels.</p>
    {:else}
      <ul class="scp-list" role="radiogroup" aria-label="Channel candidates">
        {#each filteredChannels as ch (ch.id)}
          {@const state = channelState(ch.id)}
          {@const stateClass = channelStateClass(state)}
          <li class="scp-list-item">
            <label class="scp-row" class:selected={selectedId === ch.id}>
              <input
                type="radio"
                name="select-channel"
                value={ch.id}
                checked={selectedId === ch.id}
                onchange={() => (selectedId = ch.id)}
              />
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
            </label>
          </li>
        {/each}
      </ul>
    {/if}

    <button type="submit" class="scp-hidden-submit" tabindex="-1" aria-hidden="true"></button>
  </form>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" disabled={confirmDisabled} onclick={handleConfirm}>
        Confirm
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .scp-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  .scp-search {
    padding: 6px 10px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
    font-family: var(--fluent-fontFamily);
    font-size: var(--fluent-fontSizeBase300);
  }
  .scp-search:focus {
    outline: none;
    border-color: var(--fluent-strokeFocus2);
    box-shadow: 0 0 0 2px var(--fluent-strokeFocusHalo);
  }
  .scp-empty {
    color: var(--fluent-neutralForeground2);
    margin: 0.5rem 0;
    font-size: var(--fluent-fontSizeBase200);
  }
  .scp-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 18rem;
    overflow-y: auto;
    border: 1px solid var(--fluent-neutralStroke2, #e2e2e2);
    border-radius: 4px;
  }
  .scp-row {
    display: grid;
    grid-template-columns: auto auto 1fr auto auto;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    cursor: pointer;
    border-bottom: 1px solid var(--fluent-neutralStroke2, #f0f0f0);
  }
  .scp-row:last-child {
    border-bottom: none;
  }
  .scp-row:hover {
    background: var(--fluent-neutralBackground1Hover, #f5f5f5);
  }
  .scp-row.selected {
    background: var(--fluent-neutralBackground1Selected, #eef4ff);
  }
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
  .scp-hidden-submit {
    position: absolute;
    width: 0;
    height: 0;
    padding: 0;
    border: 0;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
  }
</style>
