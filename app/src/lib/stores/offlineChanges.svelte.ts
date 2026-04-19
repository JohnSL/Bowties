/**
 * Offline change store for baseline/planned row tracking.
 *
 * Phase 2 foundation: this holds pending rows loaded from or written to
 * offline-changes.yaml. UI integration is added in later phases.
 */

import type { OfflineChangeInput, OfflineChangeRow } from '$lib/api/sync';
import { listOfflineChanges, replaceOfflineChanges, revertOfflineChange } from '$lib/api/sync';

class OfflineChangesStore {
  private _persistedRows = $state<OfflineChangeRow[]>([]);
  private _draftRows = $state<OfflineChangeRow[]>([]);
  private _busy = $state<boolean>(false);

  get rows(): OfflineChangeRow[] {
    return this.effectiveRows;
  }

  get persistedRows(): OfflineChangeRow[] {
    return this._persistedRows;
  }

  get draftRows(): OfflineChangeRow[] {
    return this._draftRows;
  }

  get effectiveRows(): OfflineChangeRow[] {
    const merged = new Map<string, OfflineChangeRow>();
    for (const row of this._persistedRows) {
      merged.set(this.targetKeyForRow(row), row);
    }
    for (const row of this._draftRows) {
      const key = this.targetKeyForRow(row);
      const persisted = merged.get(key);
      if (persisted && row.plannedValue === row.baselineValue) {
        merged.delete(key);
      } else if (row.plannedValue !== row.baselineValue) {
        merged.set(key, row);
      }
    }
    return [...merged.values()];
  }

  get isBusy(): boolean {
    return this._busy;
  }

  get pendingCount(): number {
    return this.pendingApplyCount;
  }

  get pendingApplyCount(): number {
    return this._persistedRows.filter((r) => r.status === 'pending').length;
  }

  get draftCount(): number {
    return this._draftRows.filter((r) => r.status === 'pending').length;
  }

  setRows(rows: OfflineChangeRow[]): void {
    this._persistedRows = rows;
    this._draftRows = [];
  }

  upsertRow(row: OfflineChangeRow): void {
    const key = this.targetKeyForRow(row);
    const idx = this._draftRows.findIndex((r) => this.targetKeyForRow(r) === key);
    if (idx >= 0) {
      this._draftRows[idx] = row;
      this._draftRows = [...this._draftRows];
      return;
    }
    this._draftRows = [...this._draftRows, row];
  }

  upsertConfigChange(change: {
    nodeId: string;
    space: number;
    offset: string;
    baselineValue: string;
    plannedValue: string;
  }): void {
    const existingPersisted = this.findPersistedConfigChange(change.nodeId, change.space, change.offset);
    const existingDraft = this.findDraftConfigChange(change.nodeId, change.space, change.offset);

    const baselineValue =
      existingDraft?.baselineValue ??
      existingPersisted?.baselineValue ??
      change.baselineValue;

    const nextRow: OfflineChangeRow = {
      changeId:
        existingDraft?.changeId ??
        existingPersisted?.changeId ??
        `local-${Date.now()}-${Math.random().toString(16).slice(2)}`,
      kind: 'config',
      nodeId: change.nodeId,
      space: change.space,
      offset: change.offset,
      baselineValue,
      plannedValue: change.plannedValue,
      status: 'pending',
    };

    if (nextRow.plannedValue === nextRow.baselineValue && !existingPersisted) {
      if (existingDraft) this.removeDraftByTarget(nextRow);
      return;
    }

    this.upsertRow(nextRow);
  }

  upsertBowtieMetadataChange(change: {
    eventIdHex: string;
    baselineValue: string;
    plannedValue: string;
  }): void {
    const baselineKey = `event:${change.eventIdHex}`;
    const existingPersisted = this.findPersistedBowtieMetadataChange(change.eventIdHex);
    const existingDraft = this.findDraftBowtieMetadataChange(change.eventIdHex);

    const baselineValue =
      existingDraft?.baselineValue ??
      existingPersisted?.baselineValue ??
      baselineKey;

    const nextRow: OfflineChangeRow = {
      changeId:
        existingDraft?.changeId ??
        existingPersisted?.changeId ??
        `local-${Date.now()}-${Math.random().toString(16).slice(2)}`,
      kind: 'bowtieMetadata',
      baselineValue,
      plannedValue: change.plannedValue,
      status: 'pending',
    };

    if (nextRow.plannedValue === nextRow.baselineValue && !existingPersisted) {
      if (existingDraft) this.removeDraftByTarget(nextRow);
      return;
    }

    this.upsertRow(nextRow);
  }

