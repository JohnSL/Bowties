<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { ReadProgressState, NodeReadState } from '$lib/api/types';

  /** Whether the modal is visible */
  export let visible: boolean = false;
  /** Current discovery phase */
  export let phase: 'discovering' | 'querying' | 'refreshing' | 'reading' | 'complete' | 'cancelled' = 'discovering';
  /** Config reading progress (only meaningful during 'reading' phase) */
  export let readProgress: ReadProgressState | null = null;
  /** Per-node progress states for the reading phase */
  export let nodeReadStates: NodeReadState[] = [];
  /** Whether cancellation is in flight */
  export let isCancelling: boolean = false;
  /** Cancel callback */
  export let onCancel: () => void = () => {};

  // Trap focus within the modal
  function handleKeydown(event: KeyboardEvent) {
    if (!visible) return;
    if (event.key === 'Tab') {
      trapFocus(event);
    }
    // Intentionally no Escape handling — user must use Cancel or wait for completion
  }

  function trapFocus(event: KeyboardEvent) {
    const modal = document.querySelector('.discovery-modal-content');
    if (!modal) return;
    const focusable = modal.querySelectorAll(
      'button:not([disabled]), [tabindex]:not([tabindex="-1"])'
    );
    if (focusable.length === 0) return;
    const first = focusable[0] as HTMLElement;
    const last = focusable[focusable.length - 1] as HTMLElement;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  /** Human-readable phase description */
  function phaseLabel(p: typeof phase): string {
    switch (p) {
      case 'discovering': return 'Discovering nodes on the network…';
      case 'refreshing':  return 'Refreshing nodes on the network…';
      case 'querying':    return 'Querying node information…';
      case 'reading':     return 'Reading node configurations…';
      case 'complete':    return 'Complete';
      case 'cancelled':   return 'Cancelled';
    }
  }

  /** Whether to show the indeterminate spinner (no percentage known) */
  $: indeterminate = phase === 'discovering' || phase === 'querying' || phase === 'refreshing';

  /** Whether the cancel button should be shown */
  $: showCancel = phase !== 'complete' && phase !== 'cancelled';

  /** Whether to show per-node progress list */
  $: showNodeList = phase === 'reading' && nodeReadStates.length > 0;

  /** Progress percentage for the bar */
  $: percentage = readProgress?.percentage ?? 0;

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });
  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

