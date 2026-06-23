/**
 * cdiInspectionOrchestrator — owner of the read-only CDI *inspection* surface:
 * the CDI XML viewer modal and the menu-driven CDI re-download dialog.
 *
 * This is deliberately separate from config acquisition: inspecting or
 * re-fetching a node's CDI does not read configuration values. The viewer's
 * load sequencing (cache hit → download fallback → error) is hidden behind
 * `openViewer`; callers only express intent and read reactive getters.
 *
 * State is owned internally as `$state`; dependencies are injected via the
 * constructor so the module stays decoupled from Tauri and unit-testable.
 */

import { getCdiErrorMessage, isCdiError, type GetCdiXmlResponse, type ViewerStatus } from '$lib/types/cdi';

export interface CdiInspectionDeps {
  getCdiXml: (nodeId: string) => Promise<GetCdiXmlResponse>;
  downloadCdi: (nodeId: string) => Promise<GetCdiXmlResponse>;
  /** Resolve a human-readable display name for a node (ADR-0003 edit-layer path). */
  resolveNodeName: (nodeId: string) => string;
}

/**
 * Load CDI XML for the viewer: try the cache, fall back to a download on a
 * `CdiNotRetrieved` miss, and translate any failure into an error state.
 */
export async function loadCdiViewerState(
  nodeId: string,
  getCdiXml: (nodeId: string) => Promise<GetCdiXmlResponse>,
  downloadCdi: (nodeId: string) => Promise<GetCdiXmlResponse>,
): Promise<{ errorMessage: string | null; status: ViewerStatus; xmlContent: string | null }> {
  try {
    let response: GetCdiXmlResponse;
    try {
      response = await getCdiXml(nodeId);
    } catch (cacheError: unknown) {
      if (isCdiError(cacheError, 'CdiNotRetrieved')) {
        response = await downloadCdi(nodeId);
      } else {
        throw cacheError;
      }
    }

    if (response.xmlContent) {
      return { errorMessage: null, status: 'success', xmlContent: response.xmlContent };
    }

    return {
      errorMessage: 'No CDI data available for this node.',
      status: 'error',
      xmlContent: null,
    };
  } catch (error) {
    return { errorMessage: getCdiErrorMessage(error), status: 'error', xmlContent: null };
  }
}

export class CdiInspectionOrchestrator {
  readonly #deps: CdiInspectionDeps;

  #viewerVisible = $state(false);
  #viewerNodeId = $state<string | null>(null);
  #viewerXmlContent = $state<string | null>(null);
  #viewerStatus = $state<ViewerStatus>('idle');
  #viewerErrorMessage = $state<string | null>(null);

  #redownloadVisible = $state(false);
  #redownloadNodeId = $state<string | null>(null);
  #redownloadNodeName = $state<string | null>(null);

  constructor(deps: CdiInspectionDeps) {
    this.#deps = deps;
  }

  // ── Reactive getters ──────────────────────────────────────────────────────

  get viewerVisible(): boolean { return this.#viewerVisible; }
  get viewerNodeId(): string | null { return this.#viewerNodeId; }
  get viewerXmlContent(): string | null { return this.#viewerXmlContent; }
  get viewerStatus(): ViewerStatus { return this.#viewerStatus; }
  get viewerErrorMessage(): string | null { return this.#viewerErrorMessage; }

  get redownloadVisible(): boolean { return this.#redownloadVisible; }
  get redownloadNodeId(): string | null { return this.#redownloadNodeId; }
  get redownloadNodeName(): string | null { return this.#redownloadNodeName; }

  // ── Public intent ─────────────────────────────────────────────────────────

  /** Open the CDI XML viewer for a node and load its content. */
  async openViewer(nodeId: string): Promise<void> {
    this.#viewerVisible = true;
    this.#viewerNodeId = nodeId;
    this.#viewerXmlContent = null;
    this.#viewerStatus = 'loading';
    this.#viewerErrorMessage = 'Checking cache…';

    const loaded = await loadCdiViewerState(nodeId, this.#deps.getCdiXml, this.#deps.downloadCdi);
    this.#viewerXmlContent = loaded.xmlContent;
    this.#viewerStatus = loaded.status;
    this.#viewerErrorMessage = loaded.errorMessage;
  }

  /** Close the CDI XML viewer and reset its state. */
  closeViewer(): void {
    this.#viewerVisible = false;
    this.#viewerNodeId = null;
    this.#viewerXmlContent = null;
    this.#viewerStatus = 'idle';
    this.#viewerErrorMessage = null;
  }

  /** Open the compact CDI re-download dialog for a node. */
  openRedownload(nodeId: string): void {
    this.#redownloadNodeId = nodeId;
    this.#redownloadNodeName = this.#deps.resolveNodeName(nodeId);
    this.#redownloadVisible = true;
  }

  /** Close the CDI re-download dialog and reset its state. */
  closeRedownload(): void {
    this.#redownloadVisible = false;
    this.#redownloadNodeId = null;
    this.#redownloadNodeName = null;
  }
}
