/**
 * Unit tests for configSidebarStore.pruneToAvailableNodes().
 *
 * Validates that sidebar selection state is preserved for nodes that survive
 * a transition (e.g. disconnect → offline) and cleared for nodes that don't.
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { configSidebarStore } from './configSidebar';

beforeEach(() => {
  configSidebarStore.reset();
});

describe('pruneToAvailableNodes', () => {
  it('keeps selectedNodeId when the node is in the available set', () => {
    configSidebarStore.setSelectedNode('02.01.57.00.00.01');
    configSidebarStore.pruneToAvailableNodes(new Set(['02.01.57.00.00.01']));

    expect(get(configSidebarStore).selectedNodeId).toBe('02.01.57.00.00.01');
  });

  it('clears selectedNodeId when the node is NOT in the available set', () => {
    configSidebarStore.setSelectedNode('02.01.57.00.00.01');
    configSidebarStore.pruneToAvailableNodes(new Set(['FF.FF.FF.FF.FF.FF']));

    expect(get(configSidebarStore).selectedNodeId).toBe(null);
  });

  it('keeps selectedSegment when its nodeId is in the available set', () => {
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg-1', 'Segment 1');
    configSidebarStore.pruneToAvailableNodes(new Set(['02.01.57.00.00.01']));

    const state = get(configSidebarStore);
    expect(state.selectedSegment).toEqual({ nodeId: '02.01.57.00.00.01', segmentId: 'seg-1' });
  });

  it('clears selectedSegment when its nodeId is NOT in the available set', () => {
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg-1', 'Segment 1');
    configSidebarStore.pruneToAvailableNodes(new Set(['FF.FF.FF.FF.FF.FF']));

    expect(get(configSidebarStore).selectedSegment).toBe(null);
  });

  it('filters expandedNodeIds to only those in the available set', () => {
    configSidebarStore.toggleNodeExpanded('node-A');
    configSidebarStore.toggleNodeExpanded('node-B');
    configSidebarStore.toggleNodeExpanded('node-C');

    configSidebarStore.pruneToAvailableNodes(new Set(['node-A', 'node-C']));

    expect(get(configSidebarStore).expandedNodeIds).toEqual(['node-A', 'node-C']);
  });

  it('always clears cardDeck, nodeLoadingStates, and nodeErrors', () => {
    configSidebarStore.selectSegment('02.01.57.00.00.01', 'seg-1', 'Segment 1');
    configSidebarStore.setNodeLoading('02.01.57.00.00.01', 'loading');

    configSidebarStore.pruneToAvailableNodes(new Set(['02.01.57.00.00.01']));

    const state = get(configSidebarStore);
    expect(state.cardDeck).toBe(null);
    expect(state.nodeLoadingStates).toEqual({});
    expect(state.nodeErrors).toEqual({});
  });

  it('degrades to full reset when called with an empty set', () => {
    configSidebarStore.setSelectedNode('02.01.57.00.00.01');
    configSidebarStore.toggleNodeExpanded('02.01.57.00.00.01');

    configSidebarStore.pruneToAvailableNodes(new Set());

    const state = get(configSidebarStore);
    expect(state.selectedNodeId).toBe(null);
    expect(state.selectedSegment).toBe(null);
    expect(state.expandedNodeIds).toEqual([]);
    expect(state.cardDeck).toBe(null);
  });
});
