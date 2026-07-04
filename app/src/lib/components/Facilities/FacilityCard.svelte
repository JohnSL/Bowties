<script lang="ts">
  import type { Facility, FacilityStatus } from '$lib/api/facilities';
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import { channelsStore } from '$lib/stores/channels.svelte';
  import { eventStateStore } from '$lib/stores/eventState.svelte';
  import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
  import {
    deriveChannelState,
    channelStateLabel,
    roleForChannelState,
    type ChannelState,
  } from '$lib/utils/channelState';
  import FacilitySlot from './FacilitySlot.svelte';

  let {
    facility,
    template,
    resolvedEventIds,
    onRename,
    onDelete,
    onSelectChannel,
    onAddChannel,
    onRemoveFromSlot,
  }: {
    facility: Facility;
    template?: BehaviorTemplate;
    /** Map from channelId to state-name → eventId (Spec 018 / S5 D6). */
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
    onRename?: (facilityId: string, newName: string) => void;
    onDelete?: (facilityId: string) => void;
    /** Spec 018 / S4 — producer-side input slot's Select channel intent. */
    onSelectChannel?: (facilityId: string, slotLabel: string) => void;
    /** Spec 018 / S5 — consumer-side output slot's Add channel intent. */
    onAddChannel?: (facilityId: string, slotLabel: string) => void;
    onRemoveFromSlot?: (facilityId: string, slotLabel: string, currentChannelId: string) => void;
  } = $props();

  // Spec 018 / S6 (D5): status is derived by the effectiveLayoutStore facade
  // per ADR-0004 (single-owner derivation). FacilityCard renders the pill
  // from the facade call — no local slot-fullness check.
  let status = $derived<FacilityStatus>(
    effectiveLayoutStore.facilityStatus(facility.facilityId),
  );

  let isEditingName = $state(false);
  let nameEditValue = $state('');

  function startRename() {
    nameEditValue = facility.name;
    isEditingName = true;
  }
  function commitRename() {
    isEditingName = false;
    const trimmed = nameEditValue.trim();
    if (trimmed.length === 0) return;
    if (trimmed === facility.name) return;
    onRename?.(facility.facilityId, trimmed);
  }
  function handleNameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') { isEditingName = false; }
  }
  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
  function handleDelete() {
    onDelete?.(facility.facilityId);
  }

  function slotsOrdered(): Array<[string, string[]]> {
    if (template) {
      return template.slots.map((s) => [s.label, facility.slotBindings[s.label] ?? []]);
    }
    return Object.entries(facility.slotBindings);
  }

  function formatConnectorLabel(connectorId: string): string {
    const match = connectorId.match(/^connector-([a-z])$/i);
    if (match) return `Connector ${match[1].toUpperCase()}`;
    return connectorId;
  }

  /**
   * Resolve the FacilitySlot filled-state display from the channel id.
   * UI is max-1 in S4: we pick element 0 of the Vec, leaving multi-binding
   * rendering to a future slice when ABS aspect-slot repeaters arrive.
   */
  function displayFor(binding: string[]):
    | { currentChannelId: string; currentChannelDisplay: { name: string; ownership: 'hardware-owned' | 'user-owned'; groupLabel: string; locationLabel: string; state: ChannelState; stateLabel: string } }
    | { currentChannelId: undefined; currentChannelDisplay: undefined } {
    if (binding.length === 0) {
      return { currentChannelId: undefined, currentChannelDisplay: undefined };
    }
    const id = binding[0];
    const channel = channelsStore.channels.find((c) => c.id === id);
    if (!channel) {
      return { currentChannelId: undefined, currentChannelDisplay: undefined };
    }
    const ids = resolvedEventIds?.get(id);
    const role = roleForChannelState(channel.role);
    const positiveId = role === 'lamp-indicator' ? ids?.['lit'] : ids?.['occupied'];
    const negativeId = role === 'lamp-indicator' ? ids?.['unlit'] : ids?.['clear'];
    const state = deriveChannelState(eventStateStore.events, positiveId, negativeId, role);
    const groupLabel = channel.binding.kind === 'connectorInput'
      ? formatConnectorLabel(channel.binding.connector)
      : 'Direct Lamp Control';
    const locationLabel = channel.binding.kind === 'connectorInput'
      ? `Input ${channel.binding.input}`
      : `Row ${channel.binding.rowOrdinal}`;
    return {
      currentChannelId: id,
      currentChannelDisplay: {
        name: channel.name,
        ownership: channel.ownership,
        groupLabel,
        locationLabel,
        state,
        stateLabel: channelStateLabel(state),
      },
    };
  }
