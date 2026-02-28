<script lang="ts">
  /**
   * TreeLeafRow — renders a single leaf node from the unified tree.
   *
   * Horizontal layout: label (fixed width) + value + inline description.
   * Descriptions visible by default; truncated at ~100 chars with expand.
   * Maps enum values to labels via `constraints.mapEntries`.
   * Event IDs in monospace dotted hex, "(not set)" for all-zeros.
   *
   * Spec: plan-cdiConfigNavigator, Steps 4/5/6.
   */
  import type { LeafConfigNode, TreeConfigValue, TreeMapEntry } from '$lib/types/nodeTree';
  import type { BowtieCard } from '$lib/api/tauri';
  import { bowtieName } from '$lib/api/tauri';
  import { goto } from '$app/navigation';

  export let leaf: LeafConfigNode;
  /** Current nesting depth — adjusts compact mode at depth >= 3 */
  export let depth: number = 0;
  /**
   * Cross-reference: the BowtieCard this event ID slot participates in.
   * When present, renders a "Used in: …" navigable link.
   */
  export let usedIn: BowtieCard | undefined = undefined;

  const DESC_TRUNCATE_THRESHOLD = 120;
  const DESC_TRUNCATE_AT = 100;

  let descExpanded = false;

  /** Format a TreeConfigValue for display */
  function formatValue(v: TreeConfigValue | null): string {
    if (v === null) return '—';
    switch (v.type) {
      case 'int':     return formatIntValue(v.value);
      case 'string':  return v.value || '(empty)';
      case 'float':   return v.value.toFixed(4);
      case 'eventId': return formatEventId(v.bytes);
    }
  }

  /** Map int values to enum labels when mapEntries exist */
  function formatIntValue(value: number): string {
    if (leaf.constraints?.mapEntries) {
      const entry = leaf.constraints.mapEntries.find((e: TreeMapEntry) => e.value === value);
      if (entry) return entry.label;
    }
    return String(value);
  }

  /** Format event ID bytes; all-zero = "(not set)" */
  function formatEventId(bytes: number[]): string {
    if (bytes.every(b => b === 0)) return '(not set)';
    return bytes.map(b => b.toString(16).padStart(2, '0')).join('.');
  }

  /** Whether the value should use monospace font */
  $: isMonoValue = leaf.elementType === 'eventId' || leaf.elementType === 'int' || leaf.elementType === 'float';

  /** Truncated description text (if needed) */
  $: descText = leaf.description ?? '';
  $: needsTruncation = descText.length > DESC_TRUNCATE_THRESHOLD;
  $: displayDesc = needsTruncation && !descExpanded
    ? descText.slice(0, DESC_TRUNCATE_AT) + '…'
    : descText;

  function handleNavigateToBowties() {
    if (usedIn) {
      goto('/bowties?highlight=' + usedIn.event_id_hex);
    }
  }
</script>

<div class="field-row" class:compact={depth >= 3} role="listitem">
  <span class="field-label">{leaf.name}</span>

  <div class="field-content">
    <span class="field-value" class:mono={isMonoValue}>
      {formatValue(leaf.value)}
    </span>

    {#if descText}
      <span class="field-desc">
        {displayDesc}
        {#if needsTruncation}
          <button
            class="desc-expand-btn"
            on:click={() => (descExpanded = !descExpanded)}
            aria-label={descExpanded ? 'Collapse description' : 'Expand description'}
          >{descExpanded ? '[−]' : '[+]'}</button>
        {/if}
      </span>
    {/if}

    {#if leaf.eventRole}
      <span class="event-role">
        <span class="role-tag role-{leaf.eventRole.toLowerCase()}">{leaf.eventRole}</span>
      </span>
    {/if}

    {#if usedIn}
      <span class="used-in">
        → <button
          class="used-in-link"
          on:click={handleNavigateToBowties}
          title="View bowtie for event {usedIn.event_id_hex}"
          aria-label="View bowtie connection for {bowtieName(usedIn)}"
        >{bowtieName(usedIn)}</button>
      </span>
    {/if}
  </div>
</div>

<style>
  /* ══════════════════════════════════════════
     Fluent UI Design — TreeLeafRow
     ══════════════════════════════════════════ */

  .field-row {
    display: flex;
    align-items: baseline;
    gap: var(--field-gap, 8px);
    min-height: 26px;
    padding: 2px 4px;
    font-family: 'Segoe UI', -apple-system, BlinkMacSystemFont, 'Helvetica Neue', Arial, sans-serif;
    transition: background-color 0.1s ease;
    border-radius: 3px;
  }

  .field-row:hover {
    background-color: rgba(0,0,0,0.02);            /* very subtle hover for scannability */
  }

  .field-row.compact {
    --field-gap: 4px;
    min-height: 22px;
    padding: 1px 4px;
  }

  .field-label {
    flex: 0 0 var(--field-label-width, 120px);
    text-align: right;
    color: #605e5c;                                /* colorNeutralForeground2 — subdued so values stand out */
    font-size: 12px;
    font-weight: 400;
    line-height: 1.45;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
    font-size: 13px;
    line-height: 1.45;
  }

  .field-value {
    color: #242424;                                /* colorNeutralForeground1 */
    word-break: break-word;
  }

  .field-value.mono {
    font-family: 'Cascadia Code', 'Cascadia Mono', 'SF Mono', 'Fira Code', 'Consolas', monospace;
    font-size: 12px;
    letter-spacing: -0.01em;
  }

  .field-desc {
    color: #8a8886;                                /* warmer hint gray */
    font-size: 12px;
    font-style: italic;                            /* hint-like feel */
    line-height: 1.35;
  }

  .desc-expand-btn {
    background: none;
    border: none;
    padding: 0 2px;
    font-size: 12px;
    font-style: normal;
    color: #0078d4;                                /* colorBrandForeground1 */
    cursor: pointer;
    font-weight: 600;
  }

  .desc-expand-btn:hover {
    text-decoration: underline;
    color: #106ebe;                                /* colorBrandForeground1Hover */
  }

  .desc-expand-btn:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 1px;
    border-radius: 2px;
  }

  .event-role {
    font-size: 11px;
  }

  .role-tag {
    font-weight: 600;
    padding: 1px 6px;
    border-radius: 4px;                            /* borderRadiusMedium */
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .role-tag.role-producer {
    color: #0b6a0b;                                /* colorPaletteGreenForeground1 */
    background: #dff6dd;                           /* colorPaletteGreenBackground1 */
  }

  .role-tag.role-consumer {
    color: #0078d4;                                /* colorBrandForeground1 */
    background: #deecf9;                           /* colorPaletteBlueBg1 */
  }

  .role-tag.role-ambiguous {
    color: #605e5c;                                /* colorNeutralForeground2 */
    background: #f3f2f1;                           /* colorNeutralBackground3 */
  }

  .used-in {
    font-size: 11px;
    color: #8a8886;                                /* warmer gray */
  }

  .used-in-link {
    background: none;
    border: none;
    padding: 0;
    font-size: 11px;
    color: #0078d4;                                /* colorBrandForeground1 */
    cursor: pointer;
    text-decoration: underline;
  }

  .used-in-link:hover {
    color: #106ebe;                                /* colorBrandForeground1Hover */
  }

  .used-in-link:focus-visible {
    outline: 2px solid #0078d4;
    outline-offset: 1px;
    border-radius: 2px;
  }
</style>
