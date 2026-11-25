<script lang="ts">
  /**
   * AspectsStep — page 2 of the Automatic Block Signals editor: how many
   * aspects the signals show and how turnouts restrict them, for all signals
   * and for the selected one, with the selected signal's rules.
   */
  import type { ApproachLighting, AspectCount, SignalPreview, TurnoutNotLined } from '$lib/api/signaling';
  import type { BlockSignalsEditorView } from '$lib/stores/blockSignalsEditor.svelte';
  import type { BlockSignalsEditorActions } from '$lib/orchestration/blockSignalsEditorOrchestrator';
  import TrackDiagram from './TrackDiagram.svelte';
  import HelpTip from './HelpTip.svelte';
  import { trackInputFor } from './trackInput';
  import { occupancyStatus, type BlockOccupancyMap } from './occupancy';
  import {
    INDICATION_LABEL,
    conditionText,
    exitEndText,
    indicationTone,
    nextSignalText,
    rulesNote,
    turnoutName,
  } from './aspectText';

  let {
    view,
    actions,
    selectedSignalId = $bindable(null),
    occupancy,
  }: {
    view: BlockSignalsEditorView;
    actions: BlockSignalsEditorActions;
    selectedSignalId?: string | null;
    occupancy?: BlockOccupancyMap;
  } = $props();

  const ASPECT_CHOICES: Array<[AspectCount, string, string]> = [
    [2, '2 — occupancy only', 'Clear / Stop. The signal looks only at its own block.'],
    [3, '3 — occupancy and next signal', 'Adds Approach when the next signal shows Stop. The usual choice.'],
    [4, '4 — adds Advance Approach', 'Warns two signals before a Stop. Uncommon on layouts.'],
  ];
  const LIGHTING_CHOICES: Array<[ApproachLighting, string]> = [
    ['alwaysLit', 'Always lit'],
    ['whenApproached', 'Lit when a train approaches'],
  ];
  const TURNOUT_CHOICES: Array<[TurnoutNotLined, string]> = [
    ['stop', 'Stop'],
    ['restricting', 'Restricting'],
  ];

  /** This release builds only 3-aspect, always-lit signals that show Stop for an unlined turnout. */
  const IN_SCOPE = { aspects: 3, approachLighting: 'alwaysLit', turnoutNotLined: 'stop' } as const;

  const signals = $derived(view.preview?.signals ?? []);
  const signal = $derived<SignalPreview | undefined>(
    signals.find((candidate) => candidate.signalId === selectedSignalId) ?? signals[0],
  );
  const both = $derived(view.movements === 'bothDirections');
  const blockNames = (ids: string[]) =>
    ids.map((id) => view.blocks.find((block) => block.blockId === id)?.name ?? id).join(', ');
  const lastWithList = $derived(signals.filter((s) => s.run === 'withList').at(-1));
  const firstWithList = $derived(signals.find((s) => s.run === 'withList'));
  const note = $derived(signal ? rulesNote(signal) : null);

  function contextLine(current: SignalPreview): string {
    const turnouts = current.turnoutIds.map((id) => turnoutName(view.blocks, id)).join(', ');
    return [
      `Protects ${blockNames(current.protectedBlockIds)}`,
      turnouts ? `turnouts: ${turnouts}` : '',
      `next signal: ${nextSignalText(current, signals)}`,
    ].filter(Boolean).join(' · ');
  }

  function override(change: Partial<{ aspects: AspectCount; approachLighting: ApproachLighting }>): void {
    if (!signal) return;
    void actions.setSignalOverride(signal.signalId, { ...signal.settings, ...change });
  }
</script>

