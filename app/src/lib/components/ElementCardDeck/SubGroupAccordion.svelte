<script lang="ts">
  import type { CardSubGroup, CardField } from '$lib/stores/configSidebar';
  import type { ConfigValueWithMetadata, ConfigValue } from '$lib/api/types';

  export let subGroup: CardSubGroup;
  export let nodeId: string;
  export let configValues: Map<string, ConfigValueWithMetadata>;
  /** Current nesting depth — used to indent deeper levels */
  export let depth: number = 0;
  /**
   * Whether this group is collapsible (accordion). True for replicated groups
   * (replication > 1). False for non-replicated groups — rendered inline,
   * always visible.
   */
  export let collapsible: boolean = true;

  let expanded = false;

  function getValue(field: CardField): ConfigValueWithMetadata | null {
    const key = `${nodeId}:${field.elementPath.join('/')}`;
    return configValues.get(key) ?? null;
  }

  function formatValue(meta: ConfigValueWithMetadata | null): string {
    if (!meta) return '—';
    const v: ConfigValue = meta.value;
    switch (v.type) {
      case 'Int':     return String(v.value);
      case 'String':  return v.value || '(empty)';
      case 'Float':   return v.value.toFixed(4);
      case 'EventId': return v.value.map((b: number) => b.toString(16).padStart(2, '0')).join('.');
      case 'Invalid': return `(error: ${v.error})`;
      default:        return '—';
    }
  }
</script>

{#if collapsible}
  <!-- Replicated group — collapsible accordion (collapsed by default) -->
  <div class="subgroup-accordion" style="--depth: {depth}">
    <button
      class="subgroup-header"
      class:expanded
      on:click={() => (expanded = !expanded)}
      aria-expanded={expanded}
    >
      <span class="expand-icon" aria-hidden="true">{expanded ? '▾' : '▸'}</span>
      <span class="subgroup-name">{subGroup.name}</span>
    </button>

    {#if expanded}
      <div class="subgroup-body">
        {#if subGroup.description}
          <p class="subgroup-description">{subGroup.description}</p>
        {/if}

        {#if subGroup.fields.length > 0}
          <div class="fields-list" role="list">
            {#each subGroup.fields as field (field.elementPath.join('/'))}
              <div class="field-row" role="listitem">
                <span class="field-name">{field.name}</span>
                {#if field.description}
                  <p class="field-description">{field.description}</p>
                {/if}
                <span class="field-value">{formatValue(getValue(field))}</span>
              </div>
            {/each}
          </div>
        {/if}

        <!-- Recursive: pass collapsible based on each child's replication -->
        {#each subGroup.subGroups as deeper (deeper.groupPath.join('/'))}
          <svelte:self
            subGroup={deeper}
            {nodeId}
            {configValues}
            depth={depth + 1}
            collapsible={deeper.replication > 1}
          />
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <!-- Non-replicated group — inline section, always visible -->
  <div class="inline-section" style="--depth: {depth}">
    <div class="inline-header">
      <span class="inline-name">{subGroup.name}</span>
      {#if subGroup.description}
        <p class="subgroup-description">{subGroup.description}</p>
      {/if}
    </div>

    {#if subGroup.fields.length > 0}
      <div class="fields-list" role="list">
        {#each subGroup.fields as field (field.elementPath.join('/'))}
          <div class="field-row" role="listitem">
            <span class="field-name">{field.name}</span>
            {#if field.description}
              <p class="field-description">{field.description}</p>
            {/if}
            <span class="field-value">{formatValue(getValue(field))}</span>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Recursive -->
    {#each subGroup.subGroups as deeper (deeper.groupPath.join('/'))}
      <svelte:self
        subGroup={deeper}
        {nodeId}
        {configValues}
        depth={depth + 1}
        collapsible={deeper.replication > 1}
      />
    {/each}
  </div>
{/if}

<style>
  /* ── Accordion (collapsible / replicated groups) ── */
  .subgroup-accordion {
    margin-top: 4px;
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    overflow: hidden;
  }

  .subgroup-header {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 7px 12px;
    background: var(--subgroup-header-bg, #f5f5f5);
    border: none;
    cursor: pointer;
    text-align: left;
    gap: 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #333);
    transition: background-color 0.1s;
  }

  .subgroup-header:hover {
    background-color: var(--hover-bg, #ebebeb);
  }

  .subgroup-header.expanded {
    border-bottom: 1px solid var(--border-color, #e0e0e0);
  }

  .expand-icon {
    flex-shrink: 0;
    font-size: 22px;
    color: var(--text-secondary, #666);
    width: 18px;
    line-height: 1;
  }

  .subgroup-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .subgroup-body {
    padding: 0 12px 8px;
    background: var(--card-bg, #fff);
  }

  /* ── Inline section (non-replicated groups) ── */
  .inline-section {
    margin-top: 8px;
    padding-left: calc(var(--depth, 0) * 8px);
  }

  .inline-header {
    margin-bottom: 4px;
  }

  .inline-name {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary, #555);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  /* ── Shared ── */
  .subgroup-description {
    margin: 3px 0 4px;
    font-size: 11px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
    white-space: pre-wrap;                         /* preserve newlines from CDI descriptions */
  }

  .fields-list {
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    background: var(--card-bg, #fff);
    overflow: hidden;
    margin-bottom: 4px;
  }

  .field-row {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border-light, #f0f0f0);
  }

  .field-row:last-child {
    border-bottom: none;
  }

  .field-name {
    display: block;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #333);
  }

  .field-description {
    margin: 2px 0 3px;
    font-size: 11px;
    color: var(--text-secondary, #666);
    line-height: 1.4;
    white-space: pre-wrap;                         /* preserve newlines from CDI descriptions */
  }

  .field-value {
    display: block;
    font-size: 12px;
    font-family: monospace;
    color: var(--text-secondary, #555);
  }
</style>
