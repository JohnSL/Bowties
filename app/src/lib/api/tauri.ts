import { invoke } from '@tauri-apps/api/core';

/**
 * Node ID represented as 6-byte array (serialized directly as array from Rust)
 */
export type NodeID = number[];

/**
 * SNIP (Simple Node Identification Protocol) data fields
 */
export interface SNIPData {
  manufacturer: string;
  model: string;
  hardware_version: string;
  software_version: string;
  user_name: string;
  user_description: string;
}

/**
 * Status of SNIP data retrieval operation
 */
export type SNIPStatus = 
  | 'Unknown' 
  | 'InProgress' 
  | 'Complete' 
  | 'Partial' 
  | 'NotSupported' 
  | 'Timeout' 
  | 'Error';

/**
 * Connection status of a node
 */
export type ConnectionStatus = 
  | 'Unknown' 
  | 'Verifying' 
  | 'Connected' 
  | 'NotResponding';

/**
 * CDI (Configuration Description Information) data
 */
export interface CdiData {
  xml_content: string;
  retrieved_at: string;  // ISO 8601 timestamp
}

/**
 * Discovered LCC node with SNIP data
 */
export interface DiscoveredNode {
  node_id: NodeID;
  alias: number;
  snip_data: SNIPData | null;
  snip_status: SNIPStatus;
  connection_status: ConnectionStatus;
  last_verified: string | null;  // ISO 8601 timestamp
  last_seen: string;              // ISO 8601 timestamp
  cdi: CdiData | null;            // CDI XML data if available
}

/**
 * Response from query_snip command
 */
export interface QuerySnipResponse {
  alias: number;
  snip_data: SNIPData | null;
  status: SNIPStatus;
}

/**
 * Discover all nodes on the LCC network
 * @param timeout_ms - Maximum time to wait for responses (default: 250ms)
 * @returns Promise with discovered nodes
 */
export async function discoverNodes(timeout_ms?: number): Promise<DiscoveredNode[]> {
  return invoke<DiscoveredNode[]>('discover_nodes', { timeout_ms });
}

/**
 * Query SNIP data for a specific node
 * @param alias - Destination node alias (1-4095)
 * @param timeout_ms - Timeout for SNIP request (default: 5000ms)
 * @returns Promise with SNIP data and status
 */
export async function querySnip(
  alias: number, 
  timeout_ms?: number
): Promise<QuerySnipResponse> {
  return invoke<QuerySnipResponse>('query_snip_single', { alias, timeout_ms });
}

/**
 * Query SNIP data for multiple nodes concurrently
 * @param aliases - Array of destination node aliases
 * @param timeout_ms - Timeout per node (default: 5000ms)
 * @returns Promise with batch results
 */
export async function querySnipBatch(
  aliases: number[], 
  timeout_ms?: number
): Promise<QuerySnipResponse[]> {
  return invoke<QuerySnipResponse[]>('query_snip_batch', { aliases, timeout_ms });
}

/**
 * Verify the status of a single node
 * @param alias - Destination node alias (1-4095)
 * @param timeout_ms - Timeout for verification (default: 500ms)
 * @returns Promise with boolean indicating if node is online
 */
export async function verifyNodeStatus(
  alias: number,
  timeout_ms?: number
): Promise<boolean> {
  return invoke<boolean>('verify_node_status', { alias, timeout_ms });
}

/**
 * Refresh all discovered nodes to check their current status
 * @param timeout_ms - Timeout per node (default: 500ms)
 * @returns Promise with updated node list
 */
export async function refreshAllNodes(timeout_ms?: number): Promise<DiscoveredNode[]> {
  return invoke<DiscoveredNode[]>('refresh_all_nodes', { timeout_ms });
}
