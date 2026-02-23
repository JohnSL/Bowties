/**
 * T010: Vitest unit tests for ElementCard.svelte
 * TDD — written before implementation; must FAIL until ElementCard.svelte exists.
 *
 * Covers:
 * - Collapsed-by-default render (FR-008)
 * - Card title from cardTitle prop
 * - Expand reveals CardElementTree fields and sub-groups inline (FR-011)
 * - isLoading spinner
 * - loadError error state
 * - "(no configurable fields)" when fields and subGroups are empty
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import ElementCard from './ElementCard.svelte';
import type { CardData } from '$lib/stores/configSidebar';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

function makeCard(overrides: Partial<CardData> = {}): CardData {
  return {
    cardId: 'test-card',
    groupPath: ['seg:0', 'elem:0'],
    cdGroupName: 'Line',
    isReplicated: true,
    instanceIndex: 1,
    cardTitle: 'Line 1 (unnamed)',
    elements: null,
    isLoading: false,
    loadError: null,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('ElementCard.svelte', () => {
  it('renders the card title in the header', () => {
    render(ElementCard, {
      props: { card: makeCard({ cardTitle: 'Yard Button (Line 3)' }), nodeId: '02.01.57.00.00.01' },
    });
    expect(screen.getByText('Yard Button (Line 3)')).toBeInTheDocument();
  });

  it('is collapsed by default (FR-008)', () => {
    render(ElementCard, {
      props: { card: makeCard(), nodeId: '02.01.57.00.00.01' },
    });
    // Card body should not be visible when collapsed
    expect(screen.queryByRole('region', { name: /card body/i })).not.toBeInTheDocument();
  });

  it('shows spinner when isLoading is true', () => {
    render(ElementCard, {
      props: { card: makeCard({ isLoading: true }), nodeId: '02.01.57.00.00.01' },
    });
    // Some loading indicator should be present
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('shows error message when loadError is set', () => {
    render(ElementCard, {
      props: { card: makeCard({ loadError: 'Failed to load elements' }), nodeId: '02.01.57.00.00.01' },
    });
    expect(screen.getByText(/failed to load/i)).toBeInTheDocument();
  });

  it('shows (no configurable fields) message when elements has empty fields and subGroups', () => {
    const card = makeCard({
      elements: { groupName: 'Line', groupDescription: null, fields: [], subGroups: [] },
    });
    render(ElementCard, {
      props: { card, nodeId: '02.01.57.00.00.01' },
    });
    // Expand the card first
    fireEvent.click(screen.getByText('Line 1 (unnamed)'));
    expect(screen.getByText(/no configurable fields/i)).toBeInTheDocument();
  });

  it('calls get_card_elements via invoke when expanded with no elements loaded', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    (invoke as any).mockResolvedValue({
      groupName: 'Line',
      groupDescription: null,
      fields: [],
      subGroups: [],
    });

    const card = makeCard({ elements: null });
    render(ElementCard, {
      props: { card, nodeId: '02.01.57.00.00.01' },
    });

    await fireEvent.click(screen.getByText('Line 1 (unnamed)'));
    expect(invoke).toHaveBeenCalledWith('get_card_elements', expect.objectContaining({
      nodeId: '02.01.57.00.00.01',
      groupPath: ['seg:0', 'elem:0'],
    }));
  });

  it('renders field names from CardElementTree when expanded (FR-011)', () => {
    const card = makeCard({
      elements: {
        groupName: 'Line',
        groupDescription: null,
        fields: [
          {
            elementPath: ['seg:0', 'elem:0', 'elem:0'],
            name: 'User Name',
            description: null,
            dataType: 'string',
            memoryAddress: 100,
            sizeBytes: 16,
            defaultValue: null,
            addressSpace: 253,
          },
        ],
        subGroups: [],
      },
    });
    render(ElementCard, {
      props: { card, nodeId: '02.01.57.00.00.01' },
    });

    fireEvent.click(screen.getByText('Line 1 (unnamed)'));
    expect(screen.getByText('User Name')).toBeInTheDocument();
  });
});
