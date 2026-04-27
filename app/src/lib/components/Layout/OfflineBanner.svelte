<script lang="ts">
  import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';

  interface Props {
    capturedAt: string | null;
    layoutId?: string | null;
    isConnected?: boolean;
    isSyncDismissed?: boolean;
    onsyncrequest?: () => void;
  }

  let { capturedAt, layoutId = null, isConnected = false, isSyncDismissed = false, onsyncrequest }: Props = $props();

  const capturedText = $derived(capturedAt ? new Date(capturedAt).toLocaleString() : 'unknown time');
  const showSyncButton = $derived(isConnected && isSyncDismissed && offlineChangesStore.pendingCount > 0);
</script>

<div class="offline-banner" role="status" aria-live="polite">
  <strong>Offline</strong>
  <span class="sep">•</span>
  <span>Captured {capturedText}</span>
  {#if layoutId}
    <span class="sep">•</span>
    <span>{layoutId}</span>
  {/if}
  {#if offlineChangesStore.pendingCount > 0}
    <span class="sep">•</span>
    <span class="pending-badge">
      {offlineChangesStore.pendingCount} pending {offlineChangesStore.pendingCount === 1 ? 'change' : 'changes'}
    </span>
  {/if}
  {#if showSyncButton}
    <span class="sep">•</span>
    <button class="sync-btn" onclick={onsyncrequest}>Open Sync Panel</button>
  {/if}
</div>

<style>
  .offline-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    margin: 8px;
    border-radius: 8px;
    background: #f0fdfa;
    border: 1px solid rgba(15, 118, 110, 0.3);
    color: #134e4a;
    font-size: 13px;
  }

  .sep {
    opacity: 0.6;
  }

  .pending-badge {
    font-weight: 500;
    background: rgba(15, 118, 110, 0.14);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .sync-btn {
    cursor: pointer;
    background: #0f766e;
    color: #fff;
    border: none;
    border-radius: 4px;
    padding: 2px 8px;
    font-size: 13px;
    font-weight: 500;
  }

  .sync-btn:hover {
    background: #115e59;
  }
</style>
