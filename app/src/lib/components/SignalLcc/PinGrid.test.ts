import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import PinGrid from './PinGrid.svelte';

const outputs = [
  { ordinal: 1, label: '#1 H1-R' },
  { ordinal: 2, label: '#2 H1-G' },
  { ordinal: 3, label: '#3 H2-R' },
];

const legend = [
  { state: 'free' as const, text: 'Free' },
  { state: 'sibling' as const, text: 'Another block' },
];

function renderGrid(overrides: Record<string, unknown> = {}) {
  const onPinClick = vi.fn();
  const pinView = vi.fn((ordinal: number) =>
    ordinal === 2
      ? { state: 'sibling' as const, who: 'North', ariaLabel: '#2 H1-G: Used by North', disabled: true }
      : { state: 'free' as const, who: 'Free', ariaLabel: `#${ordinal}: Free` },
  );
  render(PinGrid, { props: { outputs, pinView, onPinClick, legend, ariaLabel: 'Pins on Yard', ...overrides } });
  return { onPinClick, pinView };
}

describe('PinGrid', () => {
  it('lays pins out in one labelled group with a row header per head', () => {
    renderGrid();
    const group = screen.getByRole('group', { name: 'Pins on Yard' });
    expect(group.textContent).toContain('H1');
    expect(group.textContent).toContain('H2');
    expect(screen.getAllByRole('button')).toHaveLength(3);
  });

  it('shows each pin label and who line', () => {
    renderGrid();
    const north = screen.getByRole('button', { name: '#2 H1-G: Used by North' });
    expect(north).toHaveTextContent('#2 H1-G');
    expect(north).toHaveTextContent('North');
  });

  it('disables a pin the view marks disabled and reports clicks on the rest by ordinal', async () => {
    const { onPinClick } = renderGrid();
    expect(screen.getByRole('button', { name: '#2 H1-G: Used by North' })).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: '#3: Free' }));
    expect(onPinClick).toHaveBeenCalledWith(3);
  });

  it('draws a swatch and text for every legend entry', () => {
    renderGrid();
    const key = document.querySelector('.pin-key');
    expect(key?.textContent).toContain('Free');
    expect(key?.textContent).toContain('Another block');
    expect(key?.querySelector('i.free')).not.toBeNull();
    expect(key?.querySelector('i.sibling')).not.toBeNull();
  });

  it('exposes the held-outside mark and owner for focus return', () => {
    renderGrid({
      pinView: () => ({ state: 'outside', who: 'Mast 1', ariaLabel: 'x', mark: true, dataOwner: 'Mast 1', inspected: true }),
    });
    const button = screen.getAllByRole('button')[0];
    expect(button.dataset.owner).toBe('Mast 1');
    expect(button).toHaveTextContent('⚠');
    expect(button.classList.contains('inspected')).toBe(true);
  });
});
