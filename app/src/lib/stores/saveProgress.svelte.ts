/**
 * saveProgressStore — phase tracker for the three-phase save flow.
 *
 * Spec 013 / S3. Mirrors the `save-progress` events emitted by the backend
 * `save_layout_with_bus_writes` command (see `commands/layout_capture.rs`):
 *
 *   - `saving-layout`   — phase 1, layout-file write
 *   - `writing-config`  — phase 2, per-field bus writes (with current/total)
 *   - `reconciling`     — phase 3, post-bus-write layout re-save
 *   - `complete`        — final phase, includes `failedCount`
 *
 * The store also exposes direct setters so the frontend orchestrator can drive
 * the offline-save path (which does not go through `save_layout_with_bus_writes`
 * and therefore does not emit backend `save-progress` events).
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
  | 'error';

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
  private _unlisten: UnlistenFn | null = null;

  get phase(): SavePhase { return this._phase; }
  get busWriteCurrent(): number { return this._busWriteCurrent; }
  get busWriteTotal(): number { return this._busWriteTotal; }
  get currentLabel(): string | null { return this._currentLabel; }
  get failedCount(): number { return this._failedCount; }
  get errorMessage(): string | null { return this._errorMessage; }

  /** True when a save is in progress (phase neither idle nor complete). */
  get isActive(): boolean {
    return this._phase !== 'idle' && this._phase !== 'complete' && this._phase !== 'error';
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

  /** Mark the save as started (used by the orchestrator for offline saves). */
  begin(): void {
    this._phase = 'saving-layout';
    this._busWriteCurrent = 0;
    this._busWriteTotal = 0;
    this._currentLabel = null;
    this._failedCount = 0;
    this._errorMessage = null;
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
