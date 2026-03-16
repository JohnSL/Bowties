/**
 * CDI (Configuration Description Information) API
 * 
 * Tauri command wrappers for CDI operations.
 */

import { invoke } from '@tauri-apps/api/core';
import type { GetCdiXmlResponse } from '$lib/types/cdi';

// T102: AbortController for request cancellation
let currentAbortController: AbortController | null = null;

/**
 * Get CDI XML for a specific node
 * 
 * Retrieves the Configuration Description Information (CDI) XML document
 * from cache (memory or file). If not cached, returns CdiNotRetrieved error.
 * Use downloadCdi to retrieve from network.
 * 
 * @param nodeId - Node ID as hex string (e.g., "01.02.03.04.05.06")
 * @returns Promise resolving to CDI XML data and metadata
 * @throws String error message if retrieval fails
 * 
 * @example
 * ```typescript
 * try {
 *   const response = await getCdiXml('01.02.03.04.05.06');
 *   if (response.xmlContent) {
 *     console.log('CDI XML:', response.xmlContent);
 *     console.log('Size:', response.sizeBytes, 'bytes');
 *   }
 * } catch (error) {
 *   console.error('CDI retrieval failed:', error);
 * }
 * ```
 */
export async function getCdiXml(nodeId: string): Promise<GetCdiXmlResponse> {
  return await invoke<GetCdiXmlResponse>('get_cdi_xml', {
    nodeId,
  });
}

/**
 * Download CDI XML from a node over the network
 * 
 * Retrieves CDI using the Memory Configuration Protocol and caches it
 * both in memory and on disk for future use.
 * 
 * @param nodeId - Node ID as hex string (e.g., "01.02.03.04.05.06")
 * @returns Promise resolving to CDI XML data and metadata
 * @throws String error message if download fails
 * 
 * @example
 * ```typescript
 * try {
 *   const response = await downloadCdi('01.02.03.04.05.06');
 *   console.log('Downloaded CDI:', response.xmlContent);
 * } catch (error) {
 *   console.error('CDI download failed:', error);
 * }
 * ```
 */
export async function downloadCdi(nodeId: string): Promise<GetCdiXmlResponse> {
  return await invoke<GetCdiXmlResponse>('download_cdi', {
    nodeId,
  });
}

// ============================================================================
// CDI Navigation API
// ============================================================================

/**
 * Discovered node information
 */
export interface DiscoveredNode {
    nodeId: string;
    nodeName: string;
    hasCdi: boolean;
}

/**
 * Response from get_discovered_nodes
 */
export interface GetDiscoveredNodesResponse {
    nodes: DiscoveredNode[];
}

/**
 * Segment information
 */
export interface SegmentInfo {
    id: string;
    name: string | null;
    description: string | null;
    space: number;
    hasGroups: boolean;
    hasElements: boolean;
    metadata?: Record<string, unknown>;
}

/**
 * CDI structure response
 */
export interface CdiStructureResponse {
    nodeId: string;
    nodeName: string;
    segments: SegmentInfo[];
    maxDepth: number;
}

/**
 * Column item for navigation
 */
export interface ColumnItem {
    id: string;
    name: string;
    fullName?: string;
    type?: string;
    hasChildren: boolean;
    metadata?: Record<string, unknown>;
}

/**
 * Column items response
 */
export interface GetColumnItemsResponse {
    depth: number;
    columnType: string;
    items: ColumnItem[];
}

/**
 * Constraint information
 */
export interface Constraint {
    type: 'range' | 'map' | 'length';
    description: string;
    value: {
        min?: number;
        max?: number;
        entries?: Array<{ value: number; label: string }>;
        maxLength?: number;
    };
}

/**
 * Element details response
 */
export interface ElementDetailsResponse {
    name: string;
    description: string | null;
    dataType: string;
    fullPath: string;
    elementPath: string[];
    constraints: Constraint[];
    defaultValue: string | null;
    memoryAddress: number;
}

/**
 * Group instance from replication
 */
export interface GroupInstance {
    index: number;
    name: string;
    address: number;
}

/**
 * Expanded replicated group response
 */
export interface ExpandReplicatedGroupResponse {
    groupName: string;
    replicationCount: number;
    instances: GroupInstance[];
}

/**
 * Get list of discovered nodes for Nodes column
 * 
 * @returns List of discovered nodes with CDI availability status
 * @throws Error if command fails
 */
export async function getDiscoveredNodes(): Promise<GetDiscoveredNodesResponse> {
    return await invoke<GetDiscoveredNodesResponse>('get_discovered_nodes');
}

/**
 * Parse and return the CDI structure for a node
 * 
 * @param nodeId - Node ID in dotted hex format (e.g., '01.02.03.04.05.06')
 * @returns Complete CDI structure with parsed segments and elements
 * @throws Error if CDI not available or parse error
 */
