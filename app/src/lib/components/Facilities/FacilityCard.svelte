<script lang="ts">
  import type { Facility, FacilityStatus } from '$lib/api/facilities';
  import type { BehaviorTemplate } from '$lib/api/behaviorTemplates';
  import FacilitySlot from './FacilitySlot.svelte';

  let {
    facility,
    template,
    onRename,
    onDelete,
  }: {
    facility: Facility;
    template?: BehaviorTemplate;
    onRename?: (facilityId: string, newName: string) => void;
    onDelete?: (facilityId: string) => void;
  } = $props();

  // Spec 018 / S1: derived from slot fullness; never persisted on the entity.
  let status = $derived<FacilityStatus>(
    Object.values(facility.slotBindings).every((v) => v !== null) ? 'Wired' : 'Incomplete',
  );

  let isEditingName = $state(false);
  let nameEditValue = $state('');

  function startRename() {
    nameEditValue = facility.name;
    isEditingName = true;
  }
  function commitRename() {
    isEditingName = false;
    const trimmed = nameEditValue.trim();
    if (trimmed.length === 0) return;
    if (trimmed === facility.name) return;
    onRename?.(facility.facilityId, trimmed);
  }
  function handleNameKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') { isEditingName = false; }
  }
  function focusInput(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
  function handleDelete() {
    onDelete?.(facility.facilityId);
  }

  function slotsOrdered(): Array<[string, string | null]> {
    // Render slots in the template's declared order when known; otherwise
    // fall back to the persisted key order.
    if (template) {
      return template.slots.map((s) => [s.label, facility.slotBindings[s.label] ?? null]);
    }
    return Object.entries(facility.slotBindings);
  }
</script>

<article class="facility-card" data-testid="facility-card" data-facility-id={facility.facilityId}>
  <header class="facility-header">
    {#if isEditingName}
      <input
        class="facility-name-input"
        type="text"
        bind:value={nameEditValue}
        onblur={commitRename}
        onkeydown={handleNameKeydown}
        use:focusInput
        aria-label="Edit facility name"
      />
    {:else}
      <button type="button" class="facility-name" onclick={startRename} title="Click to rename">
        {facility.name}
      </button>
    {/if}
    <span class="status-pill" class:wired={status === 'Wired'} class:incomplete={status === 'Incomplete'}>
      {status}
    </span>
    <span class="template-label">{template?.displayName ?? facility.templateId}</span>
    <div class="actions">
      <button type="button" class="btn-text" onclick={startRename} aria-label="Rename facility">Rename</button>
      <button type="button" class="btn-text danger" onclick={handleDelete} aria-label="Delete facility">Delete</button>
    </div>
  </header>

  <ul class="slot-list">
    {#each slotsOrdered() as [label, binding] (label)}
      <FacilitySlot slotLabel={label} {binding} {template} />
    {/each}
  </ul>
</article>

<style>
  .facility-card {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border: 1px solid var(--border-color, #d4d4d4);
    border-radius: 0.5rem;
    background: var(--surface-color, #fff);
  }
  .facility-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .facility-name,
  .facility-name-input {
    background: transparent;
    border: none;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-primary, #222);
    padding: 0;
    cursor: text;
  }
  .facility-name:hover {
    text-decoration: underline;
  }
  .facility-name-input {
    border-bottom: 1px solid var(--accent-color, #2563eb);
    outline: none;
  }
  .status-pill {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.15rem 0.5rem;
    border-radius: 999px;
    border: 1px solid currentColor;
  }
  .status-pill.incomplete {
    color: var(--warning-color, #b45309);
  }
  .status-pill.wired {
    color: var(--success-color, #15803d);
  }
  .template-label {
    font-size: 0.8rem;
    color: var(--text-muted, #666);
  }
  .actions {
    margin-left: auto;
    display: flex;
    gap: 0.25rem;
  }
  .btn-text {
    background: none;
    border: none;
    font-size: 0.85rem;
    color: var(--accent-color, #2563eb);
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: 0.25rem;
  }
  .btn-text:hover {
    background: var(--bg-subtle, #f5f5f5);
  }
  .btn-text.danger {
    color: var(--error-color, #b00020);
  }
  .slot-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
</style>
