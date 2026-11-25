/**
 * Pending change store for baseline/planned row tracking.
 *
 * Phase 2 foundation: this holds Pending Change rows loaded from or written to
 * the layout's pending-changes file.
 */

import type { PendingChangeRow, PendingChangeOrigin } from '$lib/api/sync';
import { listPendingChanges, revertPendingChange, setPendingChange } from '$lib/api/sync';
import { effectiveValue, type LeafConfigNode, type TreeConfigValue } from '$lib/types/nodeTree';
import { normalizeNodeId } from '$lib/utils/nodeId';
import type { StagedRepair } from '$lib/types/connectorProfile';
import { isWaitingPendingChange } from '$lib/utils/pendingChangeStatus';
import { parseOfflineStoredValueForLeaf } from '$lib/utils/treeConfigValuePersistence';

// Filters over `_draftRows` keep `status === 'pending'`: a draft is a local row
// that has never been recorded or classified, so it is always 'pending'. Filters
// over persisted / saved / effective rows use `isWaitingPendingChange`.
class PendingChangesStore {
  private _persistedRows = $state<PendingChangeRow[]>([]);
  private _savedRows = $state<PendingChangeRow[]>([]);
  private _draftRows = $state<PendingChangeRow[]>([]);
  private _busy = $state<boolean>(false);

  get rows(): PendingChangeRow[] {
    return this.effectiveRows;
  }

  get persistedRows(): PendingChangeRow[] {
    return this._persistedRows;
  }

  get draftRows(): PendingChangeRow[] {
    return this._draftRows;
  }

  get savedRows(): PendingChangeRow[] {
    return this._savedRows;
  }

  get effectiveRows(): PendingChangeRow[] {
    const merged = new Map<string, PendingChangeRow>();
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
    return this._persistedRows.filter(isWaitingPendingChange).length;
  }

  get draftCount(): number {
    return this._draftRows.filter(
      (r) => r.status === 'pending' && r.plannedValue !== r.baselineValue,
    ).length;
  }

  /**
   * Count of persisted pending rows that have been reverted in the working
   * set but not yet saved to disk. Used by the presenter to count unsaved
   * edits from persisted-row reversions.
   */
  get revertedPersistedCount(): number {
    const persistedKeys = new Set(
      this._persistedRows.map((r) => this.targetKeyForRow(r)),
    );
    return this._savedRows.filter(
      (r) => isWaitingPendingChange(r) && !persistedKeys.has(this.targetKeyForRow(r)),
    ).length;
  }

  /**
   * Config rows whose change is not yet saved in the layout file: draft rows
   * that differ from their baseline, plus persisted rows reverted in the
   * working set. Rows already saved and still waiting are not included (they
   * are `pendingApplyCount`).
   */
  get unsavedConfigRows(): PendingChangeRow[] {
    const persistedKeys = new Set(this._persistedRows.map((r) => this.targetKeyForRow(r)));
    const drafts = this._draftRows.filter(
      (r) => r.kind === 'config' && r.status === 'pending' && r.plannedValue !== r.baselineValue,
    );
    const reverted = this._savedRows.filter(
      (r) =>
        r.kind === 'config' &&
        isWaitingPendingChange(r) &&
        !persistedKeys.has(this.targetKeyForRow(r)),
    );
    return [...drafts, ...reverted];
  }

  setRows(rows: PendingChangeRow[]): void {
    this._persistedRows = [...rows];
    this._savedRows = [...rows];
    this._draftRows = [];
  }

