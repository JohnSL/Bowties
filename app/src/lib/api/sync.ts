import { invoke } from '@tauri-apps/api/core';

export type PendingChangeKind = 'config' | 'bowtieMetadata' | 'bowtieEvent';

export interface PendingChangeInput {
  kind: PendingChangeKind;
  nodeId?: string;
  space?: number;
  offset?: string;
  baselineValue: string;
  plannedValue: string;
}

export type PendingChangeStatus =
  | 'pending'
  | 'conflict'
  | 'clean'
  | 'alreadyApplied'
  | 'skipped'
  | 'applied'
  | 'failed';

export interface PendingChangeRow {
  changeId: string;
  kind: PendingChangeKind;
  nodeId?: string;
  space?: number;
  offset?: string;
  baselineValue: string;
  plannedValue: string;
  status: PendingChangeStatus;
  error?: string;
  updatedAt?: string;
}

export type LayoutMatchClassification = 'likely_same' | 'uncertain' | 'likely_different';

export interface LayoutMatchStatus {
  overlapPercent: number;
  classification: LayoutMatchClassification;
  expectedThresholds: {
    likelySameMin: 80;
    uncertainMin: 40;
  };
}

export interface SyncRow {
  changeId: string;
  nodeId?: string;
  nodeName?: string;
  fieldLabel?: string;
  baselineValue: string;
  plannedValue: string;
  busValue?: string;
  resolution: 'unresolved' | 'apply' | 'skip';
  error?: string;
}

export interface SyncSession {
  conflictRows: SyncRow[];
  cleanRows: SyncRow[];
  alreadyAppliedCount: number;
  nodeMissingRows: SyncRow[];
}

export type SyncMode = 'target_layout_bus' | 'bench_other_bus';

export interface ApplySyncResult {
  applied: string[];
  skipped: string[];
  failed: Array<{ changeId: string; reason: string }>;
  readOnlyCleared: string[];
}

export async function setPendingChange(change: PendingChangeInput): Promise<string> {
  return invoke<string>('set_pending_change', { change });
}

export async function revertPendingChange(changeId: string): Promise<{ removed: boolean }> {
  const removed = await invoke<boolean>('revert_pending_change', { changeId });
  return { removed };
}

export async function listPendingChanges(): Promise<PendingChangeRow[]> {
  return invoke<PendingChangeRow[]>('list_pending_changes');
}

export async function computeLayoutMatchStatus(discoveredNodeIds: string[]): Promise<LayoutMatchStatus> {
  return invoke<LayoutMatchStatus>('compute_layout_match_status', { discoveredNodeIds });
}

export async function buildSyncSession(): Promise<SyncSession> {
  return invoke<SyncSession>('build_sync_session');
}

export async function setSyncMode(mode: SyncMode): Promise<{ mode: string }> {
  const selectedMode = await invoke<string>('set_sync_mode', { mode });
  return { mode: selectedMode };
}

export async function applySyncChanges(applyChangeIds: string[], skipChangeIds: string[]): Promise<ApplySyncResult> {
  return invoke<ApplySyncResult>('apply_sync_changes', { applyChangeIds, skipChangeIds });
}
