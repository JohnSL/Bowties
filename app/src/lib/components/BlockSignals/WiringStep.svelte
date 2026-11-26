<script lang="ts">
  /**
   * WiringStep — page 4 of the Automatic Block Signals editor: for the
   * selected signal, the Signal LCC it runs on and the pins driving its red
   * and green lamps, with the problems the last check found. It renders the
   * store's view and emits wiring intents; the check itself is the store's.
   */
  import type { SignalOutputHost } from '$lib/api/signaling';
  import { canonicalizeNodeId } from '$lib/utils/nodeRoster';
  import type { NodeConfigTree } from '$lib/types/nodeTree';
  import { configFocusStore } from '$lib/stores/configFocus.svelte';
  import { resolveSignalOutputOwnerTarget } from '$lib/utils/channelConfigNavigation';
  import type { BlockSignalsEditorView } from '$lib/stores/blockSignalsEditor.svelte';
  import type { BlockSignalsEditorActions } from '$lib/orchestration/blockSignalsEditorOrchestrator';
  import SignalLccPicker from '$lib/components/SignalLcc/SignalLccPicker.svelte';
  import OutsideOwnerRelease from '$lib/components/SignalLcc/OutsideOwnerRelease.svelte';
  import PinGrid, { type PinGridPinView, type PinGridState } from '$lib/components/SignalLcc/PinGrid.svelte';
  import { signalLccName as hostName } from '$lib/components/SignalLcc/signalLccLabel';
  import TrackDiagram from './TrackDiagram.svelte';
  import { occupancyStatus, type BlockOccupancyMap } from './occupancy';
  import { signalOutputOwnerDescription } from '$lib/utils/signalOutputLabels';
  import { SEARCHLIGHT_TYPE_NAME } from './aspectText';
  import { trackInputFor } from './trackInput';
  import { attentionText, issueText } from './wiringText';

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
  const evidenceText = (h: SignalOutputHost): string =>
    h.evidence === 'unavailable'
      ? [`This Signal LCC's configuration hasn't been read yet, so its pins can't be shown.`, h.evidenceDetail].filter(Boolean).join(' ')
      : `This Signal LCC's pins can't be used until its configuration is fixed: ${h.evidenceDetail ?? 'its configuration could not be interpreted.'}`;
  const pinLabel = (ordinal: number) => host?.outputs.find((o) => o.ordinal === ordinal)?.label ?? `#${ordinal}`;

  const attention = $derived(signal ? view.wiringAttentionFor(signal.signalId) : []);
  const nextUnwiredId = $derived(signal ? view.nextAttentionSignalId(signal.signalId) : null);
  const attentionTexts = $derived([
    ...attention.map((item) => attentionText(item, host?.outputs ?? [])),
    ...view.facilityWiringIssues.map((issue) => issueText(issue, host?.outputs ?? [])),
  ]);

  type PinOutput = SignalOutputHost['outputs'][number];
  type PinState = PinGridState;

  /** The facility being edited owns this pin's mast, so the backend lets its signals reuse it. */
  const isOwnFacility = (output: PinOutput) => !!output.facility && output.facility.facilityId === view.facilityId;

  /** The other signal in this edit that holds this pin on the chosen node. */
  const siblingOf = (output: PinOutput) =>
    wiring && signal ? view.pinHolder(signal.signalId, wiring.nodeKey, output.ordinal) : null;

  function pinState(output: PinOutput): PinState {
    if (output.ordinal === pinOf(selectedLamp)) return 'selected';
    if (output.ordinal === wiring?.redOutput || output.ordinal === wiring?.greenOutput) return 'mine';
    if (siblingOf(output)) return 'sibling';
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
    if (state === 'sibling') {
      const holder = siblingOf(output);
      return holder ? `${holder.name} · ${holder.lamp === 'red' ? 'Red' : 'Green'}` : '';
    }
    if (state === 'facility') return output.facility?.name ?? '';
    if (state === 'outside') return output.owner ?? '';
    return 'Free';
  }

  function pinAriaLabel(output: PinOutput, state: PinState): string {
    if (heldOutside(output)) return `${output.label}: ${lampName(output)}, held by ${output.owner} outside Bowties. Show details`;
    if (state === 'selected' || state === 'mine') return `${output.label}: This signal`;
    if (state === 'sibling') {
      const holder = siblingOf(output);
      return `${output.label}: Used by ${holder?.name} (${holder?.lamp})`;
    }
    if (state === 'facility') return `${output.label}: Used by ${output.facility?.name}`;
    if (state === 'outside') return `${output.label}: Used by ${output.owner}, not managed by Bowties. Show details`;
    return `${output.label}: Free`;
  }

  const PIN_LEGEND: Array<{ state: PinState; text: string }> = [
    { state: 'selected', text: 'Selected lamp' },
    { state: 'mine', text: 'This signal' },
    { state: 'facility', text: 'Used by a facility' },
    { state: 'sibling', text: 'Used by another signal' },
    { state: 'outside', text: 'Used outside Bowties' },
    { state: 'free', text: 'Free' },
  ];

  /** What the shared pin grid shows for one pin of the chosen node. */
  function pinView(ordinal: number): PinGridPinView {
    const output = host?.outputs.find((candidate) => candidate.ordinal === ordinal);
    if (!output) return { state: 'free', who: '', ariaLabel: `#${ordinal}`, disabled: true };
    const state = pinState(output);
    const owned = state === 'outside' || heldOutside(output);
    return {
      state,
      who: pinWho(output, state),
      ariaLabel: pinAriaLabel(output, state),
      disabled: state === 'facility' || state === 'sibling',
      mark: heldOutside(output),
      inspected: owned && output.owner === inspected?.owner,
      dataOwner: owned ? (output.owner ?? undefined) : undefined,
    };
  }

  function pinClick(ordinal: number): void {
    const output = host?.outputs.find((candidate) => candidate.ordinal === ordinal);
    if (output) pinClicked(output, pinState(output));
  }

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
  const logicList = (numbers: number[]) => numbers.map((n) => `Logic ${n}`).join(', ');

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

  /** The pin the user clicked to open the card; the lamp goes there when the release is confirmed. */
  let inspectedPin = $state<number | null>(null);
  /** The lamp already on the clicked pin, when it is one this signal chose; otherwise null. */
  let inspectedLamp = $state<Lamp | null>(null);

  function pinClicked(output: PinOutput, state: PinState): void {
    if (state === 'sibling') return;
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
  }
