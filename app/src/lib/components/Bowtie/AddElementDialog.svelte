<!--
  T030: AddElementDialog.svelte
  Modal dialog to add a producer or consumer element to an existing bowtie.

  Props:
    visible: boolean — whether the dialog is shown
    role: 'Producer' | 'Consumer' — which side to add
    bowtieName: string — display name of the bowtie (for dialog title)
    onConfirm: callback with the selected ElementSelection
    onCancel: callback when user cancels
-->

<script lang="ts">
  import ElementPicker from './ElementPicker.svelte';
  import type { ElementSelection } from '$lib/types/bowtie';

  interface Props {
    visible: boolean;
    role: 'Producer' | 'Consumer';
    bowtieName: string;
    onConfirm: (selection: ElementSelection) => void;
    onCancel: () => void;
  }

  let { visible, role, bowtieName, onConfirm, onCancel }: Props = $props();

  let selection = $state<ElementSelection | null>(null);

  // Reset selection when dialog opens
  $effect(() => {
    if (visible) selection = null;
  });

  function handleConfirm() {
    if (selection) onConfirm(selection);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onCancel();
  }
</script>

{#if visible}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="dialog-backdrop"
    role="dialog"
    aria-modal="true"
    aria-label="Add {role} to {bowtieName}"
    tabindex="-1"
    onkeydown={handleKeydown}
  >
    <div
      class="dialog"
      role="document"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <header class="dialog-header">
        <h2 class="dialog-title">
          Add <span class="role-badge role-{role.toLowerCase()}">{role}</span>
          to <span class="bowtie-name">{bowtieName}</span>
        </h2>
        <button class="close-btn" onclick={onCancel} aria-label="Close dialog">✕</button>
      </header>

      <div class="dialog-body">
        <ElementPicker
          roleFilter={role}
          onSelect={(s) => { selection = s; }}
          selectedElement={selection}
        />
      </div>

      <footer class="dialog-footer">
        <button class="btn btn-secondary" onclick={onCancel}>Cancel</button>
        <button
          class="btn btn-primary"
          onclick={handleConfirm}
          disabled={!selection}
        >
          Add {role}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .dialog {
    background: #fff;
    border-radius: 8px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
    width: 520px;
    max-width: 95vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 10px;
    border-bottom: 1px solid #e5e7eb;
    flex-shrink: 0;
  }

  .dialog-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: #1f2937;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
  }

  .role-badge {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 2px 8px;
    border-radius: 4px;
    display: inline-block;
  }

  .role-badge.role-producer {
    color: #0b6a0b;
    background: #dff6dd;
  }

  .role-badge.role-consumer {
    color: #0078d4;
    background: #deecf9;
  }

  .bowtie-name {
    font-family: 'ui-monospace', monospace;
    font-size: 0.9rem;
    color: #374151;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 0.9rem;
    color: #6b7280;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .close-btn:hover {
    background: #f3f4f6;
    color: #374151;
  }

  .dialog-body {
    flex: 1;
    overflow: hidden;
    padding: 12px;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 16px;
    border-top: 1px solid #e5e7eb;
    flex-shrink: 0;
  }

  .btn {
    padding: 6px 16px;
    font-size: 0.85rem;
    font-weight: 500;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.15s, border-color 0.15s;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    color: #374151;
    background: #fff;
    border-color: #d1d5db;
  }

  .btn-secondary:hover:not(:disabled) {
    background: #f9fafb;
  }

  .btn-primary {
    color: #fff;
    background: #2563eb;
    border-color: #2563eb;
  }

  .btn-primary:hover:not(:disabled) {
    background: #1d4ed8;
  }
</style>
