import { invoke } from '@tauri-apps/api/core';
import type { NodeConfigTree } from '$lib/types/nodeTree';
import type { LayoutFile, LayoutEditDelta } from '$lib/types/bowtie';
import { toCanonicalNodeKey, type NodeKeyInput } from '$lib/utils/nodeKey';

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
  /**
   * Canonical (uppercase, no-dots) node IDs of every snapshot written to
   * the companion `nodes/` directory after this save (S8). Frontend uses
   * this to distinguish saved nodes from unsaved discovered nodes.
   */
  persistedNodeIds: string[];
  /** Node snapshots written to disk. Cached by the page so disconnect can
   *  rehydrate the offline view without re-opening the layout. */
  nodeSnapshots: OfflineNodeSnapshot[];
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
  /**
   * True when the layout journal rolled back an interrupted prior
   * save before this open (ADR-0006). UI should surface a one-line
   * notice that the previous save was incomplete and has been
   * restored.
   */
  recoveryOccurred: boolean;
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
  /** Authoritative identity — canonical NodeID for real nodes, `"placeholder:<uuid>"` for placeholders. */
  nodeKey: string;
  /** Canonical dotted-hex NodeID. Present for real nodes, absent for placeholders. */
  nodeId?: string;
  /** Bundled profile stem (e.g. `"Mustangpeak-Engineering_TurnoutBoss"`). Present for placeholders only. */
  profileStem?: string;
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
  /** Canonical node IDs persisted on disk after this save (S8). */
  persistedNodeIds: string[];
  /** Node snapshots written to disk. Cached by the page so disconnect can
   *  rehydrate the offline view without re-opening the layout. */
  nodeSnapshots: OfflineNodeSnapshot[];
}

export async function captureLayoutSnapshot(includeProducerEvents = true): Promise<CaptureSummary> {
  return invoke<CaptureSummary>('capture_layout_snapshot', { includeProducerEvents });
}

/** Persist the layout to its companion directory (writes layout file + node snapshots). */
export async function saveLayoutDirectory(
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

/** Open a layout from its `.layout` manifest path. */
export async function openLayoutDirectory(path: string): Promise<OpenLayoutResult> {
  return invoke<OpenLayoutResult>('open_layout_directory', { path });
}

export async function closeLayout(decision: CloseLayoutDecision): Promise<CloseLayoutResult> {
  return invoke<CloseLayoutResult>('close_layout', { decision });
}

export async function createNewLayoutCapture(): Promise<NewLayoutResult> {
  return invoke<NewLayoutResult>('create_new_layout_capture');
}

export async function buildOfflineNodeTree(nodeId: NodeKeyInput): Promise<NodeConfigTree> {
  return invoke<NodeConfigTree>('build_offline_node_tree', { nodeId: toCanonicalNodeKey(nodeId) });
}

/**
 * Persist a single Configuration Mode variant selection for a node into the
 * active layout's `nodeModeSelections` map (Spec 014 / S6).
 *
 * `nodeKey` may be a canonical NodeID (uppercase, no dots) or a
 * `placeholder:<uuidv4>` key. Backend immediately writes through to disk
 * and returns the updated `SaveLayoutResult`.
 */
export async function setNodeModeSelection(
  nodeKey: NodeKeyInput,
  modeId: string,
  variantId: string,
): Promise<SaveLayoutResult> {
  return invoke<SaveLayoutResult>('set_node_mode_selection', { nodeKey: toCanonicalNodeKey(nodeKey), modeId, variantId });
}

// ── Placeholder boards (Spec 014 / S8) ────────────────────────────────────

/**
 * Picker-ready summary of a bundled board-model profile. Matches the Rust
 * `BundledProfileSummary` struct on the wire (camelCase via serde).
 */
export interface BundledProfileSummary {
  /** Profile filename stem (e.g. `"RR-CirKits_Tower-LCC"`); FR-019 identity. */
  stem: string;
  manufacturer: string;
  model: string;
}

/**
 * List every bundled board-model profile available for placeholder creation.
 *
 * Returns summaries sorted by `(manufacturer, model)`. Malformed bundle
 * entries are silently skipped backend-side, so this call never throws on
 * a single bad profile.
 */
export async function listBundledProfiles(): Promise<BundledProfileSummary[]> {
  return invoke<BundledProfileSummary[]>('list_bundled_profiles_command');
}

/**
 * Add a placeholder board by synthesizing it from a bundled profile
 * (Spec 014 / S8.10).
 *
 * Calls the backend factory, which mints a `placeholder:<uuid>` key,
 * loads the bundled CDI, builds the config tree, and inserts a
 * `Synthesized` proxy into the node registry. Returns the minted
 * `nodeKey` so the frontend can seed its roster and route to it.
 */
export async function addPlaceholderBoardIpc(
  profileStem: string,
): Promise<{ nodeKey: string }> {
  return invoke<{ nodeKey: string }>('add_placeholder_board', { profileStem });
}

/**
 * Read the unified config tree for a node (Spec 007 / S8.10).
 *
 * Dispatches through the backend registry uniformly — both live nodes and
 * synthesized placeholders are resolved via the same `get_node_tree` IPC.
 */
export async function getNodeTree(
  nodeKey: NodeKeyInput,
): Promise<NodeConfigTree> {
  return invoke<NodeConfigTree>('get_node_tree', { nodeId: toCanonicalNodeKey(nodeKey) });
}

// ── Per-layout connection registry (Spec 013 / S4) ─────────────────────────

/** Adapter / transport variant matching the Rust `AdapterType` enum. */
export type LayoutAdapterType = 'tcp' | 'gridConnectSerial' | 'slcanSerial';

/** Hardware flow control mode matching the Rust `FlowControl` enum. */
export type LayoutFlowControl = 'none' | 'rtsCts';

/**
 * A saved connection entry persisted inside a layout manifest's
 * `connections` field. Mirrors the Rust `ConnectionConfig` type
 * (camelCase via serde).
 */
export interface LayoutConnectionConfig {
  id: string;
  name: string;
  adapterType: LayoutAdapterType;
  host?: string | null;
  port?: number | null;
  serialPort?: string | null;
  baudRate?: number | null;
  flowControl?: LayoutFlowControl;
}

/** Read the saved connections list from a layout's manifest file. */
export async function getLayoutConnections(path: string): Promise<LayoutConnectionConfig[]> {
  return invoke<LayoutConnectionConfig[]>('get_layout_connections', { path });
}

/** Replace the saved connections list on a layout's manifest file. */
export async function saveLayoutConnections(
  path: string,
  connections: LayoutConnectionConfig[],
): Promise<void> {
  await invoke('save_layout_connections', { path, connections });
}
