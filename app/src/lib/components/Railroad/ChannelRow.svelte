<script lang="ts">
  import type { InformationChannel } from '$lib/api/channels';

  let {
    channel,
    nodeName,
    onRename,
  }: {
    channel: InformationChannel;
    nodeName: (nodeKey: string) => string;
    onRename?: (id: string, newName: string) => void;
  } = $props();

  let isEditingName = $state(false);
  let nameEditValue = $state('');

  function startRename() {
    nameEditValue = channel.name;
    isEditingName = true;
  }

  function commitRename() {
    isEditingName = false;
    const trimmed = nameEditValue.trim();
    if (trimmed.length === 0) return; // reject empty — retain previous name
    if (trimmed === channel.name) return; // no change — suppress no-op
    onRename?.(channel.id, trimmed);
  }

  function handleNameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') { isEditingName = false; }
  }

  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<li class="channel-card">
  <div class="card-content">
    {#if isEditingName}
      <input
        class="channel-name-input"
        type="text"
        bind:value={nameEditValue}
        onblur={commitRename}
        onkeydown={handleNameKeydown}
        use:focusInput
        aria-label="Edit channel name"
      />
    {:else}
      <button class="channel-name" onclick={startRename} title="Click to rename">
        {channel.name}
      </button>
    {/if}
    <div class="channel-hardware">
      {nodeName(channel.hardwareRef.nodeKey)} — {channel.hardwareRef.connector} — Input {channel.hardwareRef.input}
    </div>
  </div>
</li>

<style>
  .channel-card {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--border-subtle, #e2e2e2);
    border-radius: 0.375rem;
    background: var(--surface-color, #fff);
  }
  .channel-card:hover {
    border-color: var(--border-color, #ccc);
  }
  .card-content {
    flex: 1;
    min-width: 0;
  }
  .channel-name {
    display: block;
    font-size: 0.875rem;
    font-weight: 500;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
    color: var(--text-primary, #222);
  }
  .channel-name:hover {
    text-decoration: underline;
  }
  .channel-name-input {
    display: block;
    font-size: 0.875rem;
    font-weight: 500;
    font: inherit;
    font-weight: 500;
    padding: 0;
    border: 1px solid var(--border-focus, #4a9eff);
    border-radius: 3px;
    outline: none;
    width: 100%;
  }
  .channel-hardware {
    font-size: 0.75rem;
    color: var(--text-muted, #999);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
