/**
 * Tests for configFocusStore (configFocus.svelte.ts).
 *
 * Step 4 of plan-config-nav-refactor:
 * Verifies that focusConfigField sets both signals independently,
 * and that each clear method only clears its own signal.
 */

import { describe, it, expect, beforeEach } from 'vitest';

// Import AFTER mocks so the Svelte 5 rune compiler runs in a clean state.
const { configFocusStore } = await import('$lib/stores/configFocus.svelte');

const FOCUS = { nodeId: 'node1', elementPath: ['seg:0', 'elem:0'] };

beforeEach(() => {
  // Reset both signals to null before each test
  configFocusStore.clearFocus();
});

describe('configFocusStore — Step 4 split signals', () => {
  it('focusConfigField sets both navigationRequest and leafFocusRequest', () => {
    configFocusStore.focusConfigField(FOCUS.nodeId, FOCUS.elementPath);
    expect(configFocusStore.navigationRequest).toEqual(FOCUS);
    expect(configFocusStore.leafFocusRequest).toEqual(FOCUS);
  });

  it('clearNavigation clears only navigationRequest, leaving leafFocusRequest intact', () => {
    configFocusStore.focusConfigField(FOCUS.nodeId, FOCUS.elementPath);
    configFocusStore.clearNavigation();
    expect(configFocusStore.navigationRequest).toBeNull();
    expect(configFocusStore.leafFocusRequest).toEqual(FOCUS);
  });

  it('clearLeafFocus clears only leafFocusRequest, leaving navigationRequest intact', () => {
    configFocusStore.focusConfigField(FOCUS.nodeId, FOCUS.elementPath);
    configFocusStore.clearLeafFocus();
    expect(configFocusStore.leafFocusRequest).toBeNull();
    expect(configFocusStore.navigationRequest).toEqual(FOCUS);
  });

  it('clearFocus clears both signals', () => {
    configFocusStore.focusConfigField(FOCUS.nodeId, FOCUS.elementPath);
    configFocusStore.clearFocus();
    expect(configFocusStore.navigationRequest).toBeNull();
    expect(configFocusStore.leafFocusRequest).toBeNull();
  });
});
