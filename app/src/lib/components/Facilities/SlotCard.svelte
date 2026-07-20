<script lang="ts">
  /**
   * SlotCard — shared card component for facility slot display (Spec 020 / S4).
   *
   * Provides a consistent card layout across all facility types:
   * - Header row: slot label (left) + state badge (right)
   * - Channel name + ownership badge
   * - Meta line (group · location)
   * - Built-in slot management actions: "Add channel..." (empty) / "Remove from slot" (filled)
   * - Optional extra content (lamp breakdown, etc.) via snippet
   *
   * The card does NOT include "INPUTS"/"OUTPUTS" column headings — those
   * belong to the parent layout (FacilityCard) above the card.
   */
  import { channelStateClass, type ChannelState } from '$lib/utils/channelState';
  import type { Snippet } from 'svelte';

  let {
    label,
    state,
    stateLabel,
    channelName,
    channelId,
    ownership,
    meta,
    empty = false,
    slotLabel,
    onAddChannel,
    onRemoveFromSlot,
    extraContent,
    emptyContent,
    'data-slot': dataSlot,
    'data-testid': dataTestId,
    'data-slot-label': dataSlotLabel,
  }: {
    /** Slot display label (e.g. "Block", "Signal", "Downstream"). */
    label: string;
    /** Channel state for the badge dot. Undefined when empty/unbound. */
    state?: ChannelState;
    /** State label text for the badge (e.g. "Clear", "Occupied"). */
    stateLabel?: string;
    /** Channel name (bold, primary info). */
    channelName?: string;
    /** Bound channel id (needed for Remove action). */
    channelId?: string;
    /** Channel ownership type. */
    ownership?: 'hardware-owned' | 'user-owned';
    /** Meta line text (e.g. "Connector A · Input 6"). */
    meta?: string;
    /** Whether the slot is empty/unbound. */
    empty?: boolean;
    /** Slot label identifier passed to callbacks. */
    slotLabel?: string;
    /** Handler for Add channel action (empty state). */
    onAddChannel?: (slotLabel: string) => void;
    /** Handler for Remove from slot action (filled state). */
    onRemoveFromSlot?: (slotLabel: string, channelId: string) => void;
    /** Optional extra content below the meta line (e.g. lamp breakdown). */
    extraContent?: Snippet;
    /** Custom empty state content (overrides default "Add channel..." button). */
    emptyContent?: Snippet;
    /** Data attribute for slot identification in comprehension view. */
    'data-slot'?: string;
    'data-testid'?: string;
    'data-slot-label'?: string;
  } = $props();

  const stateClass = $derived(state ? channelStateClass(state) : undefined);
</script>

