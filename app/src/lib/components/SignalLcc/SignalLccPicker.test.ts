import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import type { DiscoveredNode } from '$lib/api/tauri';
import { nodeInfoStore } from '$lib/stores/nodeInfo';
import SignalLccPicker from './SignalLccPicker.svelte';

const NODE_A = '020157100997';
const NODE_B = '020157100A01';

const snipNode = (userName: string): DiscoveredNode =>
  ({ snip_data: { user_name: userName, model: 'Signal-LCC', manufacturer: 'RR-CirKits, Inc.' } }) as unknown as DiscoveredNode;

beforeEach(() => {
  nodeInfoStore.set(new Map([[NODE_A, snipNode('Yard Signals')], [NODE_B, snipNode('Depot Signals')]]));
});
afterEach(() => nodeInfoStore.set(new Map()));

describe('SignalLccPicker', () => {
  it('shows the selected node as display name over model and address on the labelled field', () => {
    render(SignalLccPicker, { props: { nodeKeys: [NODE_A, NODE_B], selected: NODE_A, label: 'Board', onChoose: vi.fn() } });
    const field = screen.getByRole('button', { name: /Board/ });
    expect(field.textContent?.replace(/\s+/g, ' ')).toContain('Yard Signals Signal-LCC · 02.01.57.10.09.97');
  });

  it('invites a choice when nothing is selected', () => {
    render(SignalLccPicker, { props: { nodeKeys: [NODE_A], selected: null, label: 'Board', onChoose: vi.fn() } });
    expect(screen.getByRole('button', { name: /Board/ })).toHaveTextContent('Choose a Signal LCC');
  });

  it('opens a searchable dialog sorted by name and reports the confirmed choice', async () => {
    const onChoose = vi.fn();
    render(SignalLccPicker, { props: { nodeKeys: [NODE_A, NODE_B], selected: null, label: 'Board', onChoose } });
    await fireEvent.click(screen.getByRole('button', { name: /Board/ }));
    const dialog = screen.getByRole('dialog', { name: 'Choose a Signal LCC' });
    expect(within(dialog).getAllByRole('radio').map((r) => r.getAttribute('value'))).toEqual([NODE_B, NODE_A]);
    await fireEvent.click(within(dialog).getByRole('radio', { name: /Yard Signals/ }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Confirm' }));
    expect(onChoose).toHaveBeenCalledWith(NODE_A);
    expect(screen.queryByRole('dialog', { name: 'Choose a Signal LCC' })).not.toBeInTheDocument();
  });

  it('does not open when disabled', async () => {
    render(SignalLccPicker, { props: { nodeKeys: [NODE_A], selected: null, label: 'Board', onChoose: vi.fn(), disabled: true } });
    expect(screen.getByRole('button', { name: /Board/ })).toBeDisabled();
  });
});
