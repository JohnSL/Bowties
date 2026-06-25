// Spec 014 / S6 + ADR-0012 integration test
//
// Asserts the unified Configuration Mode selection seam:
//   selector change → in-memory draft → isDirty → collectDeltas →
//   save workflow applies deltas to backend.
//
// ADR-0012: connector selections are in-memory drafts. No IPC is called
// on user interaction — deltas are collected at save time.

import { describe, expect, it, beforeEach, vi } from 'vitest';

const getConnectorProfileMock = vi.fn();

vi.mock('$lib/api/connectorProfiles', () => ({
  getConnectorProfile: getConnectorProfileMock,
}));

vi.mock('$lib/api/layout', () => ({}));

const { connectorSelectionsStore } = await import('./connectorSelections.svelte');

const PROFILE = {
  nodeId: '05.02.01.02.03.00',
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
};

beforeEach(() => {
  getConnectorProfileMock.mockReset();
  connectorSelectionsStore.reset();
});

describe('connectorSelections store (ADR-0012) — in-memory draft lifecycle', () => {
  it('selecting a board updates in-memory and marks dirty', async () => {
    const nodeId = '05.02.01.02.03.00';
    await connectorSelectionsStore.loadNode(nodeId, PROFILE);

    expect(connectorSelectionsStore.isDirty).toBe(false);

    await connectorSelectionsStore.updateSlotSelection(nodeId, 'connector-a', 'BOD4-CP');

    expect(connectorSelectionsStore.isDirty).toBe(true);
    expect(connectorSelectionsStore.editCount).toBe(1);
  });

  it('collectDeltas produces SetNodeModeSelection for a selected board', async () => {
    const nodeId = '05.02.01.02.03.00';
    await connectorSelectionsStore.loadNode(nodeId, PROFILE);
    await connectorSelectionsStore.updateSlotSelection(nodeId, 'connector-a', 'BOD4-CP');

    const deltas = connectorSelectionsStore.collectDeltas();
    expect(deltas).toEqual([
      {
        type: 'setNodeModeSelection',
        nodeKey: '050201020300',
        modeId: 'connector-a',
        variantId: 'BOD4-CP',
      },
    ]);
  });

  it('clearing a slot produces ClearNodeModeSelection delta', async () => {
    const nodeId = '05.02.01.02.03.00';
    // Simulate: loaded from layout with BOD4-CP already selected
    await connectorSelectionsStore.loadNode(nodeId, PROFILE);
    // Manually set selection to simulate a persisted baseline
    await connectorSelectionsStore.updateSlotSelection(nodeId, 'connector-a', 'BOD4-CP');
    connectorSelectionsStore.hydrateBaseline(); // mark current as saved

    // Now clear it
    await connectorSelectionsStore.updateSlotSelection(nodeId, 'connector-a', null);

    expect(connectorSelectionsStore.isDirty).toBe(true);
    const deltas = connectorSelectionsStore.collectDeltas();
    expect(deltas).toEqual([
      {
        type: 'clearNodeModeSelection',
        nodeKey: '050201020300',
        modeId: 'connector-a',
      },
    ]);
  });

  it('discard reverts to baseline', async () => {
    const nodeId = '05.02.01.02.03.00';
    await connectorSelectionsStore.loadNode(nodeId, PROFILE);
    await connectorSelectionsStore.updateSlotSelection(nodeId, 'connector-a', 'BOD4-CP');

    expect(connectorSelectionsStore.isDirty).toBe(true);

    connectorSelectionsStore.discard();

    expect(connectorSelectionsStore.isDirty).toBe(false);
    expect(connectorSelectionsStore.collectDeltas()).toEqual([]);
  });

  it('hydrateBaseline after save resets dirty state', async () => {
    const nodeId = '05.02.01.02.03.00';
    await connectorSelectionsStore.loadNode(nodeId, PROFILE);
    await connectorSelectionsStore.updateSlotSelection(nodeId, 'connector-a', 'BOD4-CP');

    expect(connectorSelectionsStore.isDirty).toBe(true);

    connectorSelectionsStore.hydrateBaseline();

    expect(connectorSelectionsStore.isDirty).toBe(false);
    expect(connectorSelectionsStore.collectDeltas()).toEqual([]);
  });
});
