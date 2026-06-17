<script lang="ts">
  /**
   * SaveControls — Global Save button and progress display.
   *
   * Operates on ALL modified tree leaves across every node and segment.
   * The Rust backend writes all modified values in one batch via
   * `write_modified_values`, then sends Update Complete per node.
   *
   * Per FR-013a (progress feedback), FR-022 (Update Complete after all writes).
   *
   * Spec: 007-edit-node-config.
   */
  import type { SaveProgress, SaveState } from '$lib/types/nodeTree';
  import { discardModifiedValues } from '$lib/api/config';
  import { toast } from '@zerodevx/svelte-toast';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { effectiveNodeStore } from '$lib/layout';
  import { connectorSelectionsStore } from '$lib/stores/connectorSelections.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
  import { configChangesStore } from '$lib/stores/configChanges.svelte';
  import { discardAllConfigDrafts } from '$lib/orchestration/configDraftOrchestrator';
  import { deriveSaveControlsViewState } from '$lib/components/ElementCardDeck/saveControlsPresenter';
  import DiscardConfirmDialog from '$lib/components/DiscardConfirmDialog.svelte';

  interface Props {
    /**
     * When true the component is embedded inside the app toolbar and strips
     * its standalone container padding, margin, border, and flex-wrap so it
     * sits flush within the 40px toolbar height. The functional behaviour is
     * identical in both variants.
     */
    toolbar?: boolean;
    /**
     * Save handler provided by the page route. Returns `true` when a save
     * was performed, `false` when the user cancelled. The page owns
     * orchestrator wiring (online vs offline path, draft staging,
     * post-save cleanup) \u2014 this component just delegates.
     */
    onSave?: () => Promise<boolean>;
    onSaveAs?: () => Promise<boolean>;
  }

  let {
    toolbar = false,
    onSave = async () => false,
    onSaveAs = async () => false,
  }: Props = $props();

  // ── Reactive state ──────────────────────────────────────────────────────────

  let saveProgress = $state<SaveProgress>({
    state: 'idle',
    total: 0,
    completed: 0,
    failed: 0,
    currentFieldLabel: null,
  });

  let viewState = $derived(deriveSaveControlsViewState({
    bowtieMetadataEditCount: bowtieMetadataStore.editCount,
    bowtieMetadataIsDirty: bowtieMetadataStore.isDirty,
    configDraftCount: configChangesStore.draftEntries().length,
    connectorWarningCount: connectorSelectionsStore.totalWarningCount,
    layoutIsDirty: layoutStore.isDirty,
    layoutIsOfflineMode: layoutStore.isOfflineMode,
    offlineDraftCount: offlineChangesStore.draftCount,
    offlineDraftRows: offlineChangesStore.draftRows,
    revertedPersistedCount: offlineChangesStore.revertedPersistedCount,
    saveProgressState: saveProgress.state,
    treeNodeIds: [...nodeTreeStore.trees.keys()],
    unsavedInMemoryNodeCount: effectiveNodeStore.unsavedInMemoryNodeIds.length,
  }));

  // Whether the discard confirmation dialog is open.
  let showDiscardDialog = $state(false);

  function rehydrateConnectorSelectionsFromLayout(): void {
    const layout = layoutStore.layout;
    connectorSelectionsStore.hydrateFromLayout(layout);
  }

  // ── Save handler ────────────────────────────────────────────────────────────
  //
  // Thin delegate: the page route owns draft staging, orchestrator wiring,
  // and post-save cleanup (markClean, clearMetadata, offline-changes reload,
  // configChangesStore commit). This handler exists only to:
  //   1. Honour the presenter's `canSave` gate (no parallel gate divergence).
  //   2. Drive `saveProgress` so the UI reflects in-flight state.
  //   3. Surface failures via toast.
  // Mode-specific behaviour lives entirely in `onSave` (and the orchestrator
  // it calls), not here.

  async function handleSave() {
    if (!viewState.canSave) return;

    const totalEdits = viewState.pendingEditCount;
    saveProgress = {
      state: 'saving',
      total: totalEdits,
      completed: 0,
      failed: 0,
      currentFieldLabel: layoutStore.isOfflineMode ? 'Offline layout changes' : 'Saving\u2026',
    };

    try {
      const saved = await onSave();
      if (saved) {
        saveProgress = { ...saveProgress, state: 'completed', currentFieldLabel: null, completed: totalEdits };
      } else {
        // User cancelled (e.g. Save As dialog dismissed). ADR-0001: zero bus writes.
        saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('[SaveControls] save failed:', msg);
      toast.push(`Save failed: ${msg}`, {
        classes: ['warn'],
        duration: 7000,
        pausable: true,
      });
      saveProgress = { ...saveProgress, state: 'partial-failure', currentFieldLabel: null, failed: totalEdits };
    }

    if (saveProgress.state === 'completed') {
      setTimeout(() => {
        saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
      }, 2000);
    }
  }

  function handleDiscard() {
    showDiscardDialog = true;
  }

  // T020: Unified discard — clear both tree modifications and bowtie metadata
  async function handleConfirmDiscard() {
    showDiscardDialog = false;

    if (layoutStore.isOfflineMode) {
      await offlineChangesStore.revertAllPending();
      discardAllConfigDrafts();
      bowtieMetadataStore.clearAll();
      layoutStore.revertToSaved();
      rehydrateConnectorSelectionsFromLayout();
      saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
      return;
    }

    await discardModifiedValues();
    discardAllConfigDrafts();
    bowtieMetadataStore.clearAll();
    layoutStore.revertToSaved();
    rehydrateConnectorSelectionsFromLayout();
    saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
  }

  function handleCancelDiscard() {
    showDiscardDialog = false;
  }

  // Exposed imperative API — callable via bind:this from a parent component.
  // Used by the File → Save Layout (Ctrl+S) and Save Layout As… menu listeners.
  export function triggerSave(): void {
    handleSave();
  }

  export async function triggerSaveAs(): Promise<void> {
    // Thin delegate \u2014 the page route's `saveCurrentCaptureToFile(true)`
    // owns all cleanup via the orchestrator's wired callbacks.
    await onSaveAs();
  }
</script>

{#if viewState.hasEdits || viewState.isSaving || saveProgress.state === 'completed' || saveProgress.state === 'partial-failure'}
<div
  class="save-controls"
  class:save-controls--toolbar={toolbar}
  role={toolbar ? 'group' : 'toolbar'}
  aria-label="Configuration save controls"
>

  {#if viewState.isSaving}
  <span class="save-progress" role="status" aria-live="polite">
    {#if saveProgress.currentFieldLabel}
      Writing "{saveProgress.currentFieldLabel}"…
    {:else}
      Saving {saveProgress.completed}/{saveProgress.total}…
    {/if}
  </span>
  <span class="progress-count" aria-hidden="true">
    {saveProgress.completed}/{saveProgress.total}
  </span>

{:else if saveProgress.state === 'completed'}
  <span class="save-status save-status--ok" role="status">✓ Saved</span>

{:else if saveProgress.state === 'partial-failure'}
  <span class="save-status save-status--warn" role="status">
    ⚠ {saveProgress.completed} saved, {saveProgress.failed} failed
  </span>

{:else if viewState.hasEdits}
  <!-- Idle with pending edits -->
  <div class="pending-summary" role="status">
    <span class="pending-hint">{viewState.pendingHintText}</span>
    {#if viewState.connectorWarningCount > 0}
      <span class="repair-warning">{viewState.connectorWarningCount} connector warning{viewState.connectorWarningCount === 1 ? '' : 's'}</span>
    {/if}
  </div>
{/if}

<button
  class="save-btn"
  disabled={!viewState.canSave}
  onclick={handleSave}
  aria-busy={viewState.isSaving}
>
  {viewState.isSaving ? 'Saving…' : 'Save'}
</button>

<button
  class="discard-btn"
  disabled={viewState.isSaving || !viewState.hasEdits}
  onclick={handleDiscard}
  aria-label="Discard pending changes"
>
  Discard
</button>

</div>
{/if}

{#if showDiscardDialog}
  <DiscardConfirmDialog
    fieldCount={viewState.discardFieldCount}
    nodeCount={viewState.discardNodeCount}
    onConfirm={handleConfirmDiscard}
    onCancel={handleCancelDiscard}
  />
{/if}

<style>
  /* ══════════════════════════════════════════
     Fluent UI Design — SaveControls
     ══════════════════════════════════════════ */

  .save-controls {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    margin-bottom: 8px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    font-size: 12px;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    flex-wrap: wrap;
    transition: background-color 0.15s ease, border-color 0.15s ease;
  }

  /* When there are pending edits (or saving/done), show the full toolbar background */

  /* Toolbar variant — flush inside the 40px app toolbar, left-separator, tighter gap */
  .save-controls--toolbar {
    padding: 0 2px 0 6px;
    margin-bottom: 0;
    margin-left: 2px;
    border-left-color: #d1d5db;
    border-radius: 0;
    flex-wrap: nowrap;
    gap: 4px;
  }
  .pending-hint {
    flex: 1;
    color: #835b00;                                /* amber: unsaved changes */
    font-style: italic;
  }

  .pending-summary {
    flex: 1;
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }

  .repair-warning {
    color: #ca5010;
  }

  .save-progress {
    flex: 1;
    color: #323130;
  }

  .progress-count {
    color: #605e5c;
    font-variant-numeric: tabular-nums;
  }

  .save-status {
    flex: 1;
  }

  .save-status--ok {
    color: #0b6a0b;                                /* colorPaletteGreenForeground1 */
    font-weight: 500;
  }

  .save-status--warn {
    color: #ca5010;                                /* colorPaletteOrangeForeground1 */
    font-weight: 500;
  }

  /* ── Save button — white pill with blue accent text ── */

  .save-btn {
    display: flex;
    align-items: center;
    padding: 4px 10px;
    font-size: 13px;
    font-weight: 500;
    font-family: inherit;
    color: #0078d4;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, border-color 0.12s, box-shadow 0.12s;
  }

  .save-btn:hover:not(:disabled) {
    background: #eff6ff;
    border-color: #bfdbfe;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .save-btn:active:not(:disabled) {
    background: #dbeafe;
  }

  .save-btn:disabled {
    background: #fafafa;
    border-color: #ebebeb;
    box-shadow: none;
    color: #bbb;
    cursor: not-allowed;
    pointer-events: none;
  }

  /* ── Discard button — white pill ghost ── */

  .discard-btn {
    display: flex;
    align-items: center;
    padding: 4px 10px;
    font-size: 13px;
    font-family: inherit;
    color: #374151;
    background: #ffffff;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, border-color 0.12s, box-shadow 0.12s;
  }

  .discard-btn:hover:not(:disabled) {
    background: #f0f4ff;
    border-color: #c7d2fe;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .discard-btn:active:not(:disabled) {
    background: #e0e7ff;
  }

  .discard-btn:disabled {
    background: #fafafa;
    border-color: #ebebeb;
    box-shadow: none;
    color: #bbb;
    cursor: not-allowed;
    pointer-events: none;
  }
</style>
