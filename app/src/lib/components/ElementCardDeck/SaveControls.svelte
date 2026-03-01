<script lang="ts">
  /**
   * SaveControls — Global Save button and progress display.
   *
   * Operates on ALL pending edits across every node and segment, not just
   * the currently visible one. This allows users to freely navigate between
   * segments and nodes while accumulating edits, then save everything in one
   * click.
   *
   * Per FR-013a (progress feedback), FR-014 (blocked when invalid),
   * FR-022 (Update Complete after all writes), FR-021 (value cache sync).
   *
   * Spec: 007-edit-node-config.
   */
  import type { SaveProgress, SaveState } from '$lib/types/nodeTree';
  import { pendingEditsStore, pendingEditsVersion } from '$lib/stores/pendingEdits.svelte';
  import { nodeTreeStore } from '$lib/stores/nodeTree.svelte';
  import { updateNodeSnipField } from '$lib/stores/nodeInfo';
  import { serializeConfigValue } from '$lib/utils/serialize';
  import { writeConfigValue, sendUpdateComplete } from '$lib/api/config';
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

  // Subscribe to the version counter so $derived below re-evaluates on every
  // store mutation. Svelte 5 $derived doesn't reliably track Map.values()
  // iteration inside external class methods without this explicit dependency.
  let _version = $derived($pendingEditsVersion);

  // Derived: all dirty or error edits across every node/segment (T046: retry includes error)
  let dirtyEdits = $derived(_version >= 0 ? pendingEditsStore.getRetryableAll() : []);
  let hasEdits = $derived(dirtyEdits.length > 0);
  let hasInvalid = $derived(_version >= 0 && pendingEditsStore.hasInvalid);
  let canSave = $derived(hasEdits && !hasInvalid && saveProgress.state !== 'saving');
  let isSaving = $derived(saveProgress.state === 'saving');
  // Distinct node count for the discard confirmation message.
  let dirtyNodeCount = $derived(new Set(dirtyEdits.map((e) => e.nodeId)).size);

  // Whether the discard confirmation dialog is open.
  let showDiscardDialog = $state(false);

  // ── Save handler ────────────────────────────────────────────────────────────

  async function handleSave() {
    const edits = pendingEditsStore.getRetryableAll();
    if (edits.length === 0) return;

    saveProgress = {
      state: 'saving',
      total: edits.length,
      completed: 0,
      failed: 0,
      currentFieldLabel: edits[0]?.fieldLabel ?? null,
    };

    let failCount = 0;
    // Track which nodes had at least one successful write (for Update Complete)
    const successNodeIds = new Set<string>();

    for (let i = 0; i < edits.length; i++) {
      const edit = edits[i];
      saveProgress = { ...saveProgress, currentFieldLabel: edit.fieldLabel };

      // Transition store to writing state
      pendingEditsStore.markWriting(edit.key);

      try {
        // Serialize the pending value to bytes
        const bytes = serializeConfigValue(edit.pendingValue, edit.elementType, edit.size);

        // Write to node — each edit carries its own nodeId
        const result = await writeConfigValue(edit.nodeId, edit.address, edit.space, bytes);

        if (result.success) {
          // Mark clean (removes from store)
          pendingEditsStore.markClean(edit.key);
          saveProgress = { ...saveProgress, completed: saveProgress.completed + 1 };
          successNodeIds.add(edit.nodeId);

          // Update in-memory tree cache so the leaf shows the written value (FR-021)
          nodeTreeStore.updateLeafValue(edit.nodeId, edit.fieldPath, edit.pendingValue);

          // If this is an ACDI User space field (0xFB), patch nodeInfoStore so the
          // sidebar node name / tooltip updates immediately without re-discovery.
          //   offset 1  = user_name
          //   offset 64 = user_description
          const ACDI_USER_SPACE = 0xFB;
          if (edit.space === ACDI_USER_SPACE && edit.pendingValue.type === 'string') {
            if (edit.address === 1) {
              updateNodeSnipField(edit.nodeId, 'user_name', edit.pendingValue.value);
            } else if (edit.address === 64) {
              updateNodeSnipField(edit.nodeId, 'user_description', edit.pendingValue.value);
            }
          }
        } else {
          failCount++;
          pendingEditsStore.markError(
            edit.key,
            result.errorMessage ?? 'Write failed'
          );
          saveProgress = { ...saveProgress, failed: saveProgress.failed + 1 };
        }
      } catch (err: unknown) {
        failCount++;
        const msg = err instanceof Error ? err.message : String(err);
        pendingEditsStore.markError(edit.key, msg);
        saveProgress = { ...saveProgress, failed: saveProgress.failed + 1 };
      }
    }

    // Send Update Complete to each node that received at least one successful write (FR-022)
    for (const nid of successNodeIds) {
      try {
        await sendUpdateComplete(nid);
      } catch {
        // Non-fatal — node might not require it in all cases
      }
    }

    const finalState: SaveState = failCount === 0 ? 'completed' : 'partial-failure';
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

  function handleConfirmDiscard() {
    showDiscardDialog = false;
    pendingEditsStore.clearAll();
    saveProgress = { state: 'idle', total: 0, completed: 0, failed: 0, currentFieldLabel: null };
  }

  function handleCancelDiscard() {
    showDiscardDialog = false;
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
  {#if hasInvalid}
    <span class="invalid-hint" role="status">Fix invalid fields before saving</span>
  {:else}
    <span class="pending-hint" role="status">
      {dirtyEdits.length} unsaved change{dirtyEdits.length === 1 ? '' : 's'}
    </span>
  {/if}
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
    fieldCount={dirtyEdits.length}
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

  .invalid-hint {
    flex: 1;
    color: #a4262c;                                /* colorPaletteRedForeground1 */
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
