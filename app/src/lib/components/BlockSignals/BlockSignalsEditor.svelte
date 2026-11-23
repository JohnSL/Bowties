<script lang="ts">
  /**
   * BlockSignalsEditor — the transient "New Automatic Block Signals" dialog.
   * Blocks, Aspects, Appearance and Wiring are available; Review is listed but
   * disabled. Done on Wiring is offered only when the store allows finishing.
   *
   * The shared look of both steps (panels, outlined choices, section labels,
   * two-choice selectors, tables) lives here, scoped under `.abs-editor`.
   */
  import type { SelectedDetectedBlock } from '$lib/api/signaling';
  import type { NodeConfigTree } from '$lib/types/nodeTree';
  import type { BlockSignalsEditorView, BlockSignalsStep } from '$lib/stores/blockSignalsEditor.svelte';
  import type { BlockSignalsEditorActions } from '$lib/orchestration/blockSignalsEditorOrchestrator';
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';
  import BlocksStep from './BlocksStep.svelte';
  import AspectsStep from './AspectsStep.svelte';
  import AppearanceStep from './AppearanceStep.svelte';
  import WiringStep from './WiringStep.svelte';
  import type { BlockOccupancyMap } from './occupancy';

  let {
    view,
    actions: intents,
    detectedBlocks,
    occupancy,
    nodeTree,
  }: {
    view: BlockSignalsEditorView;
    actions: BlockSignalsEditorActions;
    detectedBlocks: SelectedDetectedBlock[];
    /** Live occupancy by block id; missing blocks show as unavailable. */
    occupancy?: BlockOccupancyMap;
    /** Resolves a node's configuration tree, for jumping to a pin's owner in Configuration. */
    nodeTree?: (nodeKey: string) => NodeConfigTree | undefined;
  } = $props();

  let pickerOpen = $state(false);
  let selectedSignalId = $state<string | null>(null);

  const STEP_TITLE: Record<BlockSignalsStep, string> = {
    blocks: 'Blocks',
    aspects: 'Aspects',
    appearance: 'Appearance',
    wiring: 'Wiring',
    review: 'Review',
  };
  const STEPS = $derived(view.steps.map((item) => ({ ...item, title: STEP_TITLE[item.id] })));
  const hasBlocks = $derived(view.blocks.length > 0);
  const dialogHeading = $derived(
    view.mode === 'edit'
      ? `Edit Automatic Block Signals · ${view.facilityName} · ${STEP_TITLE[view.step]}`
      : `New Automatic Block Signals · ${STEP_TITLE[view.step]}`,
  );
</script>

