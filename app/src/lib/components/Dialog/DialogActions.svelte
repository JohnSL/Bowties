<script lang="ts">
  /**
   * DialogActions — right-aligned footer button cluster.
   *
   * Used inside `<Dialog>`'s `actions` snippet. Authors place `<Button>`s as
   * children; Cancel-style actions come first, primary action last (Fluent v9
   * convention).
   */
  import type { Snippet } from 'svelte';

  interface Props {
    children: Snippet;
  }

  let { children }: Props = $props();
</script>

<div class="fluent-dialog-actions">
  {@render children()}
</div>

<style>
  .fluent-dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-wrap: wrap;
  }
</style>
