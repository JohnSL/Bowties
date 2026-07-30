/**
 * Tests for SlotCard.svelte — channel-to-config navigation.
 *
 * When `configTargets` is supplied, the card renders a blue link showing the
 * target label (1:1) or a "N config sections ▾" trigger with popover (1:N).
 * Without it, no config-nav element appears (pre-existing behavior).
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import { fireEvent } from '@testing-library/dom';
import SlotCard from './SlotCard.svelte';

describe('SlotCard — channel-to-config navigation', () => {
  it('renders a single config target as a blue link with the target label', async () => {
    const onClick = vi.fn();
    render(SlotCard, {
      label: 'Block',
      meta: 'Connector A · Input 6',
      configTargets: [{ label: 'Line 2', nodeId: 'N1', elementPath: ['seg:0', 'elem:0#2'] }],
      onConfigTargetClick: onClick,
    });

    const link = screen.getByTestId('slot-config-nav');
    expect(link.tagName).toBe('BUTTON');
    expect(link).toHaveTextContent('Line 2');

    await fireEvent.click(link);

    expect(onClick).toHaveBeenCalledWith({ label: 'Line 2', nodeId: 'N1', elementPath: ['seg:0', 'elem:0#2'] });
  });

  it('renders a multi-target trigger that opens a popover on click', async () => {
    const onClick = vi.fn();
    const targets = [
      { label: 'Lamp #1', nodeId: 'N1', elementPath: ['seg:0', 'elem:0#1'] },
      { label: 'Lamp #2', nodeId: 'N1', elementPath: ['seg:0', 'elem:0#2'] },
    ];
    render(SlotCard, {
      label: 'Signal',
      meta: 'Direct Lamp Control · Rows 1–2',
      configTargets: targets,
      onConfigTargetClick: onClick,
    });

    const trigger = screen.getByTestId('slot-config-nav');
    expect(trigger).toHaveTextContent('2 config sections ▾');

    expect(screen.queryByTestId('config-nav-popover')).not.toHaveAttribute('data-open');

    await fireEvent.click(trigger);

    expect(screen.getByTestId('config-nav-popover')).toHaveAttribute('data-open', '');
    const items = screen.getByTestId('config-nav-popover').querySelectorAll('button');
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent('Lamp #1');
    expect(items[1]).toHaveTextContent('Lamp #2');

    await fireEvent.click(items[1]);

    expect(onClick).toHaveBeenCalledWith(targets[1]);
  });

  it('renders meta as a plain span when configTargets is not provided', () => {
    render(SlotCard, {
      label: 'Block',
      meta: 'Connector A · Input 6',
    });

    expect(screen.queryByTestId('slot-config-nav')).not.toBeInTheDocument();
    const meta = screen.getByText('Connector A · Input 6');
    expect(meta.tagName).toBe('SPAN');
    expect(meta).toHaveClass('slot-card-meta');
  });
});
