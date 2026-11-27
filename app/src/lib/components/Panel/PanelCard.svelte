<script lang="ts">
  import { channelsView } from '$lib/layout';

  /**
   * PanelCard — the Railroad-tab card for a Panel facility: name, type chip, derived status
   * and per-board waiting writes (from the effectiveLayoutStore facade), a sub line, and a
   * tile per block in panel order with its pin and live block occupancy. Tiles are read-only;
   * Edit and Remove emit intent with the facility id.
   */
  import type { Facility } from '$lib/api/facilities';
  import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';

import { eventStateStore } from '$lib/stores/eventState.svelte';
  import { blockOccupancyMap, occupancyOf } from '$lib/components/BlockSignals/occupancy';
  import ChannelStateIndicator from '$lib/components/Railroad/ChannelStateIndicator.svelte';
  import StatusBadge from '$lib/components/Facilities/StatusBadge.svelte';
  import WriteOutcomeList from '$lib/components/Facilities/WriteOutcomeList.svelte';

  let { facility, resolvedEventIds, onEdit, onRemove }: {
    facility: Facility;
    onEdit?: (facilityId: string) => void;
    onRemove?: (facilityId: string) => void;
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
  } = $props();

  const status = $derived(effectiveLayoutStore.panelStatus(facility.facilityId));
  const entries = $derived(effectiveLayoutStore.panelCardEntries(facility.facilityId));
  const writeOutcome = $derived(effectiveLayoutStore.facilityWriteOutcome(facility.facilityId));
  const occupancy = $derived(blockOccupancyMap(channelsView.channels, resolvedEventIds, eventStateStore.events));

  const boards = $derived([...new Set(entries.flatMap((entry) => (entry.lamp ? [entry.lamp.boardName] : [])))]);
  const count = $derived(entries.length);

  const statusText = $derived(
    status.status === 'Incomplete' && status.needsLamp > 0
      ? `Incomplete · ${status.needsLamp} ${status.needsLamp === 1 ? 'needs' : 'need'} a lamp`
      : status.status,
  );

  const READING_LABEL = { occupied: 'Block occupied', clear: 'Block clear', unknown: 'No reading' } as const;

  function readingState(blockId: string): 'occupied' | 'clear' | 'unknown' {
    const value = occupancyOf(occupancy, blockId);
    return value === 'unavailable' ? 'unknown' : value;
  }

  function reading(blockId: string): string {
    const value = occupancyOf(occupancy, blockId);
    return value === 'unavailable' ? 'no reading' : value;
  }
</script>

<article class="panel-card" data-testid="panel-card" data-facility-id={facility.facilityId}>
  <header class="head">
    <span class="name">{facility.name}</span>
    <span class="type">Panel</span>
    {#if writeOutcome.hasWaiting}
      <StatusBadge tone="incomplete" text="Waiting" />
    {:else}
      <StatusBadge tone={status.status === 'Wired' ? 'wired' : status.status === 'Incomplete' ? 'incomplete' : 'neutral'} text={statusText} />
    {/if}
    <div class="actions">
      <button type="button" class="btn-link" onclick={() => onEdit?.(facility.facilityId)} aria-label="Edit facility">Edit</button>
      <button type="button" class="btn-link danger" onclick={() => onRemove?.(facility.facilityId)} aria-label="Remove facility">Remove</button>
    </div>
  </header>
  {#if writeOutcome.hasWaiting}
    <WriteOutcomeList boards={writeOutcome.boards} />
  {/if}
  <div class="sub">
    {#if count === 0}
      <span>No blocks yet.</span>
    {:else}
      <span>{count} {count === 1 ? 'block' : 'blocks'}{boards.length > 0 ? ` · lamps on ${boards.join(', ')}` : ''}</span>
    {/if}
  </div>
  {#if count > 0}
    <ul class="lamp-grid">
      {#each entries as entry (entry.entryId)}
        <li class="tile" data-testid="panel-tile">
          <ChannelStateIndicator state={readingState(entry.blockId)} label={READING_LABEL[readingState(entry.blockId)]} size="tile" />
          <div>
            <div class="t-name">{entry.blockMissing ? 'Block no longer detected' : entry.blockName}</div>
            <div class="t-meta" class:problem={entry.blockMissing || !entry.lamp || entry.lamp.boardMissing || entry.lamp.pin === null}>
              {#if entry.lamp?.boardMissing}
                Board missing
              {:else if entry.lamp && entry.lamp.pin !== null}
                {entry.lamp.pinLabel}{entry.blockMissing ? '' : ` · ${reading(entry.blockId)}`}
              {:else if entry.lamp}
                needs a pin{entry.blockMissing ? '' : ` · ${reading(entry.blockId)}`}
              {:else}
                needs a lamp{entry.blockMissing ? '' : ` · ${reading(entry.blockId)}`}
              {/if}
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</article>

<style>
  .panel-card {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding: 0.75rem 0.875rem;
    border: 1px solid var(--border-color, #d1d1d1);
    border-radius: 6px;
    background: var(--surface-color, #fff);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex-wrap: wrap;
  }
  .name {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text-primary, #242424);
  }
  .type {
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
  }
  .actions {
    display: flex;
    gap: 0.25rem;
    margin-left: auto;
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
  .sub {
    display: flex;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: var(--text-muted, #616161);
  }
  .lamp-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
    gap: 0.5rem;
    margin: 0.25rem 0 0;
    padding: 0;
    list-style: none;
  }
  .tile {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.375rem 0.5rem;
    border: 1px solid var(--border-color, #d1d1d1);
    border-radius: 6px;
  }
  .t-name {
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--text-primary, #242424);
  }
  .t-meta {
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
  }
  .t-meta.problem {
    color: #bc4b09;
  }
</style>
