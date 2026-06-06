import { invoke } from '@tauri-apps/api/core';
import type {
  CompatibilityPreviewRequest,
  CompatibilityPreviewResponse,
  ConnectorProfileView,
  ConnectorSelectionDocument,
} from '$lib/types/connectorProfile';
import { toCanonicalNodeKey, type NodeKeyInput } from '$lib/utils/nodeKey';

export async function getConnectorProfile(nodeId: NodeKeyInput): Promise<ConnectorProfileView | null> {
  return invoke<ConnectorProfileView | null>('get_connector_profile', { nodeId: toCanonicalNodeKey(nodeId) });
}

export async function getConnectorSelections(
  nodeId: NodeKeyInput,
): Promise<ConnectorSelectionDocument | null> {
  return invoke<ConnectorSelectionDocument | null>('get_connector_selections', { nodeId: toCanonicalNodeKey(nodeId) });
}

export async function putConnectorSelections(
  document: ConnectorSelectionDocument,
): Promise<ConnectorSelectionDocument> {
  return invoke<ConnectorSelectionDocument>('put_connector_selections', { document });
}

export async function previewConnectorCompatibility(
  request: CompatibilityPreviewRequest,
): Promise<CompatibilityPreviewResponse> {
  return invoke<CompatibilityPreviewResponse>('preview_connector_compatibility', { request });
}