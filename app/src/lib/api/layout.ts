import { invoke } from '@tauri-apps/api/core';
import type { NodeConfigTree } from '$lib/types/nodeTree';

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
}

export interface OpenLayoutResult {
  layoutId: string;
  capturedAt: string;
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

export async function captureLayoutSnapshot(includeProducerEvents = true): Promise<CaptureSummary> {
  return invoke<CaptureSummary>('capture_layout_snapshot', { includeProducerEvents });
}

export async function saveLayoutDirectory(path: string, overwrite = true): Promise<SaveLayoutResult> {
  return invoke<SaveLayoutResult>('save_layout_directory', { path, overwrite });
}

export async function saveLayoutFile(
  path: string,
  overwrite = true,
): Promise<SaveLayoutResult> {
  return invoke<SaveLayoutResult>('save_layout_directory', { path, overwrite });
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
