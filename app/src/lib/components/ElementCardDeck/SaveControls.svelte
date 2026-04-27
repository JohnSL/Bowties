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
  import { writeModifiedValues, discardModifiedValues } from '$lib/api/config';
  import { toast } from '@zerodevx/svelte-toast';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
  import { updateNodeSnipField } from '$lib/stores/nodeInfo';
  import DiscardConfirmDialog from '$lib/components/DiscardConfirmDialog.svelte';
  import { deriveSaveControlsViewState } from './saveControlsPresenter';

  interface Props {
    /**
     * When true the component is embedded inside the app toolbar and strips
     * its standalone container padding, margin, border, and flex-wrap so it
     * sits flush within the 40px toolbar height. The functional behaviour is
     * identical in both variants.
     */
    toolbar?: boolean;
    /**
     * Optional offline-save handler provided by the page route.
     * Returns true when a save was performed, false when user cancelled.
     */
    onOfflineSave?: () => Promise<boolean>;
    onOfflineSaveAs?: () => Promise<boolean>;
  }

  let {
    toolbar = false,
    onOfflineSave = async () => false,
    onOfflineSaveAs = async () => false,
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
    layoutIsDirty: layoutStore.isDirty,
    layoutIsOfflineMode: layoutStore.isOfflineMode,
    offlineDraftCount: offlineChangesStore.draftCount,
    offlineDraftRows: offlineChangesStore.draftRows,
    saveProgressState: saveProgress.state,
    trees: nodeTreeStore.trees,
  }));

  // Whether the discard confirmation dialog is open.
  let showDiscardDialog = $state(false);

  function reapplyPersistedOfflinePendingValues(): void {
    nodeTreeStore.clearAllModifiedValues();
    nodeTreeStore.applyOfflinePendingValues(offlineChangesStore.persistedRows);
  }

  // ── Save handler ────────────────────────────────────────────────────────────

  async function handleSave() {
    if (layoutStore.isOfflineMode) {
      const localPending = offlineChangesStore.draftCount;
      saveProgress = {
        state: 'saving',
        total: localPending,
        completed: 0,
        failed: 0,
        currentFieldLabel: 'Offline layout changes',
      };

      try {
        const saved = await onOfflineSave();
        if (saved) {
          await offlineChangesStore.reloadFromBackend();
          reapplyPersistedOfflinePendingValues();
          bowtieMetadataStore.clearAll();
          layoutStore.markClean();
          saveProgress = { ...saveProgress, state: 'completed', currentFieldLabel: null, completed: localPending };
        } else {
          // Save was cancelled (for example, Save As dialog dismissed).
          saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
        }
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('[SaveControls] offline save failed:', msg);
        toast.push(`Offline save failed: ${msg}`, {
          classes: ['warn'],
          duration: 7000,
          pausable: true,
        });
        saveProgress = { ...saveProgress, state: 'partial-failure', currentFieldLabel: null, failed: localPending };
      }

      if (saveProgress.state === 'completed') {
        setTimeout(() => {
          saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
        }, 2000);
      }
      return;
    }

    const hasNodeEdits = viewState.hasConfigEdits;
    const hasYamlEdits = bowtieMetadataStore.isDirty;

    if (!hasNodeEdits && !hasYamlEdits) return;

    saveProgress = {
      state: 'saving',
      total: viewState.dirtyCount + (hasYamlEdits ? 1 : 0),
      completed: 0,
      failed: 0,
      currentFieldLabel: hasNodeEdits ? 'Writing configuration…' : 'Layout metadata',
    };

    let failCount = 0;

    // Write all modified tree leaves in one Rust batch
    if (hasNodeEdits) {
      try {
        const result = await writeModifiedValues();
        if ((result.readOnlyRejected ?? 0) > 0) {
          const n = result.readOnlyRejected;
          toast.push(
            `${n} read-only field${n === 1 ? '' : 's'} reverted — device rejected the write`,
            { classes: ['warn'], duration: 6000, pausable: true }
          );
        }
        saveProgress = {
          ...saveProgress,
          completed: result.succeeded,
          failed: result.failed,
        };
        failCount += result.failed;
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('[SaveControls] writeModifiedValues failed:', msg);
        failCount += viewState.dirtyCount;
        saveProgress = { ...saveProgress, failed: viewState.dirtyCount };
      }
    }

    // T019: After node writes, save bowtie metadata to YAML layout file
    let yamlSaveOk = true;
    if (bowtieMetadataStore.isDirty) {
      saveProgress = { ...saveProgress, currentFieldLabel: 'Layout metadata' };
      try {
        const saved = await onOfflineSave();
        if (saved) {
          bowtieMetadataStore.clearAll();
        } else {
          yamlSaveOk = false;
        }
      } catch (err: unknown) {
        yamlSaveOk = false;
        const msg = err instanceof Error ? err.message : String(err);
        console.error('[SaveControls] Layout save failed:', msg);
      }
    }

    const finalState: SaveState = failCount === 0 && yamlSaveOk ? 'completed' : 'partial-failure';
    saveProgress = { ...saveProgress, state: finalState, currentFieldLabel: null };

    // Auto-dismiss 'completed' state after 2s
    if (finalState === 'completed') {
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
      reapplyPersistedOfflinePendingValues();
      bowtieMetadataStore.clearAll();
      layoutStore.revertToSaved();
      saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
      return;
    }

    await discardModifiedValues();
    bowtieMetadataStore.clearAll();
    layoutStore.revertToSaved();
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
    const saved = await onOfflineSaveAs();
    if (saved) {
      await offlineChangesStore.reloadFromBackend();
      reapplyPersistedOfflinePendingValues();
      bowtieMetadataStore.clearAll();
      layoutStore.markClean();
    }
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
  <span class="pending-hint" role="status">
    {viewState.pendingHintText}
  </span>
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
