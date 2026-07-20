<script lang="ts">
  /**
   * FacilitySlot — thin wrapper around SlotCard for non-compiled facility cards.
   *
   * Resolves slot-specific props (label, empty state, action buttons) from
   * the template definition and delegates rendering to SlotCard.
   */
  import type { BehaviorTemplate, SlotDefinition } from '$lib/api/behaviorTemplates';
  import type { ChannelState } from '$lib/utils/channelState';
  import SlotCard from './SlotCard.svelte';

  let {
    slotLabel,
    template,
    currentChannelId,
    currentChannelDisplay,
    onSelectChannel,
    onAddChannel,
    onRemoveFromSlot,
  }: {
    slotLabel: string;
    template?: BehaviorTemplate;
    currentChannelId?: string;
    currentChannelDisplay?: {
      name: string;
      ownership: 'hardware-owned' | 'user-owned';
      groupLabel: string;
      locationLabel: string;
      state: ChannelState;
      stateLabel: string;
    };
    onSelectChannel?: (slotLabel: string) => void;
    onAddChannel?: (slotLabel: string) => void;
    onRemoveFromSlot?: (slotLabel: string, currentChannelId: string) => void;
  } = $props();

  function definition(): SlotDefinition | undefined {
    return template?.slots.find((s) => s.label === slotLabel);
  }

  function cardLabel(): string {
    const def = definition();
    return def?.displayLabel ?? slotLabel;
  }

  function requiredRoleHint(): string {
    const def = definition();
    if (!def) return '';
    return `Requires a ${def.requiredRole} channel.`;
  }

  const filled = $derived(currentChannelId !== undefined && currentChannelDisplay !== undefined);
  const slotIsConsumer = $derived(definition()?.kind === 'consumer');
</script>

<SlotCard
  label={cardLabel()}
  state={currentChannelDisplay?.state}
  stateLabel={currentChannelDisplay?.stateLabel}
  channelName={currentChannelDisplay?.name}
  ownership={currentChannelDisplay?.ownership}
  meta={currentChannelDisplay ? `${currentChannelDisplay.groupLabel} · ${currentChannelDisplay.locationLabel}` : undefined}
  empty={!filled}
  data-slot={slotLabel}
  data-testid="facility-slot"
  data-slot-label={slotLabel}
>
  {#snippet emptyContent()}
    <div class="slot-empty-row">
      <span class="slot-empty-text">empty</span>
      <div class="slot-empty-actions">
        {#if slotIsConsumer}
          <button
            type="button"
            class="btn btn-sm"
            onclick={() => onAddChannel?.(slotLabel)}
            title={requiredRoleHint()}
            data-testid="add-channel-button"
          >Add channel…</button>
        {:else}
          <button
            type="button"
            class="btn btn-sm"
            onclick={() => onSelectChannel?.(slotLabel)}
            title={requiredRoleHint()}
            data-testid="select-channel-button"
          >Select channel…</button>
        {/if}
      </div>
    </div>
  {/snippet}
  {#snippet actions()}
    {#if filled}
      <button
        type="button"
        class="btn-link danger"
        onclick={() => onRemoveFromSlot?.(slotLabel, currentChannelId!)}
        data-testid="remove-from-slot-button"
      >Remove from slot</button>
    {/if}
  {/snippet}
</SlotCard>

<style>
  .slot-empty-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .slot-empty-text {
    color: var(--text-muted, #616161);
    font-style: italic;
    font-size: 0.8125rem;
  }
  .slot-empty-actions {
    display: flex;
    gap: 0.375rem;
  }
  .btn {
    font: inherit;
    font-size: 0.75rem;
    padding: 0.3rem 0.75rem;
    border-radius: 4px;
    border: 1px solid var(--border-strong, #c7c7c7);
    background: #fff;
    color: var(--text-primary, #242424);
    cursor: pointer;
    line-height: 1.4;
  }
  .btn:hover:not(:disabled) {
    background: var(--bg-hover, #f5f5f5);
  }
  .btn-sm {
    font-size: 0.6875rem;
    padding: 0.2rem 0.5rem;
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
</style>
