<!--
  T017: ConnectorArrow.svelte
  Renders the centre column of a bowtie card: a large right-pointing arrow (→)
  indicating the event connection between producers and consumers (FR-005).
-->

<script lang="ts">
  // No props - arrow is now standalone with no event ID display
</script>

<div class="connector-arrow" aria-label="event connector arrow">
  <span class="arrow-symbol" aria-hidden="true">→</span>
</div>

<style>
  .connector-arrow {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 0 12px;
    flex-shrink: 0;
    align-self: center;
    margin-top: -10px;
  }

  .arrow-symbol {
    font-size: 3rem;
    color: #0078d4;
    line-height: 1;
    user-select: none;
  }
</style>
