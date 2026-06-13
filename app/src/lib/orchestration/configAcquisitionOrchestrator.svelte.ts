/**
 * configAcquisitionOrchestrator — deep owner of the "acquire a node's
 * configuration" workflow.
 *
 * One workflow, one owner. Reading a node's configuration means:
 *   1. Preflight: check the local CDI cache for every candidate node.
 *   2. If any node is missing CDI, surface the missing-CDI download dialog and
 *      stash the pending nodes; on download, fetch CDI then read config.
 *   3. Otherwise read config values immediately, reporting per-node progress.
 *   4. Support cancellation of an in-flight read.
 *
 * The CDI *download* dialog is a sub-step of this workflow, not a separate
 * concern — hence it lives here rather than in the CDI-inspection owner.
 *
 * State is owned internally as `$state` and exposed via reactive getters; the
 * route subscribes and delegates intent (`readRemaining`, `readSingleNode`,
 * `cancel`, `downloadMissingCdi`, `cancelDownload`). All sequencing is hidden
 * behind that narrow interface — there are no cross-orchestrator calls.
 *
 * Dependencies are injected via the constructor so the workflow stays
 * decoupled from Tauri/stores and is unit-testable with plain mocks.
 *
 * Boundary note: the shared "which nodes have cached CDI" set lives in
 * `cdiCacheStore` (spec S6) — its single owner. This workflow merges newly
 * cached nodes into the store (preflight + post-download); the refresh
 * reconciler drops stale nodes; the native-menu effect reads it. Tests reset
 * the store between cases.
 */

import type { DiscoveredNode } from '$lib/api/tauri';
import type { NodeReadState, ReadAllConfigValuesResponse, ReadProgressState } from '$lib/api/types';
import type { GetCdiXmlResponse } from '$lib/types/cdi';
import { canonicalizeNodeId } from '$lib/utils/nodeRoster';
import { formatNodeId } from '$lib/utils/nodeId';
import { cdiCacheStore } from '$lib/stores/cdiCache.svelte';
import {
  createWaitingNodeReadStates,
  executeConfigReadCandidates,
  getUnreadConfigEligibleNodes,
  resolveConfigReadPreflight,
  type ConfigReadNodeCandidate,
} from './configReadOrchestrator';

export type ConfigReadPhase = 'reading' | 'building-catalog' | 'complete' | 'cancelled';

/** A node missing CDI in the local cache, queued for download. */
export interface CdiNodeCandidate {
  nodeId: string;
  nodeName: string;
  downloadStatus?: 'waiting' | 'downloading' | 'done' | 'failed';
}

export interface ConfigAcquisitionDeps {
  /** Current roster of discovered nodes (route's reactive `nodes` projection). */
  getNodes: () => DiscoveredNode[];
  /** Canonical IDs of nodes already config-read. */
  getReadNodeIds: () => Set<string>;
  getCdiXml: (nodeId: string) => Promise<GetCdiXmlResponse>;
  downloadCdi: (nodeId: string) => Promise<GetCdiXmlResponse>;
  readAllConfigValues: (
    nodeId: string,
    nodeIndex: number,
    totalNodes: number,
  ) => Promise<ReadAllConfigValuesResponse>;
  cancelConfigReading: () => Promise<void>;
  markNodeConfigRead: (nodeId: string) => void;
  /** Reload a node's tree after a direct read (preserves expansion). */
  refreshTree: (nodeId: string) => Promise<unknown>;
  /** Load a node's tree after a post-download read (fresh build). */
  loadTree: (nodeId: string) => Promise<unknown>;
  recomputeConnectorCompatibility: (nodeId: string) => Promise<void> | void;
  /** Surface a workflow error to the route's error banner (route-owned). */
  setErrorMessage: (message: string) => void;
  warn?: (message: string, error?: unknown) => void;
}

type ProgressUpdate =
  | { type: 'close' }
  | { type: 'building-catalog'; readProgress: ReadProgressState }
  | { type: 'reading'; nodeReadStates: NodeReadState[]; readProgress: ReadProgressState };

/**
 * Pure projection of a backend `config-read-progress` payload onto the
 * per-node progress array. Terminal statuses close the progress UI.
 */
