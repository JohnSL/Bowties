import type { CloseLayoutResult, OpenLayoutResult, OfflineNodeSnapshot, SnapshotValueNode } from '$lib/api/layout';
import type { DiscoveredNode } from '$lib/api/tauri';
import type { ActiveLayoutMode } from '$lib/stores/layout.svelte';
import type { ConfigNode, NodeConfigTree, TreeConfigValue } from '$lib/types/nodeTree';
import { layoutStore } from '$lib/stores/layout.svelte';
import { offlineChangesStore } from '$lib/stores/offlineChanges.svelte';
import {
  failLayoutOpen,
  finishLayoutHydration,
  finishOfflineReplay,
  startLayoutHydration,
  startLayoutOpen,
  startOfflineReplay,
} from '$lib/stores/layoutOpenLifecycle';

interface OpenOfflineLayoutWithReplayArgs {
  path: string;
  openLayout: (path: string) => Promise<OpenLayoutResult>;
  hydrateOfflineSnapshots: (snapshots: OfflineNodeSnapshot[]) => Promise<void>;
  applyPersistedOfflinePendingToTrees: () => void;
  onOpened?: () => void;
}

interface ClearActiveLayoutWithResetArgs {
  activeLayoutMode: ActiveLayoutMode | null | undefined;
  closeLayout: (decision: 'discard') => Promise<CloseLayoutResult>;
  clearRecentLayout: () => Promise<void>;
  resetLayoutState: () => Promise<void>;
  onRecentLayoutClearError?: (error: unknown) => void;
}

interface BuildOfflineTreesFromSnapshotsArgs {
  snapshots: OfflineNodeSnapshot[];
  buildOfflineNodeTree: (nodeId: string) => Promise<NodeConfigTree>;
  onTreeBuildWarning?: (message: string) => void;
}

interface RehydrateOfflineStateFromSnapshotsArgs extends BuildOfflineTreesFromSnapshotsArgs {
  nodeIdStringToBytes: (nodeId: string) => number[];
  publishNodes: (nodes: DiscoveredNode[]) => void;
  clearConfigReadStatus: () => void;
  resetNodeTrees: () => void;
  setTree: (nodeId: string, tree: NodeConfigTree) => void;
  markNodeConfigRead: (nodeId: string) => void;
}

interface ResetLayoutStateForNoLayoutArgs {
  connected: boolean;
  reprobeLiveNodes?: boolean;
  clearPartialCaptureNodes: () => void;
  clearCurrentLayoutSnapshots: () => void;
  clearNodes: () => void;
  clearConfigReadStatus: () => void;
  resetNodeTrees: () => void;
  clearMetadata: () => void;
  clearOfflineChanges: () => void;
  resetLayoutStore: () => void;
  resetSyncSessionAutoTrigger: () => void;
  probeForNodes: () => Promise<void>;
}

interface ResetFreshLiveSessionStateArgs {
  hasLayoutFile: boolean;
  clearNodes: () => void;
  clearConfigReadStatus: () => void;
  resetSidebar: () => void;
  resetNodeTrees: () => void;
  clearNodesWithCdi: () => void;
}

interface RestoreRecentOfflineLayoutArgs {
  getRecentLayout: () => Promise<{ path?: string | null } | null>;
  restoreLayout: (path: string) => Promise<OpenLayoutResult>;
  clearRecentLayout: () => Promise<void>;
  resetLayoutStateForNoLayout: (reprobeLiveNodes?: boolean) => Promise<void>;
  resetLayoutOpenPhase: () => void;
  onRestored: (result: OpenLayoutResult) => void;
  onWarning: (message: string, error: unknown) => void;
}

export function buildOfflineDiscoveryNodes(
  snapshots: OfflineNodeSnapshot[],
  nodeIdStringToBytes: (nodeId: string) => number[],
): DiscoveredNode[] {
  return snapshots.map((snapshot, index) => ({
    node_id: nodeIdStringToBytes(snapshot.nodeId),
    alias: 0x700 + index,
    snip_data: {
      manufacturer: snapshot.snip.manufacturerName,
      model: snapshot.snip.modelName,
      hardware_version: '',
      software_version: snapshot.cdiRef.version,
      user_name: snapshot.snip.userName,
      user_description: snapshot.snip.userDescription,
    },
    snip_status: 'Complete',
    connection_status: 'Unknown',
    last_verified: null,
    last_seen: snapshot.capturedAt,
    cdi: null,
    pip_flags: null,
    pip_status: 'Unknown',
  }));
}

