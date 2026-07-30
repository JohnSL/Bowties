<script lang="ts">
  /**
   * SingleSelectList — shared single-select radiogroup list primitive.
   *
   * Owns accessible radiogroup semantics, selection state presentation,
   * keyboard navigation, and consistent row/group-header styling.
   * Domain dialogs supply items (with optional group headers), row
   * content via the `row` snippet, and receive selection changes via
   * `onSelect`.
   */
  import type { Snippet } from 'svelte';

  export interface ListItem<T = string> {
    /** Unique key used for selection tracking. */
    key: T;
    /** When true, rendered as a non-selectable group header instead of a selectable row. */
    isHeader?: boolean;
  }

  let {
    items,
    selectedKey = $bindable<string | undefined>(undefined),
    ariaLabel,
    name,
    onSelect,
    row,
    header,
    emptyMessage = 'No items available.',
  }: {
    items: ListItem<string>[];
    selectedKey?: string | undefined;
    ariaLabel: string;
    /** The `name` attribute for the radio group. */
    name: string;
    onSelect?: (key: string) => void;
    /** Snippet to render a selectable row. Receives the item key. */
    row: Snippet<[string]>;
    /** Snippet to render a group header. Receives the item key. */
    header?: Snippet<[string]>;
    emptyMessage?: string;
  } = $props();

  const selectableItems = $derived(items.filter((i) => !i.isHeader));

  function handleChange(key: string) {
    selectedKey = key;
    onSelect?.(key);
  }
</script>

{#if selectableItems.length === 0}
  <p class="ssl-empty">{emptyMessage}</p>
{:else}
  <ul class="ssl-list" role="radiogroup" aria-label={ariaLabel}>
    {#each items as item (item.key)}
      {#if item.isHeader}
        <li class="ssl-group-header" data-testid="ssl-group-header">
          {#if header}
            {@render header(item.key)}
          {:else}
            {item.key}
          {/if}
        </li>
      {:else}
        <li class="ssl-list-item">
          <label class="ssl-row" class:selected={selectedKey === item.key}>
            <input
              type="radio"
              {name}
              value={item.key}
              checked={selectedKey === item.key}
              onchange={() => handleChange(item.key)}
              data-testid="ssl-radio"
            />
            {@render row(item.key)}
          </label>
        </li>
      {/if}
    {/each}
  </ul>
{/if}

<style>
  .ssl-empty {
    color: var(--fluent-neutralForeground2);
    margin: 0.5rem 0;
    font-size: var(--fluent-fontSizeBase200);
  }
  .ssl-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 18rem;
    overflow-y: auto;
    border: 1px solid var(--fluent-neutralStroke2, #e2e2e2);
    border-radius: 4px;
  }
  .ssl-list-item {
    margin: 0;
  }
  .ssl-group-header {
    list-style: none;
    margin: 0;
    padding: 0.4rem 0.6rem;
    background: var(--fluent-neutralBackground3, #f7f7f7);
    border-bottom: 1px solid var(--fluent-neutralStroke2, #e2e2e2);
    color: var(--fluent-neutralForeground1);
    font-weight: 600;
    font-size: var(--fluent-fontSizeBase200);
    position: sticky;
    top: 0;
  }
  .ssl-row {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    cursor: pointer;
    border-bottom: 1px solid var(--fluent-neutralStroke2, #f0f0f0);
  }
  .ssl-row:last-child {
    border-bottom: none;
  }
  .ssl-row:hover {
    background: var(--fluent-neutralBackground1Hover, #f5f5f5);
  }
  .ssl-row.selected {
    background: var(--fluent-neutralBackground1Selected, #eef4ff);
  }
  .ssl-row input[type='radio'] {
    margin: 0;
  }
</style>
