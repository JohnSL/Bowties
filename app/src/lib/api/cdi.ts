/**
 * CDI (Configuration Description Information) API
 * 
 * Tauri command wrappers for CDI operations.
 */

import { invoke } from '@tauri-apps/api/core';
import type { GetCdiXmlResponse } from '$lib/types/cdi';

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
