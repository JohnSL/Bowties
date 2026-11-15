<script lang="ts">
  /**
   * FacilityTypeIcon — the small picture beside a facility type in Add
   * facility. Types without a picture render nothing.
   */
  let { templateId }: { templateId: string } = $props();
</script>

{#if templateId === 'block-indicator'}
  <svg class="facility-type-icon" viewBox="0 0 56 42" aria-hidden="true">
    <line x1="3" y1="21" x2="21" y2="21" stroke="#56606d" stroke-width="2" />
    <line x1="35" y1="21" x2="53" y2="21" stroke="#56606d" stroke-width="2" />
    <circle cx="28" cy="21" r="6" fill="#c50f1f" stroke="#20242a" stroke-width="2" />
  </svg>
{:else if templateId === 'automatic-block-signals'}
  <svg class="facility-type-icon" viewBox="0 0 56 42" aria-hidden="true">
    <line x1="3" y1="28" x2="53" y2="28" stroke="#56606d" stroke-width="2" />
    <line x1="15" y1="28" x2="15" y2="13" stroke="#737b86" stroke-width="2" />
    <circle cx="15" cy="9" r="4" fill="#17a65b" stroke="#20242a" stroke-width="2" />
    <line x1="31" y1="28" x2="31" y2="13" stroke="#737b86" stroke-width="2" />
    <circle cx="31" cy="9" r="4" fill="#e0a800" stroke="#20242a" stroke-width="2" />
    <line x1="47" y1="28" x2="47" y2="13" stroke="#737b86" stroke-width="2" />
    <circle cx="47" cy="9" r="4" fill="#c50f1f" stroke="#20242a" stroke-width="2" />
  </svg>
{:else if templateId === 'interlocking-plant'}
  <svg class="facility-type-icon" viewBox="0 0 56 42" aria-hidden="true">
    <line x1="3" y1="30" x2="53" y2="30" stroke="#56606d" stroke-width="2" />
    <line x1="16" y1="30" x2="23" y2="13" stroke="#56606d" stroke-width="2" />
    <line x1="23" y1="13" x2="35" y2="13" stroke="#56606d" stroke-width="2" />
    <line x1="35" y1="13" x2="42" y2="30" stroke="#56606d" stroke-width="2" />
    <circle cx="10" cy="36" r="2.6" fill="#c50f1f" />
    <circle cx="46" cy="36" r="2.6" fill="#c50f1f" />
    <circle cx="23" cy="7" r="2.6" fill="#c50f1f" />
    <circle cx="35" cy="7" r="2.6" fill="#c50f1f" />
  </svg>
{/if}

<style>
  .facility-type-icon { display: block; flex: 0 0 44px; width: 44px; height: 34px; }
</style>
