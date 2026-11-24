/**
 * saveProgressStore — phase tracker for the three-phase save flow.
 *
 * Spec 013 / S3. Mirrors the `save-progress` events emitted by the backend
 * `save_layout_with_bus_writes` command (see `commands/layout_capture.rs`):
 *
 *   - `saving-layout`   — phase 1, layout-file write
 *   - `writing-config`  — phase 2, bus writes for the rows this Save recorded (current/total)
 *   - `reconciling`     — phase 3, post-bus-write layout re-save
 *   - `complete`        — final phase, includes `failedCount`
 *   - `saved-with-notice` — frontend-only: the Save finished but Pending Changes
 *     are still waiting (failed or recorded earlier); the
 *     dialog stays up with the counts and reasons until dismissed
 *
 * The store also exposes direct setters so the frontend orchestrator can drive
 * the offline-save path (which does not go through `save_layout_with_bus_writes`
 * and therefore does not emit backend `save-progress` events). The backend emits
 * no per-field labels; `writing-config` carries only current/total.
 *
 * Lifecycle:
 *   - `startListening()` registers a Tauri listener; safe to call repeatedly.
 *   - `stopListening()` removes the listener on teardown.
 *
 * The dialog component (`SaveProgressDialog.svelte`) reads `phase`,
 * `busWriteCurrent`, `busWriteTotal`, `currentLabel`, and `failedCount`.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type SavePhase =
  | 'idle'
  | 'saving-layout'
  | 'writing-config'
  | 'reconciling'
  | 'complete'
  | 'saved-with-notice'
  | 'error';

/** Phase 2 outcome of a connected Save (see `SaveWithBusWriteResult`). */
export interface SaveWriteOutcome {
  counts: { failed: number; waiting: number };
  errors: string[];
}

interface SaveProgressPayload {
  phase: SavePhase;
  current?: number;
  total?: number;
  label?: string;
  failedCount?: number;
}

class SaveProgressStore {
  private _phase = $state<SavePhase>('idle');
  private _busWriteCurrent = $state<number>(0);
  private _busWriteTotal = $state<number>(0);
  private _currentLabel = $state<string | null>(null);
  private _failedCount = $state<number>(0);
  private _errorMessage = $state<string | null>(null);
  private _notice = $state<string | null>(null);
  private _noticeDetails = $state<string[]>([]);
  private _unlisten: UnlistenFn | null = null;

  get phase(): SavePhase { return this._phase; }
  get busWriteCurrent(): number { return this._busWriteCurrent; }
  get busWriteTotal(): number { return this._busWriteTotal; }
  get currentLabel(): string | null { return this._currentLabel; }
  get failedCount(): number { return this._failedCount; }
  get errorMessage(): string | null { return this._errorMessage; }
  get notice(): string | null { return this._notice; }
  get noticeDetails(): string[] { return this._noticeDetails; }

  /** True when a save is in progress (phase neither idle nor finished). */
  get isActive(): boolean {
    return (
      this._phase !== 'idle' &&
      this._phase !== 'complete' &&
      this._phase !== 'saved-with-notice' &&
      this._phase !== 'error'
    );
  }

  /** True when the dialog should be displayed. */
  get isVisible(): boolean {
    return this._phase !== 'idle';
  }

  /** Apply a payload (from a backend event or direct call). */
  apply(payload: SaveProgressPayload): void {
    this._phase = payload.phase;
    if (payload.phase === 'writing-config') {
      this._busWriteCurrent = payload.current ?? this._busWriteCurrent;
      this._busWriteTotal = payload.total ?? this._busWriteTotal;
      this._currentLabel = payload.label ?? null;
    }
    if (payload.phase === 'complete') {
      this._failedCount = payload.failedCount ?? 0;
      this._currentLabel = null;
    }
  }

  /**
   * Finish a successful Save. When Pending Changes are still waiting, keep a
   * notice (with the reasons) visible instead of dismissing; the Save never
   * opens the Sync Panel. `outcome` is absent for an offline Save.
   */
  finish(outcome?: SaveWriteOutcome): void {
    const waiting = outcome?.counts.waiting ?? 0;
    if (!outcome || (waiting === 0 && outcome.errors.length === 0)) {
      this.apply({ phase: 'complete', failedCount: 0 });
      return;
    }
    this._failedCount = outcome.counts.failed;
    this._notice =
      waiting === 1
        ? 'Saved. 1 change is still waiting to be written.'
        : `Saved. ${waiting} changes are still waiting to be written.`;
    this._noticeDetails = [...outcome.errors];
    this._currentLabel = null;
    this._phase = 'saved-with-notice';
  }

  /** Mark the save as started (used by the orchestrator for offline saves). */
  begin(): void {
    this._phase = 'saving-layout';
    this._busWriteCurrent = 0;
    this._busWriteTotal = 0;
    this._currentLabel = null;
    this._failedCount = 0;
    this._errorMessage = null;
    this._notice = null;
    this._noticeDetails = [];
  }

  /**
   * Mark the save as failed (used by the orchestrator on caught errors).
   *
   * Pass the user-facing message so the dialog can display it and
   * stay visible until the user dismisses it.
   */
  fail(message?: string | null): void {
    this._phase = 'error';
    this._currentLabel = null;
    this._errorMessage = message ?? null;
  }

  /** Reset the store to idle. */
  reset(): void {
    this._phase = 'idle';
    this._busWriteCurrent = 0;
    this._busWriteTotal = 0;
    this._currentLabel = null;
    this._failedCount = 0;
    this._errorMessage = null;
    this._notice = null;
    this._noticeDetails = [];
  }

  resetForNewLayout(): void {
    this.reset();
  }

  /** Register a Tauri listener for `save-progress`. */
  async startListening(): Promise<void> {
    if (this._unlisten) return;
    this._unlisten = await listen<SaveProgressPayload>('save-progress', (event) => {
      this.apply(event.payload);
    });
  }

  /** Remove the Tauri listener. */
  stopListening(): void {
    if (this._unlisten) {
      this._unlisten();
      this._unlisten = null;
    }
  }
}

export const saveProgressStore = new SaveProgressStore();
