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
  import { countModifiedLeaves } from '$lib/types/nodeTree';
  import { writeModifiedValues, discardModifiedValues } from '$lib/api/config';
  import { bowtieMetadataStore } from '$lib/stores/bowtieMetadata.svelte';
  import { layoutStore } from '$lib/stores/layout.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { updateNodeSnipField } from '$lib/stores/nodeInfo';
  import DiscardConfirmDialog from '$lib/components/DiscardConfirmDialog.svelte';

  interface Props {
    /**
     * When true the component is embedded inside the app toolbar and strips
     * its standalone container padding, margin, border, and flex-wrap so it
     * sits flush within the 40px toolbar height. The functional behaviour is
     * identical in both variants.
     */
    toolbar?: boolean;
  }

  let { toolbar = false }: Props = $props();

  // ── Reactive state ──────────────────────────────────────────────────────────

  let saveProgress = $state<SaveProgress>({
    state: 'idle',
    total: 0,
    completed: 0,
    failed: 0,
    currentFieldLabel: null,
  });

  // Derive dirty count from the tree's modifiedValue leaves
  let dirtyCount = $derived.by(() => {
    let count = 0;
    for (const tree of nodeTreeStore.trees.values()) {
      count += countModifiedLeaves(tree);
    }
    return count;
  });

  let hasConfigEdits = $derived(dirtyCount > 0);
  let hasMetadataEdits = $derived(bowtieMetadataStore.isDirty);
  // T019/T020: Unified dirty state across both config edits and bowtie metadata
  let hasEdits = $derived(hasConfigEdits || hasMetadataEdits);
  let canSave = $derived(hasEdits && saveProgress.state !== 'saving');
  let isSaving = $derived(saveProgress.state === 'saving');
  // Count distinct nodes with modified leaves for discard confirmation
  let dirtyNodeCount = $derived.by(() => {
    const nodeIds = new Set<string>();
    for (const [nodeId, tree] of nodeTreeStore.trees) {
      if (countModifiedLeaves(tree) > 0) nodeIds.add(nodeId);
    }
    return nodeIds.size;
  });

  // Whether the discard confirmation dialog is open.
  let showDiscardDialog = $state(false);

  // ── Save handler ────────────────────────────────────────────────────────────

  async function handleSave() {
    const hasNodeEdits = hasConfigEdits;
    const hasYamlEdits = bowtieMetadataStore.isDirty;

    if (!hasNodeEdits && !hasYamlEdits) return;

    saveProgress = {
      state: 'saving',
      total: dirtyCount + (hasYamlEdits ? 1 : 0),
      completed: 0,
      failed: 0,
      currentFieldLabel: hasNodeEdits ? 'Writing configuration…' : 'Layout metadata',
    };

    let failCount = 0;

    // Write all modified tree leaves in one Rust batch
    if (hasNodeEdits) {
      try {
        const result = await writeModifiedValues();
        saveProgress = {
          ...saveProgress,
          completed: result.succeeded,
          failed: result.failed,
        };
        failCount += result.failed;
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('[SaveControls] writeModifiedValues failed:', msg);
        failCount += dirtyCount;
        saveProgress = { ...saveProgress, failed: dirtyCount };
      }
    }

    // T019: After node writes, save bowtie metadata to YAML layout file
    let yamlSaveOk = true;
    if (bowtieMetadataStore.isDirty) {
      saveProgress = { ...saveProgress, currentFieldLabel: 'Layout metadata' };
      try {
        const saved = await layoutStore.saveCurrentLayout();
        if (saved) bowtieMetadataStore.clearAll();
      } catch (err: unknown) {
        yamlSaveOk = false;
        const msg = err instanceof Error ? err.message : String(err);
        console.error('[SaveControls] Layout save failed:', msg);
        // Attempt Save As as fallback
        try {
          const saved = await layoutStore.saveLayoutAs();
          if (saved) {
            bowtieMetadataStore.clearAll();
            yamlSaveOk = true;
          }
        } catch {
          // Save As also cancelled/failed — metadata remains dirty
        }
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
    const saved = await layoutStore.saveLayoutAs();
    if (saved) bowtieMetadataStore.clearAll();
  }
</script>

{#if hasEdits || isSaving || saveProgress.state === 'completed' || saveProgress.state === 'partial-failure'}
<div
  class="save-controls"
  class:save-controls--toolbar={toolbar}
  role={toolbar ? 'group' : 'toolbar'}
  aria-label="Configuration save controls"
>

  {#if isSaving}
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

{:else if hasEdits}
  <!-- Idle with pending edits -->
  <span class="pending-hint" role="status">
    {dirtyCount + (hasMetadataEdits ? bowtieMetadataStore.editCount : 0)} unsaved change{(dirtyCount + (hasMetadataEdits ? bowtieMetadataStore.editCount : 0)) === 1 ? '' : 's'}
  </span>
{/if}

<button
  class="save-btn"
  disabled={!canSave}
  onclick={handleSave}
  aria-busy={isSaving}
>
  {isSaving ? 'Saving…' : 'Save'}
</button>

<button
  class="discard-btn"
  disabled={isSaving || !hasEdits}
  onclick={handleDiscard}
  aria-label="Discard pending changes"
>
  Discard
</button>

</div>
{/if}

{#if showDiscardDialog}
  <DiscardConfirmDialog
    fieldCount={dirtyCount}
    nodeCount={dirtyNodeCount}
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
