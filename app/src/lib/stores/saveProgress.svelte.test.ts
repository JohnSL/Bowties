/**
 * Tests for the saveProgressStore (S3 / spec 013).
 *
 * Validates phase transitions driven by both Tauri `save-progress` events and
 * direct setters used by the offline-save orchestrator path.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

type Listener = (event: { payload: unknown }) => void;
const listeners = new Map<string, Listener>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, fn: Listener) => {
    listeners.set(name, fn);
    return () => listeners.delete(name);
  }),
}));

import { saveProgressStore } from './saveProgress.svelte';

function emit(payload: unknown): void {
  const fn = listeners.get('save-progress');
  if (!fn) throw new Error('save-progress listener not registered');
  fn({ payload });
}

describe('saveProgressStore', () => {
  beforeEach(() => {
    listeners.clear();
    saveProgressStore.stopListening();
    saveProgressStore.reset();
  });

  it('starts idle and not visible', () => {
    expect(saveProgressStore.phase).toBe('idle');
    expect(saveProgressStore.isVisible).toBe(false);
    expect(saveProgressStore.isActive).toBe(false);
  });

  it('transitions through phases driven by save-progress events', async () => {
    await saveProgressStore.startListening();

    emit({ phase: 'saving-layout' });
    expect(saveProgressStore.phase).toBe('saving-layout');
    expect(saveProgressStore.isVisible).toBe(true);
    expect(saveProgressStore.isActive).toBe(true);

    emit({ phase: 'writing-config', current: 2, total: 5, label: 'User Name' });
    expect(saveProgressStore.phase).toBe('writing-config');
    expect(saveProgressStore.busWriteCurrent).toBe(2);
    expect(saveProgressStore.busWriteTotal).toBe(5);
    expect(saveProgressStore.currentLabel).toBe('User Name');

    emit({ phase: 'reconciling' });
    expect(saveProgressStore.phase).toBe('reconciling');

    emit({ phase: 'complete', failedCount: 1 });
    expect(saveProgressStore.phase).toBe('complete');
    expect(saveProgressStore.failedCount).toBe(1);
    expect(saveProgressStore.isActive).toBe(false);
    expect(saveProgressStore.isVisible).toBe(true);
  });

  it('begin() resets counters and enters saving-layout', () => {
    saveProgressStore.apply({ phase: 'writing-config', current: 3, total: 4 });
    saveProgressStore.begin();
    expect(saveProgressStore.phase).toBe('saving-layout');
    expect(saveProgressStore.busWriteCurrent).toBe(0);
    expect(saveProgressStore.busWriteTotal).toBe(0);
    expect(saveProgressStore.failedCount).toBe(0);
  });

  it('reset() returns to idle', () => {
    saveProgressStore.apply({ phase: 'complete', failedCount: 2 });
    saveProgressStore.reset();
    expect(saveProgressStore.phase).toBe('idle');
    expect(saveProgressStore.failedCount).toBe(0);
    expect(saveProgressStore.isVisible).toBe(false);
  });

  it('fail() moves to error phase', () => {
    saveProgressStore.begin();
    saveProgressStore.fail();
    expect(saveProgressStore.phase).toBe('error');
    expect(saveProgressStore.isActive).toBe(false);
    expect(saveProgressStore.errorMessage).toBeNull();
  });

  it('fail(message) stores the error message for the dialog', () => {
    saveProgressStore.begin();
    saveProgressStore.fail('Save failed: disk full');
    expect(saveProgressStore.phase).toBe('error');
    expect(saveProgressStore.errorMessage).toBe('Save failed: disk full');
    expect(saveProgressStore.isVisible).toBe(true);
  });

  it('reset() clears the error message', () => {
    saveProgressStore.fail('boom');
    saveProgressStore.reset();
    expect(saveProgressStore.phase).toBe('idle');
    expect(saveProgressStore.errorMessage).toBeNull();
  });

  it('startListening is idempotent', async () => {
    await saveProgressStore.startListening();
    await saveProgressStore.startListening();
    // Only one listener registered means emit still works exactly once.
    emit({ phase: 'reconciling' });
    expect(saveProgressStore.phase).toBe('reconciling');
  });

  it('stopListening removes the listener', async () => {
    await saveProgressStore.startListening();
    saveProgressStore.stopListening();
    expect(listeners.has('save-progress')).toBe(false);
  });
});