export function computeConfigReadProgressUpdate(
  currentNodeReadStates: NodeReadState[],
  payload: ReadProgressState,
): ProgressUpdate {
  if (payload.status.type === 'Cancelled' || payload.status.type === 'Complete') {
    return { type: 'close' };
  }

  if (payload.status.type === 'BuildingCatalog') {
    return { type: 'building-catalog', readProgress: payload };
  }

  let nodeReadStates = currentNodeReadStates;
  if (currentNodeReadStates.length > 0) {
    const idx = payload.currentNodeIndex;
    nodeReadStates = currentNodeReadStates.map((state, index) => {
      if (index < idx) return { ...state, status: 'complete', percentage: 100 };
      if (index === idx) {
        if (payload.status.type === 'NodeComplete') {
          return { ...state, status: 'complete', percentage: 100 };
        }
        return { ...state, status: 'reading', percentage: payload.percentage };
      }
      return state;
    });
  }

  return { type: 'reading', nodeReadStates, readProgress: payload };
}

/** Mark every queued download node as waiting before a batch begins. */
export function createWaitingCdiDownloadNodes(nodes: CdiNodeCandidate[]): CdiNodeCandidate[] {
  return nodes.map((node) => ({ ...node, downloadStatus: 'waiting' }));
}

/** Set a single download node's status by index, leaving the rest untouched. */
export function updateCdiDownloadNodeStatus(
  nodes: CdiNodeCandidate[],
  index: number,
  downloadStatus: NonNullable<CdiNodeCandidate['downloadStatus']>,
): CdiNodeCandidate[] {
  return nodes.map((node, nodeIndex) => (
    nodeIndex === index ? { ...node, downloadStatus } : node
  ));
}

/**
 * After a CDI download batch, resolve which nodes should now be config-read:
 * every still-pending node whose CDI is now cached. Falls back to the
 * just-downloaded nodes when no broader pending set was recorded.
 */
export function resolvePostDownloadReadNodes(args: {
  nodesToDownload: CdiNodeCandidate[];
  nodesWithCdi: ReadonlySet<string>;
  pendingConfigNodes: CdiNodeCandidate[];
}): CdiNodeCandidate[] {
  const allPending = args.pendingConfigNodes.length > 0
    ? args.pendingConfigNodes
    : args.nodesToDownload;
  return allPending.filter((node) => args.nodesWithCdi.has(node.nodeId));
}

export class ConfigAcquisitionOrchestrator {
  readonly #deps: ConfigAcquisitionDeps;

  #readProgress = $state<ReadProgressState | null>(null);
  #nodeReadStates = $state<NodeReadState[]>([]);
  #discoveryPhase = $state<ConfigReadPhase>('reading');
  #discoveryModalVisible = $state(false);
  #isCancelling = $state(false);
  #readingRemaining = $state(false);
  #cdiDownloadDialogVisible = $state(false);
  #cdiMissingNodes = $state<CdiNodeCandidate[]>([]);
  #cdiDownloading = $state(false);
  #cdiDownloadedCount = $state(0);
  #pendingConfigNodes = $state<CdiNodeCandidate[]>([]);

  constructor(deps: ConfigAcquisitionDeps) {
    this.#deps = deps;
  }

  // ── Reactive getters ────────────────────────────────────────────────────