<div class="slot-card" class:slot-card-empty={empty} data-slot={dataSlot} data-testid={dataTestId} data-slot-label={dataSlotLabel}>
  <div class="slot-card-header">
    <span class="slot-card-label">{label}</span>
    {#if stateClass && stateLabel}
      <span class="slot-card-badge">
        <span
          class="slot-card-dot"
          class:occupied={stateClass === 'occupied'}
          class:clear={stateClass === 'clear'}
          class:lit={stateClass === 'lit'}
          class:unlit={stateClass === 'unlit'}
          class:unknown={stateClass === 'unknown'}
          class:no-config={stateClass === 'no-config'}
          class:signal-stop={stateClass === 'signal-stop'}
          class:signal-approach={stateClass === 'signal-approach'}
          class:signal-clear={stateClass === 'signal-clear'}
          class:signal-dark={stateClass === 'signal-dark'}
          aria-hidden="true"
        ></span>
        <span class="slot-card-state-label">{stateLabel}</span>
      </span>
    {/if}
  </div>

  {#if empty}
    {#if emptyContent}
      {@render emptyContent()}
    {:else}
      <div class="slot-card-empty-row">
        <span class="slot-card-empty-text">empty</span>
        {#if onAddChannel && slotLabel}
          <button
            type="button"
            class="slot-card-btn"
            onclick={() => onAddChannel!(slotLabel!)}
            data-testid="add-channel-button"
          >Add channel…</button>
        {/if}
      </div>
    {/if}
  {:else}
    {#if channelName}
      <div class="slot-card-name-row">
        <span class="slot-card-channel-name" data-testid="slot-channel-name">{channelName}</span>
        {#if ownership}
          <span
            class="slot-card-ownership"
            class:hw={ownership === 'hardware-owned'}
            class:user={ownership === 'user-owned'}
          >{ownership === 'hardware-owned' ? 'HW' : 'USER'}</span>
        {/if}
      </div>
    {/if}
    {#if meta}
      <span class="slot-card-meta">{meta}</span>
    {/if}
    {#if extraContent}
      {@render extraContent()}
    {/if}
    {#if onRemoveFromSlot && slotLabel && channelId}
      <div class="slot-card-actions">
        <button
          type="button"
          class="slot-card-btn-link danger"
          onclick={() => onRemoveFromSlot!(slotLabel!, channelId!)}
          data-testid="remove-from-slot-button"
        >Remove from slot</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .slot-card {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.5rem 0.625rem;
    border: 1px solid var(--border-color, #e5e5e5);
    border-radius: 5px;
    background: var(--bg-subtle, #fafafa);
    margin-bottom: 0.375rem;
  }
  .slot-card-empty {
    border-style: dashed;
  }
  .slot-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.125rem;
  }
  .slot-card-label {
    font-weight: 600;
    font-size: 0.625rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted, #616161);
  }
  .slot-card-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
  }
  .slot-card-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-muted, #616161);
    background: transparent;
    flex-shrink: 0;
  }
  .slot-card-dot.occupied { background: #d55e00; border-color: #d55e00; }
  .slot-card-dot.clear { background: #009e73; border-color: #009e73; }
  .slot-card-dot.lit {
    background: #e6c200;
    border-color: #e6c200;
    box-shadow: 0 0 4px rgba(230, 194, 0, 0.6);
  }
  .slot-card-dot.unlit { background: #555; border-color: #555; }
  .slot-card-dot.signal-stop { background: #d55e00; border-color: #d55e00; }
  .slot-card-dot.signal-approach {
    background: #e6c200;
    border-color: #e6c200;
    box-shadow: 0 0 4px rgba(230, 194, 0, 0.6);
  }
  .slot-card-dot.signal-clear {
    background: #009e73;
    border-color: #009e73;
    box-shadow: 0 0 4px rgba(0, 158, 115, 0.4);
  }
  .slot-card-dot.signal-dark { background: #333; border-color: #333; opacity: 0.5; }
  .slot-card-dot.no-config,
  .slot-card-dot.unknown {
    background: transparent;
    border-style: dashed;
    opacity: 0.6;
  }
  .slot-card-state-label {
    font-size: 0.6875rem;
    font-weight: 500;
    color: var(--text-secondary, #424242);
  }
  .slot-card-name-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }
  .slot-card-channel-name {
    font-weight: 600;
    font-size: 0.8125rem;
    color: var(--text-primary, #242424);
  }
  .slot-card-ownership {
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.0625rem 0.4rem;
    border-radius: 8px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .slot-card-ownership.hw { background: #dbeafe; color: #1e40af; }
  .slot-card-ownership.user { background: #ede9fe; color: #5b21b6; }
  .slot-card-meta {
    display: block;
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
    margin-top: 0.125rem;
  }
  .slot-card-empty-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .slot-card-empty-text {
    color: var(--text-muted, #616161);
    font-style: italic;
    font-size: 0.8125rem;
  }
  .slot-card-btn {
    font: inherit;
    font-size: 0.6875rem;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    border: 1px solid var(--border-strong, #c7c7c7);
    background: #fff;
    color: var(--text-primary, #242424);
    cursor: pointer;
    line-height: 1.4;
  }
  .slot-card-btn:hover {
    background: var(--bg-hover, #f5f5f5);
  }
  .slot-card-actions {
    margin-top: 0.25rem;
  }
  .slot-card-btn-link {
    background: none;
    border: none;
    color: var(--accent-color, #0f6cbd);
    padding: 0.125rem 0.25rem;
    cursor: pointer;
    font-size: 0.75rem;
    line-height: 1.4;
    font-family: inherit;
  }
  .slot-card-btn-link:hover {
    text-decoration: underline;
  }
  .slot-card-btn-link.danger {
    color: #b91c1c;
  }
</style>
