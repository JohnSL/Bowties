import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import BlockSignalsEditor from './BlockSignalsEditor.svelte';
import { fakeView, spyActions } from './editorTestSupport';
import type { BlockSignalsStep } from '$lib/stores/blockSignalsEditor.svelte';

function open(step: BlockSignalsStep, extra = {}) {
  const actions = spyActions();
  render(BlockSignalsEditor, { props: { view: fakeView({ step, ...extra }), actions, detectedBlocks: [] } });
  return { actions, stepper: screen.getByRole('list', { name: 'Automatic Block Signals steps' }) };
}

describe('BlockSignalsEditor shell', () => {
  it('lists Blocks, Aspects, Appearance, Wiring and Review with only Review disabled, and no "later" text', () => {
    const { stepper } = open('aspects');
    const buttons = within(stepper).getAllByRole('button');
    expect(buttons.map((b) => b.textContent?.replace(/^\d/, '').trim())).toEqual(['Blocks', 'Aspects', 'Appearance', 'Wiring', 'Review']);
    expect(buttons.map((b) => (b as HTMLButtonElement).disabled)).toEqual([false, false, false, false, true]);
    expect(document.body.textContent).not.toMatch(/later release/i);
  });

  it('offers Back and Next: Appearance on Aspects, and navigates', async () => {
    const { actions } = open('aspects');
    await fireEvent.click(screen.getByRole('button', { name: 'Next: Appearance' }));
    expect(actions.navigate).toHaveBeenCalledWith('appearance');
    await fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(actions.navigate).toHaveBeenCalledWith('blocks');
  });

  it('offers Next: Wiring on Appearance and Done on Wiring, enabled only when the store allows finishing', async () => {
    const appearance = open('appearance');
    await fireEvent.click(screen.getByRole('button', { name: 'Next: Wiring' }));
    expect(appearance.actions.navigate).toHaveBeenCalledWith('wiring');
  });

  it('disables Done on Wiring until canFinish, then emits finish', async () => {
    const { actions } = open('wiring');
    expect(screen.getByRole('button', { name: 'Done' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Back' })).toBeEnabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(actions.navigate).toHaveBeenCalledWith('appearance');
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(actions.cancel).toHaveBeenCalled();
  });

  it('enables Done when canFinish', async () => {
    const { actions } = open('wiring', { canFinish: true });
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(actions.finish).toHaveBeenCalled();
  });
});

describe('scope gating on Blocks and Aspects', () => {
  it('shows APB disabled without any "later release" text', () => {
    open('blocks', { movements: 'bothDirections' });
    expect(screen.getByRole('radio', { name: /Coordinate traffic through the single-track section/ })).toBeDisabled();
    expect(document.body.textContent).not.toMatch(/later release/i);
  });

  it('shows Both directions disabled with no "later" text while One direction stays selectable', () => {
    open('blocks');
    expect(screen.getByRole('radio', { name: /Both directions/ })).toBeDisabled();
    expect(screen.getByRole('radio', { name: /One direction/ })).toBeEnabled();
    expect(document.body.textContent).not.toMatch(/later release/i);
  });

  it('disables Aspects choices this release cannot build, with no "later" text', () => {
    open('aspects');
    expect(screen.getAllByRole('radio', { name: /^3 — occupancy and next signal/ })[0]).toBeEnabled();
    expect(screen.getAllByRole('radio', { name: /^2 — occupancy only/ })[0]).toBeDisabled();
    expect(screen.getAllByRole('radio', { name: /^4 — adds Advance Approach/ })[0]).toBeDisabled();
    expect(screen.getByRole('radio', { name: 'Restricting' })).toBeDisabled();
    expect(screen.getByRole('radio', { name: 'Stop' })).toBeEnabled();
    expect(screen.getByRole('radio', { name: 'Lit when a train approaches' })).toBeDisabled();
    expect(screen.getByRole('radio', { name: 'Always lit' })).toBeEnabled();
    expect(document.body.textContent).not.toMatch(/later release/i);
  });
});
