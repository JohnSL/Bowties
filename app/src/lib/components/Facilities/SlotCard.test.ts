import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import SlotCard from './SlotCard.svelte';

describe('SlotCard configuration navigation', () => {
  it('renders and emits a single target', async () => {
    const onClick = vi.fn();
    const target = { label: 'Line 2', nodeId: 'N1', elementPath: ['seg:0', 'elem:0#2'] };
    render(SlotCard, { label: 'Block', configTargets: [target], onConfigTargetClick: onClick });

    await fireEvent.click(screen.getByTestId('slot-config-nav'));

    expect(onClick).toHaveBeenCalledWith(target);
  });

  it('offers each target when a channel spans multiple configuration sections', async () => {
    const onClick = vi.fn();
    const targets = [
      { label: 'Lamp 1', nodeId: 'N1', elementPath: ['seg:0', 'elem:0#1'] },
      { label: 'Lamp 2', nodeId: 'N1', elementPath: ['seg:0', 'elem:0#2'] },
    ];
    render(SlotCard, { label: 'Signal', configTargets: targets, onConfigTargetClick: onClick });

    await fireEvent.click(screen.getByTestId('slot-config-nav'));
    const items = screen.getByTestId('config-nav-popover').querySelectorAll('button');
    expect(items).toHaveLength(2);

    await fireEvent.click(items[1]);
    expect(onClick).toHaveBeenCalledWith(targets[1]);
    expect(screen.queryByTestId('config-nav-popover')).not.toBeInTheDocument();
  });

  it('keeps metadata plain when no configuration target is available', () => {
    render(SlotCard, { label: 'Block', meta: 'Connector A · Input 6' });

    expect(screen.queryByTestId('slot-config-nav')).not.toBeInTheDocument();
    expect(screen.getByText('Connector A · Input 6')).toHaveClass('slot-card-meta');
  });
});