  findDraftConfigChange(nodeId: string, space: number, offset: string): OfflineChangeRow | null {
    return this._draftRows.find(
      (r) =>
        r.kind === 'config' &&
        r.status === 'pending' &&
        this.normalizeNodeId(r.nodeId) === this.normalizeNodeId(nodeId) &&
        r.space === space &&
        r.offset === offset
    ) ?? null;
  }

  findPersistedConfigChange(nodeId: string, space: number, offset: string): OfflineChangeRow | null {
    return this._persistedRows.find(
      (r) =>
        r.kind === 'config' &&
        r.status === 'pending' &&
        this.normalizeNodeId(r.nodeId) === this.normalizeNodeId(nodeId) &&
        r.space === space &&
        r.offset === offset
    ) ?? null;
  }

  findDraftBowtieMetadataChange(eventIdHex: string): OfflineChangeRow | null {
    const baselineKey = `event:${eventIdHex}`;
    return this._draftRows.find(
      (r) => r.kind === 'bowtieMetadata' && r.status === 'pending' && r.baselineValue === baselineKey
    ) ?? null;
  }

  findPersistedBowtieMetadataChange(eventIdHex: string): OfflineChangeRow | null {
    const baselineKey = `event:${eventIdHex}`;
    return this._persistedRows.find(
      (r) => r.kind === 'bowtieMetadata' && r.status === 'pending' && r.baselineValue === baselineKey
    ) ?? null;
  }

  hasPersistedConfigChange(nodeId: string, space: number, offset: string): boolean {
    return this.findPersistedConfigChange(nodeId, space, offset) !== null;
  }

  removeRow(changeId: string): void {
    this._draftRows = this._draftRows.filter((r) => r.changeId !== changeId);
  }

  /**
   * Revert a single offline change back to its captured baseline.
   * Removes the change from both draft and persisted rows, and calls
   * the backend IPC to remove it from the persisted offline-changes file.
   */
  async revertToBaseline(changeId: string): Promise<boolean> {
    this._busy = true;
    try {
      // Remove from draft rows immediately
      this._draftRows = this._draftRows.filter((r) => r.changeId !== changeId);

      // If persisted, call the backend to remove it and reload
      const wasPersisted = this._persistedRows.some((r) => r.changeId === changeId);
      if (wasPersisted) {
        await revertOfflineChange(changeId);
        await this.reloadFromBackend();
      }
      return true;
    } catch {
      return false;
    } finally {
      this._busy = false;
    }
  }

  clear(): void {
    this._persistedRows = [];
    this._draftRows = [];
    this._busy = false;
  }

  setBusy(value: boolean): void {
    this._busy = value;
  }

  async reloadFromBackend(): Promise<void> {
    this._busy = true;
    try {
      const rows = await listOfflineChanges();
      this._persistedRows = rows;
      this._draftRows = [];
    } finally {
      this._busy = false;
    }
  }

  async revertAllPending(): Promise<number> {
    const pending = this._draftRows.filter((r) => r.status === 'pending').length;
    this._draftRows = [];
    return pending;
  }

  async flushPendingToBackend(): Promise<number> {
    this._busy = true;
    try {
      const merged = this.effectiveRows.filter((r) => r.status === 'pending');
      const payload: OfflineChangeInput[] = merged.map((r) => ({
        kind: r.kind,
        nodeId: r.nodeId,
        space: r.space,
        offset: r.offset,
        baselineValue: r.baselineValue,
        plannedValue: r.plannedValue,
      }));

      const count = await replaceOfflineChanges(payload);
      await this.reloadFromBackend();
      return count;
    } finally {
      this._busy = false;
    }
  }

  private normalizeNodeId(nodeId?: string): string {
    return (nodeId ?? '').replace(/\./g, '').toUpperCase();
  }

  private targetKeyForRow(row: OfflineChangeRow): string {
    if (row.kind === 'config') {
      return `config:${this.normalizeNodeId(row.nodeId)}:${row.space ?? 0}:${row.offset ?? ''}`;
    }
    if (row.kind === 'bowtieMetadata') {
      return `bowtieMetadata:${row.baselineValue}`;
    }
    return `${row.kind}:${row.changeId}`;
  }

  private removeDraftByTarget(row: OfflineChangeRow): void {
    const key = this.targetKeyForRow(row);
    this._draftRows = this._draftRows.filter((r) => this.targetKeyForRow(r) !== key);
  }
}

export const offlineChangesStore = new OfflineChangesStore();
