<script lang="ts">
  /**
   * FacilitySlot — single slot inside a facility card (Spec 018).
   *
   * Visual layout mirrors mockups.html §3 (empty) and §6 (filled): a
   * vertical card with an uppercase slot label on top. Empty state
   * pairs an italic "empty" label with the Select-channel button.
   * Filled state shows the bound channel's state dot, name +
   * ownership badge, location meta, and Rebind / Remove link actions
   * (no Rename per S4 D6 — channel rename lives in the Channels
   * panel).
   *
   * Vec-shaped bindings (Spec 018 / S4 — D8): the component takes
   * the parent-resolved `currentChannelId` / `currentChannelDisplay`
   * shape, leaving the parent (FacilityCard) responsible for picking
   * element 0 from the underlying `slotBindings[label]` Vec. The
   * UI is intentionally max-1 in S4 even though the wire form is
   * plural.
   */
  import type { BehaviorTemplate, SlotDefinition } from '$lib/api/behaviorTemplates';
  import type { OccupancyState } from '$lib/utils/channelState';

  let {
    slotLabel,
    template,
    currentChannelId,
    currentChannelDisplay,
    onSelectChannel,
    onRebindChannel,
    onRemoveFromSlot,
  }: {
    slotLabel: string;
    template?: BehaviorTemplate;
    /** Channel id currently bound (UI is max-1 in S4 — see D8 comment). */
    currentChannelId?: string;
    /** Display metadata for the currently bound channel. */
    currentChannelDisplay?: {
      name: string;
      ownership: 'hardware-owned' | 'user-owned';
      groupLabel: string;
      locationLabel: string;
      state: OccupancyState;
      stateLabel: string;
    };
    onSelectChannel?: (slotLabel: string) => void;
    onRebindChannel?: (slotLabel: string, currentChannelId: string) => void;
    onRemoveFromSlot?: (slotLabel: string, currentChannelId: string) => void;
  } = $props();

  function definition(): SlotDefinition | undefined {
    return template?.slots.find((s) => s.label === slotLabel);
  }

  function slotKindLabel(): string {
    const def = definition();
    if (!def) return '';
    return def.kind === 'producer' ? '(input)' : '(output)';
  }

  function headerLabel(): string {
    const capitalized = slotLabel.charAt(0).toUpperCase() + slotLabel.slice(1);
    const kind = slotKindLabel();
    return kind ? `${capitalized} ${kind}` : capitalized;
  }

  function requiredRoleHint(): string {
    const def = definition();
    if (!def) return '';
    return `Requires a ${def.requiredRole} channel.`;
  }

  const filled = $derived(currentChannelId !== undefined && currentChannelDisplay !== undefined);
</script>

<div class="slot" class:filled class:empty={!filled} data-testid="facility-slot" data-slot-label={slotLabel}>
  <span class="slot-label">{headerLabel()}</span>

  {#if !filled}
    <div class="slot-empty-row">
      <span class="slot-empty-text">empty</span>
      <div class="slot-empty-actions">
        <button
          type="button"
          class="btn btn-sm"
          onclick={() => onSelectChannel?.(slotLabel)}
          title={requiredRoleHint()}
          data-testid="select-channel-button"
        >Select channel…</button>
      </div>
    </div>
  {:else}
    {@const ch = currentChannelDisplay!}
    <div class="slot-filled" data-testid="filled-slot">
      <span
        class="state-dot"
        class:occupied={ch.state === 'occupied'}
        class:clear={ch.state === 'clear'}
        class:unknown={ch.state === 'unknown'}
        class:no-config={ch.state === 'no-config'}
        title={ch.stateLabel}
        aria-hidden="true"
      ></span>
      <div class="slot-filled-text">
        <div class="slot-channel-name-row">
          <span class="slot-channel-name" data-testid="slot-channel-name">{ch.name}</span>
          <span
            class="ownership-badge"
            class:hw={ch.ownership === 'hardware-owned'}
            class:user={ch.ownership === 'user-owned'}
          >{ch.ownership === 'hardware-owned' ? 'HW' : 'USER'}</span>
        </div>
        <span class="slot-channel-meta">{ch.groupLabel} · {ch.locationLabel} · {ch.stateLabel.toLowerCase()}</span>
      </div>
    </div>
    <div class="slot-filled-actions" data-testid="filled-slot-actions">
      <button
        type="button"
        class="btn-link"
        onclick={() => onRebindChannel?.(slotLabel, currentChannelId!)}
        data-testid="rebind-channel-button"
      >Rebind…</button>
      <button
        type="button"
        class="btn-link danger"
        onclick={() => onRemoveFromSlot?.(slotLabel, currentChannelId!)}
        data-testid="remove-from-slot-button"
      >Remove from slot</button>
    </div>
  {/if}
</div>

<style>
  .slot {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.4rem;
    min-height: 78px;
    padding: 0.625rem 0.75rem;
    border: 1px solid var(--border-color, #d1d1d1);
    border-radius: 5px;
    background: var(--bg-subtle, #fafafa);
  }
  .slot.empty {
    border-style: dashed;
    background: #fafbfc;
  }
  .slot-label {
    font-size: 0.625rem;
    font-weight: 600;
    color: var(--text-muted, #616161);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
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
  .btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .btn-sm {
    font-size: 0.6875rem;
    padding: 0.2rem 0.5rem;
  }
  .slot-filled {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
  }
  .slot-filled-text {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    min-width: 0;
  }
  .slot-channel-name-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-wrap: wrap;
  }
  .slot-channel-name {
    font-weight: 500;
    color: var(--text-primary, #242424);
    font-size: 0.8125rem;
  }
  .slot-channel-meta {
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
  }
  .ownership-badge {
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.0625rem 0.4rem;
    border-radius: 8px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .ownership-badge.hw { background: #dbeafe; color: #1e40af; }
  .ownership-badge.user { background: #ede9fe; color: #5b21b6; }
  .state-dot {
    display: inline-block;
    flex-shrink: 0;
    width: 10px;
    height: 10px;
    margin-top: 0.2rem;
    border-radius: 50%;
    border: 1.5px solid var(--text-muted, #616161);
    background: transparent;
  }
  .state-dot.occupied { background: #d55e00; border-color: #d55e00; }
  .state-dot.clear { background: #009e73; border-color: #009e73; }
  .state-dot.no-config {
    background: transparent;
    border-style: dashed;
    opacity: 0.6;
  }
  .slot-filled-actions {
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
</style>
