/**
 * T020: Vitest unit tests for EmptyState.svelte (US2)
 * TDD — written first; must FAIL until EmptyState.svelte exists.
 *
 * Covers (FR-006, FR-012):
 * - Illustration placeholder renders
 * - Message text "No connections yet" is present
 * - "+ New Connection" button renders but is disabled/inert (FR-012)
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import EmptyState from './EmptyState.svelte';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('EmptyState.svelte', () => {
  it('renders illustration placeholder element', () => {
    render(EmptyState);
    // The illustration placeholder must exist in the DOM.
    const illustration = document.querySelector('.illustration-placeholder, [aria-label*="illustration"], [data-testid="illustration"]');
    expect(illustration).toBeTruthy();
  });

  it('displays "No connections yet" message text (FR-006)', () => {
    render(EmptyState);
    // Use heading role to disambiguate from body copy which also contains the phrase.
    expect(screen.getByRole('heading', { name: /no connections yet/i })).toBeInTheDocument();
  });

  it('renders "+ New Connection" button (FR-012)', () => {
    render(EmptyState);
    const btn = screen.getByRole('button', { name: /new connection/i });
    expect(btn).toBeInTheDocument();
  });

  it('+ New Connection button is disabled/inert in this phase (FR-012)', () => {
    render(EmptyState);
    const btn = screen.getByRole('button', { name: /new connection/i });
    // Button must be disabled (aria-disabled=true or disabled attribute).
    const isDisabled = btn.hasAttribute('disabled') || btn.getAttribute('aria-disabled') === 'true';
    expect(isDisabled).toBe(true);
  });
});
