<script lang="ts">
  /**
   * FacilitySlot — thin wrapper around SlotCard for non-compiled facility cards.
   *
   * Resolves the slot display label from the template definition and
   * delegates rendering + slot management actions to SlotCard.
   */
  import type { BehaviorTemplate, SlotDefinition } from '$lib/api/behaviorTemplates';
  import type { ChannelState } from '$lib/utils/channelState';
  import SlotCard from './SlotCard.svelte';

  let {
    slotLabel,
    template,
    currentChannelId,
    currentChannelDisplay,
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
    /** Handler for the "Add channel..." action on empty slots. */
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

  const filled = $derived(currentChannelId !== undefined && currentChannelDisplay !== undefined);
</script>

<SlotCard
  label={cardLabel()}
  state={currentChannelDisplay?.state}
  stateLabel={currentChannelDisplay?.stateLabel}
  channelName={currentChannelDisplay?.name}
  channelId={currentChannelId}
  ownership={currentChannelDisplay?.ownership}
  meta={currentChannelDisplay ? `${currentChannelDisplay.groupLabel} · ${currentChannelDisplay.locationLabel}` : undefined}
  empty={!filled}
  slotLabel={slotLabel}
  onAddChannel={onAddChannel}
  onRemoveFromSlot={onRemoveFromSlot}
  data-testid="facility-slot"
  data-slot-label={slotLabel}
/>
