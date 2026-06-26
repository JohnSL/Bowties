import { describe, it, expect, vi, beforeEach } from 'vitest';
import { startEventStateListening } from './eventStateOrchestrator';
import { eventStateStore } from '$lib/stores/eventState.svelte';

// Mock Tauri listen
let capturedHandler: ((event: { payload: { eventId: string; timestamp: string } }) => void) | null = null;
const mockUnlisten = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (_eventName: string, handler: unknown) => {
    capturedHandler = handler as typeof capturedHandler;
    return mockUnlisten;
  }),
}));

describe('eventStateOrchestrator', () => {
  beforeEach(() => {
    capturedHandler = null;
    mockUnlisten.mockClear();
    eventStateStore.clear();
  });

  it('records events in the event state store when listener fires', async () => {
    await startEventStateListening();

    expect(capturedHandler).not.toBeNull();
    capturedHandler!({ payload: { eventId: '0501010101000001', timestamp: '2026-06-25T12:00:00.000Z' } });

    expect(eventStateStore.lastSeen('0501010101000001')).toBe(new Date('2026-06-25T12:00:00.000Z').getTime());
  });

  it('updates timestamp on repeated events', async () => {
    await startEventStateListening();

    capturedHandler!({ payload: { eventId: '0501010101000001', timestamp: '2026-06-25T12:00:00.000Z' } });
    capturedHandler!({ payload: { eventId: '0501010101000001', timestamp: '2026-06-25T12:01:00.000Z' } });

    expect(eventStateStore.lastSeen('0501010101000001')).toBe(new Date('2026-06-25T12:01:00.000Z').getTime());
  });

  it('teardown removes listener and clears store', async () => {
    const teardown = await startEventStateListening();

    capturedHandler!({ payload: { eventId: '0501010101000001', timestamp: '2026-06-25T12:00:00.000Z' } });
    expect(eventStateStore.size).toBe(1);

    teardown();

    expect(mockUnlisten).toHaveBeenCalledOnce();
    expect(eventStateStore.size).toBe(0);
  });
});
