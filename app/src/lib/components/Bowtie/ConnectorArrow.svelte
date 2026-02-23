<!--
  T017: ConnectorArrow.svelte
  Renders the centre column of a bowtie card: a right-pointing arrow (→)
  with the event ID displayed beneath it (FR-005).

  Props:
    eventIdHex: string — dotted-hex event ID shown below the arrow
-->

<script lang="ts">
  interface Props {
    eventIdHex: string;
  }

  let { eventIdHex }: Props = $props();
</script>

<div class="connector-arrow" aria-label="event connector arrow">
  <span class="arrow-symbol" aria-hidden="true">→</span>
  <span class="event-id">{eventIdHex}</span>
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
  }

  .arrow-symbol {
    font-size: 1.5rem;
    color: var(--arrow-color, #64748b);
    line-height: 1;
    user-select: none;
  }

  .event-id {
    font-size: 0.65rem;
    font-family: 'ui-monospace', 'SFMono-Regular', monospace;
    color: var(--text-muted, #64748b);
    text-align: center;
    word-break: break-all;
    max-width: 96px;
  }
</style>
