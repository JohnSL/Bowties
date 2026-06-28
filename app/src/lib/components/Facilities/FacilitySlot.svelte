<script lang="ts">
  import type { BehaviorTemplate, SlotDefinition } from '$lib/api/behaviorTemplates';

  let {
    slotLabel,
    binding,
    template,
  }: {
    slotLabel: string;
    binding: string | null;
    template?: BehaviorTemplate;
  } = $props();

  function definition(): SlotDefinition | undefined {
    return template?.slots.find((s) => s.label === slotLabel);
  }

  function displayLabel(): string {
    const def = definition();
    // Title-case the slot label as a fallback (input → Input, output → Output)
    return def
      ? def.label.charAt(0).toUpperCase() + def.label.slice(1)
      : slotLabel.charAt(0).toUpperCase() + slotLabel.slice(1);
  }

  function requiredRoleHint(): string {
    const def = definition();
    if (!def) return '';
    return `Requires a ${def.requiredRole} channel.`;
  }
</script>

<li class="slot" data-testid="facility-slot">
  <span class="slot-label">{displayLabel()}</span>
  {#if binding === null}
    <span class="slot-empty" title={requiredRoleHint()}>empty</span>
    <span class="slot-hint">{requiredRoleHint()}</span>
  {:else}
    <span class="slot-bound" title="Bound to channel {binding}">channel {binding.slice(0, 8)}…</span>
  {/if}
</li>

<style>
  .slot {
    display: grid;
    grid-template-columns: 6rem auto 1fr;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.5rem;
    border: 1px dashed var(--border-subtle, #e2e2e2);
    border-radius: 0.25rem;
  }
  .slot-label {
    font-weight: 500;
    font-size: 0.9rem;
    color: var(--text-primary, #222);
  }
  .slot-empty {
    font-style: italic;
    color: var(--text-muted, #999);
    font-size: 0.85rem;
  }
  .slot-bound {
    color: var(--text-primary, #222);
    font-size: 0.85rem;
    font-family: var(--font-mono, ui-monospace, monospace);
  }
  .slot-hint {
    color: var(--text-muted, #888);
    font-size: 0.8rem;
  }
</style>
