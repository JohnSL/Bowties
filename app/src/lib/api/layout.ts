import { invoke } from '@tauri-apps/api/core';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import type { LayoutFile, LayoutEditDelta } from '$lib/types/bowtie';

export type { LayoutConnectorSelections } from '$lib/types/bowtie';

export interface CaptureSummary {
  capturedAt: string;
  nodeCount: number;
  completeCount: number;
  partialCount: number;
}

export interface SaveLayoutResult {
  manifestPath: string;
  nodeFilesWritten: number;
  warnings: string[];
  /** The persisted layout file data (ADR-0002: backend returns authoritative copy). */
  layout: LayoutFile;
}

export interface OpenLayoutResult {
  layoutId: string;
  capturedAt: string;
  layout: LayoutFile;
  offlineMode: boolean;
  nodeCount: number;
  partialNodes: string[];
  pendingOfflineChangeCount: number;
  nodeSnapshots: OfflineNodeSnapshot[];
}

export interface SnapshotLeafValue {
  value: string;
  space?: number;
  offset?: string;
}

export interface SnapshotValueBranch {
  [key: string]: SnapshotValueNode;
}

export type SnapshotValueNode = SnapshotLeafValue | SnapshotValueBranch;

export interface OfflineNodeSnapshot {
  nodeId: string;
  capturedAt: string;
  captureStatus: 'complete' | 'partial';
  missing: string[];
  snip: {
    userName: string;
    userDescription: string;
    manufacturerName: string;
    modelName: string;
  };
  cdiRef: {
    cacheKey: string;
    version: string;
    fingerprint: string;
  };
  config: Record<string, SnapshotValueNode>;
  producerIdentifiedEvents: string[];
}

export type CloseLayoutDecision = 'save' | 'discard' | 'cancel';

export interface CloseLayoutResult {
  closed: boolean;
  reason?: string;
}

export interface NewLayoutResult {
  layoutId: string;
  createdAt: string;
}

export interface WriteModifiedResult {
  total: number;
  succeeded: number;
  failed: number;
  readOnlyRejected: number;
}

/** Result of `save_layout_with_bus_writes` — the three-phase save command. */
export interface SaveWithBusWriteResult {
  /** Layout was successfully saved to disk. */
  layoutSaved: boolean;
  /** Bus write outcome (null if offline or no pending writes). */
  busWrites: WriteModifiedResult | null;
  /** Whether a reconcile re-save was performed (≥1 bus write succeeded). */
  reconciled: boolean;
  /** Whether the bowtie catalog was rebuilt by the backend. */
  catalogRebuilt: boolean;
  /** Partial-capture node IDs from the initial layout save. */
  warnings: string[];
  /** The persisted layout file data (ADR-0002: backend returns authoritative copy). */
  layout: LayoutFile;
}

export async function captureLayoutSnapshot(includeProducerEvents = true): Promise<CaptureSummary> {
  return invoke<CaptureSummary>('capture_layout_snapshot', { includeProducerEvents });
}

export async function saveLayoutDirectory(
  path: string,
  overwrite = true,
  deltas: LayoutEditDelta[] = [],
): Promise<SaveLayoutResult> {
  return invoke<SaveLayoutResult>('save_layout_directory', { path, overwrite, deltas });
}

export async function saveLayoutFile(
  path: string,
  overwrite = true,
  deltas: LayoutEditDelta[] = [],
): Promise<SaveLayoutResult> {
  return invoke<SaveLayoutResult>('save_layout_directory', { path, overwrite, deltas });
}

/** Three-phase save: layout first, then bus writes (if connected), then reconcile. */
export async function saveLayoutWithBusWrites(
  path: string,
  deltas: LayoutEditDelta[] = [],
  overwrite = true,
): Promise<SaveWithBusWriteResult> {
  return invoke<SaveWithBusWriteResult>('save_layout_with_bus_writes', { path, overwrite, deltas });
}

export async function openLayoutDirectory(path: string): Promise<OpenLayoutResult> {
  return invoke<OpenLayoutResult>('open_layout_directory', { path });
}

export async function openLayoutFile(path: string): Promise<OpenLayoutResult> {
  return invoke<OpenLayoutResult>('open_layout_directory', { path });
}

export async function closeLayout(decision: CloseLayoutDecision): Promise<CloseLayoutResult> {
  return invoke<CloseLayoutResult>('close_layout', { decision });
}

export async function createNewLayoutCapture(): Promise<NewLayoutResult> {
  return invoke<NewLayoutResult>('create_new_layout_capture');
}

export async function buildOfflineNodeTree(nodeId: string): Promise<NodeConfigTree> {
  return invoke<NodeConfigTree>('build_offline_node_tree', { nodeId });
}
