<script lang="ts">
  /**
   * PanelCard — the Railroad-tab card for a Panel facility. Renders the name, type
   * chip, derived status (from the effectiveLayoutStore facade) and block count.
   * Edit and Remove arrive with the editing slice; per-block lamp tiles with S2.
   */
  import type { Facility } from '$lib/api/facilities';
  import { effectiveLayoutStore } from '$lib/layout/effectiveLayoutStore.svelte';

  let { facility }: { facility: Facility } = $props();

  const status = $derived(effectiveLayoutStore.panelStatus(facility.facilityId));
  const count = $derived(facility.panel?.entries.length ?? 0);
</script>

<article class="panel-card" data-testid="panel-card" data-facility-id={facility.facilityId}>
  <header class="head">
    <span class="name">{facility.name}</span>
    <span class="type">Panel</span>
    <span class="status" class:wired={status.status === 'Wired'} class:incomplete={status.status === 'Incomplete'}>{status.status}</span>
  </header>
  <div class="sub">
    {#if count === 0}
      <span>No blocks yet.</span>
    {:else}
      <span>{count} {count === 1 ? 'block' : 'blocks'}</span>
      {#if status.needsLamp > 0}
        <span class="needs">{status.needsLamp} needs a lamp</span>
      {/if}
    {/if}
  </div>
</article>

<style>
  .panel-card {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding: 0.75rem 0.875rem;
    border: 1px solid var(--border-color, #d1d1d1);
    border-radius: 6px;
    background: var(--surface-color, #fff);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex-wrap: wrap;
  }
  .name {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text-primary, #242424);
  }
  .type {
    font-size: 0.6875rem;
    color: var(--text-muted, #616161);
  }
  .status {
    font-size: 0.6875rem;
    font-weight: 600;
    padding: 0.125rem 0.5rem;
    border-radius: 10px;
    background: #ececec;
    color: #424242;
  }
  .status.incomplete {
    background: #fff4ce;
    color: #bc4b09;
  }
  .status.wired {
    background: #dcfce7;
    color: #166534;
  }
  .sub {
    display: flex;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: var(--text-muted, #616161);
  }
  .needs {
    color: #bc4b09;
  }
</style>
