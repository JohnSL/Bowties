<script lang="ts">
  import type { Facility, FacilityStatus } from '$lib/api/facilities';
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
  import TrackDiagram from '$lib/components/BlockSignals/TrackDiagram.svelte';
  import { trackInputFor } from '$lib/components/BlockSignals/trackInput';
  import type { BlockOccupancyMap } from '$lib/components/BlockSignals/occupancy';
  import type { FacilityDiagramEntry } from '$lib/stores/facilityDiagrams.svelte';
  import WriteOutcomeList from './WriteOutcomeList.svelte';
  import StatusBadge from './StatusBadge.svelte';

  let {
    facility,
    template,
    onRename,
    onEdit,
    onDelete,
    diagram,
    occupancy,
  }: {
    facility: Facility;
    template?: BehaviorTemplate;
    onRename?: (facilityId: string, newName: string) => void;
    onEdit?: (facilityId: string) => void;
    onDelete?: (facilityId: string) => void;
    /** Backend preview state for an Automatic Block Signals card's track diagram. */
    diagram?: FacilityDiagramEntry;
    occupancy?: BlockOccupancyMap;
  } = $props();

  // Spec 018 / S6 (D5): status is derived by the effectiveLayoutStore facade
  // per ADR-0004 (single-owner derivation). FacilityCard renders the pill
  // from the facade call — no local slot-fullness check.
  let status = $derived<FacilityStatus>(
    effectiveLayoutStore.facilityStatus(facility.facilityId),
  );

  // Spec 020 / E5-T4: per-board write outcome from the Pending Change rows this
  // facility staged. While any are waiting the card is not shown as complete.
  const writeOutcome = $derived(effectiveLayoutStore.facilityWriteOutcome(facility.facilityId));

  // Spec 020: the Automatic Block Signals editor creates and owns the facility's
  // signals, so its card draws them read-only as a track diagram (edited via the same editor).
  const isAutomaticBlockSignals = $derived(
    (template?.templateId ?? facility.templateId) === 'automatic-block-signals',
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
      {#if writeOutcome.hasWaiting}
        <StatusBadge tone="incomplete" text="Waiting" />
      {:else}
        <StatusBadge tone={status === 'Wired' ? 'wired' : 'incomplete'} text={status} />
      {/if}
    </div>
    <div class="actions">
      {#if isAutomaticBlockSignals}
        <button type="button" class="btn-link" onclick={() => onEdit?.(facility.facilityId)} aria-label="Edit facility">Edit</button>
      {/if}
      <button type="button" class="btn-link" onclick={startRename} aria-label="Rename facility">Rename</button>
      <button type="button" class="btn-link danger" onclick={handleDelete} aria-label="Delete facility">Delete</button>
    </div>
  </header>

  {#if writeOutcome.hasWaiting}
    <WriteOutcomeList boards={writeOutcome.boards} />
  {/if}

  {#if isAutomaticBlockSignals}
    {#if diagram?.status === 'ready' && diagram.preview}
      <TrackDiagram input={trackInputFor(diagram.preview, 'summary')} {occupancy} />
    {:else if diagram?.status === 'failed'}
      <p class="diagram-note">The track diagram couldn't be drawn. {diagram.error}</p>
    {:else}
      <p class="diagram-note">Drawing the track…</p>
    {/if}
  {/if}
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
  .diagram-note {
    margin: 0;
    font-size: 0.75rem;
    color: var(--text-muted, #616161);
  }
</style>
