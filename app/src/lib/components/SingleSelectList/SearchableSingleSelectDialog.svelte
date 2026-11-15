<script lang="ts">
  import type { Snippet } from 'svelte';
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';
  import SingleSelectList, { type ListItem } from './SingleSelectList.svelte';

  let {
    title: dialogTitle,
    filterLabel,
    items,
    searchTextFor,
    ariaLabel,
    name,
    row,
    emptyMessage = 'No matching items.',
    onConfirm,
    onCancel,
  }: {
    title: string;
    filterLabel: string;
    items: ListItem<string>[];
    searchTextFor: (key: string) => string;
    ariaLabel: string;
    name: string;
    row: Snippet<[string]>;
    emptyMessage?: string;
    onConfirm: (key: string) => void | Promise<void>;
    onCancel: () => void;
  } = $props();

  let selectedKey = $state<string | undefined>();
  let search = $state('');

  const filteredItems = $derived.by(() => {
    const query = search.trim().toLowerCase();
    return query.length === 0
      ? items
      : items.filter((item) => searchTextFor(item.key).toLowerCase().includes(query));
  });

  $effect(() => {
    if (selectedKey !== undefined && !filteredItems.some((item) => item.key === selectedKey)) {
      selectedKey = undefined;
    }
  });

  async function confirm() {
    if (selectedKey !== undefined) await onConfirm(selectedKey);
  }

</script>

{#snippet leading()}
  <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
{/snippet}

<Dialog open width="md" ariaLabel={dialogTitle} initialFocus="first" onCancel={onCancel}>
  {#snippet title()}
    <DialogTitle>{dialogTitle}</DialogTitle>
  {/snippet}

  <form class="selection-dialog-form" onsubmit={(event) => { event.preventDefault(); confirm(); }}>
    <input
      class="selection-dialog-search"
      type="search"
      placeholder="Search…"
      aria-label={filterLabel}
      bind:value={search}
    />
    <SingleSelectList
      items={filteredItems}
      bind:selectedKey
      {ariaLabel}
      {name}
      {row}
      {emptyMessage}
    />
    <button class="hidden-submit" type="submit" tabindex="-1" aria-hidden="true"></button>
  </form>

  {#snippet actions()}
    <DialogActions {leading}>
      <Button appearance="primary" disabled={selectedKey === undefined} onclick={confirm}>Confirm</Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .selection-dialog-form { display: flex; flex-direction: column; gap: 12px; }
  .selection-dialog-search {
    padding: 6px 10px;
    border: 1px solid var(--fluent-neutralStroke1);
    border-radius: 4px;
    background: var(--fluent-neutralBackground1);
    color: var(--fluent-neutralForeground1);
    font: inherit;
  }
  .selection-dialog-search:focus {
    outline: none;
    border-color: var(--fluent-strokeFocus2);
    box-shadow: 0 0 0 2px var(--fluent-strokeFocusHalo);
  }
  .hidden-submit { position: absolute; width: 0; height: 0; border: 0; opacity: 0; pointer-events: none; }
</style>
