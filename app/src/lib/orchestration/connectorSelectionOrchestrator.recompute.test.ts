/**
 * E3b-1 B4: connector repair uses the same draft path whether or not the bus is
 * connected. A connected repair produces a configuration draft (written by the
 * next Save as a Pending Change), never an immediate IPC.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { ConnectorProfileView, ConnectorSelectionDocument } from '$lib/types/connectorProfile';
import type { LeafConfigNode, NodeConfigTree } from '$lib/types/nodeTree';

const { writeConfigValueSpy, treeRef, profileRef, documentRef } = vi.hoisted(() => ({
  writeConfigValueSpy: vi.fn(async () => true),
  treeRef: { current: null as unknown },
  profileRef: { current: null as unknown },
  documentRef: { current: null as unknown },
}));

vi.mock('$lib/api/config', () => ({
  writeConfigValue: writeConfigValueSpy,
}));

vi.mock('$lib/stores/nodeTree.svelte', () => ({
  nodeTreeStore: {
    getTree: () => treeRef.current,
    getLeafByLocation: (_nodeId: string, space: number, address: number) => {
      const tree = treeRef.current as NodeConfigTree;
      const leaf = tree.segments[0].children[0] as LeafConfigNode;
      return leaf.space === space && leaf.address === address ? leaf : null;
    },
    trees: new Map(),
  },
}));

vi.mock('$lib/stores/connectorSelections.svelte', () => ({
  connectorSelectionsStore: {
    getProfile: () => profileRef.current,
    getDocument: () => documentRef.current,
    setCompatibilityWarnings: vi.fn(),
  },
}));

import { configChangesStore } from '$lib/stores/configChanges.svelte';
import { layoutStore } from '$lib/stores/layout.svelte';
import { recomputeConnectorCompatibility } from './connectorSelectionOrchestrator';

const NODE_ID = '05.02.01.02.03.00';

function makeLeaf(): LeafConfigNode {
  return {
    kind: 'leaf',
    name: 'Input Function',
    description: null,
    elementType: 'int',
    address: 0,
    size: 1,
    space: 253,
    path: ['seg:0', 'elem:0#1', 'elem:0'],
    value: { type: 'int', value: 3 },
    eventRole: null,
    constraints: {
      min: 0,
      max: 8,
      defaultValue: null,
      mapEntries: [
        { value: 1, label: 'Normal' },
        { value: 2, label: 'Active Lo' },
        { value: 3, label: 'Alt Action Hi' },
      ],
    },
  };
}

function makeProfile(): ConnectorProfileView {
  return {
    nodeId: NODE_ID,
    carrierKey: 'rr-cirkits::tower-lcc',
    slots: [
      {
        slotId: 'connector-a',
        label: 'Connector A',
        order: 0,
        allowNoneInstalled: true,
        supportedDaughterboardIds: ['BOD-8-SM'],
        affectedPaths: ['Port I/O/Line#1'],
        resolvedAffectedPaths: [['seg:0', 'elem:0#1']],
        supportedDaughterboardConstraints: [
          {
            daughterboardId: 'BOD-8-SM',
            validityRules: [
              {
                targetPath: 'Port I/O/Line/Input Function',
                resolvedPath: ['seg:0', 'elem:0', 'elem:0'],
                effect: 'allowValues',
                allowedValues: [1, 2],
                allowedValueLabels: ['Normal', 'Active Lo'],
              },
            ],
          },
        ],
      },
    ],
    supportedDaughterboards: [{ daughterboardId: 'BOD-8-SM', displayName: 'BOD-8-SM' }],
  };
}

function makeDocument(): ConnectorSelectionDocument {
  return {
    nodeId: NODE_ID,
    carrierKey: 'rr-cirkits::tower-lcc',
    slotSelections: [{ slotId: 'connector-a', selectedDaughterboardId: 'BOD-8-SM', status: 'selected' }],
  };
}

describe('recomputeConnectorCompatibility while connected', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configChangesStore.clearAllDrafts();
    layoutStore.reset();
    layoutStore.setConnected(true);
    treeRef.current = {
      nodeId: NODE_ID,
      identity: null,
      segments: [{ name: 'Port I/O', description: null, origin: 0, space: 253, children: [makeLeaf()] }],
    } satisfies NodeConfigTree;
    profileRef.current = makeProfile();
    documentRef.current = makeDocument();
  });

  it('stages the repair as a configuration draft and makes no IPC call', async () => {
    expect(layoutStore.isOfflineMode).toBe(false);

    await recomputeConnectorCompatibility(NODE_ID);

    expect(writeConfigValueSpy).not.toHaveBeenCalled();
    expect(configChangesStore.draftEntries().map((e) => e.value)).toEqual([{ type: 'int', value: 1 }]);
  });
});
