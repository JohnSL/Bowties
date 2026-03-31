<script lang="ts">
  import '../app.css';
  import { SvelteToast } from '@zerodevx/svelte-toast';
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

{@render children()}

<div class="toast-wrap">
  <SvelteToast />
</div>

<style>
  .toast-wrap {
    /* Position bottom-right, Fluent UI–ish styling */
    --toastContainerTop: auto;
    --toastContainerRight: 1.5rem;
    --toastContainerBottom: 1.5rem;
    --toastContainerLeft: auto;
    --toastWidth: 22rem;
    --toastBorderRadius: 4px;
    --toastMsgPadding: 0.6rem 0.75rem;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    font-size: 13px;
  }

  /* Warning style — amber, used for read-only rejection notices */
  .toast-wrap :global(._toastItem.warn) {
    --toastBackground: #fff4ce;
    --toastColor: #4f3a04;
    --toastBarBackground: #835b00;
  }
</style>
