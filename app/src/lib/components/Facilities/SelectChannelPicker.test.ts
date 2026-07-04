/**
 * Spec 018 / S4 component test for `SelectChannelPicker`. Verifies the
 * picker dialog's contract: candidate rendering, search filter, Confirm
 * gating (Confirm enables once any candidate is selected), Cancel via Esc / button.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import SelectChannelPicker from './SelectChannelPicker.svelte';
import type { InformationChannel } from '$lib/api/channels';

function bod(input: number, name = `BOD A${input}`): InformationChannel {
  return {
    id: `ch-${input}`,
    name,
    role: 'block-occupancy',
    style: 'bod-block-detector-input',
    ownership: 'hardware-owned',
    binding: { kind: 'connectorInput', nodeKey: 'N1', connector: 'connector-a', input },
  };
}

const baseChannelState = () => ({ kind: 'unknown' as const });

describe('SelectChannelPicker — select mode', () => {
  let onConfirm: ReturnType<typeof vi.fn<(channelId: string) => void>>;
  let onCancel: ReturnType<typeof vi.fn<() => void>>;

  beforeEach(() => {
    onConfirm = vi.fn<(channelId: string) => void>();
    onCancel = vi.fn<() => void>();
  });

  it('renders all candidate channels', () => {
    render(SelectChannelPicker, {
      props: {
        slotLabel: 'input',
        requiredRole: 'block-occupancy',
        candidateChannels: [bod(1), bod(2), bod(3)],
        channelState: baseChannelState,
        onConfirm,
        onCancel,
      },
    });
    expect(screen.getByText('BOD A1')).toBeInTheDocument();
    expect(screen.getByText('BOD A2')).toBeInTheDocument();
    expect(screen.getByText('BOD A3')).toBeInTheDocument();
  });

  it('renders the slot label in the title', () => {
    render(SelectChannelPicker, {
      props: {
        slotLabel: 'input',
        requiredRole: 'block-occupancy',
        candidateChannels: [bod(1)],
        channelState: baseChannelState,
        onConfirm,
        onCancel,
      },
    });
    expect(screen.getByText(/Select channel for 'input'/)).toBeInTheDocument();
  });

  it('search filter narrows the list by name', async () => {
    render(SelectChannelPicker, {
      props: {
        slotLabel: 'input',
        requiredRole: 'block-occupancy',
        candidateChannels: [bod(1, 'North Yard'), bod(2, 'South Yard'), bod(3, 'Block 7')],
        channelState: baseChannelState,
        onConfirm,
        onCancel,
      },
    });
    const search = screen.getByRole('searchbox', { name: /filter channels/i });
    await fireEvent.input(search, { target: { value: 'yard' } });
    expect(screen.getByText('North Yard')).toBeInTheDocument();
    expect(screen.getByText('South Yard')).toBeInTheDocument();
    expect(screen.queryByText('Block 7')).not.toBeInTheDocument();
  });

  it('Confirm is disabled until a row is selected; click invokes onConfirm', async () => {
    render(SelectChannelPicker, {
      props: {
        slotLabel: 'input',
        requiredRole: 'block-occupancy',
        candidateChannels: [bod(1), bod(2)],
        channelState: baseChannelState,
        onConfirm,
        onCancel,
      },
    });
    const confirmBtn = screen.getByRole('button', { name: /confirm/i });
    expect(confirmBtn).toBeDisabled();

    const row1Radio = within(screen.getByRole('radiogroup')).getAllByRole('radio')[0];
    await fireEvent.change(row1Radio);
    expect(confirmBtn).not.toBeDisabled();

    await fireEvent.click(confirmBtn);
    expect(onConfirm).toHaveBeenCalledWith('ch-1');
  });

  it('Cancel button invokes onCancel', async () => {
    render(SelectChannelPicker, {
      props: {
        slotLabel: 'input',
        requiredRole: 'block-occupancy',
        candidateChannels: [bod(1)],
        channelState: baseChannelState,
        onConfirm,
        onCancel,
      },
    });
    const cancelBtn = screen.getByRole('button', { name: /cancel/i });
    await fireEvent.click(cancelBtn);
    expect(onCancel).toHaveBeenCalled();
  });
});

describe('SelectChannelPicker — Rebind retired (S6 D4)', () => {
  it('renders the Select-channel title only (no Rebind title branch)', () => {
    render(SelectChannelPicker, {
      props: {
        slotLabel: 'input',
        requiredRole: 'block-occupancy',
        candidateChannels: [bod(1), bod(2)],
        channelState: baseChannelState,
        onConfirm: vi.fn<(channelId: string) => void>(),
        onCancel: vi.fn<() => void>(),
      },
    });
    expect(screen.getByText(/Select channel for 'input'/)).toBeInTheDocument();
    expect(screen.queryByText(/Rebind/)).toBeNull();
    // No pre-selection — the radio group starts empty.
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio') as HTMLInputElement[];
    expect(radios.every((r) => !r.checked)).toBe(true);
  });

  it('Confirm enables as soon as any candidate is selected', async () => {
    render(SelectChannelPicker, {
      props: {
        slotLabel: 'input',
        requiredRole: 'block-occupancy',
        candidateChannels: [bod(1), bod(2)],
        channelState: baseChannelState,
        onConfirm: vi.fn<(channelId: string) => void>(),
        onCancel: vi.fn<() => void>(),
      },
    });
    const confirmBtn = screen.getByRole('button', { name: /confirm/i });
    expect(confirmBtn).toBeDisabled(); // nothing selected yet
    const radios = within(screen.getByRole('radiogroup')).getAllByRole('radio');
    await fireEvent.change(radios[0]);
    expect(confirmBtn).not.toBeDisabled();
  });
});