  upsertRow(row: PendingChangeRow): void {
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
    origin?: PendingChangeOrigin;
  }): void {
    const existingPersisted = this.findPersistedConfigChange(change.nodeId, change.space, change.offset);
    const existingDraft = this.findDraftConfigChange(change.nodeId, change.space, change.offset);

    const baselineValue = this.currentEffectivePlannedValueForConfigChange(
      change.nodeId,
      change.space,
      change.offset,
      change.baselineValue,
    );

    const nextRow: DraftPendingChangeRow = {
      changeId:
        existingDraft?.changeId ??
        `local-${Date.now()}-${Math.random().toString(16).slice(2)}`,
      kind: 'config',
      nodeId: change.nodeId,
      space: change.space,
      offset: change.offset,
      baselineValue,
      plannedValue: change.plannedValue,
      status: 'pending',
      ...(change.origin ? { origin: change.origin } : {}),
    };

    if (nextRow.plannedValue === nextRow.baselineValue) {
      if (existingDraft) {
        this.removeDraftByTarget(nextRow);
      } else if (existingPersisted) {
        this.upsertRow(nextRow);
      }
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

    const nextRow: PendingChangeRow = {
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

  findDraftConfigChange(nodeId: string, space: number, offset: string): PendingChangeRow | null {
    return this._draftRows.find(
      (r) =>
        r.kind === 'config' &&
        r.status === 'pending' &&
        normalizeNodeId(r.nodeId) === normalizeNodeId(nodeId) &&
        r.space === space &&
        r.offset === offset
    ) ?? null;
  }

  findPersistedConfigChange(nodeId: string, space: number, offset: string): PendingChangeRow | null {
    return this._persistedRows.find(
      (r) =>
        r.kind === 'config' &&
        isWaitingPendingChange(r) &&
        normalizeNodeId(r.nodeId) === normalizeNodeId(nodeId) &&
        r.space === space &&
        r.offset === offset
    ) ?? null;
  }

  findEffectiveConfigChange(nodeId: string, space: number, offset: string): PendingChangeRow | null {
    return this.effectiveRows.find(
      (r) =>
        r.kind === 'config' &&
        isWaitingPendingChange(r) &&
        normalizeNodeId(r.nodeId) === normalizeNodeId(nodeId) &&
        r.space === space &&
        r.offset === offset
    ) ?? null;
  }

  resolveEffectiveCurrentValue(nodeId: string, leaf: LeafConfigNode): TreeConfigValue | null {
    const effectiveRow = this.findEffectiveConfigChange(nodeId, leaf.space, this.offsetHex(leaf.address));
    if (!effectiveRow) {
      return effectiveValue(leaf);
    }

    return parseOfflineStoredValueForLeaf(leaf, effectiveRow.plannedValue) ?? effectiveValue(leaf);
  }

  findDraftBowtieMetadataChange(eventIdHex: string): PendingChangeRow | null {
    const baselineKey = `event:${eventIdHex}`;
    return this._draftRows.find(
      (r) => r.kind === 'bowtieMetadata' && r.status === 'pending' && r.baselineValue === baselineKey
    ) ?? null;
  }

  findPersistedBowtieMetadataChange(eventIdHex: string): PendingChangeRow | null {
    const baselineKey = `event:${eventIdHex}`;
    return this._persistedRows.find(
      (r) => r.kind === 'bowtieMetadata' && isWaitingPendingChange(r) && r.baselineValue === baselineKey
    ) ?? null;
  }

  hasPersistedConfigChange(nodeId: string, space: number, offset: string): boolean {
    return this.findPersistedConfigChange(nodeId, space, offset) !== null;
  }

  removeRow(changeId: string): void {
    this._draftRows = this._draftRows.filter((r) => r.changeId !== changeId);
  }

  async revertDraftChange(changeId: string): Promise<boolean> {
    this._busy = true;
    try {
      const draftRow = this._draftRows.find((r) => r.changeId === changeId);
      if (!draftRow) {
        return false;
      }

      this._draftRows = this._draftRows.filter((r) => r.changeId !== changeId);
      return true;
    } finally {
      this._busy = false;
    }
  }

  async revertPersistedChange(changeId: string): Promise<boolean> {
    this._busy = true;
    try {
      const wasPersisted = this._persistedRows.some((r) => r.changeId === changeId);
      if (!wasPersisted) {
        return false;
      }

      await revertPendingChange(changeId);
      this._persistedRows = this._persistedRows.filter((r) => r.changeId !== changeId);
      return true;
    } catch {
      return false;
    } finally {
      this._busy = false;
    }
  }

  /**
   * Revert a single pending change back to its captured baseline.
   * Removes the change from the current in-memory working set.
   * Persisted rows are removed from the backend cache, but remain in the
   * saved snapshot until the user explicitly saves or discards.
   */
  async revertToBaseline(changeId: string): Promise<boolean> {
    if (this._draftRows.some((r) => r.changeId === changeId)) {
      return this.revertDraftChange(changeId);
    }

    return this.revertPersistedChange(changeId);
  }

  clear(): void {
    this._persistedRows = [];
    this._savedRows = [];
    this._draftRows = [];
    this._busy = false;
  }

  resetForNewLayout(): void {
    this.clear();
  }

  applyConnectorCompatibilityConfigChanges(nodeId: string, repairs: StagedRepair[]): void {
    for (const repair of repairs) {
      if (repair.space == null || !repair.offset) {
        continue;
      }

      this.upsertConfigChange({
        nodeId,
        space: repair.space,
        offset: repair.offset,
        baselineValue: repair.baselineValue,
        plannedValue: repair.plannedValue,
      });
    }
  }

  /**
   * Remove draft config change rows for the given locations.
   *
   * Used by the connector orchestrator to clear stale connector-repair drafts
   * before recomputing compatibility, so old cancellation rows don't suppress
   * persisted pending values on re-selection.
   */
  clearDraftConfigChanges(nodeId: string, locations: { space: number; offset: string }[]): void {
    if (locations.length === 0) return;
    const normalizedId = normalizeNodeId(nodeId);
    const keySet = new Set(
      locations.map((loc) => `config:${normalizedId}:${loc.space ?? 0}:${loc.offset ?? ''}`),
    );
    const before = this._draftRows.length;
    this._draftRows = this._draftRows.filter((r) => !keySet.has(this.targetKeyForRow(r)));
    if (this._draftRows.length !== before) {
      this._draftRows = [...this._draftRows];
    }
  }

  setBusy(value: boolean): void {
    this._busy = value;
  }

  async reloadFromBackend(): Promise<void> {
    this._busy = true;
    try {
      const rows = await listPendingChanges();
      this._persistedRows = [...rows];
      this._savedRows = [...rows];
      this._draftRows = [];
    } finally {
      this._busy = false;
    }
  }

  async revertAllPending(): Promise<number> {
    const pending = this._draftRows.filter((r) => r.status === 'pending').length;
    // Restore working rows to the last known saved snapshot.
    this._persistedRows = [...this._savedRows];
    this._draftRows = [];
    return pending;
  }

  /**
   * Record this Save's drafts as Pending Change rows in the backend and return
   * the IDs of the rows it recorded (E3b D1: a connected Save writes only
   * these). Rows recorded earlier (including failed / conflict rows) are left
   * untouched; a draft that returns a persisted row to its baseline removes
   * that row. Reloads the rows from the backend afterwards.
   */
  async recordDraftsToBackend(): Promise<string[]> {
    this._busy = true;
    try {
      const recordedIds: string[] = [];
      for (const draft of this._draftRows) {
        if (draft.plannedValue === draft.baselineValue) {
          const persisted = this._persistedRows.find(
            (r) => this.targetKeyForRow(r) === this.targetKeyForRow(draft),
          );
          if (persisted) await revertPendingChange(persisted.changeId);
          continue;
        }
        const id = await setPendingChange({
          kind: draft.kind,
          nodeId: draft.nodeId,
          space: draft.space,
          offset: draft.offset,
          baselineValue: draft.baselineValue,
          plannedValue: draft.plannedValue,
          ...(draft.origin ? { origin: draft.origin } : {}),
        });
        if (draft.kind === 'config') recordedIds.push(id);
      }
      await this.reloadFromBackend();
      return recordedIds;
    } finally {
      this._busy = false;
    }
  }

  private targetKeyForRow(row: PendingChangeRow): string {
    if (row.kind === 'config') {
      return `config:${normalizeNodeId(row.nodeId)}:${row.space ?? 0}:${row.offset ?? ''}`;
    }
    if (row.kind === 'bowtieMetadata') {
      return `bowtieMetadata:${row.baselineValue}`;
    }
    return `${row.kind}:${row.changeId}`;
  }

  private offsetHex(address: number): string {
    return `0x${address.toString(16).toUpperCase().padStart(8, '0')}`;
  }

  private removeDraftByTarget(row: PendingChangeRow): void {
    const key = this.targetKeyForRow(row);
    this._draftRows = this._draftRows.filter((r) => this.targetKeyForRow(r) !== key);
  }

  private currentEffectivePlannedValueForConfigChange(
    nodeId: string,
    space: number,
    offset: string,
    fallbackBaselineValue: string,
  ): string {
    const existingDraft = this.findDraftConfigChange(nodeId, space, offset);
    if (existingDraft) {
      return existingDraft.baselineValue;
    }

    const existingPersisted = this.findPersistedConfigChange(nodeId, space, offset);
    if (existingPersisted) {
      return existingPersisted.baselineValue;
    }

    return fallbackBaselineValue;
  }

}

export const pendingChangesStore = new PendingChangesStore();
