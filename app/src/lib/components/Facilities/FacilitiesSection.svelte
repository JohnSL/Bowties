<script lang="ts">
  import { facilitiesStore } from '$lib/stores/facilities.svelte';
  import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import * as facilityOrchestrator from '$lib/orchestration/facilityOrchestrator';
  import AddFacilityDialog from './AddFacilityDialog.svelte';
  import FacilityCard from './FacilityCard.svelte';

  let {
    resolvedEventIds,
    onSelectChannel,
    onAddChannel,
    onRemoveFromSlot,
  }: {
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
    onSelectChannel?: (facilityId: string, slotLabel: string) => void;
    /** Spec 018 / S5 — consumer-side Add-channel intent emitter. */
    onAddChannel?: (facilityId: string, slotLabel: string) => void;
    onRemoveFromSlot?: (facilityId: string, slotLabel: string, currentChannelId: string) => void;
  } = $props();

  let showAddDialog = $state(false);

  function templateFor(templateId: string): BehaviorTemplate | undefined {
    return behaviorTemplatesStore.findByTemplateId(templateId);
  }

  function handleAddConfirm(template: BehaviorTemplate, name: string) {
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
          />
        </li>
      {/each}
    </ul>
  {/if}

  {#if showAddDialog}
    <AddFacilityDialog
      templates={behaviorTemplatesStore.templates}
      onConfirm={handleAddConfirm}
      onCancel={handleAddCancel}
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
