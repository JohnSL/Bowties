<script lang="ts">
  import type { InformationChannel } from '$lib/api/channels';
  import {
    channelStateClass,
    channelStateLabel,
    type ChannelState,
  } from '$lib/utils/channelState';

  const ROLE_LABELS: Record<string, string> = {
    'block-occupancy': 'Block occupancy',
    'lamp-indicator': 'Lamp indicator',
  };

  /**
   * Spec 018 / S5 — lamp-indicator info-tooltip text. Made discoverable on
   * every consumer row to satisfy AC #5 (D5 deferral: the user must set Lamp
   * Selection manually before the lamp will follow the bowtie's commands).
   */
  const LAMP_INDICATOR_TOOLTIP =
    'Live state is derived from observed Lamp On / Lamp Off commands on the bus. ' +
    'Set Lamp Selection on this row in the Config tab to drive the lamp.';

  const STATE_TOOLTIPS: Record<string, string> = {
    'no-config': 'No configuration data — channel cannot resolve event IDs yet.',
    unknown: 'Unknown — no events received',
    clear: 'Clear',
    occupied: 'Occupied',
    lit: 'Lit',
    unlit: 'Unlit',
  };

  const DEFAULT_STATE: ChannelState = { kind: 'unknown' };

  let {
    channel,
    channelState = DEFAULT_STATE,
    usedBy,
    onRename,
  }: {
    channel: InformationChannel;
    /** Spec 018 / S5 D3 — typed discriminated state for this channel. */
    channelState?: ChannelState;
    /**
     * Slot bindings consuming this channel as `{facility, slot}` pairs.
     * Empty / undefined → rendered as em-dash (Spec 018 / S3 baseline).
     * Spec 018 / S4 fills this from facility slot bindings; multi-binding
     * scenarios (e.g. ABS) supply more than one entry.
     */
    usedBy?: ReadonlyArray<{ facilityName: string; slotLabel: string }>;
    onRename?: (id: string, newName: string) => void;
  } = $props();

  let isEditingName = $state(false);
  let nameEditValue = $state('');

  function startRename() {
    nameEditValue = channel.name;
    isEditingName = true;
  }

  function commitRename() {
    isEditingName = false;
    const trimmed = nameEditValue.trim();
    if (trimmed.length === 0) return;
    if (trimmed === channel.name) return;
    onRename?.(channel.id, trimmed);
  }

  function handleNameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') {
      isEditingName = false;
    }
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function formatConnectorLabel(connectorId: string): string {
    const match = connectorId.match(/^connector-([a-z])$/i);
    if (match) return `Connector ${match[1].toUpperCase()}`;
    return connectorId;
  }

  let roleLabel = $derived(ROLE_LABELS[channel.role] ?? channel.role);
  let stateClass = $derived(channelStateClass(channelState));
  let stateLabel = $derived(channelStateLabel(channelState));
  let stateTooltip = $derived(STATE_TOOLTIPS[stateClass] ?? stateLabel);
  let isLampIndicator = $derived(channel.role === 'lamp-indicator');
  let ownershipLabel = $derived(channel.ownership === 'hardware-owned' ? 'HW' : 'USER');
  let ownershipTitle = $derived(
    channel.ownership === 'hardware-owned'
      ? 'Hardware-owned — lifecycle follows the backing hardware-config selection'
      : 'User-owned — created via a facility slot',
  );
  let location = $derived.by(() => {
    if (channel.binding.kind === 'connectorInput') {
      const connectorLabel = formatConnectorLabel(channel.binding.connector);
      return `${connectorLabel} · Input ${channel.binding.input}`;
    }
    if (channel.binding.kind === 'lampRow') {
      return `Row ${channel.binding.rowOrdinal}`;
    }
    return '';
  });
  let usedByText = $derived.by(() => {
    if (!usedBy || usedBy.length === 0) return '—';
    return usedBy.map((b) => `${b.facilityName} / ${b.slotLabel}`).join('; ');
  });
</script>

