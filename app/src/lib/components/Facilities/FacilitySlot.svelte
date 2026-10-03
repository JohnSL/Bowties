<script lang="ts">
  /**
   * Facility-specific adapter for the role-neutral SlotCard presentation.
   * Resolves template language and emits Select/Add intent by slot kind.
   */
  import type { BehaviorTemplate, SlotDefinition } from '$lib/api/behaviorTemplates';
  import type { ChannelState } from '$lib/utils/channelState';
  import type { ConfigTarget } from '$lib/utils/channelConfigNavigation';
  import SlotCard from './SlotCard.svelte';

  let {
    slotLabel,
    template,
    currentChannelId,
    currentChannelDisplay,
    onSelectChannel,
    onAddChannel,
    onRemoveFromSlot,
    configTargets,
    onConfigTargetClick,
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
    configTargets?: ConfigTarget[];
    onConfigTargetClick?: (target: ConfigTarget) => void;
  } = $props();

  function definition(): SlotDefinition | undefined {
    return template?.slots.find((slot) => slot.label === slotLabel);
  }

  function headerLabel(): string {
    const def = definition();
    const name = def?.displayLabel ?? slotLabel;
    const capitalized = name.charAt(0).toUpperCase() + name.slice(1);
    if (!def) return capitalized;
    return `${capitalized} ${def.kind === 'producer' ? '(input)' : '(output)'}`;
  }

  function requiredRoleHint(): string {
    const def = definition();
    return def ? `Requires a ${def.requiredRole} channel.` : '';
  }

  const filled = $derived(currentChannelId !== undefined && currentChannelDisplay !== undefined);
  const slotIsConsumer = $derived(definition()?.kind === 'consumer');
</script>

<SlotCard
  label={headerLabel()}
  state={currentChannelDisplay?.state}
  stateLabel={currentChannelDisplay?.stateLabel}
  channelName={currentChannelDisplay?.name}
  channelId={currentChannelId}
  ownership={currentChannelDisplay?.ownership}
  meta={currentChannelDisplay
    ? `${currentChannelDisplay.groupLabel} · ${currentChannelDisplay.locationLabel} · ${currentChannelDisplay.stateLabel.toLowerCase()}`
    : undefined}
  empty={!filled}
  {slotLabel}
  onAddChannel={slotIsConsumer ? onAddChannel : onSelectChannel}
  {onRemoveFromSlot}
  {configTargets}
  {onConfigTargetClick}
  data-testid="facility-slot"
  data-slot-label={slotLabel}
>
  {#snippet emptyContent()}
    <div class="slot-empty-row">
      <span class="slot-empty-text">empty</span>
      <button
        type="button"
        class="btn btn-sm"
        onclick={() => slotIsConsumer ? onAddChannel?.(slotLabel) : onSelectChannel?.(slotLabel)}
        title={requiredRoleHint()}
        data-testid={slotIsConsumer ? 'add-channel-button' : 'select-channel-button'}
      >{slotIsConsumer ? 'Add channel…' : 'Select channel…'}</button>
    </div>
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
  .btn:hover:not(:disabled) { background: var(--bg-hover, #f5f5f5); }
  .btn-sm {
    font-size: 0.6875rem;
    padding: 0.2rem 0.5rem;
  }
</style>
