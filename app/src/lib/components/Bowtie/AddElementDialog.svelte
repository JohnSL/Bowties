<!--
  T030: AddElementDialog.svelte
  Modal dialog to add a producer or consumer element to an existing bowtie.

  dialog-shell-refactor (Slice 5): wraps the Fluent `Dialog` shell. Body
  uses a fixed-height flex column so the inner `ElementPicker` scroll
  container can size correctly.

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
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';

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
</script>

<Dialog
  open={visible}
  width={520}
  ariaLabel={`Add ${role} to ${bowtieName}`}
  initialFocus="none"
  onCancel={onCancel}
>
  {#snippet title()}
    <DialogTitle>
      <span class="aed-title-text">
        Add <span class="role-badge role-{role.toLowerCase()}">{role}</span>
        to <span class="bowtie-name">{bowtieName}</span>
      </span>
    </DialogTitle>
  {/snippet}

  <div class="aed-body">
    <ElementPicker
      roleFilter={role}
      onSelect={(s) => { selection = s; }}
      selectedElement={selection}
    />
  </div>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" onclick={handleConfirm} disabled={!selection}>
        Add {role}
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .aed-title-text {
    display: inline-flex;
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
    color: var(--fluent-neutralForeground2);
  }

  /* Body fills a fixed height so the inner ElementPicker scroll container
     can size correctly. Matches the NewConnectionDialog pattern. */
  .aed-body {
    display: flex;
    flex-direction: column;
    height: min(60vh, 520px);
    min-height: 0;
  }
</style>
