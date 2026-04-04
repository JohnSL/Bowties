<script lang="ts">
  interface Props {
    capturedAt: string | null;
    layoutId?: string | null;
  }

  let { capturedAt, layoutId = null }: Props = $props();

  const capturedText = $derived(capturedAt ? new Date(capturedAt).toLocaleString() : 'unknown time');
</script>

<div class="offline-banner" role="status" aria-live="polite">
  <strong>Offline</strong>
  <span class="sep">•</span>
  <span>Captured {capturedText}</span>
  {#if layoutId}
    <span class="sep">•</span>
    <span>{layoutId}</span>
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
    background: #fff7ed;
    border: 1px solid #fdba74;
    color: #7c2d12;
    font-size: 13px;
  }

  .sep {
    opacity: 0.6;
  }
</style>
