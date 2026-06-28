<script lang="ts">
  /**
   * UnsavedChangesDialog — modal shown when the user tries to
   * open / close / disconnect / exit with unsaved edits in the layout.
   *
   * Spec 018 / S1.2 + dialog-shell-refactor (Slice 2): now wraps the
   * Fluent `Dialog` shell. All chrome (overlay, focus trap, Esc, ×,
   * header/footer dividers) lives in the shell; this component only
   * formats the per-bucket breakdown and wires action buttons.
   *
   * Keyboard (provided by `Dialog`):
   *   Esc / overlay click / × → Cancel (safe default)
   *   Tab / Shift+Tab         → cycles within Cancel ↔ Confirm
   *   Enter on a focused button → triggers that button
   *
   * Note: initial focus is now on Cancel (Fluent norm for destructive
   * alertdialogs). The previous implementation focused Confirm.
   *
   * All counts come from `effectiveNodeStore.dirtyBreakdown`
   * (ADR-0011 extension 2026-06-28).
   */
  import type { DirtyBreakdown } from '$lib/layout';
  import Dialog from './Dialog/Dialog.svelte';
  import DialogTitle from './Dialog/DialogTitle.svelte';
  import DialogActions from './Dialog/DialogActions.svelte';
  import Button from './Dialog/Button.svelte';

  interface Props {
    message: string;
    breakdown: DirtyBreakdown;
    confirmLabel: string;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let { message, breakdown, confirmLabel, onConfirm, onCancel }: Props = $props();

  const lines = $derived(formatBreakdown(breakdown));

  function plural(n: number, singular: string, plural?: string): string {
    return n === 1 ? singular : (plural ?? `${singular}s`);
  }

  function formatBreakdown(b: DirtyBreakdown): string[] {
    const out: string[] = [];
    if (b.config > 0) {
      const fields = `${b.config} ${plural(b.config, 'config edit')}`;
      const across = b.configNodes > 0
        ? ` across ${b.configNodes} ${plural(b.configNodes, 'node')}`
        : '';
      out.push(`${fields}${across}`);
    }
    if (b.metadata > 0) {
      out.push(`${b.metadata} bowtie metadata ${plural(b.metadata, 'edit')}`);
    }
    if (b.facilities > 0) {
      out.push(`${b.facilities} facility ${plural(b.facilities, 'edit')}`);
    }
    if (b.channels > 0) {
      out.push(`${b.channels} channel ${plural(b.channels, 'edit')}`);
    }
    if (b.connectorSelections > 0) {
      out.push(
        `${b.connectorSelections} connector selection ${plural(b.connectorSelections, 'change')}`,
      );
    }
    if (b.offlineDrafts > 0) {
      out.push(`${b.offlineDrafts} offline ${plural(b.offlineDrafts, 'draft')}`);
    }
    if (b.offlineRevertedPersisted > 0) {
      out.push(
        `${b.offlineRevertedPersisted} reverted persisted ${plural(b.offlineRevertedPersisted, 'change')}`,
      );
    }
    if (b.layoutStruct > 0) {
      out.push('layout structure edits');
    }
    if (b.unsavedNewNodes > 0) {
      out.push(
        `${b.unsavedNewNodes} new ${plural(b.unsavedNewNodes, 'node')} not yet added to the layout`,
      );
    }
    if (b.unsavedRemovedNodes > 0) {
      out.push(
        `${b.unsavedRemovedNodes} ${plural(b.unsavedRemovedNodes, 'node')} removed but not yet saved`,
      );
    }
    return out;
  }
</script>

<Dialog
  open
  width="sm"
  role="alertdialog"
  {onCancel}
>
  {#snippet title()}
    <DialogTitle glyph="warning">Unsaved Changes</DialogTitle>
  {/snippet}

  <p class="uc-message">{message}</p>
  {#if lines.length > 0}
    <ul class="uc-breakdown">
      {#each lines as line}
        <li>{line}</li>
      {/each}
    </ul>
  {/if}

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" intent="danger" onclick={onConfirm}>
        {confirmLabel}
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .uc-message {
    margin: 0 0 12px 0;
    color: var(--fluent-neutralForeground1);
  }
  .uc-breakdown {
    margin: 0;
    padding-left: 20px;
    color: var(--fluent-neutralForeground2);
  }
  .uc-breakdown li {
    margin: 2px 0;
  }
</style>
