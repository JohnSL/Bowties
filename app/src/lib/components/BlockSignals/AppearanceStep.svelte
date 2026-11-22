<script lang="ts">
  /**
   * AppearanceStep — page 3 of the Automatic Block Signals editor: what the
   * signals look like. This release builds one red/green searchlight head per
   * signal, always lit, so the settings are shown with their fixed values and
   * the other choices disabled, beside the lamps each indication lights.
   */
  import type { BlockSignalsEditorView } from '$lib/stores/blockSignalsEditor.svelte';
  import { INDICATION_LABEL, indicationTone } from './aspectText';

  let { view }: { view: BlockSignalsEditorView } = $props();

  const SIGNAL_TYPES: Array<[string, string, boolean]> = [
    ['Searchlight (red and green)', 'One head with a single lens that shows red, green, or both together.', true],
    ['Color light', 'Separate lamps for each color.', false],
    ['Position light', 'Rows of lights whose angle shows the indication.', false],
  ];
  const LIGHTING: Array<[string, boolean]> = [
    ['Always lit', true],
    ['Lit when a train approaches', false],
  ];
  const INDICATION_LAMPS = [
    { indication: 'stop', lamps: 'Red' },
    { indication: 'approach', lamps: 'Red and green' },
    { indication: 'clear', lamps: 'Green' },
  ] as const;

  const signal = $derived(view.preview?.signals[0]);
</script>

<div class="step-body">
  <div class="columns appearance-columns">
    <section class="panel subtle" aria-label="Signal type">
      <h3>Settings for all signals</h3>
      <fieldset class="choices">
        <legend class="field-label">Signal type</legend>
        {#each SIGNAL_TYPES as [title, description, available]}
          <label class="radio-choice" class:selected={available} class:unavailable={!available}>
            <input type="radio" name="abs-signal-type" checked={available} disabled />
            <span><span class="radio-title">{title}</span><span class="radio-desc">{description}</span></span>
          </label>
        {/each}
      </fieldset>

      <fieldset class="choices">
        <legend class="field-label">Approach lighting</legend>
        <div class="segmented">
          {#each LIGHTING as [label, available]}
            <label class="segment" class:selected={available} class:unavailable={!available}>
              <input type="radio" name="abs-appearance-lighting" checked={available} disabled />{label}
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="overrides">
        <h3>Selected signal only{signal ? ` · ${signal.name}` : ''}</h3>
        <label class="radio-choice unavailable">
          <input type="checkbox" disabled />
          <span><span class="radio-title">Override for this signal</span><span class="radio-desc">For example, a leftover color light among searchlights.</span></span>
        </label>
      </div>
    </section>

    <section class="panel" aria-label="Appearance for each indication">
      <h3>Appearance for each indication{signal ? ` · ${signal.name}` : ''}</h3>
      <p class="muted small">One searchlight head, always lit.</p>
      <table class="abs-table">
        <thead><tr><th scope="col">Indication</th><th scope="col">Mast</th></tr></thead>
        <tbody>
          {#each INDICATION_LAMPS as row}
            <tr>
              <td class={`aspect ${indicationTone(row.indication)}`}>{INDICATION_LABEL[row.indication]}</td>
              <td>{row.lamps}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  </div>
</div>

<style>
  .appearance-columns { grid-template-columns: minmax(300px, .9fr) minmax(0, 1.25fr); margin-top: 0; }
  .overrides { margin-top: 18px; padding-top: 12px; border-top: 1px solid var(--abs-border); }
  .panel > p + table { margin-top: 10px; }
  .aspect { font-weight: 600; }
  .aspect.stop { color: #c50f1f; }
  .aspect.approach { color: #8a5a00; }
  .aspect.clear { color: #107c10; }
</style>
