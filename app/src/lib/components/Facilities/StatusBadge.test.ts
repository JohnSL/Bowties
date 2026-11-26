import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import StatusBadge from './StatusBadge.svelte';

describe('StatusBadge', () => {
  it('shows its text in the tone it is given, with no dot', () => {
    render(StatusBadge, { tone: 'wired', text: 'Wired' });
    const badge = screen.getByTestId('status-badge');
    expect(badge).toHaveTextContent('Wired');
    expect(badge).toHaveClass('wired');
    expect(badge.children).toHaveLength(0);
  });

  it('defaults to the neutral tone', () => {
    render(StatusBadge, { text: 'Empty' });
    expect(screen.getByTestId('status-badge')).toHaveClass('neutral');
  });
});
