<script lang="ts">
  /**
   * TreeLeafRow — renders a single leaf node from the unified tree.
   *
   * Reads value directly from LeafConfigNode.value (populated by merge_config_values).
   * Handles Int, String, Float, EventId, Action, and Blob leaf types.
   *
   * Spec: 007-unified-node-tree, Phase 4.
   */
  import type { LeafConfigNode, TreeConfigValue } from '$lib/types/nodeTree';
  import type { BowtieCard } from '$lib/api/tauri';
  import { bowtieName } from '$lib/api/tauri';
  import { goto } from '$app/navigation';

  export let leaf: LeafConfigNode;
  /**
   * Cross-reference: the BowtieCard this event ID slot participates in.
   * When present, renders a "Used in: …" navigable link.
   */
  export let usedIn: BowtieCard | undefined = undefined;

  let showDescription = false;

  /** Format a TreeConfigValue for display */
  function formatValue(v: TreeConfigValue | null): string {
    if (v === null) return '—';
    switch (v.type) {
      case 'int':     return String(v.value);
      case 'string':  return v.value || '(empty)';
      case 'float':   return v.value.toFixed(4);
      case 'eventId': return formatEventId(v.bytes);
    }
  }

  /** Format event ID bytes; all-zero = "(free)" */
  function formatEventId(bytes: number[]): string {
    if (bytes.every(b => b === 0)) return '(free)';
    return bytes.map(b => b.toString(16).padStart(2, '0')).join('.');
  }

  function handleNavigateToBowties() {
    if (usedIn) {
      goto('/bowties?highlight=' + usedIn.event_id_hex);
    }
  }
</script>

<div class="field-row" role="listitem">
  <div class="field-header">
    <span class="field-label">
      {leaf.name}
      {#if leaf.description}
        <button
          class="description-toggle"
          title="Show/hide description"
          aria-label="Toggle field description"
          on:click={() => (showDescription = !showDescription)}
        >?</button>
      {/if}
    </span>

    <span class="field-value">
      {formatValue(leaf.value)}
    </span>
  </div>

  {#if showDescription && leaf.description}
    <div class="field-description">{leaf.description}</div>
  {/if}

  {#if leaf.eventRole}
    <div class="event-role">
      Role: <span class="role-tag role-{leaf.eventRole.toLowerCase()}">{leaf.eventRole}</span>
    </div>
  {/if}

  {#if usedIn}
    <div class="used-in">
      Used in:
      <button
        class="used-in-link"
        on:click={handleNavigateToBowties}
        title="View bowtie for event {usedIn.event_id_hex}"
        aria-label="View bowtie connection for {bowtieName(usedIn)}"
      >{bowtieName(usedIn)}</button>
    </div>
  {/if}
</div>

<style>
  .field-row {
    padding: 6px 0;
    border-bottom: 1px solid var(--border-light, #f5f5f5);
  }

  .field-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .field-label {
    flex: 1;
    min-width: 0;
    color: var(--text-primary, #333);
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .description-toggle {
    background: none;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 50%;
    width: 16px;
    height: 16px;
    font-size: 10px;
    cursor: pointer;
    color: var(--text-secondary, #666);
    padding: 0;
    line-height: 1;
    flex-shrink: 0;
  }

  .field-value {
    color: var(--text-secondary, #555);
    font-family: monospace;
    font-size: 12px;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-description {
    margin-top: 4px;
    font-size: 11px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
    padding-left: 4px;
  }

  .event-role {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-tertiary, #888);
  }

  .role-tag {
    font-weight: 500;
  }

  .role-tag.role-producer {
    color: var(--producer-color, #2e7d32);
  }

  .role-tag.role-consumer {
    color: var(--consumer-color, #1565c0);
  }

  .role-tag.role-ambiguous {
    color: var(--text-secondary, #666);
  }

  .used-in {
    margin-top: 3px;
    font-size: 11px;
    color: var(--text-tertiary, #888);
  }

  .used-in-link {
    background: none;
    border: none;
    padding: 0;
    font-size: 11px;
    color: var(--link-color, #1565c0);
    cursor: pointer;
    text-decoration: underline;
  }

  .used-in-link:hover {
    color: var(--link-hover-color, #0d47a1);
  }
</style>
