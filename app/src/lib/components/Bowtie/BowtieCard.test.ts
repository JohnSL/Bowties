/**
 * T015: Vitest unit tests for BowtieCard.svelte
 * TDD — written first; must FAIL until BowtieCard.svelte exists.
 *
 * Covers (FR-014, FR-004, FR-002):
 * - Renders card header: shows name if present, event_id_hex otherwise (FR-014)
 * - Renders producer column entries (FR-004)
 * - Renders consumer column entries (FR-004)
 * - Renders ambiguous_entries section when non-empty
 * - Hides ambiguous_entries section when ambiguous_entries is empty
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import BowtieCard from './BowtieCard.svelte';
import type { BowtieCard as BowtieCardType } from '$lib/api/tauri';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

function makeEntry(overrides: { node_name?: string; element_label?: string } = {}) {
  return {
    node_id: '02.01.57.00.00.01',
    node_name: overrides.node_name ?? 'Signal Node',
    element_path: ['seg:0', 'elem:0', 'elem:0'],
    element_label: overrides.element_label ?? 'Output Active',
    event_id: [5, 2, 1, 0, 0, 1, 0, 0],
    role: 'Producer' as const,
  };
}

function makeCard(overrides: Partial<BowtieCardType> = {}): BowtieCardType {
  return {
    event_id_hex: '05.02.01.00.00.01.00.00',
    event_id_bytes: [5, 2, 1, 0, 0, 1, 0, 0],
    producers: [makeEntry({ node_name: 'Button Node', element_label: 'Button Pressed' })],
    consumers: [{ ...makeEntry({ node_name: 'Signal Node', element_label: 'Go to Green' }), role: 'Consumer' as const }],
    ambiguous_entries: [],
    name: null,
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('BowtieCard.svelte', () => {
  // FR-014: Card header shows name if present, event_id_hex if not.

  it('shows event_id_hex as header when name is null (FR-014)', () => {
    render(BowtieCard, { props: { card: makeCard({ name: null }) } });
    // event_id_hex appears in both the header and ConnectorArrow; check at least one exists.
    expect(screen.getAllByText('05.02.01.00.00.01.00.00').length).toBeGreaterThanOrEqual(1);
  });

  it('shows the assigned name in header when name is set (FR-014)', () => {
    render(BowtieCard, {
      props: { card: makeCard({ name: 'Yard Button → Signal' }) },
    });
    expect(screen.getByText('Yard Button → Signal')).toBeInTheDocument();
  });

  // FR-004: Renders producer and consumer columns.

  it('renders producer entry node_name', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Button Node')).toBeInTheDocument();
  });

  it('renders producer entry element_label', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Button Pressed')).toBeInTheDocument();
  });

  it('renders consumer entry node_name', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Signal Node')).toBeInTheDocument();
  });

  it('renders consumer entry element_label', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    expect(screen.getByText('Go to Green')).toBeInTheDocument();
  });

  // Ambiguous entries section.

  it('shows ambiguous section when ambiguous_entries is non-empty', () => {
    const card = makeCard({
      ambiguous_entries: [
        { ...makeEntry({ node_name: 'Unknown Node', element_label: 'Mystery Slot' }), role: 'Ambiguous' as const },
      ],
    });
    render(BowtieCard, { props: { card } });
    // Use the unique element_label from the ambiguous entry to confirm the section rendered.
    expect(screen.getByText('Mystery Slot')).toBeInTheDocument();
  });

  it('hides ambiguous section when ambiguous_entries is empty', () => {
    render(BowtieCard, { props: { card: makeCard({ ambiguous_entries: [] }) } });
    expect(screen.queryByText(/needs clarification/i)).not.toBeInTheDocument();
  });

  // FR-004: Three-column layout markers.

  it('renders a connector arrow between columns', () => {
    render(BowtieCard, { props: { card: makeCard() } });
    // ConnectorArrow renders an arrow element containing '→'
    expect(screen.getByText('→')).toBeInTheDocument();
  });
});
