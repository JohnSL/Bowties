<!--
  LogicTargetSelector — Node picker for logic compilation target (Spec 020 / S1).

  Displays a list of candidate nodes that can serve as logic targets
  (Tower LCC nodes with conditional line capacity). Shows capacity
  per node and lets the user select one.

  Boundary: Component — renders props, emits intent via callbacks.
  No async, no IPC, no lifecycle management.
-->
<script lang="ts">
  import type { LogicCapacity } from '$lib/api/logicAdapter';

  interface Props {
    /** Available candidate nodes with their keys and display names. */
    candidates: Array<{ nodeKey: string; displayName: string; capacity?: LogicCapacity }>;
    /** Currently selected node key, if any. */
    selectedNodeKey?: string;
    /** Callback when the user selects a node. */
    onSelect: (nodeKey: string) => void;
    /** Callback when the user cancels the selection. */
    onCancel: () => void;
  }

  let { candidates, selectedNodeKey, onSelect, onCancel }: Props = $props();
</script>

<div class="logic-target-selector">
  <h3>Select Logic Target Node</h3>
  <p class="description">
    Choose a Tower LCC node to host the compiled signal logic.
  </p>

  {#if candidates.length === 0}
    <p class="empty">No candidate nodes available.</p>
  {:else}
    <ul class="candidate-list">
      {#each candidates as candidate (candidate.nodeKey)}
        <li class="candidate" class:selected={candidate.nodeKey === selectedNodeKey}>
          <button
            type="button"
            class="candidate-button"
            onclick={() => onSelect(candidate.nodeKey)}
          >
            <span class="node-name">{candidate.displayName}</span>
            {#if candidate.capacity}
              <span class="capacity">
                {candidate.capacity.totalLines - candidate.capacity.usedLines}/{candidate.capacity.totalLines} lines available
              </span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="actions">
    <button type="button" class="cancel-btn" onclick={onCancel}>Cancel</button>
  </div>
</div>

<style>
  .logic-target-selector {
    padding: 1rem;
  }
  .description {
    color: var(--vscode-descriptionForeground);
    font-size: 0.85rem;
    margin-bottom: 0.75rem;
  }
  .empty {
    color: var(--vscode-descriptionForeground);
    font-style: italic;
  }
  .candidate-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .candidate {
    margin-bottom: 0.25rem;
  }
  .candidate-button {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    background: var(--vscode-list-hoverBackground);
    border: 1px solid var(--vscode-panel-border);
    border-radius: 4px;
    color: var(--vscode-foreground);
    cursor: pointer;
    font-size: 0.85rem;
  }
  .candidate-button:hover {
    background: var(--vscode-list-activeSelectionBackground);
    color: var(--vscode-list-activeSelectionForeground);
  }
  .selected .candidate-button {
    border-color: var(--vscode-focusBorder);
    background: var(--vscode-list-activeSelectionBackground);
    color: var(--vscode-list-activeSelectionForeground);
  }
  .capacity {
    font-size: 0.75rem;
    color: var(--vscode-descriptionForeground);
  }
  .actions {
    margin-top: 0.75rem;
    display: flex;
    justify-content: flex-end;
  }
  .cancel-btn {
    padding: 0.35rem 0.75rem;
    background: var(--vscode-button-secondaryBackground);
    color: var(--vscode-button-secondaryForeground);
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .cancel-btn:hover {
    background: var(--vscode-button-secondaryHoverBackground);
  }
</style>
