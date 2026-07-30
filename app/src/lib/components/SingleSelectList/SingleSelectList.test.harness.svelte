<script lang="ts">
  /** Test harness for SingleSelectList — renders item keys as row text. */
  import SingleSelectList from './SingleSelectList.svelte';
  import type { ListItem } from './SingleSelectList.svelte';

  let {
    items,
    selectedKey = $bindable<string | undefined>(undefined),
    ariaLabel,
    name,
    onSelect,
    emptyMessage,
  }: {
    items: ListItem<string>[];
    selectedKey?: string | undefined;
    ariaLabel: string;
    name: string;
    onSelect?: (key: string) => void;
    emptyMessage?: string;
  } = $props();
</script>

<SingleSelectList
  {items}
  bind:selectedKey
  {ariaLabel}
  {name}
  {onSelect}
  {emptyMessage}
>
  {#snippet row(key)}
    <span data-testid="row-label">{key}</span>
  {/snippet}
  {#snippet header(key)}
    <span data-testid="header-label">{key}</span>
  {/snippet}
</SingleSelectList>
