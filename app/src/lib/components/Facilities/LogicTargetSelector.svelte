<!--
  LogicTargetSelector — Node picker for logic compilation target (Spec 020 / S1).

  Displays a list of candidate nodes that can serve as logic targets
  (Tower LCC nodes with conditional line capacity). Shows capacity
  per node and lets the user select one, then confirm.

  Uses SingleSelectList for consistent picker interaction (ADR-0014).
  Boundary: Component — renders props, emits intent via callbacks.
  No async, no IPC, no lifecycle management.
-->
<script lang="ts">
  import Dialog from '$lib/components/Dialog/Dialog.svelte';
  import DialogTitle from '$lib/components/Dialog/DialogTitle.svelte';
  import DialogActions from '$lib/components/Dialog/DialogActions.svelte';
  import Button from '$lib/components/Dialog/Button.svelte';
  import SingleSelectList from '$lib/components/SingleSelectList/SingleSelectList.svelte';
  import type { LogicCapacity } from '$lib/api/logicAdapter';

  interface Props {
    /** Available candidate nodes with their keys and display names. */
    candidates: Array<{ nodeKey: string; displayName: string; capacity?: LogicCapacity }>;
    /** Optional initial selection seed (e.g. proximity-based suggestion). */
    selectedNodeKey?: string;
    /** Callback when the user confirms their selection. */
    onConfirm: (nodeKey: string) => void;
    /** Callback when the user cancels the selection. */
    onCancel: () => void;
  }

  let { candidates, selectedNodeKey, onConfirm, onCancel }: Props = $props();

  let localSelectedKey = $state<string | undefined>(undefined);

  // Seed from prop once on mount; validates against candidates.
  $effect(() => {
    if (localSelectedKey === undefined && selectedNodeKey && candidates.some((c) => c.nodeKey === selectedNodeKey)) {
      localSelectedKey = selectedNodeKey;
    }
  });

  const listItems = $derived(
    candidates.map((c) => ({ key: c.nodeKey })),
  );

  const confirmDisabled = $derived(localSelectedKey === undefined);

  function candidateByKey(key: string) {
    return candidates.find((c) => c.nodeKey === key);
  }

  function handleConfirm() {
    if (localSelectedKey === undefined) return;
    onConfirm(localSelectedKey);
  }
</script>

<Dialog open width="md" ariaLabel="Select Logic Target Node" onCancel={onCancel}>
  {#snippet title()}
    <DialogTitle>Select Logic Target Node</DialogTitle>
  {/snippet}

  <form
    class="lts-form"
    onsubmit={(e) => { e.preventDefault(); handleConfirm(); }}
  >
    <p class="description">
      Choose a Tower LCC node to host the compiled signal logic.
    </p>

    <SingleSelectList
      items={listItems}
      bind:selectedKey={localSelectedKey}
      ariaLabel="Logic target candidates"
      name="logic-target"
      emptyMessage="No candidate nodes available."
    >
      {#snippet row(key)}
        {@const candidate = candidateByKey(key)}
        {#if candidate}
          <span class="lts-row-content">
            <span class="node-name">{candidate.displayName}</span>
            {#if candidate.capacity}
              <span class="capacity">
                {candidate.capacity.totalLines - candidate.capacity.usedLines}/{candidate.capacity.totalLines} lines available
              </span>
            {/if}
          </span>
        {/if}
      {/snippet}
    </SingleSelectList>

    <button type="submit" class="lts-hidden-submit" tabindex="-1" aria-hidden="true"></button>
  </form>

  {#snippet actions()}
    <DialogActions>
      <Button appearance="secondary" onclick={onCancel}>Cancel</Button>
      <Button appearance="primary" disabled={confirmDisabled} onclick={handleConfirm}>
        Confirm
      </Button>
    </DialogActions>
  {/snippet}
</Dialog>

<style>
  .lts-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  .description {
    color: var(--fluent-neutralForeground2);
    font-size: var(--fluent-fontSizeBase200);
    margin: 0;
  }
  .lts-row-content {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
  }
  .node-name {
    font-weight: 500;
    color: var(--fluent-neutralForeground1);
  }
  .capacity {
    font-size: var(--fluent-fontSizeBase200);
    color: var(--fluent-neutralForeground2);
  }
  .lts-hidden-submit {
    position: absolute;
    width: 0;
    height: 0;
    padding: 0;
    border: 0;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
  }
</style>
