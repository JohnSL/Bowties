<script lang="ts">
  /**
   * OutsideOwnerRelease — the card shown for a pin held outside Bowties: who
   * holds it, and a two-step Release. Shared by the Automatic Block Signals and
   * Panel Wiring steps. Purely presentational: what releasing does is the
   * caller's `onRelease`; only the confirm step and focus are local.
   */
  import { tick } from 'svelte';

  let {
    owner,
    description = null,
    details = [],
    pinLabels,
    resets,
    keptLogic = [],
    mentionsOtherSenders = false,
    releaseLabel,
    onRelease,
    onClose,
    onShowInConfiguration,
  }: {
    owner: string;
    /** The owner's Lamp Description, when it has one. */
    description?: string | null;
    /** Extra explanatory lines shown under the description. */
    details?: string[];
    /** Labels of the pins that become free. */
    pinLabels: string[];
    /** What is reset when the release is saved (the owner, and for a mast its Logic). Defaults to the owner. */
    resets?: string[];
    /** Logic that also sends other events, so it is kept. */
    keptLogic?: number[];
    mentionsOtherSenders?: boolean;
    /** Text of the confirm button. */
    releaseLabel: string;
    onRelease: () => void;
    onClose?: () => void;
    onShowInConfiguration?: () => void;
  } = $props();

  const resetList = $derived(resets ?? [owner]);
  let confirming = $state(false);
  let releaseButton = $state<HTMLButtonElement | null>(null);
  $effect(() => {
    owner;
    confirming = false;
  });

  function cancel(): void {
    confirming = false;
    void tick().then(() => releaseButton?.focus());
  }

  /** Escape closes only the card; the editor dialog listens for Escape on the window. */
  function keydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape' || !onClose) return;
    event.stopPropagation();
    event.preventDefault();
    onClose();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="outside-card" aria-label={`${owner}, not managed by Bowties`} onkeydown={keydown}>
  {#if onClose}
    <button type="button" class="card-close" aria-label={`Close ${owner} details`} onclick={onClose}>×</button>
  {/if}
  <h4>{owner} <span class="outside-tag">Not managed by Bowties</span></h4>
  <p>{description ? `Description: "${description}"` : 'No description.'}</p>
  {#each details as line}
    <p>{line}</p>
  {/each}
  {#if confirming}
    <p>Release {owner}? {pinLabels.length > 1 ? `All ${pinLabels.length} of its pins` : 'Its pin'} ({pinLabels.join(', ')}) {pinLabels.length > 1 ? 'become' : 'becomes'} free, and {resetList.join(', ')} {resetList.length > 1 ? 'are' : 'is'} reset when you save. Discard undoes it.</p>
    {#each keptLogic as n (n)}
      <p>Logic {n} also sends other events, so it is kept; only its actions that set {owner}'s aspects are disconnected.</p>
    {/each}
    {#if mentionsOtherSenders}
      <p>Other boards or JMRI may still send events to {owner}.</p>
    {/if}
    <div class="outside-actions">
      <button type="button" class="btn" onclick={cancel}>Cancel</button>
      <button type="button" class="btn btn-primary" onclick={onRelease}>{releaseLabel}</button>
    </div>
  {:else}
    <div class="outside-actions">
      {#if onShowInConfiguration}
        <button type="button" class="btn" aria-label={`Show ${owner} in Configuration`} onclick={onShowInConfiguration}>Show in Configuration</button>
      {/if}
      <button type="button" class="btn" bind:this={releaseButton} onclick={() => (confirming = true)}>Release and use…</button>
    </div>
  {/if}
</section>

<style>
  .card-close {
    position: absolute; top: 6px; right: 6px; width: 24px; height: 24px;
    display: inline-flex; align-items: center; justify-content: center; padding: 0;
    border: none; border-radius: 4px; background: transparent; color: var(--abs-muted);
    font-size: 16px; line-height: 1; cursor: pointer;
  }
  .card-close:hover { background: #f3e6c4; color: var(--abs-text); }
  .card-close:focus-visible { outline: 2px solid var(--abs-brand); outline-offset: 1px; }
  .outside-card { position: relative; margin-top: 10px; padding: 10px 12px; border: 1px solid #e0c36a; border-radius: 5px; background: #fffaf0; font-size: 12px; }
  .outside-card h4 { display: flex; align-items: baseline; gap: 8px; margin: 0 0 4px; padding-right: 28px; font-size: 13px; }
  .outside-tag { color: #8a5d00; font-size: 11px; font-weight: 600; }
  .outside-card p { margin: 2px 0; color: var(--abs-text-secondary); }
  .outside-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 8px; }
</style>
