<script lang="ts">
  /**
   * PanelEditor — the transient Panel dialog. Steps are Blocks and Wiring;
   * Done on Wiring creates the panel, Cancel or × discards. Nothing here is
   * saved until Done. Renders the store's state and emits intent only.
   */
  import type { PanelEditorView, PanelDetectedBlock } from './panelEditorView';
  import type { PanelEditorActions } from '$lib/orchestration/panelEditorOrchestrator';
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';
  import BlocksStep from './BlocksStep.svelte';
  import WiringStep from './WiringStep.svelte';

  let {
    view,
    actions: intents,
    detectedBlocks,
  }: {
    view: PanelEditorView;
    actions: PanelEditorActions;
    detectedBlocks: PanelDetectedBlock[];
  } = $props();

  const bar = $derived(view.stepBar);
  const blocksState = $derived(
    bar.blocksCount === 0 ? 'no blocks yet' : `${bar.blocksCount} block${bar.blocksCount === 1 ? '' : 's'}`,
  );
  const wiringState = $derived.by(() => {
    if (bar.wiring === 'none') return { text: '—', tone: '' };
    if (bar.wiring === 'allWired') return { text: 'all wired', tone: 'ok' };
    const need = bar.wiring.needLamp;
    return { text: `${need} need${need === 1 ? 's' : ''} a lamp`, tone: 'attn' };
  });
  const needLamp = $derived(typeof bar.wiring === 'object' ? bar.wiring.needLamp : 0);

</script>

