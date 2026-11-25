import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import AspectsStep from './AspectsStep.svelte';
import { SIGNAL, fakeView, previewWith, spyActions } from './editorTestSupport';
import type { SignalPreview } from '$lib/api/signaling';

const WEST: SignalPreview = {
  ...SIGNAL,
  signalId: 'sig-2',
  name: 'Mill Curve East',
  entranceBlockId: 'block:mill',
  protectedBlockIds: ['block:mill'],
  next: { kind: 'treatedAsStop' },
  rules: [{ condition: { kind: 'blockOccupied' }, indication: 'stop' }],
};

const BLOCKS = [
  { blockId: 'block:cedar', name: 'Cedar Hill', joinedToPrevious: false, turnouts: [] },
  { blockId: 'block:mill', name: 'Mill curve', joinedToPrevious: false, turnouts: [] },
];

function show(extra = {}) {
  const actions = spyActions();
  const view = fakeView({ step: 'aspects', blocks: BLOCKS, preview: previewWith([SIGNAL, WEST]), ...extra });
  render(AspectsStep, { props: { view, actions } });
  return actions;
}

describe('AspectsStep selection and rules', () => {
  it('selects the first signal by default and shows its context and rules', () => {
    show();
    expect(screen.getByRole('textbox', { name: 'Selected signal name' })).toHaveValue('Cedar Hill East');
    expect(screen.getByText('Protects Cedar Hill · next signal: nothing beyond · Stop / Clear only')).toBeInTheDocument();
    const rules = screen.getByRole('table', { name: 'Rules for Cedar Hill East' });
    expect(within(rules).getByText('Protected block occupied')).toBeInTheDocument();
    expect(within(rules).getByText('Protected block clear')).toBeInTheDocument();
    expect(screen.getByText('3 aspects · settings for all signals')).toBeInTheDocument();
  });

  it('selecting another signal in the diagram shows that signal\'s rules and note', async () => {
    show();
    await fireEvent.click(screen.getByRole('button', { name: /Configure Mill Curve East/ }));
    expect(screen.getByRole('textbox', { name: 'Selected signal name' })).toHaveValue('Mill Curve East');
    expect(screen.getByRole('table', { name: 'Rules for Mill Curve East' })).toBeInTheDocument();
    expect(screen.getByText(/treated as Stop, so this signal shows Approach at best/)).toBeInTheDocument();
  });

  it('shows nothing to configure when there are no signals', () => {
    show({ preview: previewWith([]) });
    expect(screen.queryByRole('textbox', { name: 'Selected signal name' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'All signals' })).not.toBeInTheDocument();
  });

  it('emits setSignalName when the selected signal is renamed', async () => {
    const actions = show();
    await fireEvent.change(screen.getByRole('textbox', { name: 'Selected signal name' }), { target: { value: 'Cedar home' } });
    expect(actions.setSignalName).toHaveBeenCalledWith('sig-1', 'Cedar home');
  });
});

describe('AspectsStep settings', () => {
  it('enables only 3 aspects, Stop and Always lit; the other choices are disabled', () => {
    show();
    expect(screen.getByRole('radio', { name: /^2 — occupancy only/ })).toBeDisabled();
    expect(screen.getByRole('radio', { name: /^3 — occupancy and next signal/ })).toBeEnabled();
    expect(screen.getByRole('radio', { name: /^3 — occupancy and next signal/ })).toBeChecked();
    expect(screen.getByRole('radio', { name: /^4 — adds Advance Approach/ })).toBeDisabled();
    expect(screen.getByRole('radio', { name: 'Stop' })).toBeEnabled();
    expect(screen.getByRole('radio', { name: 'Restricting' })).toBeDisabled();
    expect(screen.getByRole('radio', { name: 'Always lit' })).toBeEnabled();
    expect(screen.getByRole('radio', { name: 'Lit when a train approaches' })).toBeDisabled();
  });

  it('emits the settings intent when an in-scope choice is picked', async () => {
    const actions = show({ settings: { aspects: 4, turnoutNotLined: 'restricting', approachLighting: 'whenApproached' } });
    await fireEvent.click(screen.getByRole('radio', { name: /^3 — occupancy and next signal/ }));
    expect(actions.setSettings).toHaveBeenCalledWith({ aspects: 3 });
    await fireEvent.click(screen.getByRole('radio', { name: 'Stop' }));
    expect(actions.setSettings).toHaveBeenCalledWith({ turnoutNotLined: 'stop' });
    await fireEvent.click(screen.getByRole('radio', { name: 'Always lit' }));
    expect(actions.setSettings).toHaveBeenCalledWith({ approachLighting: 'alwaysLit' });
  });

  it('emits setSignalOverride to turn the override on (with the all-signals settings) and off', async () => {
    const actions = show();
    await fireEvent.click(screen.getByRole('checkbox', { name: /Override for this signal/ }));
    expect(actions.setSignalOverride).toHaveBeenCalledWith('sig-1', { aspects: 3, approachLighting: 'alwaysLit' });
  });

  it('turning off an existing override emits null; its own choices are limited to the in-scope ones and emit merged overrides', async () => {
    const overridden = { ...SIGNAL, overridden: true, settings: { aspects: 2 as const, approachLighting: 'whenApproached' as const } };
    const actions = show({ preview: previewWith([overridden]) });
    expect(screen.getByText('2 aspects · selected-signal override')).toBeInTheDocument();

    const aspects = screen.getAllByRole('radio', { name: /^3 — occupancy and next signal/ });
    expect(aspects).toHaveLength(2);
    await fireEvent.click(aspects[1]);
    expect(actions.setSignalOverride).toHaveBeenLastCalledWith('sig-1', { aspects: 3, approachLighting: 'whenApproached' });
    const lit = screen.getAllByRole('radio', { name: 'Always lit' });
    await fireEvent.click(lit[1]);
    expect(actions.setSignalOverride).toHaveBeenLastCalledWith('sig-1', { aspects: 2, approachLighting: 'alwaysLit' });
    expect(screen.getAllByRole('radio', { name: /^2 — occupancy only/ })[1]).toBeDisabled();

    await fireEvent.click(screen.getByRole('checkbox', { name: /Override for this signal/ }));
    expect(actions.setSignalOverride).toHaveBeenLastCalledWith('sig-1', null);
  });
});
