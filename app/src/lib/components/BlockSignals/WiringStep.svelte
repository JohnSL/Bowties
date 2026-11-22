<script lang="ts">
  /**
   * WiringStep — page 4 of the Automatic Block Signals editor: for the
   * selected signal, the Signal LCC it runs on and the pins driving its red
   * and green lamps, with the problems the last check found. It renders the
   * store's view and emits wiring intents; the check itself is the store's.
   */
  import type { ComposeIssue, SignalOutputHost } from '$lib/api/signaling';
  import { resolveNodeParts } from '$lib/layout';
  import { nodeIdToDisplayHex } from '$lib/utils/nodeId';
  import type { BlockSignalsEditorView } from '$lib/stores/blockSignalsEditor.svelte';
  import type { BlockSignalsEditorActions } from '$lib/orchestration/blockSignalsEditorOrchestrator';
  import SearchableSingleSelectDialog from '$lib/components/SingleSelectList/SearchableSingleSelectDialog.svelte';
  import TrackDiagram from './TrackDiagram.svelte';
  import { occupancyStatus, type BlockOccupancyMap } from './occupancy';
  import { endChipLabel } from './aspectText';
  import { issueText } from './wiringText';

  type Lamp = 'red' | 'green';
  const LAMPS: Array<[Lamp, string]> = [['red', 'Red lamp'], ['green', 'Green lamp']];

  let {
    view,
    actions,
    selectedSignalId = $bindable(null),
    pickerOpen = $bindable(false),
    occupancy,
  }: {
    view: BlockSignalsEditorView;
    actions: BlockSignalsEditorActions;
    selectedSignalId?: string | null;
    pickerOpen?: boolean;
    occupancy?: BlockOccupancyMap;
  } = $props();

  /** The lamp the user picked to place next; otherwise the first lamp still without a pin. */
  let pickedLamp = $state<Lamp | null>(null);

  const signals = $derived(view.preview?.signals ?? []);
  const signal = $derived(signals.find((candidate) => candidate.signalId === selectedSignalId) ?? signals[0]);
  const wiring = $derived(signal ? view.wiringFor(signal.signalId) : null);
  const host = $derived(view.outputHosts.find((candidate) => candidate.nodeKey === wiring?.nodeKey));
  const pinOf = (lamp: Lamp) => (lamp === 'red' ? wiring?.redOutput : wiring?.greenOutput) ?? null;
  const selectedLamp = $derived<Lamp>(pickedLamp ?? (pinOf('red') === null || pinOf('green') !== null ? 'red' : 'green'));
  /** Common node naming: display name over "<model> · <node address>". */
  function hostLabel(nodeKey: string): { name: string; detail: string } {
    const parts = resolveNodeParts(nodeKey);
    const address = nodeIdToDisplayHex(nodeKey) || nodeKey;
    return {
      name: parts.name || address,
      detail: [parts.model, address].filter(Boolean).join(' · '),
    };
  }
  const hostName = (nodeKey: string) => hostLabel(nodeKey).name;
  const hostItems = $derived(
    [...view.outputHosts]
      .sort((a, b) => hostName(a.nodeKey).localeCompare(hostName(b.nodeKey)))
      .map((h) => ({ key: h.nodeKey })),
  );
  const evidenceText = (h: SignalOutputHost): string =>
    h.evidence === 'unavailable'
      ? [`This Signal LCC's configuration hasn't been read yet, so its pins can't be shown.`, h.evidenceDetail].filter(Boolean).join(' ')
      : `This Signal LCC's pins can't be used until its configuration is fixed: ${h.evidenceDetail ?? 'its configuration could not be interpreted.'}`;
  const pinLabel = (ordinal: number | null) =>
    ordinal === null ? 'No pin chosen' : (host?.outputs.find((o) => o.ordinal === ordinal)?.label ?? `#${ordinal}`);

  const issues = $derived.by<ComposeIssue[]>(() => {
    if (!signal) return [];
    const found = view.issuesForSignal(signal.signalId);
    const onNode = wiring ? view.issuesForNode(wiring.nodeKey) : [];
    return [...found, ...onNode.filter((issue) => !found.includes(issue))];
  });

  function pinState(output: { ordinal: number; owner: string | null }): 'mine' | 'taken' | 'free' {
    if (output.ordinal === wiring?.redOutput || output.ordinal === wiring?.greenOutput) return 'mine';
    return output.owner !== null ? 'taken' : 'free';
  }

  const PIN_STATE_TEXT = { mine: 'This signal', taken: 'Taken', free: 'Free' } as const;

  function choosePin(ordinal: number): void {
    if (!signal) return;
    if (selectedLamp === 'red') void actions.chooseRedOutput(signal.signalId, ordinal);
    else void actions.chooseGreenOutput(signal.signalId, ordinal);
    pickedLamp = null;
  }

  function chooseHost(nodeKey: string): void {
    if (signal) void actions.chooseWiringNode(signal.signalId, nodeKey);
    pickedLamp = null;
    pickerOpen = false;
  }
