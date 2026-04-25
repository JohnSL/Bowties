/**
 * Svelte 5 reactive store for the Sync Panel (US4).
 *
 * Manages sync session state: conflict resolution tracking, clean row
 * deselection, bus-match classification, sync mode, and apply lifecycle.
 */

import type {
  LayoutMatchStatus,
  SyncSession,
  SyncRow,
  SyncMode,
  ApplySyncResult,
} from '$lib/api/sync';
import {
  computeLayoutMatchStatus,
  buildSyncSession,
  setSyncMode as setSyncModeIpc,
  applySyncChanges,
} from '$lib/api/sync';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';

export type ConflictResolution = 'apply' | 'skip';

class SyncPanelStore {
  // ── State ─────────────────────────────────────────────────────────────────

  private _session = $state<SyncSession | null>(null);
  private _matchStatus = $state<LayoutMatchStatus | null>(null);
  private _syncMode = $state<SyncMode | null>(null);
  private _resolutions = $state<Map<string, ConflictResolution>>(new Map());
  private _deselected = $state<Set<string>>(new Set());
  private _applying = $state<boolean>(false);
  private _applyResult = $state<ApplySyncResult | null>(null);
  private _loading = $state<boolean>(false);
  private _error = $state<string | null>(null);
  /** True once the sync panel has been shown and dismissed or applied. */
  private _dismissed = $state<boolean>(false);

  // ── Reactive getters ──────────────────────────────────────────────────────

  get session(): SyncSession | null {
    return this._session;
  }

  get matchStatus(): LayoutMatchStatus | null {
    return this._matchStatus;
  }

  get syncMode(): SyncMode | null {
    return this._syncMode;
  }

  get isApplying(): boolean {
    return this._applying;
  }

  get isLoading(): boolean {
    return this._loading;
  }

  get applyResult(): ApplySyncResult | null {
    return this._applyResult;
  }

  get error(): string | null {
    return this._error;
  }

  get isDismissed(): boolean {
    return this._dismissed;
  }

  /** True when the sync panel should be visible. */
  get isActive(): boolean {
    return this._session !== null && !this._dismissed;
  }

  /** True when all conflicts have been resolved. */
  get allConflictsResolved(): boolean {
    if (!this._session) return true;
    return this._session.conflictRows.every(
      (row) => this._resolutions.has(row.changeId)
    );
  }

  /** Number of clean rows that are still selected (not deselected). */
  get selectedCleanCount(): number {
    if (!this._session) return 0;
    return this._session.cleanRows.filter(
      (row) => !this._deselected.has(row.changeId)
    ).length;
  }

  /** Total rows that will be applied on click. */
  get applyCount(): number {
    if (!this._session) return 0;
    let count = 0;
    for (const row of this._session.conflictRows) {
      if (this._resolutions.get(row.changeId) === 'apply') count++;
    }
    count += this.selectedCleanCount;
    return count;
  }

  /** True when the Apply button should be enabled. */
  get canApply(): boolean {
    return (
      this.allConflictsResolved &&
      !this._applying &&
      this.applyCount > 0
    );
  }

  get conflictRows(): SyncRow[] {
    return this._session?.conflictRows ?? [];
  }

  get cleanRows(): SyncRow[] {
    return this._session?.cleanRows ?? [];
  }

  get alreadyAppliedCount(): number {
    return this._session?.alreadyAppliedCount ?? 0;
  }

  get nodeMissingRows(): SyncRow[] {
    return this._session?.nodeMissingRows ?? [];
  }

  // ── Resolution tracking ───────────────────────────────────────────────────

  getResolution(changeId: string): ConflictResolution | undefined {
    return this._resolutions.get(changeId);
  }

  resolveConflict(changeId: string, choice: ConflictResolution): void {
    const next = new Map(this._resolutions);
    next.set(changeId, choice);
    this._resolutions = next;
  }

  isCleanRowDeselected(changeId: string): boolean {
    return this._deselected.has(changeId);
  }

  toggleCleanRow(changeId: string): void {
    const next = new Set(this._deselected);
    if (next.has(changeId)) {
      next.delete(changeId);
    } else {
      next.add(changeId);
    }
    this._deselected = next;
  }

  selectAllClean(): void {
    this._deselected = new Set();
  }

  deselectAllClean(): void {
    if (!this._session) return;
    this._deselected = new Set(this._session.cleanRows.map((r) => r.changeId));
  }

  // ── IPC operations ────────────────────────────────────────────────────────

  /** Compute preliminary bus-to-layout match from discovered node IDs. */
  async computeMatch(discoveredNodeIds: string[]): Promise<void> {
    this._loading = true;
    this._error = null;
    try {
      this._matchStatus = await computeLayoutMatchStatus(discoveredNodeIds);
    } catch (e) {
      this._error = e instanceof Error ? e.message : String(e);
    } finally {
      this._loading = false;
    }
  }

  /** Set user-selected sync mode for uncertain/different matches. */
  async setMode(mode: SyncMode): Promise<void> {
    try {
      await setSyncModeIpc(mode);
      this._syncMode = mode;
    } catch (e) {
      this._error = e instanceof Error ? e.message : String(e);
    }
  }

  /** Build the sync session from pending offline changes vs live bus values. */
  async loadSession(): Promise<void> {
    this._loading = true;
    this._error = null;
    this._resolutions = new Map();
    this._deselected = new Set();
    this._applyResult = null;
    this._dismissed = false;
    try {
      this._session = await buildSyncSession();
      if ((this._session?.alreadyAppliedCount ?? 0) > 0) {
        await offlineChangesStore.reloadFromBackend();
      }
    } catch (e) {
      this._error = e instanceof Error ? e.message : String(e);
    } finally {
      this._loading = false;
    }
  }

  /** Apply resolved conflicts and selected clean rows to the bus. */
  async applySelected(): Promise<ApplySyncResult | null> {
    if (!this._session || !this.canApply) return null;

    this._applying = true;
    this._error = null;
    try {
      const applyIds: string[] = [];
      const skipIds: string[] = [];

      // Conflict rows
      for (const row of this._session.conflictRows) {
        const resolution = this._resolutions.get(row.changeId);
        if (resolution === 'apply') {
          applyIds.push(row.changeId);
        } else {
          skipIds.push(row.changeId);
        }
      }

      // Clean rows
      for (const row of this._session.cleanRows) {
        if (!this._deselected.has(row.changeId)) {
          applyIds.push(row.changeId);
        } else {
          skipIds.push(row.changeId);
        }
      }

      const result = await applySyncChanges(applyIds, skipIds);
      this._applyResult = result;
      return result;
    } catch (e) {
      this._error = e instanceof Error ? e.message : String(e);
      return null;
    } finally {
      this._applying = false;
    }
  }

  /** Mark the sync panel as dismissed (user chose to close/skip). */
  dismiss(): void {
    this._dismissed = true;
  }

  /** Full reset — call when closing layout or starting fresh. */
  reset(): void {
    this._session = null;
    this._matchStatus = null;
    this._syncMode = null;
    this._resolutions = new Map();
    this._deselected = new Set();
    this._applying = false;
    this._applyResult = null;
    this._loading = false;
    this._error = null;
    this._dismissed = false;
  }
}

export const syncPanelStore = new SyncPanelStore();
