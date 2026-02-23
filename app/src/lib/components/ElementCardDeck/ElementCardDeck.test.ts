/**
 * T011: Vitest unit tests for ElementCardDeck.svelte
 * TDD — written before implementation; must FAIL until ElementCardDeck.svelte exists.
 *
 * Covers:
 * - One card per top-level CDI group (FR-006)
 * - All cards collapsed on segment load (FR-008)
 * - isLoading deck-level spinner
 * - error deck-level error state
 * - Segment change replaces all cards
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import ElementCardDeck from './ElementCardDeck.svelte';
import { configSidebarStore } from '$lib/stores/configSidebar';
import type { CardData } from '$lib/stores/configSidebar';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

function makeCard(id: string, title: string): CardData {
  return {
    cardId: id,
    groupPath: ['seg:0', `elem:${id}`],
    cdGroupName: title,
    isReplicated: false,
    instanceIndex: null,
    cardTitle: title,
    elements: null,
    isLoading: false,
    loadError: null,
  };
}

beforeEach(() => {
  configSidebarStore.reset();
  vi.clearAllMocks();
});

describe('ElementCardDeck.svelte', () => {
  it('renders one card per entry in the card deck (FR-006)', () => {
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg:0', 'Port I/O');
    configSidebarStore.setCards('02.01.57.00.00.01', 'seg:0', [
      makeCard('0', 'Line 1'),
      makeCard('1', 'Line 2'),
      makeCard('2', 'Line 3'),
    ]);
    render(ElementCardDeck, { props: { nodeId: '02.01.57.00.00.01' } });

    expect(screen.getByText('Line 1')).toBeInTheDocument();
    expect(screen.getByText('Line 2')).toBeInTheDocument();
    expect(screen.getByText('Line 3')).toBeInTheDocument();
  });

  it('shows loading spinner when cardDeck.isLoading is true', () => {
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg:0', 'Port I/O');
    // isLoading is true immediately after selectSegment
    render(ElementCardDeck, { props: { nodeId: '02.01.57.00.00.01' } });
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('shows error message when cardDeck.error is set', () => {
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg:0', 'Port I/O');
    configSidebarStore.setCardDeckLoading(false, 'Failed to load segment');
    render(ElementCardDeck, { props: { nodeId: '02.01.57.00.00.01' } });
    expect(screen.getByText(/failed to load segment/i)).toBeInTheDocument();
  });

  it('shows empty prompt when no segment is selected', () => {
    render(ElementCardDeck, { props: { nodeId: '02.01.57.00.00.01' } });
    expect(screen.getByText(/select a segment/i)).toBeInTheDocument();
  });

  it('all cards are collapsed on mount (FR-008)', () => {
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg:0', 'Port I/O');
    configSidebarStore.setCards('02.01.57.00.00.01', 'seg:0', [
      makeCard('0', 'Line 1'),
      makeCard('1', 'Line 2'),
    ]);
    render(ElementCardDeck, { props: { nodeId: '02.01.57.00.00.01' } });

    let state: any;
    configSidebarStore.subscribe(s => (state = s))();
    expect(state.cardDeck?.expandedCardIds).toHaveLength(0);
  });
});