export async function getCdiStructure(nodeId: string): Promise<CdiStructureResponse> {
    return await invoke<CdiStructureResponse>('get_cdi_structure', { nodeId });
}

/**
 * Get items for a specific column based on parent selection
 * T102: Implements request cancellation for rapid navigation
 * 
 * @param nodeId - Node ID being navigated
 * @param parentPath - Path of selected items from root to parent (empty for segments)
 * @param depth - Column depth (1=segments, 2+=groups/elements)
 * @returns Column items for the requested depth
 * @throws Error if path is invalid or CDI not loaded
 */
export async function getColumnItems(
    nodeId: string,
    parentPath: string[],
    depth: number
): Promise<GetColumnItemsResponse> {
    // Cancel any pending request
    if (currentAbortController) {
        currentAbortController.abort();
    }
    
    // Create new abort controller for this request
    currentAbortController = new AbortController();
    const signal = currentAbortController.signal;
    
    try {
        const result = await invoke<GetColumnItemsResponse>('get_column_items', {
            nodeId,
            parentPath,
            depth,
        });
        
        // Clear controller if not aborted
        if (!signal.aborted) {
            currentAbortController = null;
        }
        
        return result;
    } catch (error) {
        // Don't propagate if aborted (normal cancellation)
        if (signal.aborted) {
            throw new Error('Request cancelled');
        }
        throw error;
    }
}

/**
 * Get detailed information for a selected element
 * 
 * @param nodeId - Node ID being navigated
 * @param elementPath - Full path to element from root
 * @returns Detailed element metadata for Details Panel
 * @throws Error if element not found
 */
export async function getElementDetails(
    nodeId: string,
    elementPath: string[]
): Promise<ElementDetailsResponse> {
    return await invoke<ElementDetailsResponse>('get_element_details', {
        nodeId,
        elementPath,
    });
}

/**
 * Expand a replicated group into individual instances
 * 
 * @param nodeId - Node ID being navigated
 * @param groupPath - Path to replicated group
 * @returns Expanded group instances with computed names and addresses
 * @throws Error if group not found or not replicated
 */
export async function expandReplicatedGroup(
    nodeId: string,
    groupPath: string[]
): Promise<ExpandReplicatedGroupResponse> {
    return await invoke<ExpandReplicatedGroupResponse>('expand_replicated_group', {
        nodeId,
        groupPath,
    });
}

// ============================================================================
// Configuration Value Reading API (Feature 004-read-node-config)
// ============================================================================

/**
 * Read a single configuration value from a node (T037)
 * 
 * @param nodeId - Node ID in dotted hex format (e.g., '01.02.03.04.05.06')
 * @param elementPath - Path to the element (e.g., ['Settings', 'Network', 'Node Name'])
 * @param timeoutMs - Optional timeout in milliseconds (default: 2000)
 * @returns Configuration value with metadata
 * @throws Error if read fails
 * 
 * @example
 * ```typescript
 * try {
 *   const value = await readConfigValue('01.02.03.04.05.06', ['Settings', 'Node Name']);
 *   console.log('Current value:', value.value);
 * } catch (error) {
 *   console.error('Failed to read value:', error);
 * }
 * ```
 */
export async function readConfigValue(
    nodeId: string,
    elementPath: string[],
    timeoutMs?: number
): Promise<import('./types').ConfigValueWithMetadata> {
    return await invoke('read_config_value', {
        nodeId,
        elementPath,
        timeoutMs,
    });
}

/**
 * Read all configuration values from a node with progress tracking (T057)
 * 
 * @param nodeId - Node ID in dotted hex format (e.g., '01.02.03.04.05.06')
 * @param timeoutMs - Optional timeout per element in milliseconds (default: 2000)
 * @returns Map of configuration values with metadata and statistics
 * @throws Error if batch read fails
 * 
 * @example
 * ```typescript
 * try {
 *   const response = await readAllConfigValues('01.02.03.04.05.06');
 *   console.log(`Read ${response.successfulReads} of ${response.totalElements} values`);
 *   console.log('Values:', response.values);
 * } catch (error) {
 *   console.error('Batch read failed:', error);
 * }
 * ```
 */
export async function readAllConfigValues(
    nodeId: string,
    timeoutMs?: number,
    nodeIndex?: number,
    totalNodes?: number
): Promise<import('./types').ReadAllConfigValuesResponse> {
    return await invoke('read_all_config_values', {
        nodeId,
        timeoutMs,
        nodeIndex,
        totalNodes,
    });
}

/**
 * Cancel an ongoing configuration reading operation (T058)
 * 
 * @returns Promise that resolves when cancellation is signaled
 * @throws Error if cancellation fails
 * 
 * @example
 * ```typescript
 * try {
 *   await cancelConfigReading();
 *   console.log('Cancellation requested');
 * } catch (error) {
 *   console.error('Cancel failed:', error);
 * }
 * ```
 */
export async function cancelConfigReading(): Promise<void> {
    return await invoke('cancel_config_reading', {});
}
