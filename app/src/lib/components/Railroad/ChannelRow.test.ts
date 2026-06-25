import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import ChannelRow from './ChannelRow.svelte';
import type { InformationChannel } from '$lib/api/channels';

function makeChannel(overrides: Partial<InformationChannel> = {}): InformationChannel {
  return {
    id: 'ch-1',
    name: 'West Yard — Input 1',
    channelType: 'block-occupancy',
    hardwareRef: {
      nodeKey: '05010101FF000001',
      connector: 'connector-a',
      input: 1,
    },
    ...overrides,
  };
}

describe('ChannelRow', () => {
  const stubNodeName = (key: string) => `Node(${key})`;

  it('renders the channel name as a clickable button', () => {
    render(ChannelRow, { props: { channel: makeChannel(), nodeName: stubNodeName } });
    const btn = screen.getByTitle('Click to rename');
    expect(btn).toHaveTextContent('West Yard — Input 1');
  });

  it('enters edit mode on click and shows input', async () => {
    render(ChannelRow, { props: { channel: makeChannel(), nodeName: stubNodeName } });
    await fireEvent.click(screen.getByTitle('Click to rename'));
    expect(screen.getByLabelText('Edit channel name')).toBeInTheDocument();
  });

  it('commits rename on Enter and calls onRename', async () => {
    const onRename = vi.fn();
    render(ChannelRow, { props: { channel: makeChannel(), nodeName: stubNodeName, onRename } });
    await fireEvent.click(screen.getByTitle('Click to rename'));

    const input = screen.getByLabelText('Edit channel name');
    await fireEvent.input(input, { target: { value: 'New Name' } });
    await fireEvent.keyDown(input, { key: 'Enter' });

    expect(onRename).toHaveBeenCalledWith('ch-1', 'New Name');
  });

  it('cancels rename on Escape without calling onRename', async () => {
    const onRename = vi.fn();
    render(ChannelRow, { props: { channel: makeChannel(), nodeName: stubNodeName, onRename } });
    await fireEvent.click(screen.getByTitle('Click to rename'));

    const input = screen.getByLabelText('Edit channel name');
    await fireEvent.input(input, { target: { value: 'Discarded' } });
    await fireEvent.keyDown(input, { key: 'Escape' });

    expect(onRename).not.toHaveBeenCalled();
    // Should return to display mode
    expect(screen.getByTitle('Click to rename')).toBeInTheDocument();
  });

  it('does not call onRename when name is empty', async () => {
    const onRename = vi.fn();
    render(ChannelRow, { props: { channel: makeChannel(), nodeName: stubNodeName, onRename } });
    await fireEvent.click(screen.getByTitle('Click to rename'));

    const input = screen.getByLabelText('Edit channel name');
    await fireEvent.input(input, { target: { value: '   ' } });
    await fireEvent.keyDown(input, { key: 'Enter' });

    expect(onRename).not.toHaveBeenCalled();
  });
});
