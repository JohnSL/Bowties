import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/**
 * Owns the lifecycle of the native-menu (`menu-*`) event listener set.
 *
 * Registering ~14 OS-menu relays inline in the route's `onMount` buried the
 * listen/teardown bookkeeping in screen-composition code. This registrar hides
 * that bookkeeping behind a single call that returns one combined teardown,
 * mirroring the shape of `installMenuShortcuts`. The route still owns each
 * action's body (store access, unsaved-changes guards) and supplies them via
 * {@link MenuActionHandlers}; this module only owns the event-name table and
 * the register/unlisten lifecycle.
 */
export interface MenuActionHandlers {
  disconnect: () => void;
  refresh: () => void;
  traffic: () => void;
  viewCdi: () => void;
  redownloadCdi: () => void;
  exit: () => void;
  openLayout: () => void;
  closeLayout: () => void;
  saveLayout: () => void;
  saveLayoutAs: () => void;
  syncToBus: () => void;
  addPlaceholderBoard: () => void;
  deletePlaceholderBoard: () => void;
  diagnostics: () => void | Promise<void>;
  about: () => void;
}

/** Subset of Tauri's `listen` used here; injectable for tests. */
export type MenuListenFn = (
  event: string,
  handler: () => void,
) => Promise<UnlistenFn>;

/** Maps native menu event names to their handler key. */
export const MENU_EVENT_BINDINGS: ReadonlyArray<readonly [string, keyof MenuActionHandlers]> = [
  ['menu-disconnect', 'disconnect'],
  ['menu-refresh', 'refresh'],
  ['menu-traffic', 'traffic'],
  ['menu-view-cdi', 'viewCdi'],
  ['menu-redownload-cdi', 'redownloadCdi'],
  ['menu-exit', 'exit'],
  ['menu-open-layout', 'openLayout'],
  ['menu-close-layout', 'closeLayout'],
  ['menu-save-layout', 'saveLayout'],
  ['menu-save-layout-as', 'saveLayoutAs'],
  ['menu-sync-to-bus', 'syncToBus'],
  ['menu-add-placeholder-board', 'addPlaceholderBoard'],
  ['menu-delete-placeholder-board', 'deletePlaceholderBoard'],
  ['menu-diagnostics', 'diagnostics'],
  ['menu-about', 'about'],
] as const;

/**
 * Register all native-menu event listeners and return a single teardown that
 * removes every listener. Inject `listenFn` in tests to avoid Tauri.
 */
export async function registerMenuListeners(
  actions: MenuActionHandlers,
  listenFn: MenuListenFn = listen,
): Promise<() => void> {
  const unlistens = await Promise.all(
    MENU_EVENT_BINDINGS.map(([event, key]) =>
      listenFn(event, () => { void actions[key](); }),
    ),
  );
  return () => unlistens.forEach((unlisten) => unlisten());
}
