/**
 * Tests for pillSelection store
 *
 * Covers:
 * - Store starts empty
 * - setPillSelection stores a value at the given key
 * - setPillSelection overwrites an existing value for the same key
 * - Multiple keys are stored independently
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { pillSelections, setPillSelection } from './pillSelection';

beforeEach(() => {
  pillSelections.set(new Map());
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