{#if visible}
  <div class="discovery-modal-overlay" role="presentation">
    <div
      class="discovery-modal-content"
      class:dm-wide={showNodeList}
      role="dialog"
      aria-modal="true"
      aria-label="Discovery progress"
    >
      <!-- Header -->
      <h2 class="dm-title">
        {#if phase === 'complete'}
          ✓ Complete
        {:else if phase === 'cancelled'}
          ⚠ Cancelled
        {:else if phase === 'reading'}
          Reading Node Configurations
        {:else}
          Node Discovery
        {/if}
      </h2>

      <!-- Body -->
      <div class="dm-body">
        {#if showNodeList}
          <!-- Per-node progress list -->
          <div class="dm-node-list">
            {#each nodeReadStates as nodeState}
              <div class="dm-node-row">
                <span class="dm-node-name" title={nodeState.nodeId}>{nodeState.name}</span>
                <div class="dm-node-bar-track">
                  <div
                    class="dm-node-bar"
                    class:dm-node-bar-waiting={nodeState.status === 'waiting' || nodeState.status === 'no-cdi'}
                    class:dm-node-bar-reading={nodeState.status === 'reading'}
                    class:dm-node-bar-complete={nodeState.status === 'complete'}
                    class:dm-node-bar-failed={nodeState.status === 'failed'}
                    style="width: {nodeState.status === 'complete' ? 100 : nodeState.status === 'reading' ? nodeState.percentage : 0}%"
                  ></div>
                </div>
                <span
                  class="dm-node-badge"
                  class:dm-badge-complete={nodeState.status === 'complete'}
                  class:dm-badge-reading={nodeState.status === 'reading'}
                  class:dm-badge-failed={nodeState.status === 'failed'}
                  class:dm-badge-nocdi={nodeState.status === 'no-cdi'}
                >
                  {#if nodeState.status === 'waiting'}
                    Waiting
                  {:else if nodeState.status === 'reading'}
                    {nodeState.percentage}%
                  {:else if nodeState.status === 'complete'}
                    ✓
                  {:else if nodeState.status === 'failed'}
                    Failed
                  {:else if nodeState.status === 'no-cdi'}
                    No CDI
                  {/if}
                </span>
              </div>
            {/each}
          </div>
        {:else}
          <!-- Phase text -->
          <p class="dm-phase-text">
            {#if phase === 'reading' && readProgress}
              {#if readProgress.status.type === 'ReadingNode'}
                Reading "{readProgress.status.node_name}" ({readProgress.currentNodeIndex + 1} of {readProgress.totalNodes})
              {:else if readProgress.status.type === 'NodeComplete'}
                ✓ {readProgress.status.node_name}
              {:else}
                Starting configuration read…
              {/if}
            {:else if phase === 'complete' && readProgress}
              All {readProgress.totalNodes} {readProgress.totalNodes === 1 ? 'node' : 'nodes'} read{readProgress.status.type === 'Complete' && readProgress.status.fail_count > 0 ? ` — ${readProgress.status.fail_count} failed` : ''}
            {:else if phase === 'cancelled'}
              Operation was cancelled
            {:else}
              {phaseLabel(phase)}
            {/if}
          </p>

          <!-- Progress bar -->
          <div class="dm-bar-track" aria-hidden="true">
            {#if indeterminate}
              <div class="dm-bar-fill dm-bar-indeterminate"></div>
            {:else}
              <div class="dm-bar-fill" style="width: {percentage}%"></div>
            {/if}
          </div>

          <!-- Percentage / status -->
          {#if phase === 'reading' && readProgress}
            <span class="dm-percentage">{percentage}%</span>
          {:else if phase === 'complete'}
            <span class="dm-percentage">100%</span>
          {/if}
        {/if}
      </div>

      <!-- Footer -->
      <div class="dm-footer">
        {#if showCancel}
          <button
            class="dm-cancel-btn"
            onclick={onCancel}
            disabled={isCancelling}
          >
            {isCancelling ? 'Cancelling…' : 'Cancel'}
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .discovery-modal-overlay {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    animation: dm-fade-in 0.2s ease-out;
  }

  @keyframes dm-fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }

  .discovery-modal-content {
    background: #fff;
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
    width: 380px;
    max-width: 90vw;
    padding: 24px 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: dm-slide-in 0.25s ease-out;
  }

  @keyframes dm-slide-in {
    from { opacity: 0; transform: translateY(-12px) scale(0.97); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

  .dm-title {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #1e293b;
  }

  .dm-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .dm-phase-text {
    margin: 0;
    font-size: 13px;
    color: #475569;
    min-height: 20px;
    line-height: 1.4;
  }

  .dm-bar-track {
    width: 100%;
    height: 8px;
    background: #e2e8f0;
    border-radius: 4px;
    overflow: hidden;
  }

  .dm-bar-fill {
    height: 100%;
    background: #2563eb;
    border-radius: 4px;
    transition: width 0.3s ease-out;
  }

  .dm-bar-indeterminate {
    width: 40%;
    animation: dm-indeterminate 1.4s ease-in-out infinite;
  }

  @keyframes dm-indeterminate {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(350%); }
  }

  .dm-percentage {
    font-size: 12px;
    color: #64748b;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .dm-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    min-height: 32px;
  }

  .dm-cancel-btn {
    padding: 6px 20px;
    font-size: 13px;
    border: 1px solid #cbd5e1;
    border-radius: 6px;
    background: #fff;
    color: #334155;
    cursor: pointer;
    transition: background-color 0.15s, border-color 0.15s;
  }

  .dm-cancel-btn:hover:not(:disabled) {
    background: #f1f5f9;
    border-color: #94a3b8;
  }

  .dm-cancel-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ─── Wide modal for per-node list ────────────────────── */

  .dm-wide {
    width: 500px;
  }

  .dm-node-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 320px;
    overflow-y: auto;
  }

  .dm-node-row {
    display: grid;
    grid-template-columns: 1fr 100px 44px;
    align-items: center;
    gap: 8px;
  }

  .dm-node-name {
    font-size: 13px;
    color: #334155;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .dm-node-bar-track {
    width: 100%;
    height: 6px;
    background: #e2e8f0;
    border-radius: 3px;
    overflow: hidden;
  }

  .dm-node-bar {
    height: 100%;
    border-radius: 3px;
    transition: width 0.3s ease-out;
  }

  .dm-node-bar-waiting  { background: #cbd5e1; }
  .dm-node-bar-reading  { background: #2563eb; }
  .dm-node-bar-complete { background: #16a34a; }
  .dm-node-bar-failed   { background: #dc2626; }

  .dm-node-badge {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    color: #94a3b8;
  }

  .dm-badge-complete { color: #16a34a; font-weight: 600; }
  .dm-badge-reading  { color: #2563eb; }
  .dm-badge-failed   { color: #dc2626; }
  .dm-badge-nocdi    { color: #94a3b8; font-style: italic; }
</style>