export async function buildOfflineTreesFromSnapshots({
  snapshots,
  buildOfflineNodeTree,
  onTreeBuildWarning,
}: BuildOfflineTreesFromSnapshotsArgs): Promise<NodeConfigTree[]> {
  const results = await Promise.allSettled(
    snapshots.map((snapshot) => buildOfflineNodeTree(snapshot.nodeId)),
  );

  return snapshots.map((snapshot, index) => {
    const result = results[index];
    if (result?.status === 'fulfilled') {
      return result.value;
    }

    const reason = String(result?.status === 'rejected' ? result.reason : 'Unknown error');
    if (reason.includes('CDI not in cache')) {
      onTreeBuildWarning?.(
        `[offline] CDI not cached for node ${snapshot.nodeId} — falling back to raw address tree`,
      );
    } else {
      onTreeBuildWarning?.(
        `[offline] Could not build CDI tree for node ${snapshot.nodeId}: ${reason}`,
      );
    }

    return treeFromSnapshot(snapshot);
  });
}

export async function rehydrateOfflineStateFromSnapshots({
  snapshots,
  nodeIdStringToBytes,
  buildOfflineNodeTree,
  publishNodes,
  clearConfigReadStatus,
  resetNodeTrees,
  setTree,
  markNodeConfigRead,
  onTreeBuildWarning,
}: RehydrateOfflineStateFromSnapshotsArgs): Promise<void> {
  const offlineNodes = buildOfflineDiscoveryNodes(snapshots, nodeIdStringToBytes);
  publishNodes(offlineNodes);
  clearConfigReadStatus();
  resetNodeTrees();

  const trees = await buildOfflineTreesFromSnapshots({
    snapshots,
    buildOfflineNodeTree,
    onTreeBuildWarning,
  });

  for (const tree of trees) {
    setTree(tree.nodeId, tree);
    markNodeConfigRead(tree.nodeId);
  }
}

export async function resetLayoutStateForNoLayout({
  connected,
  reprobeLiveNodes = true,
  clearPartialCaptureNodes,
  clearCurrentLayoutSnapshots,
  clearNodes,
  clearConfigReadStatus,
  resetNodeTrees,
  clearMetadata,
  clearOfflineChanges,
  resetLayoutStore,
  resetSyncSessionAutoTrigger,
  probeForNodes,
}: ResetLayoutStateForNoLayoutArgs): Promise<void> {
  clearPartialCaptureNodes();
  clearCurrentLayoutSnapshots();
  clearNodes();
  clearConfigReadStatus();
  resetNodeTrees();
  clearMetadata();
  clearOfflineChanges();
  resetLayoutStore();
  resetSyncSessionAutoTrigger();

  if (connected && reprobeLiveNodes) {
    await probeForNodes();
  }
}

export function resetFreshLiveSessionState({
  hasLayoutFile,
  clearNodes,
  clearConfigReadStatus,
  resetSidebar,
  resetNodeTrees,
  clearNodesWithCdi,
}: ResetFreshLiveSessionStateArgs): void {
  if (hasLayoutFile) return;

  clearNodes();
  clearConfigReadStatus();
  resetSidebar();
  resetNodeTrees();
  clearNodesWithCdi();
}

export async function restoreRecentOfflineLayout({
  getRecentLayout,
  restoreLayout,
  clearRecentLayout,
  resetLayoutStateForNoLayout,
  resetLayoutOpenPhase,
  onRestored,
  onWarning,
}: RestoreRecentOfflineLayoutArgs): Promise<boolean> {
  const recent = await getRecentLayout().catch((error) => {
    onWarning('[layout] Failed to read persisted startup layout:', error);
    return null;
  });

  if (!recent?.path) {
    return false;
  }

  try {
    const restored = await restoreLayout(recent.path);
    onRestored(restored);
    return true;
  } catch (error) {
    onWarning('[layout] Failed to restore startup layout:', error);
    await clearRecentLayout().catch((clearError) => {
      onWarning('[layout] Failed to clear invalid startup layout:', clearError);
    });
    await resetLayoutStateForNoLayout(false);
    resetLayoutOpenPhase();
    return false;
  }
}

function parseOfflineValue(value: string): TreeConfigValue {
  if (/^[0-9]+$/.test(value)) {
    return { type: 'int', value: parseInt(value, 10) };
  }
  if (/^[0-9]+\.[0-9]+$/.test(value)) {
    return { type: 'float', value: parseFloat(value) };
  }
  if (/^([0-9A-F]{2}\.){7}[0-9A-F]{2}$/i.test(value)) {
    const bytes = value.split('.').map((byte) => parseInt(byte, 16));
    return { type: 'eventId', bytes, hex: value.toUpperCase() };
  }
  return { type: 'string', value };
}