<tr class="channel-row">
  <td class="state-cell">
    <span
      class="occupancy-indicator"
      class:occupied={stateClass === 'occupied'}
      class:clear={stateClass === 'clear'}
      class:lit={stateClass === 'lit'}
      class:unlit={stateClass === 'unlit'}
      class:unknown={stateClass === 'unknown'}
      class:no-config={stateClass === 'no-config'}
      title={stateTooltip}
      aria-label={stateTooltip}
      data-testid="occupancy-indicator"
    ></span>
  </td>
  <td class="name-cell">
    <div class="name-cell-content">
      {#if isEditingName}
        <input
          class="channel-name-input"
          type="text"
          bind:value={nameEditValue}
          onblur={commitRename}
          onkeydown={handleNameKeydown}
          use:focusInput
          aria-label="Edit channel name"
        />
      {:else}
        <button class="channel-name" onclick={startRename} title="Click to rename">
          {channel.name}
        </button>
      {/if}
      <span
        class="ownership-badge"
        class:hw={channel.ownership === 'hardware-owned'}
        class:user={channel.ownership === 'user-owned'}
        title={ownershipTitle}
      >{ownershipLabel}</span>
    </div>
  </td>
  <td class="role-style-cell">
    <div class="role-style">
      <span class="role">{roleLabel}</span>
      <span class="style">{channel.style}</span>
    </div>
  </td>
  <td class="location-cell">{location}</td>
  <td class="state-label-cell">
    <span class="state-label">{stateLabel}</span>
    {#if isLampIndicator}
      <span
        class="lamp-indicator-info"
        title={LAMP_INDICATOR_TOOLTIP}
        aria-label={LAMP_INDICATOR_TOOLTIP}
        data-testid="lamp-indicator-info"
      >ⓘ</span>
    {/if}
  </td>
  <td class="used-by-cell" class:unbound={!usedBy || usedBy.length === 0}>
    {usedByText}
  </td>
</tr>

<style>
  .channel-row {
    border-bottom: 1px solid var(--border-subtle, #eee);
  }
  .channel-row:hover {
    background: var(--surface-hover, #fafafa);
  }
  .state-cell {
    width: 30px;
    text-align: center;
    padding: 0.5rem 0;
  }
  .occupancy-indicator {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--text-muted, #999);
    background: transparent;
  }
  .occupancy-indicator.occupied {
    width: 10px;
    height: 10px;
    border: none;
    background: #d55e00;
  }
  .occupancy-indicator.clear {
    border: none;
    background: #009e73;
  }
  .occupancy-indicator.lit {
    width: 10px;
    height: 10px;
    border: none;
    background: #e6c200;
    box-shadow: 0 0 4px rgba(230, 194, 0, 0.6);
  }
  .occupancy-indicator.unlit {
    border: none;
    background: #555;
  }
  .occupancy-indicator.no-config {
    border-style: dashed;
    border-color: var(--text-muted, #999);
    opacity: 0.6;
  }
  .name-cell {
    padding: 0.5rem 0.6rem;
    white-space: nowrap;
  }
  .name-cell-content {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }
  .channel-name {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    color: var(--text-primary, #222);
  }
  .channel-name:hover {
    text-decoration: underline;
  }
  .channel-name-input {
    font: inherit;
    font-weight: 500;
    padding: 0.1rem 0.25rem;
    border: 1px solid var(--border-focus, #4a9eff);
    border-radius: 3px;
    outline: none;
    min-width: 14ch;
  }
  .ownership-badge {
    display: inline-block;
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    padding: 1px 6px;
    border-radius: 3px;
    line-height: 1.4;
  }
  .ownership-badge.hw {
    background: var(--badge-hw-bg, #e0f2fe);
    color: var(--badge-hw-fg, #075985);
  }
  .ownership-badge.user {
    background: var(--badge-user-bg, #fef3c7);
    color: var(--badge-user-fg, #92400e);
  }
  .role-style-cell {
    padding: 0.4rem 0.6rem;
  }
  .role-style {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
  }
  .role {
    font-size: 0.8rem;
    color: var(--text-primary, #222);
  }
  .style {
    font-size: 0.7rem;
    color: var(--text-muted, #888);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .location-cell {
    padding: 0.5rem 0.6rem;
    font-size: 0.8rem;
    color: var(--text-secondary, #555);
    white-space: nowrap;
  }
  .state-label-cell {
    padding: 0.5rem 0.6rem;
    font-size: 0.8rem;
  }
  .lamp-indicator-info {
    margin-left: 0.35rem;
    color: var(--text-muted, #999);
    cursor: help;
    user-select: none;
  }
  .used-by-cell {
    padding: 0.5rem 0.6rem;
    font-size: 0.8rem;
    color: var(--text-secondary, #555);
    white-space: nowrap;
  }
  .used-by-cell.unbound {
    color: var(--text-muted, #999);
  }
</style>
