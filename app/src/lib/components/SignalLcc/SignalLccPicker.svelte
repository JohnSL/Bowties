<script lang="ts">
  /**
   * SignalLccPicker — a labelled field showing the chosen Signal LCC (display
   * name over "<model> · <address>") that opens a searchable dialog to choose
   * another. Shared by the Automatic Block Signals and Panel Wiring steps; it
   * renders the candidates it is given and emits the confirmed node key.
   */
  import SearchableSingleSelectDialog from '$lib/components/SingleSelectList/SearchableSingleSelectDialog.svelte';
  import { signalLccLabel, signalLccName } from './signalLccLabel';

  let {
    nodeKeys,
    selected,
    label,
    onChoose,
    open = $bindable(false),
    disabled = false,
  }: {
    nodeKeys: string[];
    selected: string | null;
    label: string;
    onChoose: (nodeKey: string) => void;
    open?: boolean;
    disabled?: boolean;
  } = $props();

  const uid = $props.id();
  const items = $derived(
    [...nodeKeys].sort((a, b) => signalLccName(a).localeCompare(signalLccName(b))).map((key) => ({ key })),
  );
  const current = $derived(selected === null ? null : signalLccLabel(selected));

  function choose(nodeKey: string): void {
    onChoose(nodeKey);
    open = false;
  }
</script>

<div class="signal-lcc-picker">
  <span class="field-label" id={`${uid}-label`}>{label}</span>
  <button type="button" class="node-field" {disabled} aria-haspopup="dialog" aria-labelledby={`${uid}-label ${uid}-name`} onclick={() => (open = true)}>
    <span class="node-text" id={`${uid}-name`}>
      {#if current}
        <strong>{current.name}</strong>
        <span class="node-detail">{current.detail}</span>
      {:else}
        <strong>Choose a Signal LCC</strong>
      {/if}
    </span>
    <span class="node-action" aria-hidden="true">Change ▾</span>
  </button>
</div>

{#if open}
  <SearchableSingleSelectDialog
    title="Choose a Signal LCC"
    filterLabel="Search Signal LCC nodes"
    {items}
    searchTextFor={signalLccName}
    ariaLabel="Signal LCC nodes"
    name="signal-lcc"
    emptyMessage="No Signal LCC nodes are available."
    onConfirm={choose}
    onCancel={() => (open = false)}
  >
    {#snippet row(id)}
      {@const rowLabel = signalLccLabel(id)}
      <span class="host-row">
        <strong>{rowLabel.name}</strong>
        <span class="node-detail">{rowLabel.detail}</span>
      </span>
    {/snippet}
  </SearchableSingleSelectDialog>
{/if}

<style>
  .signal-lcc-picker { display: flex; align-items: center; gap: 12px; }
  .signal-lcc-picker .field-label { margin: 0; }
  .node-field {
    display: flex; flex: 1; justify-content: space-between; align-items: center; gap: 8px;
    min-height: 30px; padding: 5px 10px; border: 1px solid var(--abs-border-strong, var(--pe-border-strong, #b8b8b8)); border-radius: 4px;
    background: #fff; color: var(--abs-text, var(--pe-text, #242424)); font: inherit; text-align: left; cursor: pointer;
  }
  .node-field:hover:not(:disabled) { background: #f5f5f5; }
  .node-field:disabled { cursor: default; }
  .node-text { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .node-detail { font-size: 11px; color: var(--abs-muted, var(--pe-muted, #616161)); }
  .host-row { display: inline-flex; flex-direction: column; gap: 1px; }
  .node-action { color: var(--abs-brand, var(--pe-brand, #0f6cbd)); font-size: 11px; }
</style>
