<script lang="ts">
  /**
   * DialogActions — optional leading actions and right-aligned navigation.
   *
   * Used inside `<Dialog>`'s `actions` snippet. Authors place `<Button>`s as
   * children for trailing actions, primary action last. The leading snippet
   * places a wizard's Cancel action at the left edge.
   */
  import type { Snippet } from 'svelte';

  interface Props {
    /** Optional left-aligned action, such as a wizard's Cancel button. */
    leading?: Snippet;
    children: Snippet;
  }

  let { leading, children }: Props = $props();
</script>

<div class="fluent-dialog-actions">
  {#if leading}
    <div class="fluent-dialog-actions__leading" data-testid="dialog-actions-leading">
      {@render leading()}
    </div>
  {/if}
  <div class="fluent-dialog-actions__trailing" data-testid="dialog-actions-trailing">
    {@render children()}
  </div>
</div>

<style>
  .fluent-dialog-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 8px;
  }
  .fluent-dialog-actions__leading,
  .fluent-dialog-actions__trailing {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .fluent-dialog-actions__trailing {
    margin-left: auto;
  }
  @media (max-width: 390px) {
    .fluent-dialog-actions__leading,
    .fluent-dialog-actions__trailing {
      width: 100%;
    }
    .fluent-dialog-actions__trailing {
      justify-content: flex-end;
    }
  }
</style>
