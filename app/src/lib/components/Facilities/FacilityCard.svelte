<script lang="ts">
  import type { Facility, FacilityStatus } from '$lib/api/facilities';
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import { channelsStore } from '$lib/stores/channels.svelte';
  import { eventStateStore } from '$lib/stores/eventState.svelte';
  import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';
  import { resolveNodeName } from '$lib/layout';
  import {
    deriveChannelState,
    deriveSignalAspectState,
    deriveLedLampStates,
    channelStateLabel,
    channelStateClass,
    roleForChannelState,
    type ChannelState,
  } from '$lib/utils/channelState';
  import { getStyleRowCount } from '$lib/utils/channelStyles';
  import { facilitiesStore } from '$lib/stores/facilities.svelte';
  import FacilitySlot from './FacilitySlot.svelte';
  import SlotCard from './SlotCard.svelte';

  let {
    facility,
    template,
    resolvedEventIds,
    onRename,
    onDelete,
    onSelectChannel,
    onAddChannel,
    onRemoveFromSlot,
  }: {
    facility: Facility;
    template?: BehaviorTemplate;
    /** Map from channelId to state-name → eventId (Spec 018 / S5 D6). */
    resolvedEventIds?: ReadonlyMap<string, Record<string, string>>;
    onRename?: (facilityId: string, newName: string) => void;
    onDelete?: (facilityId: string) => void;
    /** Spec 018 / S4 — producer-side input slot's Select channel intent. */
    onSelectChannel?: (facilityId: string, slotLabel: string) => void;
    /** Spec 018 / S5 — consumer-side output slot's Add channel intent. */
    onAddChannel?: (facilityId: string, slotLabel: string) => void;
    onRemoveFromSlot?: (facilityId: string, slotLabel: string, currentChannelId: string) => void;
  } = $props();

  // Spec 018 / S6 (D5): status is derived by the effectiveLayoutStore facade
  // per ADR-0004 (single-owner derivation). FacilityCard renders the pill
  // from the facade call — no local slot-fullness check.
  let status = $derived<FacilityStatus>(
    effectiveLayoutStore.facilityStatus(facility.facilityId),
  );

  let isEditingName = $state(false);
  let nameEditValue = $state('');

  function startRename() {
    nameEditValue = facility.name;
    isEditingName = true;
  }
  function commitRename() {
    isEditingName = false;
    const trimmed = nameEditValue.trim();
    if (trimmed.length === 0) return;
    if (trimmed === facility.name) return;
    onRename?.(facility.facilityId, trimmed);
  }
  function handleNameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') { isEditingName = false; }
  }
  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
  function handleDelete() {
    onDelete?.(facility.facilityId);
  }

  function slotsOrdered(): Array<[string, string[]]> {
    if (template) {
      return template.slots.map((s) => [s.label, facility.slotBindings[s.label] ?? []]);
    }
    return Object.entries(facility.slotBindings);
  }

  /** Primary slots for the main slot-grid (excludes optional downstream-signal). */
  function primarySlots(): Array<[string, string[]]> {
    return slotsOrdered().filter(([label]) => label !== 'downstream-signal');
  }

  // Spec 020 / S4: compiled-template facilities show the comprehension view
  // (Inputs → Logic → Outputs) as their primary layout. No expand/collapse.
  let isCompiled = $derived(template?.compilationTarget === 'compiled');

  /** Logic target node key for compiled facilities. */
  let logicTargetNodeKey = $derived(facilitiesStore.getLogicTargetNodeKey(facility.facilityId));

  /**
   * Derive current signal evaluation from input channel states.
   * Walks the template rules in priority order, checking if the input
   * channel's state matches the rule's condition.
   */
  function currentEvaluation(): { aspect: string; reason: string } | undefined {
    if (!template?.rules || template.rules.length === 0) return undefined;
    const inputBinding = facility.slotBindings['input'];
    if (!inputBinding || inputBinding.length === 0) return undefined;
    const inputDisplay = displayFor(inputBinding);
    if (!inputDisplay.currentChannelDisplay) return undefined;
    const inputState = inputDisplay.currentChannelDisplay.state;
    if ('kind' in inputState) return undefined; // no-config or unknown

    // Check downstream-signal binding for Approach evaluation
    const downstreamBinding = facility.slotBindings['downstream-signal'];
    const downstreamDisplay = downstreamBinding?.length ? displayFor(downstreamBinding) : undefined;
    const downstreamState = downstreamDisplay?.currentChannelDisplay?.state;
    const downstreamIsStop = downstreamState && !('kind' in downstreamState) && downstreamState.state === 'stop';

    // Evaluate: Stop > Approach > Clear
    if (inputState.state === 'occupied') {
      return { aspect: 'stop', reason: 'next block occupied' };
    }
    if (inputState.state === 'clear') {
      if (downstreamIsStop) {
        return { aspect: 'approach', reason: 'downstream signal at Stop' };
      }
      return { aspect: 'clear', reason: 'next block clear' };
    }
    return undefined;
  }

  /**
   * Get per-lamp LED states for signal-aspect output channels.
   */
  function outputLampStates(binding: string[]): Array<{ label: string; isOn: boolean; color: 'red' | 'green'; rowLabel: string }> {
    if (binding.length === 0) return [];
    const id = binding[0];
    const channel = channelsStore.channels.find((c) => c.id === id);
    if (!channel || channel.role !== 'signal-aspect') return [];
    const ids = resolvedEventIds?.get(id);
    if (!ids) return [];
    const lamps = deriveLedLampStates(
      eventStateStore.events,
      ids['redOn'], ids['redOff'], ids['greenOn'], ids['greenOff'],
    );
    const startRow = channel.binding.kind === 'lampRow' ? channel.binding.rowOrdinal : 0;
    return lamps.map((lamp, i) => ({ ...lamp, rowLabel: `Row ${startRow + i}` }));
  }

  function formatConnectorLabel(connectorId: string): string {
    const match = connectorId.match(/^connector-([a-z])$/i);
    if (match) return `Connector ${match[1].toUpperCase()}`;
    return connectorId;
  }

  /**
   * Resolve the FacilitySlot filled-state display from the channel id.
   * UI is max-1 in S4: we pick element 0 of the Vec, leaving multi-binding
   * rendering to a future slice when ABS aspect-slot repeaters arrive.
   */
  function displayFor(binding: string[]):
    | { currentChannelId: string; currentChannelDisplay: { name: string; ownership: 'hardware-owned' | 'user-owned'; groupLabel: string; locationLabel: string; state: ChannelState; stateLabel: string } }
    | { currentChannelId: undefined; currentChannelDisplay: undefined } {
    if (binding.length === 0) {
      return { currentChannelId: undefined, currentChannelDisplay: undefined };
    }
    const id = binding[0];
    const channel = channelsStore.channels.find((c) => c.id === id);
    if (!channel) {
      return { currentChannelId: undefined, currentChannelDisplay: undefined };
    }
    const ids = resolvedEventIds?.get(id);
    const role = roleForChannelState(channel.role);
    let state: ChannelState;
    if (role === 'signal-aspect') {
      state = deriveSignalAspectState(eventStateStore.events, ids?.['redOn'], ids?.['redOff'], ids?.['greenOn'], ids?.['greenOff']);
    } else {
      const positiveId = role === 'lamp-indicator' ? ids?.['lit'] : ids?.['occupied'];
      const negativeId = role === 'lamp-indicator' ? ids?.['unlit'] : ids?.['clear'];
      state = deriveChannelState(eventStateStore.events, positiveId, negativeId, role);
    }
    const groupLabel = channel.binding.kind === 'connectorInput'
      ? formatConnectorLabel(channel.binding.connector)
      : 'Direct Lamp Control';
    const locationLabel = channel.binding.kind === 'connectorInput'
      ? `Input ${channel.binding.input}`
      : (() => {
          const rowCount = getStyleRowCount(channel.style);
          const start = channel.binding.rowOrdinal;
          return rowCount > 1
            ? `Rows ${start}–${start + rowCount - 1}`
            : `Row ${start}`;
        })();
    return {
      currentChannelId: id,
      currentChannelDisplay: {
        name: channel.name,
        ownership: channel.ownership,
        groupLabel,
        locationLabel,
        state,
        stateLabel: channelStateLabel(state),
      },
    };
  }
