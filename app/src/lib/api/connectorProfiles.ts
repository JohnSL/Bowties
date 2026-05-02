import { invoke } from '@tauri-apps/api/core';
import type {
  CompatibilityPreviewRequest,
  CompatibilityPreviewResponse,
  ConnectorProfileView,
  ConnectorSelectionDocument,
} from '$lib/types/connectorProfile';

export async function getConnectorProfile(nodeId: string): Promise<ConnectorProfileView | null> {
  return invoke<ConnectorProfileView | null>('get_connector_profile', { nodeId });
}

export async function getConnectorSelections(
  nodeId: string,
): Promise<ConnectorSelectionDocument | null> {
  return invoke<ConnectorSelectionDocument | null>('get_connector_selections', { nodeId });
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