</script>

<article class="facility-card" data-testid="facility-card" data-facility-id={facility.facilityId}>
  <header class="facility-header">
    <div class="facility-title">
      {#if isEditingName}
        <input
          class="facility-name-input"
          type="text"
          bind:value={nameEditValue}
          onblur={commitRename}
          onkeydown={handleNameKeydown}
          use:focusInput
          aria-label="Edit facility name"
        />
      {:else}
        <button type="button" class="facility-name" onclick={startRename} title="Click to rename">
          {facility.name}
        </button>
      {/if}
      <span class="template-label">{template?.displayName ?? facility.templateId}</span>
      <span class="status-pill" class:wired={status === 'Wired'} class:incomplete={status === 'Incomplete'}>
        <span class="pulse" aria-hidden="true"></span>{status}
      </span>
    </div>
    <div class="actions">
      <button type="button" class="btn-link" onclick={startRename} aria-label="Rename facility">Rename</button>
      <button type="button" class="btn-link danger" onclick={handleDelete} aria-label="Delete facility">Delete</button>
    </div>
  </header>

  <div class="slot-grid">
    {#each slotsOrdered() as [label, binding], i (label)}
      {@const d = displayFor(binding)}
      {#if i > 0}
        <span class="slot-arrow" aria-hidden="true">→</span>
      {/if}
      <FacilitySlot
        slotLabel={label}
        {template}
        currentChannelId={d.currentChannelId}
        currentChannelDisplay={d.currentChannelDisplay}
        onSelectChannel={(slot) => onSelectChannel?.(facility.facilityId, slot)}
        onAddChannel={(slot) => onAddChannel?.(facility.facilityId, slot)}
        onRemoveFromSlot={(slot, currentId) => onRemoveFromSlot?.(facility.facilityId, slot, currentId)}
      />
    {/each}
  </div>
</article>

<style>
  .facility-card {
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
    padding: 0.75rem 0.875rem;
    border: 1px solid var(--border-color, #d1d1d1);
    border-radius: 6px;
    background: var(--surface-color, #fff);
  }
  .facility-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .facility-title {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex-wrap: wrap;
    min-width: 0;
  }
  .facility-name,
  .facility-name-input {
    background: transparent;
    border: none;
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text-primary, #242424);
    padding: 0;
    cursor: text;
    font-family: inherit;
  }
  .facility-name:hover {
    text-decoration: underline;
  }
  .facility-name-input {
    border-bottom: 1px solid var(--accent-color, #0f6cbd);
    outline: none;
  }
  .template-label {
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
  }
  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.3125rem;
    font-size: 0.6875rem;
    font-weight: 600;
    padding: 0.125rem 0.5rem;
    border-radius: 10px;
  }
  .status-pill .pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .status-pill.incomplete {
    background: #fff4ce;
    color: #bc4b09;
  }
  .status-pill.wired {
    background: #dcfce7;
    color: #166534;
  }
  .actions {
    display: flex;
    gap: 0.25rem;
  }
  .btn-link {
    background: none;
    border: none;
    color: var(--accent-color, #0f6cbd);
    padding: 0.125rem 0.25rem;
    cursor: pointer;
    font-size: 0.75rem;
    line-height: 1.4;
    font-family: inherit;
  }
  .btn-link:hover {
    text-decoration: underline;
  }
  .btn-link.danger {
    color: #b91c1c;
  }
  .slot-grid {
    display: grid;
    grid-template-columns: 1fr 28px 1fr;
    align-items: stretch;
    gap: 0.5rem;
  }
  .slot-arrow {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted, #616161);
    font-size: 1.125rem;
    line-height: 1;
  }
  /* Single-slot templates fall back to a sensible single column. */
  .slot-grid:has(> :nth-child(1):last-child) {
    grid-template-columns: 1fr;
  }
</style>
