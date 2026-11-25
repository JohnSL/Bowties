import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';

vi.mock('$lib/api/layout', () => ({
  listBundledProfiles: vi.fn(),
}));
vi.mock('$lib/orchestration/placeholderBoardOrchestrator', () => ({
  addPlaceholderBoard: vi.fn(),
}));

import { listBundledProfiles } from '$lib/api/layout';
import { addPlaceholderBoard } from '$lib/orchestration/placeholderBoardOrchestrator';
import AddBoardDialog from './AddBoardDialog.svelte';

const PROFILES = [
  { stem: 'Mustangpeak-Engineering_TurnoutBoss', manufacturer: 'Mustangpeak Engineering', model: 'TurnoutBoss' },
  { stem: 'RR-CirKits_Inc._Signal-LCC', manufacturer: 'RR-CirKits, Inc.', model: 'Signal-LCC' },
];

describe('AddBoardDialog', () => {
  beforeEach(() => {
    vi.mocked(listBundledProfiles).mockReset().mockResolvedValue(PROFILES);
    vi.mocked(addPlaceholderBoard).mockReset().mockResolvedValue({ nodeKey: 'placeholder:abc' });
  });

  it('lists board models as a selectable radio list with model and manufacturer', async () => {
    render(AddBoardDialog, { props: { onCancel: vi.fn(), onAdded: vi.fn() } });

    const group = await screen.findByRole('radiogroup', { name: /board model/i });
    expect(within(group).getAllByRole('radio')).toHaveLength(2);
    expect(within(group).getByText('Signal-LCC')).toBeInTheDocument();
    expect(within(group).getByText('RR-CirKits, Inc.')).toBeInTheDocument();
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
  });

  it('preselects the first board model', async () => {
    render(AddBoardDialog, { props: { onCancel: vi.fn(), onAdded: vi.fn() } });

    const group = await screen.findByRole('radiogroup', { name: /board model/i });
    const radios = within(group).getAllByRole('radio') as HTMLInputElement[];
    expect(radios[0].checked).toBe(true);
    expect(radios[1].checked).toBe(false);
  });

  it('adds the selected board model and reports the new node key', async () => {
    const onAdded = vi.fn();
    render(AddBoardDialog, { props: { onCancel: vi.fn(), onAdded } });

    const group = await screen.findByRole('radiogroup', { name: /board model/i });
    await fireEvent.click(within(group).getAllByRole('radio')[1]);
    await fireEvent.click(screen.getByRole('button', { name: /add board/i }));

    await vi.waitFor(() => expect(onAdded).toHaveBeenCalledWith('placeholder:abc'));
    expect(addPlaceholderBoard).toHaveBeenCalledWith({ profileStem: 'RR-CirKits_Inc._Signal-LCC' });
  });
});