{#snippet leading()}
  <Button appearance="secondary" onclick={intents.cancel}>Cancel</Button>
{/snippet}

<Dialog
  open
  width={1120}
  bodyLayout="contained"
  ariaLabel={dialogHeading}
  initialFocus="none"
  onCancel={() => {
    if (!pickerOpen) intents.cancel();
  }}
>
  {#snippet title()}
    <DialogTitle>{dialogHeading}</DialogTitle>
  {/snippet}

  <div class="abs-editor">
    <ol class="stepper" aria-label="Automatic Block Signals steps">
      {#each STEPS as item, index}
        <li class="step {item.status}" class:active={view.step === item.id}>
          <button
            type="button"
            aria-current={view.step === item.id ? 'step' : undefined}
            disabled={item.status === 'unavailable'}
            title={item.status === 'unavailable' ? item.unavailableReason : undefined}
            onclick={() => intents.navigate(item.id)}
          >
            <span class="step-n">{index + 1}</span>{item.title}{#if item.status === 'complete'}
              <span class="step-mark" aria-hidden="true">✓</span><span class="visually-hidden">, complete</span>
            {:else if item.status === 'needsWork'}
              <span class="step-mark warn" aria-hidden="true">!</span><span class="visually-hidden">, needs attention</span>
            {:else if item.status === 'unavailable' && item.unavailableReason}
              <span class="visually-hidden">, {item.unavailableReason.toLowerCase()}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ol>

    {#if view.error}
      <p class="error" role="alert">{view.error}</p>
    {/if}
    <div class="workspace">
      {#if view.step === 'blocks'}
        <BlocksStep {view} actions={intents} {detectedBlocks} {occupancy} bind:pickerOpen />
      {:else if view.step === 'aspects'}
        <AspectsStep {view} actions={intents} {occupancy} bind:selectedSignalId />
      {:else if view.step === 'appearance'}
        <AppearanceStep {view} />
      {:else if view.step === 'wiring'}
        <WiringStep {view} actions={intents} {occupancy} {nodeTree} bind:selectedSignalId bind:pickerOpen />
      {/if}
    </div>
  </div>

  {#snippet actions()}
    <DialogActions {leading}>
      {#if view.step === 'blocks'}
        <Button appearance="primary" disabled={!hasBlocks} onclick={() => intents.navigate('aspects')}>Next: Aspects</Button>
      {:else if view.step === 'aspects'}
        <Button appearance="secondary" onclick={() => intents.navigate('blocks')}>Back</Button>
        <Button appearance="primary" onclick={() => intents.navigate('appearance')}>Next: Appearance</Button>
      {:else if view.step === 'appearance'}
        <Button appearance="secondary" onclick={() => intents.navigate('aspects')}>Back</Button>
        <Button appearance="primary" onclick={() => intents.navigate('wiring')}>Next: Wiring</Button>
      {:else if view.step === 'wiring'}
        <Button appearance="secondary" onclick={() => intents.navigate('appearance')}>Back</Button>
        <Button appearance="primary" disabled={!view.canFinish} onclick={intents.finish}>Done</Button>
      {/if}
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .abs-editor {
    --abs-border: #d1d1d1;
    --abs-border-strong: #b8b8b8;
    --abs-border-soft: #eceef1;
    --abs-subtle: #fafafa;
    --abs-selected: #ebf3fc;
    --abs-brand: #0f6cbd;
    --abs-brand-hover: #115ea3;
    --abs-text: #242424;
    --abs-text-secondary: #424242;
    --abs-muted: #616161;
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    color: var(--abs-text);
    font-size: 13px;
    line-height: 1.45;
  }

  /* steps: one joined bar, as in the storyboard */
  .stepper { display: flex; margin: 0 0 16px; padding: 0; list-style: none; }
  .step { flex: 1; min-width: 0; border: 1px solid var(--abs-border); border-right: 0; background: var(--abs-subtle); }
  .step:first-child { border-radius: 5px 0 0 5px; }
  .step:last-child { border-right: 1px solid var(--abs-border); border-radius: 0 5px 5px 0; }
  .step button { width: 100%; padding: 8px 10px; border: 0; background: transparent; color: var(--abs-text); font: inherit; font-size: 11px; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .step button:hover:not(:disabled) { background: rgba(15, 108, 189, .08); }
  .step button:disabled { cursor: not-allowed; }
  .step.complete { background: #eefaf4; }
  .step.complete button { color: #166534; }
  .step-mark { margin-left: 6px; font-weight: 700; }
  .step-mark.warn { display: inline-block; width: 14px; height: 14px; border-radius: 50%; background: #d97706; color: #fff; font-size: 10px; line-height: 14px; text-align: center; }
  .step.active { border-color: var(--abs-brand); background: var(--abs-selected); }
  .step.active + .step { border-left-color: var(--abs-brand); }
  .step.active button { color: var(--abs-text); font-weight: 650; }
  .step.unavailable button { color: #a0a0a0; }
  .step-n { margin-right: 6px; font-size: 10px; font-weight: 700; }

  .workspace { display: flex; flex: 1 1 auto; flex-direction: column; min-height: 0; overflow: hidden; }
  .error { margin: 0 0 12px; color: var(--fluent-dangerBackground); }

  /* ---- shared look of both steps ---- */
  .abs-editor :global(h3) { margin: 0 0 8px; font-size: 14px; font-weight: 600; }
  .abs-editor :global(p) { margin: 0; }
  .abs-editor :global(.field-label) {
    margin: 0 0 7px;
    color: var(--abs-muted);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: .055em;
    text-transform: uppercase;
  }
  .abs-editor :global(.muted) { color: var(--abs-muted); font-size: 12px; }
  .abs-editor :global(.small) { font-size: 11px; }
  .abs-editor :global(.form-row) {
    display: grid;
    grid-template-columns: 190px 1fr;
    gap: 12px;
    align-items: center;
    padding: 4px 0 10px;
    border-bottom: 1px solid var(--abs-border-soft);
  }
  .abs-editor :global(.form-row > span) { color: var(--abs-text-secondary); font-size: 12px; }
  .abs-editor :global(.input) {
    width: 100%;
    min-height: 30px;
    padding: 5px 8px;
    border: 1px solid var(--abs-border-strong);
    border-radius: 4px;
    background: #fff;
    color: var(--abs-text);
    font: inherit;
    box-sizing: border-box;
  }
  .abs-editor :global(.input:focus) { outline: 2px solid var(--abs-brand); outline-offset: -1px; border-color: var(--abs-brand); }
  .abs-editor :global(.panel) { min-width: 0; padding: 14px 15px; border: 1px solid var(--abs-border); border-radius: 6px; background: #fff; }
  .abs-editor :global(.panel.subtle) { background: var(--abs-subtle); }
  .abs-editor :global(.choices) { min-width: 0; margin: 0 0 14px; padding: 0; border: 0; }
  .abs-editor :global(.choices:last-child) { margin-bottom: 0; }
  .abs-editor :global(p + .choices) { margin-top: 12px; }
  .abs-editor :global(.choices > legend) { display: block; width: 100%; padding: 0; }
  .abs-editor :global(.legend-row) { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .abs-editor :global(.legend-row .field-label) { margin: 0; }
  .abs-editor :global(.choices > legend + *) { margin-top: 7px; }
  .abs-editor :global(.radio-choice) {
    display: flex;
    gap: 9px;
    align-items: flex-start;
    margin-top: 8px;
    padding: 9px 10px;
    border: 1px solid var(--abs-border);
    border-radius: 5px;
    background: #fff;
    cursor: pointer;
  }
  .abs-editor :global(.choices > legend + .radio-choice) { margin-top: 7px; }
  .abs-editor :global(.radio-choice.selected) { border-color: var(--abs-brand); background: var(--abs-selected); }
  .abs-editor :global(.radio-choice.unavailable) { cursor: default; color: #a0a0a0; background: var(--abs-subtle); }
  .abs-editor :global(.radio-choice input) { margin-top: 2px; accent-color: var(--abs-brand); }
  .abs-editor :global(.radio-choice:focus-within) { outline: 2px solid var(--abs-brand); outline-offset: 2px; }
  .abs-editor :global(.radio-title) { display: block; font-weight: 600; }
  .abs-editor :global(.radio-desc) { display: block; margin-top: 1px; color: var(--abs-muted); font-size: 11px; font-weight: 400; }
  .abs-editor :global(.segmented) { display: flex; }
  .abs-editor :global(.segment) {
    position: relative;
    flex: 1;
    padding: 7px 10px;
    border: 1px solid var(--abs-border);
    border-right: 0;
    background: #fff;
    color: var(--abs-text-secondary);
    font-size: 12px;
    text-align: center;
    cursor: pointer;
  }
  .abs-editor :global(.segment:first-child) { border-radius: 4px 0 0 4px; }
  .abs-editor :global(.segment:last-child) { border-right: 1px solid var(--abs-border); border-radius: 0 4px 4px 0; }
  .abs-editor :global(.segment.selected) { border-color: var(--abs-brand); border-right: 1px solid var(--abs-brand); color: var(--abs-brand-hover); background: var(--abs-selected); font-weight: 650; }
  .abs-editor :global(.segment.selected + .segment) { border-left: 0; }
  .abs-editor :global(.segment:focus-within) { outline: 2px solid var(--abs-brand); outline-offset: 1px; z-index: 1; }
  .abs-editor :global(.segment input) { position: absolute; opacity: 0; pointer-events: none; }
  .abs-editor :global(.abs-table) { width: 100%; border-collapse: collapse; font-size: 12px; }
  .abs-editor :global(.abs-table th) {
    padding: 7px 9px;
    border-bottom: 1px solid var(--abs-border);
    background: #f4f5f7;
    color: var(--abs-muted);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: .04em;
    text-align: left;
    text-transform: uppercase;
  }
  .abs-editor :global(.abs-table td) { padding: 8px 9px; border-bottom: 1px solid var(--abs-border-soft); vertical-align: top; }
  .abs-editor :global(.abs-table tr:last-child td) { border-bottom: 0; }
  .abs-editor :global(.btn) {
    min-height: 30px;
    padding: 5px 13px;
    border: 1px solid var(--abs-border-strong);
    border-radius: 4px;
    background: #fff;
    color: var(--abs-text);
    font: inherit;
    cursor: pointer;
  }
  .abs-editor :global(.btn:hover:not(:disabled)) { background: #f5f5f5; }
  .abs-editor :global(.btn:disabled) { cursor: default; color: #a0a0a0; }
  .abs-editor :global(.visually-hidden) { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }

  /* two columns that scroll on their own below a fixed top */
  .abs-editor :global(.step-body) { display: flex; flex: 1 1 auto; flex-direction: column; min-height: 0; }
  .abs-editor :global(.step-top) { flex: 0 0 auto; }
  .abs-editor :global(.columns) {
    display: grid;
    flex: 1 1 auto;
    gap: 18px;
    min-height: 0;
    margin-top: 12px;
  }
  .abs-editor :global(.columns > .panel) { overflow-y: auto; overscroll-behavior: contain; scrollbar-gutter: stable; }
  @media (max-width: 760px), (max-height: 640px) {
    .workspace { overflow-y: auto; }
    .abs-editor :global(.step-body), .abs-editor :global(.columns) { flex: none; }
    .abs-editor :global(.columns) { grid-template-columns: 1fr !important; }
    .abs-editor :global(.columns > .panel) { overflow: visible; }
    .abs-editor :global(.form-row) { grid-template-columns: 1fr; gap: 4px; }
  }
</style>
