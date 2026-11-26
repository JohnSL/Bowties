<script lang="ts">
  /**
   * WriteOutcomeList — per-board list of a facility's waiting or not-yet-written
   * board changes (Spec 020 / E5-T4). Shared by FacilityCard and PanelCard; the
   * caller renders it only while `effectiveLayoutStore.facilityWriteOutcome(...).hasWaiting`.
   */
  import type { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';

  type Boards = ReturnType<typeof effectiveLayoutStore.facilityWriteOutcome>['boards'];

  let { boards }: { boards: Boards } = $props();

  function plural(n: number): string {
    return `${n} ${n === 1 ? 'change' : 'changes'}`;
  }
</script>

<ul class="write-outcome" aria-label="Write outcome" data-testid="facility-write-outcome">
  {#each boards as board (board.nodeKey)}
    {#if board.waiting > 0}
      <li>{plural(board.waiting)} waiting on {board.nodeName}{board.waitingReasons.length > 0 ? `: ${board.waitingReasons[0]}` : ''}</li>
    {/if}
    {#if board.notAttempted > 0}
      <li>{plural(board.notAttempted)} not yet written to {board.nodeName}</li>
    {/if}
  {/each}
</ul>

<style>
  .write-outcome {
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 0.75rem;
    color: #bc4b09;
  }
</style>