function createOfflineLeaf(
  space: number,
  address: number,
  value: string,
  segIdx: number,
  leafIdx: number,
): ConfigNode {
  const parsed = parseOfflineValue(value);
  const elementType = parsed.type === 'eventId' ? 'eventId' : parsed.type;
  return {
    kind: 'leaf',
    name: `0x${address.toString(16).toUpperCase().padStart(8, '0')}`,
    description: null,
    elementType,
    address,
    size: 8,
    space,
    path: [`seg:${segIdx}`, `elem:${leafIdx}`],
    value: parsed,
    eventRole: null,
    constraints: null,
    actionValue: 0,
    hintSlider: null,
    hintRadio: false,
    modifiedValue: null,
    writeState: null,
    writeError: null,
    readOnly: true,
  };
}

export function treeFromSnapshot(snapshot: OfflineNodeSnapshot): NodeConfigTree {
  const merged = new Map<string, Record<string, string>>();

  const flatten = (
    node: SnapshotValueNode,
    path: string[] = [],
  ): Array<{ path: string[]; value: string; space?: number; offset?: string }> => {
    if (node && typeof node === 'object' && 'value' in node) {
      const leaf = node as { value: string; space?: number; offset?: string };
      return [{ path, value: leaf.value, space: leaf.space, offset: leaf.offset }];
    }

    if (!node || typeof node !== 'object') return [];
    const out: Array<{ path: string[]; value: string; space?: number; offset?: string }> = [];
    for (const [key, value] of Object.entries(node as Record<string, SnapshotValueNode>)) {
      out.push(...flatten(value, [...path, key]));
    }
    return out;
  };

  for (const entry of flatten(snapshot.config ?? {})) {
    if (entry.space === undefined || !entry.offset) continue;
    const spaceKey = String(entry.space);
    const offsets = merged.get(spaceKey) ?? {};
    offsets[entry.offset.toUpperCase()] = entry.value;
    merged.set(spaceKey, offsets);
  }

  const segments = Array.from(merged.entries()).map(([spaceKey, offsets], segIdx) => {
    const space = Number(spaceKey);
    const children: ConfigNode[] = Object.entries(offsets)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([offset, value], leafIdx) => {
        const address = Number.parseInt(offset.replace(/^0x/i, ''), 16) || 0;
        return createOfflineLeaf(space, address, value, segIdx, leafIdx);
      });

    return {
      name: `Space ${space}`,
      description: null,
      origin: 0,
      space,
      children,
    };
  });

  return {
    nodeId: snapshot.nodeId.match(/.{1,2}/g)?.join('.') ?? snapshot.nodeId,
    identity: {
      manufacturer: snapshot.snip.manufacturerName || null,
      model: snapshot.snip.modelName || null,
      hardwareVersion: null,
      softwareVersion: null,
    },
    segments,
  };
}

export async function clearActiveLayoutWithReset({
  activeLayoutMode,
  closeLayout,
  clearRecentLayout,
  resetLayoutState,
  onRecentLayoutClearError,
}: ClearActiveLayoutWithResetArgs): Promise<boolean> {
  if (activeLayoutMode === 'offline_file') {
    const result = await closeLayout('discard');
    if (!result.closed) return false;
  } else {
    try {
      await clearRecentLayout();
    } catch (error) {
      onRecentLayoutClearError?.(error);
    }
  }

  await resetLayoutState();
  return true;
}

export async function openOfflineLayoutWithReplay({
  path,
  openLayout,
  hydrateOfflineSnapshots,
  applyPersistedOfflinePendingToTrees,
  onOpened,
}: OpenOfflineLayoutWithReplayArgs): Promise<OpenLayoutResult> {
  startLayoutOpen();

  try {
    const result = await openLayout(path);

    startLayoutHydration();
    await hydrateOfflineSnapshots(result.nodeSnapshots);
    finishLayoutHydration();

    layoutStore.setActiveContext({
      layoutId: result.layoutId,
      rootPath: path,
      mode: 'offline_file',
      capturedAt: result.capturedAt,
      pendingOfflineChangeCount: result.pendingOfflineChangeCount,
    });

    startOfflineReplay();
    await offlineChangesStore.reloadFromBackend();
    applyPersistedOfflinePendingToTrees();
    onOpened?.();
    finishOfflineReplay();

    return result;
  } catch (error) {
    failLayoutOpen();
    throw error;
  }
}