/**
 * Offline change store for baseline/planned row tracking.
 *
 * Phase 2 foundation: this holds pending rows loaded from or written to
 * offline-changes.yaml. UI integration is added in later phases.
 */

import type { OfflineChangeRow } from '$lib/api/sync';

class OfflineChangesStore {
  private _rows = $state<OfflineChangeRow[]>([]);
  private _busy = $state<boolean>(false);

  get rows(): OfflineChangeRow[] {
    return this._rows;
  }

  get isBusy(): boolean {
    return this._busy;
  }

  get pendingCount(): number {
    return this._rows.filter((r) => r.status === 'pending').length;
  }

  setRows(rows: OfflineChangeRow[]): void {
    this._rows = rows;
  }

  upsertRow(row: OfflineChangeRow): void {
    const idx = this._rows.findIndex((r) => r.changeId === row.changeId);
    if (idx >= 0) {
      this._rows[idx] = row;
      this._rows = [...this._rows];
      return;
    }
    this._rows = [...this._rows, row];
  }

  removeRow(changeId: string): void {
    this._rows = this._rows.filter((r) => r.changeId !== changeId);
  }

  clear(): void {
    this._rows = [];
    this._busy = false;
  }

  setBusy(value: boolean): void {
    this._busy = value;
  }
}

export const offlineChangesStore = new OfflineChangesStore();