</script>

<article class="facility-card" data-testid="facility-card" data-facility-id={facility.facilityId}>
  <header class="facility-header">
    <div class="facility-title">
      {#if isEditingName}
        <input
          class="facility-name-input"
          type="text"
          bind:value={nameEditValue}
          onblur={commitRename}
          onkeydown={handleNameKeydown}
          use:focusInput
          aria-label="Edit facility name"
        />
      {:else}
        <button type="button" class="facility-name" onclick={startRename} title="Click to rename">
          {facility.name}
        </button>
      {/if}
      <span class="template-label">{template?.displayName ?? facility.templateId}</span>
      <span class="status-pill" class:wired={status === 'Wired'} class:incomplete={status === 'Incomplete'}>
        <span class="pulse" aria-hidden="true"></span>{status}
      </span>
    </div>
    <div class="actions">
      <button type="button" class="btn-link" onclick={startRename} aria-label="Rename facility">Rename</button>
      <button type="button" class="btn-link danger" onclick={handleDelete} aria-label="Delete facility">Delete</button>
    </div>
  </header>

  {#if isCompiled}
    <section class="comprehension-view" data-testid="comprehension-view">
      <!-- INPUTS column -->
      <div class="cv-column cv-inputs">
        <h4 class="cv-heading">Inputs</h4>
        {#each slotsOrdered().filter(([label]) => {
          const def = template?.slots.find(s => s.label === label);
          return def?.kind === 'producer';
        }) as [label, binding] (label)}
          {@const d = displayFor(binding)}
          {@const def = template?.slots.find(s => s.label === label)}
          <SlotCard
            label={def?.displayLabel ?? label}
            state={d.currentChannelDisplay?.state}
            stateLabel={d.currentChannelDisplay?.stateLabel}
            channelName={d.currentChannelDisplay?.name}
            channelId={d.currentChannelId}
            ownership={d.currentChannelDisplay?.ownership}
            meta={d.currentChannelDisplay ? `${d.currentChannelDisplay.groupLabel} · ${d.currentChannelDisplay.locationLabel}` : undefined}
            empty={!d.currentChannelDisplay}
            slotLabel={label}
            onAddChannel={(slot) => onSelectChannel?.(facility.facilityId, slot)}
            onRemoveFromSlot={(slot, currentId) => onRemoveFromSlot?.(facility.facilityId, slot, currentId)}
            data-slot={label}
          />
        {/each}
      </div>

      <!-- Flow arrow -->
      <div class="cv-arrow" aria-hidden="true">→</div>

      <!-- LOGIC column -->
      <div class="cv-column cv-logic">
        <h4 class="cv-heading">Logic</h4>
        <div class="cv-card cv-logic-card">
          <div class="cv-card-header">
            <span class="cv-card-label">{template?.displayName ?? 'Rules'}</span>
            {#if currentEvaluation()}
              {@const evalResult = currentEvaluation()!}
              <span class="cv-badge-row">
                <span
                  class="cv-state-dot"
                  class:signal-stop={evalResult.aspect === 'stop'}
                  class:signal-approach={evalResult.aspect === 'approach'}
                  class:signal-clear={evalResult.aspect === 'clear'}
                ></span>
                <span class="cv-state-label">{evalResult.aspect.charAt(0).toUpperCase() + evalResult.aspect.slice(1)}</span>
              </span>
            {/if}
          </div>
          <div class="cv-rules-list">
            <div class="cv-rule" class:cv-rule-active={currentEvaluation()?.aspect === 'stop'}>
              <span class="cv-rule-indicator" aria-hidden="true">{currentEvaluation()?.aspect === 'stop' ? '▶' : '•'}</span>
              Next block occupied → <span class="cv-rule-aspect-stop">Stop</span>
            </div>
            <div class="cv-rule" class:cv-rule-active={currentEvaluation()?.aspect === 'approach'}>
              <span class="cv-rule-indicator" aria-hidden="true">{currentEvaluation()?.aspect === 'approach' ? '▶' : '•'}</span>
              Downstream signal at Stop → <span class="cv-rule-aspect-approach">Approach</span>
            </div>
            <div class="cv-rule" class:cv-rule-active={currentEvaluation()?.aspect === 'clear'}>
              <span class="cv-rule-indicator" aria-hidden="true">{currentEvaluation()?.aspect === 'clear' ? '▶' : '•'}</span>
              Otherwise → <span class="cv-rule-aspect-clear">Clear</span>
            </div>
          </div>
          <div class="cv-logic-footer">
            <div class="cv-logic-target">
              <span class="cv-meta-label">Runs on:</span>
              <span class="cv-target-value">{logicTargetNodeKey ? resolveNodeName(logicTargetNodeKey) : 'Not assigned'}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Flow arrow -->
      <div class="cv-arrow" aria-hidden="true">→</div>

      <!-- OUTPUTS column -->
      <div class="cv-column cv-outputs">
        <h4 class="cv-heading">Outputs</h4>
        {#each slotsOrdered().filter(([label]) => {
          const def = template?.slots.find(s => s.label === label);
          return def?.kind === 'consumer';
        }) as [label, binding] (label)}
          {@const d = displayFor(binding)}
          {@const def = template?.slots.find(s => s.label === label)}
          {@const lamps = outputLampStates(binding)}
          <SlotCard
            label={def?.displayLabel ?? label}
            state={d.currentChannelDisplay?.state}
            stateLabel={d.currentChannelDisplay?.stateLabel}
            channelName={d.currentChannelDisplay?.name}
            channelId={d.currentChannelId}
            ownership={d.currentChannelDisplay?.ownership}
            meta={d.currentChannelDisplay ? `${d.currentChannelDisplay.groupLabel} · ${d.currentChannelDisplay.locationLabel}` : undefined}
            empty={!d.currentChannelDisplay}
            slotLabel={label}
            onAddChannel={(slot) => onAddChannel?.(facility.facilityId, slot)}
            onRemoveFromSlot={(slot, currentId) => onRemoveFromSlot?.(facility.facilityId, slot, currentId)}
            data-slot={label}
          >
            {#snippet extraContent()}
              {#if lamps.length > 0}
                <div class="cv-lamp-breakdown">
                  {#each lamps as lamp}
                    <div class="cv-lamp-row">
                      <span class="cv-lamp-dot" class:lamp-on={lamp.isOn} class:lamp-red={lamp.color === 'red'} class:lamp-green={lamp.color === 'green'}></span>
                      <span class="cv-lamp-label">{lamp.label}:</span>
                      <span class="cv-lamp-state" class:lamp-state-on={lamp.isOn}>{lamp.isOn ? 'ON' : 'off'}</span>
                      <span class="cv-lamp-row-label">— {lamp.rowLabel}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            {/snippet}
          </SlotCard>
        {/each}
      </div>
    </section>
  {:else}
    <div class="slot-grid">
      <div class="slot-column">
        <h4 class="cv-heading">Inputs</h4>
        {#each primarySlots().filter(([label]) => {
          const def = template?.slots.find(s => s.label === label);
          return def?.kind === 'producer';
        }) as [label, binding] (label)}
          {@const d = displayFor(binding)}
          <FacilitySlot
            slotLabel={label}
            {template}
            currentChannelId={d.currentChannelId}
            currentChannelDisplay={d.currentChannelDisplay}
            onAddChannel={(slot) => onSelectChannel?.(facility.facilityId, slot)}
            onRemoveFromSlot={(slot, currentId) => onRemoveFromSlot?.(facility.facilityId, slot, currentId)}
          />
        {/each}
      </div>
      <span class="slot-arrow" aria-hidden="true">→</span>
      <div class="slot-column">
        <h4 class="cv-heading">Outputs</h4>
        {#each primarySlots().filter(([label]) => {
          const def = template?.slots.find(s => s.label === label);
          return def?.kind === 'consumer';
        }) as [label, binding] (label)}
          {@const d = displayFor(binding)}
          <FacilitySlot
            slotLabel={label}
            {template}
            currentChannelId={d.currentChannelId}
            currentChannelDisplay={d.currentChannelDisplay}
            onAddChannel={(slot) => onAddChannel?.(facility.facilityId, slot)}
            onRemoveFromSlot={(slot, currentId) => onRemoveFromSlot?.(facility.facilityId, slot, currentId)}
          />
        {/each}
      </div>
    </div>
  {/if}
