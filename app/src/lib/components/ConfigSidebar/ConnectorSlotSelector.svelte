<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { ConnectorSlotSelectorViewModel } from '$lib/utils/connectorSlotSelectors';

  export let selector: ConnectorSlotSelectorViewModel;
  export let disabled = false;

  const dispatch = createEventDispatcher<{
    change: { slotId: string; selectedDaughterboardId: string | null };
  }>();

  function handleChange(event: Event): void {
    const target = event.currentTarget as HTMLSelectElement;
    dispatch('change', {
      slotId: selector.slotId,
      selectedDaughterboardId: target.value || null,
    });
  }
</script>

<label class="connector-slot-selector">
  <span class="connector-slot-label">{selector.label}</span>
  <select
    class="connector-slot-input"
    value={selector.selectedDaughterboardId ?? ''}
    {disabled}
    on:change={handleChange}
  >
    {#each selector.options as option (option.value || option.label)}
      <option value={option.value}>{option.label}</option>
    {/each}
  </select>
</label>

<style>
  .connector-slot-selector {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 12px 8px 40px;
  }

  .connector-slot-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary, #555);
  }

  .connector-slot-input {
    width: 100%;
    font-size: 12px;
    padding: 6px 8px;
    border: 1px solid var(--border-color, #c9c9c9);
    border-radius: 6px;
    background: var(--surface-color, #fff);
    color: var(--text-primary, #222);
  }
</style>