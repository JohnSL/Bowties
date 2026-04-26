import { describe, expect, it } from 'vitest';
import {
  resolveConnectTransition,
  resolveDisconnectTransition,
  resolveStartupTransition,
  shouldProbeAfterTransition,
  shouldResetFreshLiveSession,
} from './lifecycleTransitionMatrix';

describe('lifecycleTransitionMatrix', () => {
  it('covers startup online and offline mode transitions', () => {
    expect(resolveStartupTransition(false, false)).toBe('startup_disconnected_idle');
    expect(resolveStartupTransition(false, true)).toBe('startup_disconnected_idle');
    expect(resolveStartupTransition(true, false)).toBe('startup_fresh_live');
    expect(resolveStartupTransition(true, true)).toBe('startup_preserved_layout');
  });

  it('covers connect transitions for fresh-live and preserved-layout sessions', () => {
    expect(resolveConnectTransition(false)).toBe('connect_fresh_live');
    expect(resolveConnectTransition(true)).toBe('connect_preserved_layout');
  });

  it('covers disconnect transitions for rehydrate, preserve, and clear outcomes', () => {
    expect(resolveDisconnectTransition(true, true)).toBe('rehydrated_offline');
    expect(resolveDisconnectTransition(true, false)).toBe('preserved_layout');
    expect(resolveDisconnectTransition(false, false)).toBe('cleared_to_connection');
    expect(resolveDisconnectTransition(false, true)).toBe('cleared_to_connection');
  });

  it('marks only the fresh-live transitions for state reset', () => {
    expect(shouldResetFreshLiveSession('startup_fresh_live')).toBe(true);
    expect(shouldResetFreshLiveSession('connect_fresh_live')).toBe(true);
    expect(shouldResetFreshLiveSession('startup_preserved_layout')).toBe(false);
    expect(shouldResetFreshLiveSession('connect_preserved_layout')).toBe(false);
    expect(shouldResetFreshLiveSession('rehydrated_offline')).toBe(false);
  });

  it('marks only startup/connect online transitions for probing', () => {
    expect(shouldProbeAfterTransition('startup_fresh_live')).toBe(true);
    expect(shouldProbeAfterTransition('startup_preserved_layout')).toBe(true);
    expect(shouldProbeAfterTransition('connect_fresh_live')).toBe(true);
    expect(shouldProbeAfterTransition('connect_preserved_layout')).toBe(true);
    expect(shouldProbeAfterTransition('startup_disconnected_idle')).toBe(false);
    expect(shouldProbeAfterTransition('preserved_layout')).toBe(false);
    expect(shouldProbeAfterTransition('cleared_to_connection')).toBe(false);
  });
});