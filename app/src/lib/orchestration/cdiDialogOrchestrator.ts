import { getCdiErrorMessage, isCdiError, type GetCdiXmlResponse, type ViewerStatus } from '$lib/types/cdi';

export interface CdiNodeCandidate {
  nodeId: string;
  nodeName: string;
  downloadStatus?: 'waiting' | 'downloading' | 'done' | 'failed';
}

export interface CdiViewerLoadState {
  errorMessage: string | null;
  nodeId: string | null;
  status: ViewerStatus;
  visible: boolean;
  xmlContent: string | null;
}

export interface CdiRedownloadState {
  nodeId: string | null;
  nodeName: string | null;
  visible: boolean;
}

export function createOpeningCdiViewerState(nodeId: string): CdiViewerLoadState {
  return {
    errorMessage: 'Checking cache…',
    nodeId,
    status: 'loading',
    visible: true,
    xmlContent: null,
  };
}

export function createClosedCdiViewerState(): CdiViewerLoadState {
  return {
    errorMessage: null,
    nodeId: null,
    status: 'idle',
    visible: false,
    xmlContent: null,
  };
}

export async function loadCdiViewerState(
  nodeId: string,
  getCdiXml: (nodeId: string) => Promise<GetCdiXmlResponse>,
  downloadCdi: (nodeId: string) => Promise<GetCdiXmlResponse>,
): Promise<Pick<CdiViewerLoadState, 'errorMessage' | 'status' | 'xmlContent'>> {
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
      return {
        errorMessage: null,
        status: 'success',
        xmlContent: response.xmlContent,
      };
    }

    return {
      errorMessage: 'No CDI data available for this node.',
      status: 'error',
      xmlContent: null,
    };
  } catch (error) {
    return {
      errorMessage: getCdiErrorMessage(error),
      status: 'error',
      xmlContent: null,
    };
  }
}

export function createOpenCdiRedownloadState(
  nodeId: string,
  nodes: Array<{ nodeId: string; nodeName: string }>,
): CdiRedownloadState {
  const node = nodes.find((candidate) => candidate.nodeId === nodeId);
  return {
    nodeId,
    nodeName: node?.nodeName ?? nodeId,
    visible: true,
  };
}

export function createClosedCdiRedownloadState(): CdiRedownloadState {
  return {
    nodeId: null,
    nodeName: null,
    visible: false,
  };
}

export function createCancelledCdiDownloadState(): {
  cdiDownloadDialogVisible: false;
  cdiMissingNodes: CdiNodeCandidate[];
  pendingConfigNodes: CdiNodeCandidate[];
} {
  return {
    cdiDownloadDialogVisible: false,
    cdiMissingNodes: [],
    pendingConfigNodes: [],
  };
}

export function createWaitingCdiDownloadNodes(nodes: CdiNodeCandidate[]): CdiNodeCandidate[] {
  return nodes.map((node) => ({ ...node, downloadStatus: 'waiting' }));
}

export function updateCdiDownloadNodeStatus(
  nodes: CdiNodeCandidate[],
  index: number,
  downloadStatus: NonNullable<CdiNodeCandidate['downloadStatus']>,
): CdiNodeCandidate[] {
  return nodes.map((node, nodeIndex) => (
    nodeIndex === index ? { ...node, downloadStatus } : node
  ));
}

export function resolvePostDownloadReadNodes(args: {
  nodesToDownload: CdiNodeCandidate[];
  nodesWithCdi: Set<string>;
  pendingConfigNodes: CdiNodeCandidate[];
}): CdiNodeCandidate[] {
  const allPending = args.pendingConfigNodes.length > 0
    ? args.pendingConfigNodes
    : args.nodesToDownload;
  return allPending.filter((node) => args.nodesWithCdi.has(node.nodeId));
}