</article>

<style>
  .facility-card {
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
    padding: 0.75rem 0.875rem;
    border: 1px solid var(--border-color, #d1d1d1);
    border-radius: 6px;
    background: var(--surface-color, #fff);
  }
  .facility-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .facility-title {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex-wrap: wrap;
    min-width: 0;
  }
  .facility-name,
  .facility-name-input {
    background: transparent;
    border: none;
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text-primary, #242424);
    padding: 0;
    cursor: text;
    font-family: inherit;
  }
  .facility-name:hover {
    text-decoration: underline;
  }
  .facility-name-input {
    border-bottom: 1px solid var(--accent-color, #0f6cbd);
    outline: none;
  }
  .template-label {
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
  }
  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.3125rem;
    font-size: 0.6875rem;
    font-weight: 600;
    padding: 0.125rem 0.5rem;
    border-radius: 10px;
  }
  .status-pill .pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .status-pill.incomplete {
    background: #fff4ce;
    color: #bc4b09;
  }
  .status-pill.wired {
    background: #dcfce7;
    color: #166534;
  }
  .actions {
    display: flex;
    gap: 0.25rem;
  }
  .btn-link {
    background: none;
    border: none;
    color: var(--accent-color, #0f6cbd);
    padding: 0.125rem 0.25rem;
    cursor: pointer;
    font-size: 0.75rem;
    line-height: 1.4;
    font-family: inherit;
  }
  .btn-link:hover {
    text-decoration: underline;
  }
  .btn-link.danger {
    color: #b91c1c;
  }
  .slot-grid {
    display: grid;
    grid-template-columns: 1fr 28px 1fr;
    align-items: start;
    gap: 0.5rem;
    padding-top: 0.625rem;
    border-top: 1px solid var(--border-color, #e5e5e5);
  }
  .slot-column {
    display: flex;
    flex-direction: column;
  }
  .slot-arrow {
    display: flex;
    align-items: center;
    justify-content: center;
    padding-top: 1.75rem;
    color: var(--text-muted, #616161);
    font-size: 1.125rem;
    line-height: 1;
  }
  .comprehension-view {
    display: grid;
    grid-template-columns: 1fr auto 1fr auto 1fr;
    gap: 0.5rem;
    padding-top: 0.625rem;
    border-top: 1px solid var(--border-color, #e5e5e5);
    align-items: start;
  }
  .cv-heading {
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted, #616161);
    margin: 0 0 0.375rem;
  }
  .cv-arrow {
    display: flex;
    align-items: center;
    justify-content: center;
    padding-top: 1.75rem;
    color: var(--text-muted, #94a3b8);
    font-size: 1.25rem;
    line-height: 1;
  }
  .cv-card {
    padding: 0.5rem 0.625rem;
    border: 1px solid var(--border-color, #e5e5e5);
    border-radius: 5px;
    font-size: 0.8125rem;
    margin-bottom: 0.375rem;
    background: var(--bg-subtle, #fafafa);
  }
  .cv-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.25rem;
  }
  .cv-card-label {
    font-weight: 600;
    font-size: 0.75rem;
    text-transform: capitalize;
  }
  .cv-badge-row {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
  }
  .cv-state-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-muted, #616161);
    background: transparent;
    flex-shrink: 0;
  }
  .cv-state-dot.occupied { background: #d55e00; border-color: #d55e00; }
  .cv-state-dot.clear { background: #009e73; border-color: #009e73; }
  .cv-state-dot.lit { background: #e6c200; border-color: #e6c200; }
  .cv-state-dot.unlit { background: #555; border-color: #555; }
  .cv-state-dot.signal-stop { background: #d55e00; border-color: #d55e00; }
  .cv-state-dot.signal-approach { background: #e6c200; border-color: #e6c200; }
  .cv-state-dot.signal-clear { background: #009e73; border-color: #009e73; }
  .cv-state-dot.signal-dark { background: #333; border-color: #333; opacity: 0.5; }
  .cv-state-dot.unknown {
    background: transparent;
    border-style: dashed;
    opacity: 0.6;
  }
  .cv-state-label {
    font-size: 0.6875rem;
    font-weight: 500;
    color: var(--text-secondary, #424242);
  }
  .cv-logic-card {
    background: var(--surface-accent-subtle, #f5f3ff);
    border-color: var(--border-accent, #c4b5fd);
  }
  .cv-rules-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .cv-rule {
    font-size: 0.75rem;
    padding: 0.1875rem 0.375rem;
    border-radius: 3px;
    color: var(--text-secondary, #424242);
  }
  .cv-rule-active {
    font-weight: 500;
  }
  .cv-rule-indicator {
    display: inline-block;
    width: 1em;
    text-align: center;
    font-size: 0.625rem;
  }
  .cv-rule-active .cv-rule-indicator {
    color: var(--text-accent, #5b21b6);
  }
  .cv-rule-aspect-stop { font-weight: 600; color: #b91c1c; }
  .cv-rule-aspect-approach { font-weight: 600; color: #92400e; }
  .cv-rule-aspect-clear { font-weight: 600; color: #065f46; }
  .cv-logic-footer {
    margin-top: 0.5rem;
    padding-top: 0.375rem;
    border-top: 1px solid var(--border-accent, #c4b5fd);
  }
  .cv-logic-target {
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
    margin-top: 0.375rem;
  }
  .cv-meta-label {
    font-weight: 500;
  }
  .cv-target-value {
    color: var(--text-primary, #242424);
  }
  .cv-lamp-breakdown {
    margin-top: 0.375rem;
    padding-top: 0.25rem;
    border-top: 1px solid var(--border-color, #e5e5e5);
  }
  .cv-lamp-row {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.6875rem;
    padding: 0.0625rem 0;
  }
  .cv-lamp-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: #ccc;
    border: 1px solid #aaa;
  }
  .cv-lamp-dot.lamp-on.lamp-red { background: #ef4444; border-color: #dc2626; }
  .cv-lamp-dot.lamp-on.lamp-green { background: #22c55e; border-color: #16a34a; }
  .cv-lamp-dot:not(.lamp-on) { background: #d1d5db; border-color: #9ca3af; }
  .cv-lamp-label {
    color: var(--text-muted, #616161);
  }
  .cv-lamp-state {
    color: var(--text-muted, #616161);
  }
  .cv-lamp-state.lamp-state-on {
    font-weight: 600;
    color: var(--text-primary, #242424);
  }
  .cv-lamp-row-label {
    color: var(--text-muted, #94a3b8);
    font-size: 0.625rem;
    margin-left: auto;
  }
</style>