</script>

<div class="step-body">
  <div class="step-top">
    <div class="wiring-head">
      <span class="field-label">Select a signal to configure</span>
      <span class="muted small">{occupancyStatus(occupancy, view.blocks.map((block) => block.blockId))}</span>
    </div>
    <TrackDiagram
      input={{
        mode: 'signals',
        blocks: view.blocks,
        direction: view.direction,
        movements: view.movements,
        signals,
        shapes: view.turnoutShapes,
        selectedSignalId: signal?.signalId ?? null,
        endLabels: {
          start: endChipLabel('start', view.entranceEnd, view.exitEnd),
          end: endChipLabel('end', view.entranceEnd, view.exitEnd),
        },
      }}
      {occupancy}
      onSelectSignal={(signalId) => (selectedSignalId = signalId)}
    />
    {#if signal}
      <h3 class="selected-heading">{signal.name}</h3>
      <div class="node-panel" role="group" aria-label="Signal LCC for the selected signal">
        <span class="field-label" id="abs-node-label">Runs on</span>
        <button type="button" class="node-field" aria-haspopup="dialog" aria-labelledby="abs-node-label abs-node-name" onclick={() => (pickerOpen = true)}>
          <span class="node-text" id="abs-node-name">
            {#if wiring}
              {@const label = hostLabel(wiring.nodeKey)}
              <strong>{label.name}</strong>
              <span class="node-detail">{label.detail}</span>
            {:else}
              <strong>Choose a Signal LCC</strong>
            {/if}
          </span>
          <span class="node-action" aria-hidden="true">Change ▾</span>
        </button>
      </div>
    {/if}
    {#if view.finishError}
      <p class="error" role="alert">{view.finishError}</p>
    {/if}
    {#if issues.length > 0}
      <div class="attention">
        <div class="field-label">Choices needing attention</div>
        <ul class="issues" aria-label="Choices needing attention">
          {#each issues as issue}
            <li>{issueText(issue)}</li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>

  {#if signal}
    <div class="columns wiring-columns">
      <section class="panel" aria-label="Lamps">
        <h3>Lamps</h3>
        <fieldset class="choices">
          <legend class="visually-hidden">Lamp to place</legend>
          {#each LAMPS as [lamp, title] (lamp)}
            <label class="radio-choice" class:selected={selectedLamp === lamp}>
              <input type="radio" name="abs-lamp" checked={selectedLamp === lamp} disabled={!host} onchange={() => (pickedLamp = lamp)} />
              <span><span class="radio-title">{title}</span><span class="radio-desc">{pinLabel(pinOf(lamp))}</span></span>
            </label>
          {/each}
        </fieldset>
        <label class="radio-choice unavailable">
          <input type="checkbox" disabled />
          <span><span class="radio-title">Customize lamps</span></span>
        </label>
        <div class="lamp-tests">
          <button type="button" class="btn" disabled>Blink</button>
          <button type="button" class="btn" disabled>Show</button>
        </div>
      </section>

      <section class="panel" aria-label="Pins">
        {#if host}
          <h3>Pins on {hostName(host.nodeKey)}</h3>
          {#if host.evidence !== 'complete'}
            <p class="muted evidence-note">{evidenceText(host)}</p>
          {:else}
          <div class="pin-grid" role="group" aria-label={`Pins on ${hostName(host.nodeKey)}`}>
            {#each host.outputs as output (output.ordinal)}
              {@const state = pinState(output)}
              <button
                type="button"
                class={`pin ${state}`}
                disabled={state !== 'free'}
                aria-label={`${output.label}: ${state === 'taken' ? `Taken by ${output.owner}` : PIN_STATE_TEXT[state]}`}
                onclick={() => choosePin(output.ordinal)}
              >
                <span class="pin-name">{output.label}</span>
                <span class="pin-state">{PIN_STATE_TEXT[state]}</span>
              </button>
            {/each}
          </div>
          {/if}
        {:else}
          <h3>Pins</h3>
          <p class="muted">Choose a Signal LCC to see its pins.</p>
        {/if}
      </section>
    </div>
  {/if}
</div>

{#if pickerOpen}
  <SearchableSingleSelectDialog
    title="Choose a Signal LCC"
    filterLabel="Search Signal LCC nodes"
    items={hostItems}
    searchTextFor={hostName}
    ariaLabel="Signal LCC nodes"
    name="signal-lcc"
    emptyMessage="No Signal LCC nodes are available."
    onConfirm={chooseHost}
    onCancel={() => (pickerOpen = false)}
  >
    {#snippet row(id)}
      {@const label = hostLabel(id)}
      <span class="host-row">
        <strong>{label.name}</strong>
        <span class="node-detail">{label.detail}</span>
      </span>
    {/snippet}
  </SearchableSingleSelectDialog>
{/if}

<style>
  .wiring-head { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .wiring-head .field-label { margin: 0; }
  .selected-heading { margin: 10px 0 6px; }
  .node-panel { display: flex; align-items: center; gap: 12px; }
  .node-panel .field-label { margin: 0; }
  .node-field {
    display: flex; flex: 1; justify-content: space-between; align-items: center; gap: 8px;
    min-height: 30px; padding: 5px 10px; border: 1px solid var(--abs-border-strong); border-radius: 4px;
    background: #fff; color: var(--abs-text); font: inherit; text-align: left; cursor: pointer;
  }
  .node-field:hover { background: #f5f5f5; }
  .node-text { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .node-detail { font-size: 11px; color: var(--abs-muted); }
  .host-row { display: inline-flex; flex-direction: column; gap: 1px; }
  .evidence-note { margin: 0; }
  .node-action { color: var(--abs-brand); font-size: 11px; }
  .error { margin: 8px 0 0; color: var(--fluent-dangerBackground); }
  .attention { margin-top: 10px; padding: 8px 12px; border: 1px solid #e0c36a; border-radius: 5px; background: #fff8e1; }
  .attention .field-label { margin-bottom: 4px; }
  .issues { margin: 0; padding-left: 18px; font-size: 12px; }
  .wiring-columns { grid-template-columns: minmax(260px, .8fr) minmax(0, 1.4fr); }
  .lamp-tests { display: flex; gap: 8px; margin-top: 12px; }
  .pin-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(96px, 1fr)); gap: 6px; }
  .pin {
    display: flex; flex-direction: column; gap: 1px; padding: 6px 8px; border: 1px solid var(--abs-border);
    border-radius: 4px; background: #fff; color: var(--abs-text); font: inherit; font-size: 12px; text-align: left; cursor: pointer;
  }
  .pin.free:hover { border-color: var(--abs-brand); background: var(--abs-selected); }
  .pin:disabled { cursor: default; }
  .pin.mine { border-color: var(--abs-brand); background: var(--abs-selected); color: var(--abs-text); }
  .pin.taken { background: var(--abs-subtle); color: #a0a0a0; }
  .pin-name { font-weight: 600; }
  .pin-state { font-size: 10px; color: var(--abs-muted); }
</style>
