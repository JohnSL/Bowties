import { get } from 'svelte/store';
import { beforeEach, describe, expect, it } from 'vitest';
import {
  failLayoutOpen,
  finishOfflineReplay,
  layoutOpenPhase,
  resetLayoutOpenPhase,
  startLayoutHydration,
  startLayoutOpen,
  startOfflineReplay,
} from './layoutOpenLifecycle';

describe('layoutOpenLifecycle', () => {
  beforeEach(() => {
    resetLayoutOpenPhase();
  });

  it('tracks the expected happy-path phases for offline layout replay', () => {
    startLayoutOpen();
    expect(get(layoutOpenPhase)).toBe('opening_file');

    startLayoutHydration();
    expect(get(layoutOpenPhase)).toBe('hydrating_snapshots');

    startOfflineReplay();
    expect(get(layoutOpenPhase)).toBe('replaying_offline_changes');

    finishOfflineReplay();
    expect(get(layoutOpenPhase)).toBe('ready');
  });

  it('can transition to error and reset back to idle', () => {
    startLayoutOpen();
    failLayoutOpen();
    expect(get(layoutOpenPhase)).toBe('error');

    resetLayoutOpenPhase();
    expect(get(layoutOpenPhase)).toBe('idle');
  });
});