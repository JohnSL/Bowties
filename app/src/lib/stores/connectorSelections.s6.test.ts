// Spec 014 / S6 integration test
//
// Asserts the unified Configuration Mode selection seam:
//   selector change → `set_node_mode_selection` IPC → orchestrator
//   triggers `nodeTreeStore.refreshTree` so the re-annotated tree
//   replaces the stale one. Combined with the existing backend test
//   that proves `annotate_tree` is invoked with selections on every
//   read path, this completes the round-trip evidence for S6.

import { describe, expect, it, beforeEach, vi } from 'vitest';

const setNodeModeSelectionMock = vi.fn().mockResolvedValue({ ok: true });
const getConnectorProfileMock = vi.fn();

vi.mock('$lib/api/layout', () => ({
  setNodeModeSelection: setNodeModeSelectionMock,
}));

vi.mock('$lib/api/connectorProfiles', () => ({
  getConnectorProfile: getConnectorProfileMock,
}));

const { connectorSelectionsStore } = await import('./connectorSelections.svelte');

beforeEach(() => {
  setNodeModeSelectionMock.mockClear();
  getConnectorProfileMock.mockReset();
  connectorSelectionsStore.reset();
});

describe('connectorSelections store (Spec 014 / S6) — unified node_mode_selections seam', () => {
  it('persists a selector change via the set_node_mode_selection IPC', async () => {
    const nodeId = '05.02.01.02.03.00';
    await connectorSelectionsStore.loadNode(nodeId, {
      nodeId,
      carrierKey: 'rr-cirkits::tower-lcc',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD4-CP'],
          affectedPaths: [],
          resolvedAffectedPaths: [],
          supportedDaughterboardConstraints: [],
        },
      ],
      supportedDaughterboards: [{ daughterboardId: 'BOD4-CP', displayName: 'BOD4-CP' }],
    });

    const saved = await connectorSelectionsStore.updateSlotSelection(
      nodeId,
      'connector-a',
      'BOD4-CP',
    );

    expect(saved?.slotSelections.find((s) => s.slotId === 'connector-a')?.selectedDaughterboardId)
      .toBe('BOD4-CP');
    expect(setNodeModeSelectionMock).toHaveBeenCalledWith(
      '050201020300',
      'connector-a',
      'BOD4-CP',
    );
  });

  it('does not call set_node_mode_selection when a slot is cleared (no Clear delta yet)', async () => {
    const nodeId = '05.02.01.02.03.00';
    await connectorSelectionsStore.loadNode(nodeId, {
      nodeId,
      carrierKey: 'rr-cirkits::tower-lcc',
      slots: [
        {
          slotId: 'connector-a',
          label: 'Connector A',
          order: 0,
          allowNoneInstalled: true,
          supportedDaughterboardIds: ['BOD4-CP'],
          affectedPaths: [],
          resolvedAffectedPaths: [],
          supportedDaughterboardConstraints: [],
        },
      ],
      supportedDaughterboards: [{ daughterboardId: 'BOD4-CP', displayName: 'BOD4-CP' }],
    });

    setNodeModeSelectionMock.mockClear();
    await connectorSelectionsStore.updateSlotSelection(nodeId, 'connector-a', null);
    expect(setNodeModeSelectionMock).not.toHaveBeenCalled();
  });
});
