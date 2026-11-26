<script lang="ts">
  import { untrack } from 'svelte';
  import { facilitiesStore } from '$lib/stores/facilities.svelte';
  import { behaviorTemplatesStore } from '$lib/stores/behaviorTemplates.svelte';
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import type { NodeConfigTree } from '$lib/types/nodeTree';
  import * as facilityOrchestrator from '$lib/orchestration/facilityOrchestrator';
  import AddFacilityDialog from './AddFacilityDialog.svelte';
  import FacilityCard from './FacilityCard.svelte';
  import FacilityRemoveDialog from './FacilityRemoveDialog.svelte';
  import BlockSignalsEditor from '$lib/components/BlockSignals/BlockSignalsEditor.svelte';
  import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
  import { blockSignalsEditorStore } from '$lib/stores/blockSignalsEditor.svelte';
  import { channelsStore } from '$lib/stores/channels.svelte';
  import { eventStateStore } from '$lib/stores/eventState.svelte';
  import { removalConfirmationGroups } from '$lib/utils/facilityRemovalText';
import type { FacilityRemovalClaim } from '$lib/api/facilities';
  import { issueText } from '$lib/components/BlockSignals/wiringText';
  import { blockOccupancyMap } from '$lib/components/BlockSignals/occupancy';
  import { facilityDiagramOrchestrator } from '$lib/orchestration/facilityDiagramOrchestrator';
  import { facilityDiagramsStore } from '$lib/stores/facilityDiagrams.svelte';
  import PanelEditor from '$lib/components/Panel/PanelEditor.svelte';
  import PanelCard from '$lib/components/Panel/PanelCard.svelte';
  import { panelEditorStore } from '$lib/stores/panelEditor.svelte';
  import { panelEditorActions } from '$lib/orchestration/panelEditorOrchestrator';
  import {
    blockSignalsEditorActions,
    startBlockSignalsEditor,
    startEditBlockSignalsEditor,
  } from '$lib/orchestration/blockSignalsEditorOrchestrator';

  let {
    resolvedEventIds,
    nodeTree,
  }: {
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
    nodeTree?: (nodeKey: string) => NodeConfigTree | undefined;
  } = $props();

  let showAddDialog = $state(false);

  // The removal awaiting confirmation: the claims the user was shown, or why the board can't be reset.
  let removal = $state<{
    facilityId: string;
    claims: FacilityRemovalClaim[];
    reason?: string;
    error?: string;
    busy?: boolean;
  } | null>(null);
  const removalFacility = $derived(
    removal ? facilitiesStore.facilities.find((f) => f.facilityId === removal!.facilityId) : undefined,
  );
  const removalGroups = $derived(
    removalConfirmationGroups(removal?.claims ?? [], {
      boardName: (nodeKey) => effectiveLayoutStore.boardName(nodeKey),
      pinLabel: (nodeKey, lampSelection) => effectiveLayoutStore.signalPinLabel(nodeKey, lampSelection),
    }),
  );

  async function startRemoval(facilityId: string) {
    const preview = await facilityOrchestrator.previewFacilityRemoval(facilityId);
    if (!preview.ok) {
      console.error('[facility] previewFacilityRemoval failed', preview.error);
      removal = { facilityId, claims: [], reason: 'The board settings could not be checked.' };
    } else if (preview.issues.length > 0) {
      removal = { facilityId, claims: preview.claims, reason: preview.issues.map((issue) => issueText(issue, [])).join(' ') };
    } else if (preview.claims.length === 0) {
      // Nothing to reset: no confirmation, the generic removal path.
      await facilityOrchestrator.removeFacility({ facilityId }).catch((err) => {
        console.error('[facility] removeFacility failed', err);
      });
    } else {
      removal = { facilityId, claims: preview.claims };
    }
  }

  async function confirmRemoval(leaveBoard: boolean) {
    if (!removal) return;
    const { facilityId, claims } = removal;
    removal = { ...removal, busy: true };
    const result = await facilityOrchestrator.removeFacility({ facilityId, leaveBoard, previewedClaims: claims });
    if (result.ok) {
      removal = null;
    } else if ('planChanged' in result) {
      // The board changed since the preview: show the fresh plan to confirm again.
      await startRemoval(facilityId);
      if (removal && !removal.reason) removal = { ...removal, error: 'The board changed while this was open. Check the list and confirm again.' };
    } else if ('issues' in result) {
      removal = { facilityId, claims, reason: result.issues.map((issue) => issueText(issue, [])).join(' ') };
    } else {
      removal = { facilityId, claims, error: 'The facility could not be removed. Try again.' };
    }
  }

  // Live occupancy of each detected block, from the same events as the Channels panel.
  const blockOccupancy = $derived(
    blockOccupancyMap(channelsStore.channels, resolvedEventIds, eventStateStore.events),
  );
  // Saved Automatic Block Signals facilities draw their track diagram from a backend preview;
  // the orchestrator refetches only the facilities whose saved record changed.
  $effect(() => {
    const facilities = facilitiesStore.facilities;
    // sync reads and writes the diagram store; only a facilities change should re-run it.
    untrack(() => void facilityDiagramOrchestrator.sync(facilities));
  });
  const BLOCK_SIGNALS_TEMPLATE_ID = 'automatic-block-signals';
  const PANEL_TEMPLATE_ID = 'panel';

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
    'automatic-block-signals': 'Signals that respond to occupancy, the next signal, and hand-thrown turnouts.',
    [PANEL_TEMPLATE_ID]: 'Lamps on a fascia or control panel that show which blocks are occupied.',
  };

  const FACILITY_EXAMPLES: Record<string, string> = {
    [PANEL_TEMPLATE_ID]: 'Fascia mimic · dispatcher panel · helix or staging indicators',
  };

  const facilityChoices = $derived(
    [
      ...behaviorTemplatesStore.templates.map((template) => ({
        ...template,
        description: FACILITY_DESCRIPTIONS[template.templateId],
        examples: FACILITY_EXAMPLES[template.templateId],
      })),
      interlockingPlantChoice,
    ],
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
    if (template.templateId === PANEL_TEMPLATE_ID) {
      panelEditorActions.startPanelEditor(name);
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
    void startRemoval(facilityId);
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
          {#if facility.templateId === PANEL_TEMPLATE_ID}
            <PanelCard {facility} {resolvedEventIds} onEdit={panelEditorActions.startPanelEdit} onRemove={handleDelete} />
          {:else}
          <FacilityCard
            {facility}
            template={templateFor(facility.templateId)}
            onRename={handleRename}
            onEdit={startEditBlockSignalsEditor}
            onDelete={handleDelete}
            diagram={facilityDiagramsStore.get(facility.facilityId)}
            occupancy={blockOccupancy}
          />
          {/if}
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

  {#if removal && removalFacility}
    <FacilityRemoveDialog
      facilityName={removalFacility.name}
      groups={removalGroups}
      reason={removal.reason}
      error={removal.error}
      busy={removal.busy}
      onConfirm={confirmRemoval}
      onCancel={() => (removal = null)}
    />
  {/if}

  {#if panelEditorStore.open}
    <PanelEditor
      view={panelEditorStore}
      actions={panelEditorActions}
      detectedBlocks={effectiveLayoutStore.detectedBlocks}
      blockSources={effectiveLayoutStore.blockSources}
    />
  {/if}

  {#if blockSignalsEditorStore.open}
    <BlockSignalsEditor
      view={blockSignalsEditorStore}
      actions={blockSignalsEditorActions}
      detectedBlocks={effectiveLayoutStore.detectedBlocks}
      occupancy={blockOccupancy}
      {nodeTree}
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
