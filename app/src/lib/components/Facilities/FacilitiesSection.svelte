<script lang="ts">
  import { facilitiesStore } from '$lib/stores/facilities.svelte';
  import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import type { NodeConfigTree } from '$lib/types/nodeTree';
  import * as facilityOrchestrator from '$lib/orchestration/facilityOrchestrator';
  import AddFacilityDialog from './AddFacilityDialog.svelte';
  import FacilityCard from './FacilityCard.svelte';
  import BlockSignalsEditor from '$lib/components/BlockSignals/BlockSignalsEditor.svelte';
  import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
  import { blockSignalsEditorStore } from '$lib/stores/blockSignalsEditor.svelte';
  import { channelsStore } from '$lib/stores/channels.svelte';
  import { eventStateStore } from '$lib/stores/eventState.svelte';
  import { deriveChannelState } from '$lib/utils/channelState';
  import { occupancyFromChannelState, type BlockOccupancy } from '$lib/components/BlockSignals/occupancy';
  import {
    blockSignalsEditorActions,
    startBlockSignalsEditor,
  } from '$lib/orchestration/blockSignalsEditorOrchestrator';

  let {
    resolvedEventIds,
    onSelectChannel,
    onAddChannel,
    onRemoveFromSlot,
    nodeTree,
  }: {
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
    onSelectChannel?: (facilityId: string, slotLabel: string) => void;
    /** Spec 018 / S5 — consumer-side Add-channel intent emitter. */
    onAddChannel?: (facilityId: string, slotLabel: string) => void;
    onRemoveFromSlot?: (facilityId: string, slotLabel: string, currentChannelId: string) => void;
    nodeTree?: (nodeKey: string) => NodeConfigTree | undefined;
  } = $props();

  let showAddDialog = $state(false);

  // Live occupancy of each detected block, from the same events as the Channels panel.
  const blockOccupancy = $derived.by(() => {
    const states = new Map<string, BlockOccupancy>();
    if (!blockSignalsEditorStore.open) return states;
    const events = eventStateStore.events;
    for (const channel of channelsStore.channels) {
      if (channel.role !== 'block-occupancy') continue;
      const ids = resolvedEventIds?.get(channel.id);
      const state = deriveChannelState(events, ids?.['occupied'], ids?.['clear'], 'block-occupancy');
      states.set(channel.id, occupancyFromChannelState(state));
    }
    return states;
  });
  const BLOCK_SIGNALS_TEMPLATE_ID = 'automatic-block-signals';

  // Shown in Add facility so people can see what's planned; it can't be chosen yet.
  const interlockingPlantChoice = {
    templateId: 'interlocking-plant',
    displayName: 'Interlocking Plant',
    slots: [],
    mapping: [],
    description: 'Signals that govern routes over power switches at one location.',
    unavailableNote: 'Coming in a later release',
  };

  // One-line descriptions shown under each facility type in Add facility.
  const FACILITY_DESCRIPTIONS: Record<string, string> = {
    'block-indicator': "A panel light that follows one block's occupancy.",
    'automatic-block-signals': 'Signals that respond to occupancy, the next signal, and hand-thrown turnouts.',
  };

  const facilityChoices = $derived(
    behaviorTemplatesStore.templates
      .map((template) => ({ ...template, description: FACILITY_DESCRIPTIONS[template.templateId] }))
      .concat(interlockingPlantChoice),
  );

  function templateFor(templateId: string): BehaviorTemplate | undefined {
    return behaviorTemplatesStore.findByTemplateId(templateId);
  }

  function handleAddConfirm(template: BehaviorTemplate, name: string) {
    if (template.templateId === BLOCK_SIGNALS_TEMPLATE_ID) {
      startBlockSignalsEditor(name);
      showAddDialog = false;
      return;
    }
    facilitiesStore.addFacility(template, name);
    showAddDialog = false;
  }
  function handleAddCancel() {
    showAddDialog = false;
  }
  function handleRename(facilityId: string, newName: string) {
    facilitiesStore.renameFacility(facilityId, newName);
  }
  function handleDelete(facilityId: string) {
    // Spec 018 / S6 — route through the orchestrator so composed
    // bowties are torn down before the facility is removed. Calling
    // `facilitiesStore.deleteFacility` directly would leave orphan
    // bowtie metadata rows tagged with `createdByFacility === id`.
    facilityOrchestrator.deleteFacility(facilityId).catch((err) => {
      console.error('[facility] deleteFacility failed', err);
    });
  }
</script>

<section class="facilities-section" data-testid="facilities-section">
  <header class="section-header">
    <h2 class="section-heading">Facilities</h2>
    <button
      type="button"
      class="btn-primary"
      onclick={() => (showAddDialog = true)}
      disabled={!behaviorTemplatesStore.loaded || behaviorTemplatesStore.templates.length === 0}
      data-testid="add-facility-button"
    >
      + Add facility
    </button>
  </header>

  {#if facilitiesStore.isEmpty}
    <div class="empty-state" data-testid="facilities-empty-state">
      <p class="empty-title">No facilities yet</p>
      <p class="empty-hint">Click <strong>Add facility</strong> to scaffold a Block Indicator.</p>
    </div>
  {:else}
    <ul class="facility-list">
      {#each facilitiesStore.facilities as facility (facility.facilityId)}
        <li>
          <FacilityCard
            {facility}
            template={templateFor(facility.templateId)}
            {resolvedEventIds}
            onRename={handleRename}
            onDelete={handleDelete}
            {onSelectChannel}
            {onAddChannel}
            {onRemoveFromSlot}
            {nodeTree}
          />
        </li>
      {/each}
    </ul>
  {/if}

  {#if showAddDialog}
    <AddFacilityDialog
      templates={facilityChoices}
      onConfirm={handleAddConfirm}
      onCancel={handleAddCancel}
    />
  {/if}

  {#if blockSignalsEditorStore.open}
    <BlockSignalsEditor
      view={blockSignalsEditorStore}
      actions={blockSignalsEditorActions}
      detectedBlocks={effectiveLayoutStore.detectedBlocks}
      occupancy={blockOccupancy}
    />
  {/if}
</section>

<style>
  .facilities-section {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.25rem;
    border-bottom: 1px solid var(--border-subtle, #e2e2e2);
  }
  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .section-heading {
    font-size: 1rem;
    font-weight: 600;
    margin: 0;
    color: var(--text-primary, #222);
  }
  .btn-primary {
    padding: 0.4rem 0.8rem;
    border-radius: 0.25rem;
    border: 1px solid transparent;
    background: var(--accent-color, #2563eb);
    color: white;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .btn-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    border: 1px dashed var(--border-subtle, #e2e2e2);
    border-radius: 0.375rem;
    color: var(--text-muted, #666);
    text-align: center;
  }
  .empty-title {
    font-weight: 500;
    margin: 0 0 0.25rem 0;
  }
  .empty-hint {
    font-size: 0.9rem;
    margin: 0;
  }
  .facility-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
</style>
