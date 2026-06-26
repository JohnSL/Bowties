<script lang="ts">
  import type { InformationChannel } from '$lib/api/channels';
  import type { OccupancyState } from '$lib/utils/channelState';

  let {
    channel,
    nodeName,
    occupancyState = 'unknown',
    onRename,
  }: {
    channel: InformationChannel;
    nodeName: (nodeKey: string) => string;
    occupancyState?: OccupancyState;
    onRename?: (id: string, newName: string) => void;
  } = $props();

  const INDICATOR_TOOLTIP: Record<OccupancyState, string> = {
    'no-config': 'Configuration not available for this node',
    unknown: 'Unknown — no events received',
    clear: 'Clear',
    occupied: 'Occupied',
  };


  let isEditingName = $state(false);
  let nameEditValue = $state('');

  function startRename() {
    nameEditValue = channel.name;
    isEditingName = true;
  }

  function commitRename() {
    isEditingName = false;
    const trimmed = nameEditValue.trim();
    if (trimmed.length === 0) return; // reject empty — retain previous name
    if (trimmed === channel.name) return; // no change — suppress no-op
    onRename?.(channel.id, trimmed);
  }

  function handleNameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') { isEditingName = false; }
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<li class="channel-card">
  <span
    class="occupancy-indicator"
    class:occupied={occupancyState === 'occupied'}
    class:clear={occupancyState === 'clear'}
    class:unknown={occupancyState === 'unknown'}
    class:no-config={occupancyState === 'no-config'}
    title={INDICATOR_TOOLTIP[occupancyState]}
    aria-label={INDICATOR_TOOLTIP[occupancyState]}
    data-testid="occupancy-indicator"
  ></span>
  <div class="card-content">
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
    <div class="channel-hardware">
      {nodeName(channel.hardwareRef.nodeKey)} — {channel.hardwareRef.connector} — Input {channel.hardwareRef.input}
    </div>
  </div>
</li>

<style>
  .channel-card {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--border-subtle, #e2e2e2);
    border-radius: 0.375rem;
    background: var(--surface-color, #fff);
  }
  .channel-card:hover {
    border-color: var(--border-color, #ccc);
  }
  .occupancy-indicator {
    flex-shrink: 0;
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
    background: #d55e00; /* Okabe-Ito vermillion */
  }
  .occupancy-indicator.clear {
    border: none;
    background: #009e73; /* Okabe-Ito teal-green */
  }
  /* Spec 017 / S2: dashed border distinguishes "no resolvable config" from
     the solid-bordered "unknown" state. Tooltip + aria-label carry the
     full meaning; the shape change provides a non-colour perceptual channel. */
  .occupancy-indicator.no-config {
    border-style: dashed;
    border-color: var(--text-muted, #999);
    opacity: 0.6;
  }
  .card-content {
    flex: 1;
    min-width: 0;
  }
  .channel-name {
    display: block;
    font-size: 0.875rem;
    font-weight: 500;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
    color: var(--text-primary, #222);
  }
  .channel-name:hover {
    text-decoration: underline;
  }
  .channel-name-input {
    display: block;
    font-size: 0.875rem;
    font-weight: 500;
    font: inherit;
    font-weight: 500;
    padding: 0;
    border: 1px solid var(--border-focus, #4a9eff);
    border-radius: 3px;
    outline: none;
    width: 100%;
  }
  .channel-hardware {
    font-size: 0.75rem;
    color: var(--text-muted, #999);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
