/**
 * Tests for pillSelection store
 *
 * Covers:
 * - makePillKey returns the expected stable string
 * - Store starts empty
 * - setPillSelection stores a value at the given key
 * - setPillSelection overwrites an existing value for the same key
 * - Multiple keys are stored independently
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { pillSelections, setPillSelection, makePillKey } from './pillSelection';

beforeEach(() => {
  pillSelections.set(new Map());
});

describe('makePillKey', () => {
  it('returns nodeId:path for a flat sibling path', () => {
    expect(makePillKey('02.01.57.00.00.01', { path: ['seg:0', 'elem:0#1'] }))
      .toBe('02.01.57.00.00.01:seg:0/elem:0#1');
  });

  it('returns correct key for a nested sibling path', () => {
    expect(makePillKey('02.01.57.00.00.01', { path: ['seg:0', 'elem:0#2', 'elem:1#1'] }))
      .toBe('02.01.57.00.00.01:seg:0/elem:0#2/elem:1#1');
  });
});

describe('pillSelection store', () => {
  it('starts empty after reset', () => {
    expect(get(pillSelections).size).toBe(0);
  });

  it('setPillSelection stores a value at the given key', () => {
    setPillSelection('node1:seg:0/elem:0#1', 2);
    expect(get(pillSelections).get('node1:seg:0/elem:0#1')).toBe(2);
  });

  it('overwrites an existing value for the same key', () => {
    setPillSelection('node1:seg:0/elem:0#1', 1);
    setPillSelection('node1:seg:0/elem:0#1', 3);
    expect(get(pillSelections).get('node1:seg:0/elem:0#1')).toBe(3);
  });

  it('stores multiple keys independently', () => {
    setPillSelection('node1:seg:0/elem:0#1', 0);
    setPillSelection('node2:seg:0/elem:0#1', 4);
    const m = get(pillSelections);
    expect(m.get('node1:seg:0/elem:0#1')).toBe(0);
    expect(m.get('node2:seg:0/elem:0#1')).toBe(4);
  });
});