</script>

<div class="step-body">
  <div class="step-top">
    <div class="wiring-head">
      <span class="field-label">Select a signal to configure</span>
      <span class="muted small">{occupancyStatus(occupancy, view.blocks.map((block) => block.blockId))}</span>
    </div>
    <TrackDiagram
      input={trackInputFor(
        { blocks: view.blocks, direction: view.direction, movements: view.movements, entranceEnd: view.entranceEnd, exitEnd: view.exitEnd, signals, shapes: view.turnoutShapes },
        'signals',
        { selectedSignalId: signal?.signalId ?? null, issueCounts: view.wiringAttentionCounts },
      )}
      {occupancy}
      onSelectSignal={(signalId) => (selectedSignalId = signalId)}
    />
    {#if signal}
      <div class="wiring-title">
        <h3 class="selected-heading">{signal.name}</h3>
        <span class="wiring-progress">{view.wiredCount} of {view.signalCount} signals wired</span>
        <button type="button" class="btn" disabled={nextUnwiredId === null} onclick={() => { if (nextUnwiredId !== null) selectedSignalId = nextUnwiredId; }}>Next unwired signal ▶</button>
      </div>
      <div role="group" aria-label="Signal LCC for the selected signal">
        <SignalLccPicker
          nodeKeys={view.outputHosts.map((h) => h.nodeKey)}
          selected={wiring?.nodeKey ?? null}
          label="Runs on"
          onChoose={chooseHost}
          bind:open={pickerOpen}
        />
      </div>
    {/if}
    {#if view.finishError}
      <p class="error" role="alert">{view.finishError}</p>
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
        {#if attentionTexts.length > 0}
          <div class="attention">
              <ul class="issues" aria-label="Choices needing attention">
              {#each attentionTexts as text}
                <li>{text}</li>
              {/each}
            </ul>
          </div>
        {/if}
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
          <PinGrid
            outputs={host.outputs}
            {pinView}
            onPinClick={pinClick}
            legend={PIN_LEGEND}
            ariaLabel={`Pins on ${hostName(host.nodeKey)}`}
            bind:element={pinGrid}
          />
          {#if inspected}
            {@const card = inspected}
            {@const target = card.target}
            <OutsideOwnerRelease
              owner={card.owner}
              description={card.description}
              details={[
                `${card.isMast ? 'Rule to Aspect' : 'Direct Lamp Control row'} · drives ${card.pins.map((pin) => pin.label).join(', ')}`,
                ...(card.isMast && card.matchedLogic.length > 0 ? [`Aspects set by ${logicList(card.matchedLogic)}`] : []),
                `Configured on ${hostName(host.nodeKey)} outside Bowties, for example by hand, by another tool, or in earlier testing.`,
              ]}
              pinLabels={card.pins.map((pin) => pin.label)}
              resets={[card.owner, ...card.matchedLogic.filter((n) => !card.keptLogic.includes(n)).map((n) => `Logic ${n}`)]}
              keptLogic={card.keptLogic}
              mentionsOtherSenders={card.isMast}
              releaseLabel={inspectedLamp
                ? `Release ${card.owner}`
                : `Release and put ${selectedLamp === 'red' ? 'Red' : 'Green'} on ${pinLabel(inspectedPin ?? card.pins[0].ordinal)}`}
              onRelease={() => releaseAndUse(card, inspectedPin ?? card.pins[0].ordinal)}
              onClose={closeCard}
              onShowInConfiguration={target
                ? () => configFocusStore.focusConfigField(target.nodeId, target.elementPath)
                : undefined}
            />
          {/if}
          {/if}
        {:else}
          <h3>Pins</h3>
          <p class="muted">Choose a Signal LCC to see its pins.</p>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .wiring-head { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .wiring-head .field-label { margin: 0; }
  .selected-heading { margin: 10px 0 6px; }
  .evidence-note { margin: 0; }
  .error { margin: 8px 0 0; color: var(--fluent-dangerBackground); }
  .wiring-title { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 12px; margin-bottom: 12px; }
  .wiring-title .selected-heading { margin: 0; }
  .wiring-progress { margin-left: auto; color: var(--text-muted, #666); font-size: 12px; }
  .attention { margin: 10px 0; padding: 10px 12px; border-left: 3px solid #d17b0f; border-radius: 3px; background: #fff7e6; color: var(--text-secondary, #424242); font-size: 12px; }
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
</style>
