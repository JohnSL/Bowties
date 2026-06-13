import { describe, expect, it, vi } from 'vitest';
import {
  MENU_EVENT_BINDINGS,
  registerMenuListeners,
  type MenuActionHandlers,
  type MenuListenFn,
} from './menuListeners';

function makeActions() {
  return {
    disconnect: vi.fn(() => {}),
    refresh: vi.fn(() => {}),
    traffic: vi.fn(() => {}),
    viewCdi: vi.fn(() => {}),
    redownloadCdi: vi.fn(() => {}),
    exit: vi.fn(() => {}),
    openLayout: vi.fn(() => {}),
    closeLayout: vi.fn(() => {}),
    saveLayout: vi.fn(() => {}),
    saveLayoutAs: vi.fn(() => {}),
    syncToBus: vi.fn(() => {}),
    addPlaceholderBoard: vi.fn(() => {}),
    deletePlaceholderBoard: vi.fn(() => {}),
    diagnostics: vi.fn(() => {}),
  };
}

describe('registerMenuListeners', () => {
  it('registers exactly one listener per menu event binding', async () => {
    const handlers = new Map<string, () => void>();
    const listenFn: MenuListenFn = vi.fn(async (event, handler) => {
      handlers.set(event, handler);
      return () => {};
    });

    await registerMenuListeners(makeActions(), listenFn);

    expect(listenFn).toHaveBeenCalledTimes(MENU_EVENT_BINDINGS.length);
    for (const [event] of MENU_EVENT_BINDINGS) {
      expect(handlers.has(event)).toBe(true);
    }
  });

  it('dispatches each event to its mapped action', async () => {
    const handlers = new Map<string, () => void>();
    const listenFn: MenuListenFn = async (event, handler) => {
      handlers.set(event, handler);
      return () => {};
    };
    const actions = makeActions();

    await registerMenuListeners(actions, listenFn);

    for (const [event, key] of MENU_EVENT_BINDINGS) {
      handlers.get(event)!();
      expect(actions[key]).toHaveBeenCalledTimes(1);
    }
  });

  it('returns a teardown that removes every listener', async () => {
    const unlisteners = MENU_EVENT_BINDINGS.map(() => vi.fn());
    let i = 0;
    const listenFn: MenuListenFn = async () => unlisteners[i++];

    const teardown = await registerMenuListeners(makeActions(), listenFn);
    teardown();

    for (const unlisten of unlisteners) {
      expect(unlisten).toHaveBeenCalledTimes(1);
    }
  });

  it('swallows the promise from an async action (fire-and-forget)', async () => {
    const handlers = new Map<string, () => void>();
    const listenFn: MenuListenFn = async (event, handler) => {
      handlers.set(event, handler);
      return () => {};
    };
    const actions = makeActions();
    actions.diagnostics.mockResolvedValue(undefined);

    await registerMenuListeners(actions, listenFn);

    expect(() => handlers.get('menu-diagnostics')!()).not.toThrow();
    expect(actions.diagnostics).toHaveBeenCalledTimes(1);
  });
});
