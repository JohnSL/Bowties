/**
 * Tests for configReadStatus store functions.
 *
 * Covers:
 * - markNodeConfigRead — adds a node ID to the set
 * - clearConfigReadStatus — wipes the entire set
 * - removeNodesConfigRead — removes only the specified IDs, preserving the rest
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  configReadNodesStore,
  markNodeConfigRead,
  clearConfigReadStatus,
  removeNodesConfigRead,
} from './configReadStatus';

beforeEach(() => {
  clearConfigReadStatus();
});

describe('markNodeConfigRead', () => {
  it('adds a node ID to the set', () => {
    markNodeConfigRead('01.02.03.04.05.06');
    expect(get(configReadNodesStore).has('01.02.03.04.05.06')).toBe(true);
  });

  it('is idempotent — marking the same node twice leaves size at 1', () => {
    markNodeConfigRead('01.02.03.04.05.06');
    markNodeConfigRead('01.02.03.04.05.06');
    expect(get(configReadNodesStore).size).toBe(1);
  });

  it('tracks multiple node IDs independently', () => {
    markNodeConfigRead('AA.BB.CC.DD.EE.01');
    markNodeConfigRead('AA.BB.CC.DD.EE.02');
    const store = get(configReadNodesStore);
    expect(store.has('AA.BB.CC.DD.EE.01')).toBe(true);
    expect(store.has('AA.BB.CC.DD.EE.02')).toBe(true);
    expect(store.size).toBe(2);
  });
});

describe('clearConfigReadStatus', () => {
  it('empties the set', () => {
    markNodeConfigRead('01.02.03.04.05.06');
    markNodeConfigRead('AA.BB.CC.DD.EE.FF');
    clearConfigReadStatus();
    expect(get(configReadNodesStore).size).toBe(0);
  });

  it('is a no-op when already empty', () => {
    clearConfigReadStatus();
    expect(get(configReadNodesStore).size).toBe(0);
  });
});

describe('removeNodesConfigRead', () => {
  it('removes only the specified node IDs, leaving others intact', () => {
    markNodeConfigRead('01.02.03.04.05.01');
    markNodeConfigRead('01.02.03.04.05.02');
    markNodeConfigRead('01.02.03.04.05.03');

    removeNodesConfigRead(['01.02.03.04.05.01', '01.02.03.04.05.03']);

    const store = get(configReadNodesStore);
    expect(store.has('01.02.03.04.05.01')).toBe(false);
    expect(store.has('01.02.03.04.05.03')).toBe(false);
    expect(store.has('01.02.03.04.05.02')).toBe(true);
    expect(store.size).toBe(1);
  });

  it('is a no-op when passed an empty array', () => {
    markNodeConfigRead('01.02.03.04.05.06');
    removeNodesConfigRead([]);
    expect(get(configReadNodesStore).size).toBe(1);
  });

  it('is safe when a specified ID is not in the set', () => {
    markNodeConfigRead('01.02.03.04.05.06');
    removeNodesConfigRead(['FF.FF.FF.FF.FF.FF']);
    expect(get(configReadNodesStore).has('01.02.03.04.05.06')).toBe(true);
  });

  it('removes all when all IDs are specified', () => {
    markNodeConfigRead('AA.BB.CC.DD.EE.01');
    markNodeConfigRead('AA.BB.CC.DD.EE.02');
    removeNodesConfigRead(['AA.BB.CC.DD.EE.01', 'AA.BB.CC.DD.EE.02']);
    expect(get(configReadNodesStore).size).toBe(0);
  });
});
