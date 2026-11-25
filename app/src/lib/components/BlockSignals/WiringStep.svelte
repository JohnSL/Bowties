<script lang="ts">
  /**
   * WiringStep — page 4 of the Automatic Block Signals editor: for the
   * selected signal, the Signal LCC it runs on and the pins driving its red
   * and green lamps, with the problems the last check found. It renders the
   * store's view and emits wiring intents; the check itself is the store's.
   */
  import { tick } from 'svelte';
  import type { ComposeIssue, SignalOutputHost } from '$lib/api/signaling';
  import { resolveNodeParts } from '$lib/layout';
  import { nodeIdToDisplayHex } from '$lib/utils/nodeId';
  import { canonicalizeNodeId } from '$lib/utils/nodeRoster';
  import type { NodeConfigTree } from '$lib/types/nodeTree';
  import { configFocusStore } from '$lib/stores/configFocus.svelte';
  import { resolveSignalOutputOwnerTarget } from '$lib/utils/channelConfigNavigation';
  import type { BlockSignalsEditorView } from '$lib/stores/blockSignalsEditor.svelte';
  import type { BlockSignalsEditorActions } from '$lib/orchestration/blockSignalsEditorOrchestrator';
  import SearchableSingleSelectDialog from '$lib/components/SingleSelectList/SearchableSingleSelectDialog.svelte';
  import TrackDiagram from './TrackDiagram.svelte';
  import { occupancyStatus, type BlockOccupancyMap } from './occupancy';
  import { signalOutputOwnerDescription, signalOutputRows } from '$lib/utils/signalOutputLabels';
  import { endChipLabel, SEARCHLIGHT_TYPE_NAME } from './aspectText';
  import { issueText } from './wiringText';

  type Lamp = 'red' | 'green';
  const LAMPS: Array<[Lamp, string]> = [['red', 'Red'], ['green', 'Green']];

  let {
    view,
    actions,
    selectedSignalId = $bindable(null),
    pickerOpen = $bindable(false),
    occupancy,
    nodeTree,
  }: {
    view: BlockSignalsEditorView;
    actions: BlockSignalsEditorActions;
    selectedSignalId?: string | null;
    pickerOpen?: boolean;
    occupancy?: BlockOccupancyMap;
    nodeTree?: (nodeKey: string) => NodeConfigTree | undefined;
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
  const pinLabel = (ordinal: number) => host?.outputs.find((o) => o.ordinal === ordinal)?.label ?? `#${ordinal}`;

  const issues = $derived.by<ComposeIssue[]>(() => {
    if (!signal) return [];
    const found = view.issuesForSignal(signal.signalId);
    const onNode = wiring ? view.issuesForNode(wiring.nodeKey) : [];
    return [...found, ...onNode.filter((issue) => !found.includes(issue))];
  });

  type PinOutput = SignalOutputHost['outputs'][number];
  type PinState = 'selected' | 'mine' | 'facility' | 'outside' | 'free';

  /** The facility being edited owns this pin's mast, so the backend lets its signals reuse it. */
  const isOwnFacility = (output: PinOutput) => !!output.facility && output.facility.facilityId === view.facilityId;

  function pinState(output: PinOutput): PinState {
    if (output.ordinal === pinOf(selectedLamp)) return 'selected';
    if (output.ordinal === wiring?.redOutput || output.ordinal === wiring?.greenOutput) return 'mine';
    if (isOwnFacility(output)) return 'free';
    if (output.facility) return 'facility';
    return output.owner !== null ? 'outside' : 'free';
  }

  const lampName = (output: PinOutput) => LAMPS.find(([lamp]) => pinOf(lamp) === output.ordinal)?.[1] ?? '';
  const lampOf = (output: PinOutput): Lamp | null => LAMPS.find(([lamp]) => pinOf(lamp) === output.ordinal)?.[0] ?? null;
  /** A pin this signal chose that something outside Bowties still holds. */
  const heldOutside = (output: PinOutput) => lampOf(output) !== null && output.owner !== null && !output.facility;

  /** The "who" line under a pin: its lamp, its facility, its outside owner, or Free. */
  function pinWho(output: PinOutput, state: PinState): string {
    if (state === 'selected' || state === 'mine') return lampName(output);
    if (state === 'facility') return output.facility?.name ?? '';
    if (state === 'outside') return output.owner ?? '';
    return 'Free';
  }

  function pinAriaLabel(output: PinOutput, state: PinState): string {
    if (heldOutside(output)) return `${output.label}: ${lampName(output)}, held by ${output.owner} outside Bowties. Show details`;
    if (state === 'selected' || state === 'mine') return `${output.label}: This signal`;
    if (state === 'facility') return `${output.label}: Used by ${output.facility?.name}`;
    if (state === 'outside') return `${output.label}: Used by ${output.owner}, not managed by Bowties. Show details`;
    return `${output.label}: Free`;
  }

  const pinRows = $derived(signalOutputRows(host?.outputs ?? []));
  const pinColumns = $derived(Math.max(1, ...pinRows.map((row) => row.cells.length)));

  /** Owner whose details card is open, when it still exists on the chosen node. */
  let inspectedOwner = $state<string | null>(null);
  $effect(() => {
    signal?.signalId;
    host?.nodeKey;
    inspectedOwner = null;
  });
  const inspected = $derived.by(() => {
    if (!host || inspectedOwner === null) return null;
    const pins = host.outputs.filter((output) => !output.facility && output.owner === inspectedOwner);
    if (pins.length === 0) return null;
    const tree = nodeTree?.(host.nodeKey);
    const nodeKey = canonicalizeNodeId(host.nodeKey);
    const logic = view.preview?.outsideOwners?.find(
      (entry) => entry.owner === inspectedOwner && canonicalizeNodeId(entry.nodeKey) === nodeKey,
    );
    return {
      owner: inspectedOwner,
      nodeKey: host.nodeKey,
      isMast: inspectedOwner.startsWith('Mast '),
      matchedLogic: logic?.matchedLogic ?? [],
      keptLogic: logic?.keptLogic ?? [],
      pins,
      description: signalOutputOwnerDescription(tree, inspectedOwner),
      target: resolveSignalOutputOwnerTarget(tree, inspectedOwner),
    };
  });

  let pinGrid = $state<HTMLElement | null>(null);
  /** The details card is showing the release confirmation instead of the details. */
  let confirmingRelease = $state(false);
  let releaseButton = $state<HTMLButtonElement | null>(null);
  $effect(() => {
    inspectedOwner;
    confirmingRelease = false;
  });

  const logicList = (numbers: number[]) => numbers.map((n) => `Logic ${n}`).join(', ');

  function cancelRelease(): void {
    confirmingRelease = false;
    void tick().then(() => releaseButton?.focus());
  }

  /** Release the inspected owner and put the lamp on the clicked pin: its own lamp, else the selected one. */
  function releaseAndUse(card: NonNullable<typeof inspected>, ordinal: number): void {
    if (!signal) return;
    void actions.releaseAndPlaceOutput(signal.signalId, inspectedLamp ?? selectedLamp, { nodeKey: card.nodeKey, owner: card.owner }, ordinal);
    pickedLamp = null;
    inspectedOwner = null;
  }

  /** Close the details card and return focus to the first pin of its owner. */
  function closeCard(): void {
    const owner = inspectedOwner;
    inspectedOwner = null;
    if (owner === null) return;
    const pin = [...(pinGrid?.querySelectorAll<HTMLButtonElement>('button[data-owner]') ?? [])]
      .find((button) => button.dataset.owner === owner);
    pin?.focus();
  }

  /** Escape closes only the card; the editor dialog listens for Escape on the window. */
  function cardKeydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.stopPropagation();
    event.preventDefault();
    closeCard();
  }

  /** The pin the user clicked to open the card; the lamp goes there when the release is confirmed. */
  let inspectedPin = $state<number | null>(null);
  /** The lamp already on the clicked pin, when it is one this signal chose; otherwise null. */
  let inspectedLamp = $state<Lamp | null>(null);

  function pinClicked(output: PinOutput, state: PinState): void {
    if (state === 'outside' || heldOutside(output)) {
      inspectedOwner = inspectedOwner === output.owner ? null : output.owner;
      inspectedPin = output.ordinal;
      inspectedLamp = lampOf(output);
    }
    else seatPin(output.ordinal);
  }

  /** The selected lamp's click on a pin: the store places, clears or swaps. */
  function seatPin(ordinal: number): void {
    if (!signal) return;
    void actions.placeLamp(signal.signalId, selectedLamp, ordinal);
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
            <li>{issueText(issue, host?.outputs ?? [])}</li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>

  {#if signal}
    <div class="columns wiring-columns">
      <section class="panel" aria-label="Lamps">
        <h3>Lamps</h3>
        <div class="head-card">
          <h4>Head 1 · {SEARCHLIGHT_TYPE_NAME}</h4>
          {#each LAMPS as [lamp, title] (lamp)}
            {@const pin = pinOf(lamp)}
            {@const pinText = pin === null ? 'No pin' : pinLabel(pin)}
            <div class="lamp-row" class:active={selectedLamp === lamp}>
              <button
                type="button"
                class="lamp-pick"
                aria-pressed={selectedLamp === lamp}
                aria-label={`${title} lamp: ${pinText}`}
                disabled={!host}
                onclick={() => (pickedLamp = lamp)}
              >
                <span>{title}</span>
                <span class="lamp-pin" class:none={pin === null}>{pinText}</span>
              </button>
            </div>
          {/each}
        </div>
        <label class="radio-choice unavailable">
          <input type="checkbox" disabled />
          <span><span class="radio-title">Customize lamps</span></span>
        </label>
      </section>

      <section class="panel" aria-label="Pins">
        {#if host}
          <h3>Pins on {hostName(host.nodeKey)}</h3>
          {#if host.evidence !== 'complete'}
            <p class="muted evidence-note">{evidenceText(host)}</p>
          {:else}
          <div class="pin-grid" role="group" aria-label={`Pins on ${hostName(host.nodeKey)}`} style:--pin-columns={pinColumns} bind:this={pinGrid}>
            {#each pinRows as row (row.header)}
              <span class="pin-header">{row.header}</span>
              {#each row.cells as output (output.ordinal)}
                {@const state = pinState(output)}
                <button
                  type="button"
                  class={`pin ${state}`}
                  class:inspected={(state === 'outside' || heldOutside(output)) && output.owner === inspected?.owner}
                  disabled={state === 'facility'}
                  aria-label={pinAriaLabel(output, state)}
                  data-owner={state === 'outside' || heldOutside(output) ? output.owner : undefined}
                  onclick={() => pinClicked(output, state)}
                >
                  <span class="pin-label">{output.label}</span>
                  <span class="pin-who">{pinWho(output, state)}{#if heldOutside(output)} <span class="held-mark" aria-hidden="true">⚠</span>{/if}</span>
                </button>
              {/each}
            {/each}
          </div>
          {#if inspected}
            {@const card = inspected}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <section class="outside-card" aria-label={`${card.owner}, not managed by Bowties`} onkeydown={cardKeydown}>
              <button type="button" class="card-close" aria-label={`Close ${card.owner} details`} onclick={closeCard}>×</button>
              <h4>{card.owner} <span class="outside-tag">Not managed by Bowties</span></h4>
              <p>{card.description ? `Description: "${card.description}"` : 'No description.'}</p>
              <p>{card.isMast ? 'Rule to Aspect' : 'Direct Lamp Control row'} · drives {card.pins.map((pin) => pin.label).join(', ')}</p>
              {#if card.isMast && card.matchedLogic.length > 0}
                <p>Aspects set by {logicList(card.matchedLogic)}</p>
              {/if}
              <p>Configured on {hostName(host.nodeKey)} outside Bowties, for example by hand, by another tool, or in earlier testing.</p>
              {#if confirmingRelease}
                {@const resets = [card.owner, ...card.matchedLogic.filter((n) => !card.keptLogic.includes(n)).map((n) => `Logic ${n}`)]}
                {@const pinLabels = card.pins.map((pin) => pin.label).join(', ')}
                <p>Release {card.owner}? {card.pins.length > 1 ? `All ${card.pins.length} of its pins` : 'Its pin'} ({pinLabels}) {card.pins.length > 1 ? 'become' : 'becomes'} free, and {resets.join(', ')} {resets.length > 1 ? 'are' : 'is'} reset when you save. Discard undoes it.</p>
                {#each card.keptLogic as n (n)}
                  <p>Logic {n} also sends other events, so it is kept; only its actions that set {card.owner}'s aspects are disconnected.</p>
                {/each}
                {#if card.isMast}
                  <p>Other boards or JMRI may still send events to {card.owner}.</p>
                {/if}
                <div class="outside-actions">
                  <button type="button" class="btn" onclick={cancelRelease}>Cancel</button>
                  <button type="button" class="btn btn-primary" onclick={() => releaseAndUse(card, inspectedPin ?? card.pins[0].ordinal)}>
                    {#if inspectedLamp}
                      Release {card.owner}
                    {:else}
                      Release and put {selectedLamp === 'red' ? 'Red' : 'Green'} on {pinLabel(inspectedPin ?? card.pins[0].ordinal)}
                    {/if}
                  </button>
                </div>
              {:else}
              <div class="outside-actions">
                {#if card.target}
                  {@const target = card.target}
                  <button
                    type="button"
                    class="btn"
                    aria-label={`Show ${card.owner} in Configuration`}
                    onclick={() => configFocusStore.focusConfigField(target.nodeId, target.elementPath)}
                  >Show in Configuration</button>
                {/if}
                <button type="button" class="btn" bind:this={releaseButton} onclick={() => (confirmingRelease = true)}>Release and use…</button>
              </div>
              {/if}
            </section>
          {/if}
          <div class="pin-key" aria-hidden="true">
            <span><i class="selected"></i>Selected lamp</span>
            <span><i class="mine"></i>This signal</span>
            <span><i class="facility"></i>Used by a facility</span>
            <span><i class="outside"></i>Used outside Bowties</span>
            <span><i class="free"></i>Free</span>
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
  .head-card { margin-bottom: 10px; padding: 10px 12px; border: 1px solid var(--abs-border); border-radius: 6px; }
  .head-card h4 { margin: 0; font-size: 13px; }
  .lamp-row { margin-top: 6px; }
  .lamp-pick {
    display: flex; justify-content: space-between; gap: 8px; width: 100%; padding: 5px 8px;
    border: 1px solid var(--abs-border); border-radius: 4px; background: #fff; color: var(--abs-text);
    font: inherit; font-size: 12px; text-align: left; cursor: pointer;
  }
  .lamp-pick:hover:not(:disabled) { border-color: var(--abs-brand); }
  .lamp-pick:disabled { cursor: default; }
  .lamp-row.active .lamp-pick { border: 2px solid var(--abs-brand); padding: 4px 7px; background: var(--abs-selected); font-weight: 650; }
  .lamp-pin { color: var(--abs-muted); font-weight: 400; }
  .lamp-pin.none { font-style: italic; }
  .pin-grid { display: grid; grid-template-columns: auto repeat(var(--pin-columns, 4), minmax(0, 1fr)); gap: 4px; align-items: stretch; margin-top: 8px; font-size: 12px; }
  .pin-header { display: flex; align-items: center; padding: 0 8px; color: var(--abs-brand); font-size: 12px; font-weight: 700; }
  .pin {
    display: flex; flex-direction: column; align-items: center; gap: 1px; min-width: 0; padding: 4px 2px;
    border: 1px solid var(--abs-border); border-radius: 4px; background: #fff; color: var(--abs-text);
    font: inherit; font-size: 11px; line-height: 1.2; cursor: pointer;
  }
  .pin .pin-label { font-weight: 650; white-space: nowrap; }
  .pin .pin-who { max-width: 100%; overflow: hidden; color: var(--abs-muted); font-size: 10px; text-overflow: ellipsis; white-space: nowrap; }
  .pin.free:hover { border-color: var(--abs-brand); }
  .pin.mine { border-color: var(--abs-brand); background: var(--abs-selected); }
  .pin.mine .pin-who { color: var(--abs-brand-hover); font-weight: 650; }
  .pin.selected { border-color: var(--abs-brand); background: var(--abs-brand); color: #fff; box-shadow: 0 0 0 2px var(--abs-selected); }
  .pin.selected .pin-who { color: #fff; }
  .pin.facility { background: #eef1f5; color: var(--abs-muted); }
  .pin.outside { border-style: dashed; border-color: #b07a00; background: #fff8e6; color: var(--abs-text-secondary); }
  .pin.outside:hover, .pin.outside.inspected { border-style: solid; box-shadow: 0 0 0 2px #f3dca0; }
  .pin .held-mark { color: #b07a00; font-weight: 700; }
  .pin.selected .held-mark { color: #fff; }
  .pin:disabled { cursor: not-allowed; }
  .pin-key { display: flex; flex-wrap: wrap; gap: 4px 12px; margin-top: 8px; font-size: 11px; color: var(--abs-muted); }
  .pin-key span { display: inline-flex; align-items: center; gap: 4px; }
  .pin-key i { display: inline-block; width: 12px; height: 12px; border: 1px solid var(--abs-border); border-radius: 3px; background: #fff; }
  .pin-key i.selected { border-color: var(--abs-brand); background: var(--abs-brand); }
  .pin-key i.mine { border-color: var(--abs-brand); background: var(--abs-selected); }
  .pin-key i.facility { background: #eef1f5; }
  .pin-key i.outside { border-style: dashed; border-color: #b07a00; background: #fff8e6; }
  .outside-card h4 { padding-right: 28px; }
  .card-close {
    position: absolute; top: 6px; right: 6px; width: 24px; height: 24px;
    display: inline-flex; align-items: center; justify-content: center; padding: 0;
    border: none; border-radius: 4px; background: transparent; color: var(--abs-muted);
    font-size: 16px; line-height: 1; cursor: pointer;
  }
  .card-close:hover { background: #f3e6c4; color: var(--abs-text); }
  .card-close:focus-visible { outline: 2px solid var(--abs-brand); outline-offset: 1px; }
  .outside-card { position: relative; margin-top: 10px; padding: 10px 12px; border: 1px solid #e0c36a; border-radius: 5px; background: #fffaf0; font-size: 12px; }
  .outside-card h4 { display: flex; align-items: baseline; gap: 8px; margin: 0 0 4px; font-size: 13px; }
  .outside-tag { color: #8a5d00; font-size: 11px; font-weight: 600; }
  .outside-card p { margin: 2px 0; color: var(--abs-text-secondary); }
  .outside-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 8px; }
</style>