  get readProgress(): ReadProgressState | null { return this.#readProgress; }
  get nodeReadStates(): NodeReadState[] { return this.#nodeReadStates; }
  get discoveryPhase(): ConfigReadPhase { return this.#discoveryPhase; }
  get discoveryModalVisible(): boolean { return this.#discoveryModalVisible; }
  get isCancelling(): boolean { return this.#isCancelling; }
  get readingRemaining(): boolean { return this.#readingRemaining; }
  get cdiDownloadDialogVisible(): boolean { return this.#cdiDownloadDialogVisible; }
  get cdiMissingNodes(): CdiNodeCandidate[] { return this.#cdiMissingNodes; }
  get cdiDownloading(): boolean { return this.#cdiDownloading; }
  get cdiDownloadedCount(): number { return this.#cdiDownloadedCount; }
  get pendingConfigNodes(): CdiNodeCandidate[] { return this.#pendingConfigNodes; }

  // ── Public intent ───────────────────────────────────────────────────────

  /** Read config values for all unread, config-eligible nodes (batch). */
  async readRemaining(): Promise<void> {
    const unread = getUnreadConfigEligibleNodes(this.#deps.getNodes(), this.#deps.getReadNodeIds());
    if (unread.length === 0) return;
    this.#beginSession();

    // Phase A: CDI preflight — check the cache for every node before reading,
    // so the download dialog appears before any config read begins.
    const preflight = await resolveConfigReadPreflight(
      unread,
      (nodeId) => this.#hasCachedCdi(nodeId),
      'Cannot read configuration',
    );
    this.#mergeNodesWithCdi(preflight.nodesWithCdi);

    if (preflight.failureMessage) {
      this.#failSession(preflight.failureMessage);
      return;
    }

    if (preflight.missingNodes.length > 0) {
      this.#divertToDownloadDialog(preflight.pendingNodes, preflight.missingNodes);
      return;
    }

    // Phase B: every node has CDI — read config now.
    await this.#runConfigReads(
      preflight.pendingNodes,
      (nodeId) => this.#deps.refreshTree(nodeId),
      'Read remaining failed',
    );
  }

  /** Read config values for a single node (triggered from the sidebar). */
  async readSingleNode(nodeId: string): Promise<void> {
    const canonical = canonicalizeNodeId(nodeId);
    const node = this.#deps.getNodes().find(
      (candidate) => canonicalizeNodeId(formatNodeId(candidate.node_id)) === canonical,
    );
    if (!node?.snip_data) return;
    const nodeName = node.snip_data.user_name || nodeId;
    this.#beginSession([{ nodeId, name: nodeName, percentage: 0, status: 'waiting' }]);

    try {
      const preflight = await resolveConfigReadPreflight(
        [node],
        (candidateNodeId) => this.#hasCachedCdi(candidateNodeId),
        'Cannot read configuration',
      );
      this.#mergeNodesWithCdi(preflight.nodesWithCdi);

      if (preflight.failureMessage) {
        this.#failSession(preflight.failureMessage);
        return;
      }

      if (preflight.missingNodes.length > 0) {
        this.#divertToDownloadDialog(preflight.pendingNodes, preflight.missingNodes);
        return;
      }

      this.#beginSession(createWaitingNodeReadStates(preflight.pendingNodes));
      const execution = await executeConfigReadCandidates({
        nodes: preflight.pendingNodes,
        markNodeConfigRead: this.#deps.markNodeConfigRead,
        readAllConfigValues: this.#deps.readAllConfigValues,
        reloadTree: (candidateNodeId) => this.#deps.refreshTree(candidateNodeId),
        afterReloadTree: (candidateNodeId) => this.#deps.recomputeConnectorCompatibility(candidateNodeId),
        setNodeReadStates: (states) => { this.#nodeReadStates = states; },
        warn: this.#warn,
      });
      const failure = execution.failures.find(
        (entry) => entry.nodeId === nodeId && entry.status === 'failed',
      );
      if (failure?.error) {
        throw new Error(String(failure.error));
      }
      this.#finishSession();
    } catch (e) {
      this.#failSession(`Failed to read config for ${nodeName}: ${e}`);
    }
  }

  /** Request cancellation of an in-flight config read. */
  async cancel(): Promise<void> {
    if (this.#isCancelling) return;
    this.#isCancelling = true;
    try {
      await this.#deps.cancelConfigReading();
    } catch (e) {
      this.#deps.setErrorMessage(`Cancel failed: ${e}`);
      this.#isCancelling = false;
    }
  }

  /** Download CDI for the missing nodes, then read config for those now cached. */
  async downloadMissingCdi(): Promise<void> {
    this.#cdiDownloading = true;
    this.#cdiDownloadedCount = 0;
    const nodesToDownload = [...this.#cdiMissingNodes];
    this.#cdiMissingNodes = createWaitingCdiDownloadNodes(nodesToDownload);

    for (let i = 0; i < nodesToDownload.length; i++) {
      const { nodeId, nodeName } = nodesToDownload[i];
      this.#cdiMissingNodes = updateCdiDownloadNodeStatus(this.#cdiMissingNodes, i, 'downloading');
      try {
        await this.#deps.downloadCdi(nodeId);
        this.#mergeNodesWithCdi(new Set([nodeId]));
        this.#cdiMissingNodes = updateCdiDownloadNodeStatus(this.#cdiMissingNodes, i, 'done');
      } catch (e) {
        this.#warn(`Failed to download CDI for ${nodeName}:`, e);
        this.#cdiMissingNodes = updateCdiDownloadNodeStatus(this.#cdiMissingNodes, i, 'failed');
      }
      this.#cdiDownloadedCount = i + 1;
    }

    this.#cdiDownloadDialogVisible = false;
    this.#cdiDownloading = false;

    // Read config for ALL pending nodes that now have CDI (pre-existing +
    // newly downloaded); fall back to just the downloaded nodes for the
    // single-node flow.
    const nodesToRead = resolvePostDownloadReadNodes({
      nodesToDownload,
      nodesWithCdi: cdiCacheStore.nodes,
      pendingConfigNodes: this.#pendingConfigNodes,
    });
    this.#cdiMissingNodes = [];
    this.#pendingConfigNodes = [];

    if (nodesToRead.length === 0) return;

    this.#beginSession(createWaitingNodeReadStates(nodesToRead));
    try {
      await executeConfigReadCandidates({
        nodes: nodesToRead,
        hasCachedCdi: (nodeId) => this.#hasCachedCdi(nodeId),
        markNodeConfigRead: this.#deps.markNodeConfigRead,
        readAllConfigValues: this.#deps.readAllConfigValues,
        reloadTree: (nodeId) => this.#deps.loadTree(nodeId),
        afterReloadTree: (nodeId) => this.#deps.recomputeConnectorCompatibility(nodeId),
        setNodeReadStates: (states) => { this.#nodeReadStates = states; },
        warn: this.#warn,
      });
      this.#finishSession();
    } catch (e) {
      this.#failSession(`Read config after CDI download failed: ${e}`);
    }
  }

  /** Dismiss the missing-CDI download dialog without downloading. */
  cancelDownload(): void {
    this.#cdiDownloadDialogVisible = false;
    this.#cdiMissingNodes = [];
    this.#pendingConfigNodes = [];
  }

  /** Apply a backend `config-read-progress` payload to the progress UI. */
  applyProgressEvent(payload: ReadProgressState): void {
    const update = computeConfigReadProgressUpdate(this.#nodeReadStates, payload);
    if (update.type === 'close') {
      this.#closeProgressUi();
      return;
    }
    if (update.type === 'building-catalog') {
      this.#discoveryPhase = 'building-catalog';
      this.#readProgress = update.readProgress;
      return;
    }
    this.#discoveryPhase = 'reading';
    this.#nodeReadStates = update.nodeReadStates;
    this.#readProgress = update.readProgress;
  }

  // ── Internal workflow helpers ─────────────────────────────────────────────

  async #hasCachedCdi(nodeId: string): Promise<boolean> {
    const check = await this.#deps.getCdiXml(nodeId);
    return check.xmlContent !== null;
  }

  #mergeNodesWithCdi(additions: Set<string>): void {
    cdiCacheStore.add(additions);
  }

  async #runConfigReads(
    nodes: ConfigReadNodeCandidate[],
    reloadTree: (nodeId: string) => Promise<unknown>,
    failurePrefix: string,
  ): Promise<void> {
    this.#beginSession(createWaitingNodeReadStates(nodes));
    try {
      await executeConfigReadCandidates({
        nodes,
        markNodeConfigRead: this.#deps.markNodeConfigRead,
        readAllConfigValues: this.#deps.readAllConfigValues,
        reloadTree,
        afterReloadTree: (nodeId) => this.#deps.recomputeConnectorCompatibility(nodeId),
        setNodeReadStates: (states) => { this.#nodeReadStates = states; },
        warn: this.#warn,
      });
      this.#finishSession();
    } catch (e) {
      this.#failSession(`${failurePrefix}: ${e}`);
    }
  }

  #beginSession(initialNodeReadStates: NodeReadState[] = []): void {
    this.#discoveryModalVisible = true;
    this.#discoveryPhase = 'reading';
    this.#deps.setErrorMessage('');
    this.#isCancelling = false;
    this.#nodeReadStates = initialNodeReadStates;
    this.#readProgress = null;
    this.#readingRemaining = true;
  }

  #finishSession(): void {
    this.#discoveryModalVisible = false;
    this.#isCancelling = false;
    this.#nodeReadStates = [];
    this.#readProgress = null;
    this.#readingRemaining = false;
  }

  #failSession(message: string): void {
    this.#finishSession();
    this.#deps.setErrorMessage(message);
  }

  #divertToDownloadDialog(
    pendingConfigNodes: ConfigReadNodeCandidate[],
    cdiMissingNodes: ConfigReadNodeCandidate[],
  ): void {
    this.#cdiDownloadDialogVisible = true;
    this.#cdiMissingNodes = cdiMissingNodes;
    this.#discoveryModalVisible = false;
    this.#nodeReadStates = [];
    this.#pendingConfigNodes = pendingConfigNodes;
    this.#readingRemaining = false;
  }

  #closeProgressUi(): void {
    this.#discoveryModalVisible = false;
    this.#isCancelling = false;
    this.#nodeReadStates = [];
    this.#readProgress = null;
  }

  #warn = (message: string, error?: unknown): void => {
    if (this.#deps.warn) {
      this.#deps.warn(message, error);
      return;
    }
    if (error === undefined) {
      console.warn(message);
      return;
    }
    console.warn(message, error);
  };
}