<div class="step-body">
<div class="step-top">
<div class="travel-head">
  <span class="field-label">
    {#if both}
      Select a signal to configure
    {:else}
      {view.direction === 'westward' ? '← Westward travel' : 'Eastward travel →'} · select a signal to configure
    {/if}
  </span>
  <span class="muted small">{occupancyStatus(occupancy, view.blocks.map((block) => block.blockId))}</span>
</div>

<TrackDiagram
  input={trackInputFor(
    { blocks: view.blocks, direction: view.direction, movements: view.movements, entranceEnd: view.entranceEnd, exitEnd: view.exitEnd, signals, shapes: view.turnoutShapes },
    'signals',
    { selectedSignalId: signal?.signalId ?? null },
  )}
  {occupancy}
  onSelectSignal={(signalId) => (selectedSignalId = signalId)}
/>

{#if signal}
  <label class="form-row">
    <span>Selected signal name</span>
    <input
      class="input"
      aria-label="Selected signal name"
      value={signal.name}
      onchange={(event) => actions.setSignalName(signal.signalId, event.currentTarget.value)}
    />
  </label>
  <p class="muted context">{contextLine(signal)}</p>
{/if}
</div>

{#if signal}
  <div class="columns aspects-columns">
    <section class="panel subtle" aria-label="Settings for all signals and the selected signal">
      <h3>All signals</h3>
      <p class="muted small">Applies to every signal without an override.</p>
      <fieldset class="choices">
        <legend class="field-label">Aspects</legend>
        {#each ASPECT_CHOICES as [value, title, description]}
          <label class="radio-choice" class:selected={view.settings.aspects === value} class:unavailable={value !== IN_SCOPE.aspects}>
            <input type="radio" name="abs-aspects" disabled={value !== IN_SCOPE.aspects} checked={view.settings.aspects === value} onchange={() => actions.setSettings({ aspects: value })} />
            <span><span class="radio-title">{title}</span><span class="radio-desc">{description}</span></span>
          </label>
        {/each}
      </fieldset>

      <fieldset class="choices">
        <legend>
          <span class="legend-row">
            <span class="field-label">When a turnout isn't lined for the main, the signals protecting it show</span>
            <HelpTip label="About Stop and Restricting">
              This changes the aspect the signal shows. Restricting is its own aspect, such as flashing red or red over lunar.
              Railroads differ here. Either way the signal before shows Approach. No diverging indication is shown; a signal
              that shows the route belongs to an Interlocking Plant.
            </HelpTip>
          </span>
        </legend>
        <div class="segmented">
          {#each TURNOUT_CHOICES as [value, label]}
            <label class="segment" class:selected={view.settings.turnoutNotLined === value} class:unavailable={value !== IN_SCOPE.turnoutNotLined}>
              <input type="radio" name="abs-turnout-policy" disabled={value !== IN_SCOPE.turnoutNotLined} checked={view.settings.turnoutNotLined === value} onchange={() => actions.setSettings({ turnoutNotLined: value })} />{label}
            </label>
          {/each}
        </div>
      </fieldset>

      <fieldset class="choices">
        <legend>
          <span class="legend-row">
            <span class="field-label">Approach lighting</span>
            <HelpTip label="About approach lighting">
              Lit when a train approaches keeps the heads dark until a train nears the signal, as many prototype signals do.
              Dark is a display choice, never Stop, Clear, or an unknown occupancy observation.
            </HelpTip>
          </span>
        </legend>
        <div class="segmented">
          {#each LIGHTING_CHOICES as [value, label]}
            <label class="segment" class:selected={view.settings.approachLighting === value} class:unavailable={value !== IN_SCOPE.approachLighting}>
              <input type="radio" name="abs-lighting" disabled={value !== IN_SCOPE.approachLighting} checked={view.settings.approachLighting === value} onchange={() => actions.setSettings({ approachLighting: value })} />{label}
            </label>
          {/each}
        </div>
      </fieldset>

      <div class="overrides">
        <h3>Selected signal only</h3>
        <p class="muted small">
          {signal.overridden ? 'Uses a selected-signal override; changes to all signals will not replace it.' : 'Uses the settings for all signals.'}
        </p>
        <label class="radio-choice" class:selected={signal.overridden}>
          <input
            type="checkbox"
            checked={signal.overridden}
            onchange={(event) => actions.setSignalOverride(
              signal.signalId,
              event.currentTarget.checked ? { aspects: view.settings.aspects, approachLighting: view.settings.approachLighting } : null,
            )}
          />
          <span><span class="radio-title">Override for this signal</span><span class="radio-desc">For example, a 2-aspect last signal before staging.</span></span>
        </label>
        {#if signal.overridden}
          <fieldset class="choices override-choices">
            <legend class="field-label">Aspects</legend>
            {#each ASPECT_CHOICES as [value, title, description]}
              <label class="radio-choice" class:selected={signal.settings.aspects === value} class:unavailable={value !== IN_SCOPE.aspects}>
                <input type="radio" name="abs-signal-aspects" disabled={value !== IN_SCOPE.aspects} checked={signal.settings.aspects === value} onchange={() => override({ aspects: value })} />
                <span><span class="radio-title">{title}</span><span class="radio-desc">{description}</span></span>
              </label>
            {/each}
          </fieldset>
          <fieldset class="choices">
            <legend class="field-label">Approach lighting</legend>
            <div class="segmented">
              {#each LIGHTING_CHOICES as [value, label]}
                <label class="segment" class:selected={signal.settings.approachLighting === value} class:unavailable={value !== IN_SCOPE.approachLighting}>
                  <input type="radio" name="abs-signal-lighting" disabled={value !== IN_SCOPE.approachLighting} checked={signal.settings.approachLighting === value} onchange={() => override({ approachLighting: value })} />{label}
                </label>
              {/each}
            </div>
          </fieldset>
        {/if}
      </div>
    </section>

    <section class="panel" aria-labelledby="abs-rules-heading">
      <h3 id="abs-rules-heading">When this signal shows…</h3>
      <p class="muted">{signal.name}</p>
      <p class="muted small">{signal.settings.aspects} aspects · {signal.overridden ? 'selected-signal override' : 'settings for all signals'}</p>
      <table class="abs-table rules" aria-label={`Rules for ${signal.name}`}>
        <thead><tr><th scope="col">Condition</th><th scope="col">Indication</th></tr></thead>
        <tbody>
          {#each signal.rules as rule}
            <tr>
              <td>{conditionText(rule.condition, view.blocks, signal.protectedBlockIds.length)}</td>
              <td class={`aspect ${indicationTone(rule.indication)}`}>{INDICATION_LABEL[rule.indication]}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if note}
        <p class="muted small">{note}</p>
      {/if}
      <p class="muted small">
        Stop at an automatic signal means Stop and Proceed: stop, then continue at restricted speed. A number plate on the
        mast marks it as automatic.
      </p>
      {#if signal.signalId === lastWithList?.signalId}
        <p class="context">
          Beyond {blockNames([view.blocks[view.blocks.length - 1]?.blockId ?? ''])}: {exitEndText(view.exitEnd)}. Set it from the end of the track on Blocks.
        </p>
      {:else if signal.signalId === firstWithList?.signalId && view.entranceEnd.kind === 'plant'}
        <p class="context">The Plant before {blockNames([view.blocks[0]?.blockId ?? ''])} uses this signal as its next signal.</p>
      {/if}
    </section>
  </div>
{/if}
</div>

<style>
  .travel-head { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .travel-head .field-label { margin: 0; }
  .context { margin: 4px 0 0; font-size: 12px; }
  .hint { margin-top: 6px; }
  .aspects-columns { grid-template-columns: minmax(300px, .9fr) minmax(0, 1.25fr); }
  .panel > p + p { margin-top: 6px; }
  .overrides { margin-top: 18px; padding-top: 12px; border-top: 1px solid var(--abs-border); }
  .override-choices { margin-top: 12px; }
  .rules { margin: 10px 0; }
  .aspect { font-weight: 600; }
  :global(.segment.unavailable) { cursor: default; color: #a0a0a0; background: var(--abs-subtle); }
  .aspect.stop { color: #c50f1f; }
  .aspect.approach { color: #8a5a00; }
  .aspect.clear { color: #107c10; }
</style>
