<script lang="ts">
  import type { SelectedDetectedBlock } from '$lib/api/signaling';
  import SearchableSingleSelectDialog from '$lib/components/SingleSelectList/SearchableSingleSelectDialog.svelte';

  let {
    candidateBlocks,
    onConfirm,
    onCancel,
  }: {
    candidateBlocks: SelectedDetectedBlock[];
    onConfirm: (blockId: string) => void | Promise<void>;
    onCancel: () => void;
  } = $props();

  const items = $derived(
    [...candidateBlocks]
      .sort((left, right) => left.name.localeCompare(right.name))
      .map((block) => ({ key: block.blockId })),
  );
  const blockFor = (id: string) => candidateBlocks.find((block) => block.blockId === id);
</script>

<SearchableSingleSelectDialog
  title="Add block"
  filterLabel="Filter blocks"
  {items}
  searchTextFor={(id) => blockFor(id)?.name ?? id}
  ariaLabel="Detected blocks"
  name="detected-block"
  emptyMessage="No additional detected blocks are available."
  {onConfirm}
  {onCancel}
>
  {#snippet row(id)}
    {blockFor(id)?.name ?? id}
  {/snippet}
</SearchableSingleSelectDialog>
