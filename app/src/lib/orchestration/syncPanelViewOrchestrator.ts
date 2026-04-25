import type { ApplySyncResult, SyncMode, SyncSession } from '$lib/api/sync';

interface SyncPanelViewStore {
  session: SyncSession | null;
  setMode(mode: SyncMode): Promise<void>;
  loadSession(): Promise<void>;
  dismiss(): void;
  applySelected(): Promise<ApplySyncResult | null>;
}

export function hasSyncSessionRows(session: SyncSession | null): boolean {
  return !!session && (
    session.conflictRows.length > 0 ||
    session.cleanRows.length > 0 ||
    session.nodeMissingRows.length > 0
  );
}

export async function applySyncModeChoice(
  store: SyncPanelViewStore,
  mode: SyncMode,
  dismiss: () => void,
): Promise<void> {
  await store.setMode(mode);

  if (mode === 'bench_other_bus') {
    store.dismiss();
    dismiss();
    return;
  }

  await store.loadSession();
  if (!hasSyncSessionRows(store.session)) {
    store.dismiss();
    dismiss();
  }
}

export async function applySyncSelectionAndReconcile(
  store: SyncPanelViewStore,
  reconcile: (result: ApplySyncResult, session: SyncSession | null) => Promise<void>,
  dismiss: () => void,
): Promise<ApplySyncResult | null> {
  const session = store.session;
  const result = await store.applySelected();
  if (!result) return null;

  await reconcile(result, session);

  if (result.failed.length === 0) {
    store.dismiss();
    dismiss();
  }

  return result;
}