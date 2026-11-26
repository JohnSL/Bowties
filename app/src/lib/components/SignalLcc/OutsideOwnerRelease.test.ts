import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import OutsideOwnerRelease from './OutsideOwnerRelease.svelte';

const base = { owner: 'Mast 3', description: 'Yard throat', pinLabels: ['#4 H2-L', '#5 H3-G'], releaseLabel: 'Release Mast 3' };

describe('OutsideOwnerRelease', () => {
  it('shows the owner, its Lamp Description and extra details', () => {
    render(OutsideOwnerRelease, { props: { ...base, details: ['Configured on Yard outside Bowties.'], onRelease: vi.fn() } });
    const card = screen.getByRole('region', { name: 'Mast 3, not managed by Bowties' });
    expect(card).toHaveTextContent('Description: "Yard throat"');
    expect(card).toHaveTextContent('Configured on Yard outside Bowties.');
  });

  it('asks for confirmation naming the freed pins and resets, and releases only on the second click', async () => {
    const onRelease = vi.fn();
    render(OutsideOwnerRelease, { props: { ...base, resets: ['Mast 3', 'Logic 2'], onRelease } });
    await fireEvent.click(screen.getByRole('button', { name: 'Release and use…' }));
    expect(onRelease).not.toHaveBeenCalled();
    expect(screen.getByRole('region')).toHaveTextContent(
      'Release Mast 3? All 2 of its pins (#4 H2-L, #5 H3-G) become free, and Mast 3, Logic 2 are reset when you save. Discard undoes it.',
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Release Mast 3' }));
    expect(onRelease).toHaveBeenCalledTimes(1);
  });

  it('cancel returns to the details and focuses the release button', async () => {
    render(OutsideOwnerRelease, { props: { ...base, onRelease: vi.fn() } });
    await fireEvent.click(screen.getByRole('button', { name: 'Release and use…' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(await screen.findByRole('button', { name: 'Release and use…' })).toHaveFocus();
  });

  it('offers close and Show in Configuration only when given', async () => {
    const onClose = vi.fn();
    const onShow = vi.fn();
    const { unmount } = render(OutsideOwnerRelease, { props: { ...base, onRelease: vi.fn() } });
    expect(screen.queryByRole('button', { name: /Close Mast 3 details/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Show Mast 3 in Configuration/ })).not.toBeInTheDocument();
    unmount();
    render(OutsideOwnerRelease, { props: { ...base, onRelease: vi.fn(), onClose, onShowInConfiguration: onShow } });
    await fireEvent.click(screen.getByRole('button', { name: 'Show Mast 3 in Configuration' }));
    expect(onShow).toHaveBeenCalled();
    await fireEvent.keyDown(screen.getByRole('region'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
