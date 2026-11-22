import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import BlockSignalsEditor from './BlockSignalsEditor.svelte';
import AppearanceStep from './AppearanceStep.svelte';
import { fakeView, spyActions } from './editorTestSupport';

describe('AppearanceStep', () => {
  it('shows this release\'s fixed appearance with other choices disabled and no "later" text', () => {
    render(AppearanceStep, { props: { view: fakeView({ step: 'appearance' }) } });

    const settings = screen.getByRole('region', { name: 'Signal type' });
    expect(within(settings).getByRole('radio', { name: /Searchlight/ })).toBeChecked();
    expect(within(settings).getByRole('radio', { name: /Searchlight/ })).toBeDisabled();
    expect(within(settings).getByRole('radio', { name: /Color light/ })).toBeDisabled();
    expect(within(settings).getByRole('radio', { name: 'Always lit' })).toBeChecked();
    expect(within(settings).getByRole('radio', { name: 'Lit when a train approaches' })).toBeDisabled();
    expect(within(settings).getByRole('checkbox', { name: /Override for this signal/ })).toBeDisabled();
    expect(screen.queryByText(/try the signals/i)).not.toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/later release/i);
  });

  it('tabulates the lamps lit for each indication of the selected signal', () => {
    render(AppearanceStep, { props: { view: fakeView({ step: 'appearance' }) } });

    const panel = screen.getByRole('region', { name: 'Appearance for each indication' });
    expect(within(panel).getByRole('heading', { name: /Cedar Hill East/ })).toBeInTheDocument();
    const rows = within(within(panel).getByRole('table')).getAllByRole('row').slice(1).map((row) => row.textContent);
    expect(rows).toEqual(['StopRed', 'ApproachRed and green', 'ClearGreen']);
  });

  it('is reached with Next: Appearance and leaves with Back or Next: Wiring', async () => {
    const actions = spyActions();
    render(BlockSignalsEditor, { props: { view: fakeView({ step: 'appearance' }), actions, detectedBlocks: [] } });
    expect(screen.getByRole('region', { name: 'Appearance for each indication' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(actions.navigate).toHaveBeenCalledWith('aspects');
    await fireEvent.click(screen.getByRole('button', { name: 'Next: Wiring' }));
    expect(actions.navigate).toHaveBeenCalledWith('wiring');
  });
});