{#snippet leading()}
  <Button appearance="secondary" onclick={intents.cancel}>Cancel</Button>
{/snippet}

<Dialog open width={1120} bodyLayout="contained" ariaLabel={view.name} initialFocus="none" onCancel={intents.cancel}>
  {#snippet title()}
    <DialogTitle>{view.name}</DialogTitle>
  {/snippet}

  <div class="panel-editor">
    <ol class="stepper" aria-label="Panel steps">
      <li class="step" class:current={view.step === 'blocks'}>
        <button type="button" aria-current={view.step === 'blocks' ? 'step' : undefined} onclick={() => intents.goTo('blocks')}>
          <span class="step-num">1</span>Blocks <span class="step-state" class:ok={bar.blocksCount > 0}>{blocksState}</span>
        </button>
      </li>
      <li class="step" class:current={view.step === 'wiring'}>
        <button type="button" aria-current={view.step === 'wiring' ? 'step' : undefined} onclick={() => intents.goTo('wiring')}>
          <span class="step-num">2</span>Wiring <span class="step-state {wiringState.tone}">{wiringState.text}</span>
        </button>
      </li>
    </ol>

    {#if view.finishError}
      <p class="error" role="alert">{view.finishError}</p>
    {/if}
    {#if view.step === 'wiring' && needLamp > 0}
      <p class="lamp-note">{needLamp} block{needLamp === 1 ? '' : 's'} without a lamp. Done saves the panel as Incomplete.</p>
    {/if}

    <div class="workspace">
      {#if view.step === 'blocks'}
        <BlocksStep {view} actions={intents} {detectedBlocks} />
      {:else}
        <WiringStep {view} actions={intents} />
      {/if}
    </div>
  </div>

  {#snippet actions()}
    <DialogActions {leading}>
      {#if view.step === 'blocks'}
        <Button appearance="primary" onclick={() => intents.goTo('wiring')}>Next: Wiring</Button>
      {:else}
        <Button appearance="secondary" onclick={() => intents.goTo('blocks')}>Back</Button>
        <Button appearance="primary" onclick={intents.finish}>Done</Button>
      {/if}
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .panel-editor {
    --pe-border: #d1d1d1;
    --pe-border-strong: #b8b8b8;
    --pe-border-soft: #eceef1;
    --pe-subtle: #fafafa;
    --pe-selected: #ebf3fc;
    --pe-brand: #0f6cbd;
    --pe-brand-hover: #115ea3;
    --pe-text: #242424;
    --pe-muted: #616161;
    display: flex; flex: 1 1 auto; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden;
    color: var(--pe-text); font-size: 13px; line-height: 1.45;
  }
  .stepper { display: flex; margin: 0 0 16px; padding: 0; list-style: none; }
  .step { flex: 1; min-width: 0; border: 1px solid var(--pe-border); border-right: 0; background: var(--pe-subtle); }
  .step:first-child { border-radius: 5px 0 0 5px; }
  .step:last-child { border-right: 1px solid var(--pe-border); border-radius: 0 5px 5px 0; }
  .step button { width: 100%; padding: 8px 10px; border: 0; background: transparent; color: var(--pe-text); font: inherit; font-size: 11px; text-align: left; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .step button:hover { background: rgba(15, 108, 189, .08); }
  .step.current { border-color: var(--pe-brand); background: var(--pe-selected); }
  .step.current + .step { border-left-color: var(--pe-brand); }
  .step.current button { font-weight: 650; }
  .step-num { margin-right: 6px; font-size: 10px; font-weight: 700; }
  .step-state { margin-left: 6px; color: var(--pe-muted); font-weight: 400; }
  .step-state.ok { color: #166534; }
  .step-state.attn { color: #b45309; font-weight: 650; }
  .workspace { display: flex; flex: 1 1 auto; flex-direction: column; min-height: 0; overflow: hidden; }
  .error { margin: 0 0 10px; color: var(--fluent-dangerBackground); }
  .lamp-note { margin: 0 0 10px; color: var(--pe-muted); font-size: 12px; }

  .panel-editor :global(.field-label) { display: block; margin: 0 0 7px; color: var(--pe-muted); font-size: 10px; font-weight: 700; letter-spacing: .055em; text-transform: uppercase; }
  .panel-editor :global(.input) { width: 100%; min-height: 30px; padding: 5px 8px; border: 1px solid var(--pe-border-strong); border-radius: 4px; background: #fff; color: var(--pe-text); font: inherit; box-sizing: border-box; }
  .panel-editor :global(.input:focus) { outline: 2px solid var(--pe-brand); outline-offset: -1px; border-color: var(--pe-brand); }
  .panel-editor :global(.btn) { min-height: 30px; padding: 5px 13px; border: 1px solid var(--pe-border-strong); border-radius: 4px; background: #fff; color: var(--pe-text); font: inherit; cursor: pointer; }
  .panel-editor :global(.btn:hover:not(:disabled)) { background: #f5f5f5; }
  .panel-editor :global(.btn:disabled) { cursor: default; color: #a0a0a0; }
  .panel-editor :global(.btn-icon) { min-width: 30px; padding: 5px 0; }
  .panel-editor :global(.btn-small) { min-height: 24px; padding: 2px 10px; font-size: 12px; }
  .panel-editor :global(.toolbar) { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-bottom: 10px; }
  .panel-editor :global(.toolbar .field-label) { margin: 0; }
  .panel-editor :global(.panes) { display: grid; flex: 1 1 auto; grid-template-columns: 1fr 1fr; gap: 18px; min-height: 0; }
  .panel-editor :global(.pane) { display: flex; flex-direction: column; min-width: 0; min-height: 0; border: 1px solid var(--pe-border); border-radius: 6px; background: #fff; }
  .panel-editor :global(.pane-head) { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 9px 12px; border-bottom: 1px solid var(--pe-border-soft); font-weight: 600; }
  .panel-editor :global(.pane-body) { display: flex; flex: 1 1 auto; flex-direction: column; gap: 12px; padding: 12px; min-height: 0; overflow-y: auto; }
  .panel-editor :global(.chip) { padding: 1px 8px; border-radius: 10px; background: #eef1f5; color: var(--pe-muted); font-size: 11px; font-weight: 400; }
  .panel-editor :global(.chip.wired) { background: #eefaf4; color: #166534; }
  .panel-editor :global(.chip.attn) { background: #fff7e6; color: #b45309; }
  .panel-editor :global(.empty) { display: flex; flex-direction: column; gap: 4px; padding: 18px 8px; color: var(--pe-muted); text-align: center; }
  .panel-editor :global(.list) { display: flex; flex-direction: column; margin: 0; padding: 0; list-style: none; }
  .panel-editor :global(.list-group) { padding: 8px 4px 4px; color: var(--pe-muted); font-size: 10px; font-weight: 700; letter-spacing: .04em; text-transform: uppercase; }
  .panel-editor :global(.list-item) { display: flex; align-items: center; gap: 8px; padding: 6px 4px; border-bottom: 1px solid var(--pe-border-soft); }
  .panel-editor :global(.list-item.selectable) { padding: 6px 8px; border-radius: 4px; cursor: pointer; }
  .panel-editor :global(.list-item.selectable:hover) { background: #f5f5f5; }
  .panel-editor :global(.list-item.selected) { background: var(--pe-selected); }
  .panel-editor :global(.grow) { flex: 1; min-width: 0; }
  .panel-editor :global(.name) { font-weight: 600; }
  .panel-editor :global(.meta) { color: var(--pe-muted); font-size: 11px; }
  .panel-editor :global(.meta.index) { width: 18px; }
  .panel-editor :global(.shortcut-row) { display: flex; gap: 8px; }
  .panel-editor :global(.lamp) { width: 10px; height: 10px; border-radius: 50%; background: #17a65b; border: 1px solid #20242a; }
  .panel-editor :global(.lamp.none) { background: #fff; border-color: var(--pe-border-strong); }
  .panel-editor :global(.segmented) { display: flex; }
  .panel-editor :global(.segment) { position: relative; padding: 5px 12px; border: 1px solid var(--pe-border); border-right: 0; background: #fff; color: var(--pe-muted); font-size: 12px; }
  .panel-editor :global(.segment:first-child) { border-radius: 4px 0 0 4px; }
  .panel-editor :global(.segment:last-child) { border-right: 1px solid var(--pe-border); border-radius: 0 4px 4px 0; }
  .panel-editor :global(.segment.selected) { border-color: var(--pe-brand); color: var(--pe-brand-hover); background: var(--pe-selected); font-weight: 650; }
  .panel-editor :global(.segment input) { position: absolute; opacity: 0; pointer-events: none; }
  .panel-editor :global(.note) { padding: 8px 10px; border-left: 3px solid var(--pe-border-strong); border-radius: 3px; background: var(--pe-subtle); font-size: 12px; }
  .panel-editor :global(.note.warn) { border-left-color: #d17b0f; background: #fff7e6; }
  .panel-editor :global(.note.ok) { border-left-color: #17a65b; background: #eefaf4; }
  @media (max-width: 760px), (max-height: 640px) {
    .workspace { overflow-y: auto; }
    .panel-editor :global(.panes) { grid-template-columns: 1fr; flex: none; }
  }
</style>
