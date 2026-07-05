<script lang="ts">
  import type { NodeDisplayParts } from '$lib/utils/nodeDisplayName';

  /**
   * NodeLabel — shared component for rendering node identity.
   *
   * Displays the node's primary name and optional model · manufacturer
   * context using one of two orientations:
   *   - **inline** (default): single line, name bold, product muted beside it.
   *   - **compact**: two stacked lines (name above, product below).
   *
   * Always shows model · manufacturer when available, regardless of whether
   * the name happens to equal the model (no suppression rule).
   */
  let {
    parts,
    orientation = 'inline',
  }: {
    parts: NodeDisplayParts;
    orientation?: 'inline' | 'compact';
  } = $props();

  const productText = $derived.by(() => {
    // When the name IS the model (fallback, not user-assigned), showing
    // "Model · Manufacturer" would repeat the model. Show only manufacturer.
    if (!parts.isUserNamed && parts.name === parts.model) {
      return parts.manufacturer;
    }
    if (parts.model && parts.manufacturer) return `${parts.model} · ${parts.manufacturer}`;
    if (parts.model) return parts.model;
    if (parts.manufacturer) return parts.manufacturer;
    return null;
  });

  /** Hide product line when there's nothing useful to add beyond the name. */
  const showProduct = $derived(productText !== null);
</script>

<span class="node-label" class:compact={orientation === 'compact'}>
  <span class="nl-name">{parts.name}</span>
  {#if showProduct}
    <span class="nl-product">{productText}</span>
  {/if}
</span>

<style>
  .node-label {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }
  .nl-name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .nl-product {
    font-size: 0.85em;
    color: var(--fluent-neutralForeground3, #616161);
    font-weight: 400;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .compact {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
  }
</style>
