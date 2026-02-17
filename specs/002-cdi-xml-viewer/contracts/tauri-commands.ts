/**
 * Tauri Command Contracts: CDI XML Viewer
 * 
 * TypeScript interface definitions for backend Tauri commands.
 * These types are used when invoking commands via `invoke()` from the frontend.
 * 
 * @module contracts/tauri-commands
 */

// ============================================================================
// Request/Response Types
// ============================================================================

/**
 * Request to retrieve CDI XML for a specific node
 */
export interface GetCdiXmlRequest {
  /**
   * Node ID as hex string (e.g., "01.02.03.04.05.06")
   * May also accepts compact format without dots (e.g., "010203040506")
   */
  nodeId: string;
}

/**
 * Response containing CDI XML data or error information
 */
export interface GetCdiXmlResponse {
  /**
   * Raw CDI XML content as string
   * Null if CDI not available or retrieval failed
   */
  xmlContent: string | null;
  
  /**
   * Size of XML content in bytes (for display/warning purposes)
   * Null if xmlContent is null
   */
  sizeBytes: number | null;
  
  /**
   * Timestamp when CDI was retrieved from node (ISO 8601 format)
   * Null if CDI not available
   */
  retrievedAt: string | null;
}

/**
 * Error types that can be returned from CDI operations
 * Returned as string in Promise rejection
 */
export type CdiErrorType =
  | 'CdiNotRetrieved'   // CDI not yet fetched from node
  | 'CdiUnavailable'    // Node doesn't provide CDI
  | 'RetrievalFailed'   // CDI fetch operation failed
  | 'InvalidXml'        // XML parsing failed
  | 'NodeNotFound';     // Node ID not in cache

// ============================================================================
// Tauri Commands
// ============================================================================

/**
 * Get CDI XML for a specific node
 * 
 * Retrieves the Configuration Description Information (CDI) XML document
 * from the cached node data. CDI must have been previously retrieved
 * via node configuration operations.
 * 
 * @command get_cdi_xml
 * @param {GetCdiXmlRequest} request - Node ID to retrieve CDI for
 * @returns {Promise<GetCdiXmlResponse>} CDI XML data and metadata
 * @throws {string} Error message if retrieval fails (one of CdiErrorType)
 * 
 * @example
 * ```typescript
 * import { invoke } from '@tauri-apps/api/core';
 * 
 * try {
 *   const response = await invoke<GetCdiXmlResponse>('get_cdi_xml', {
 *     nodeId: '01.02.03.04.05.06'
 *   });
 *   
 *   if (response.xmlContent) {
 *     console.log('CDI XML:', response.xmlContent);
 *     console.log('Size:', response.sizeBytes, 'bytes');
 *   }
 * } catch (error) {
 *   console.error('CDI retrieval failed:', error);
 *   // Error is one of: CdiNotRetrieved, CdiUnavailable, NodeNotFound, etc.
 * }
 * ```
 */
export async function getCdiXml(nodeId: string): Promise<GetCdiXmlResponse> {
  // This is a type-safe wrapper around invoke()
  // Actual implementation in frontend: lib/api/cdi.ts
  throw new Error('Not implemented - use actual invoke() wrapper');
}

// ============================================================================
// Type Guards
// ============================================================================

/**
 * Check if error message matches a specific CDI error type
 */
export function isCdiError(error: unknown, type: CdiErrorType): boolean {
  if (typeof error !== 'string') return false;
  return error.includes(type);
}

/**
 * Extract node ID from CdiNotRetrieved or NodeNotFound error message
 * 
 * @param error - Error message from Tauri command
 * @returns Node ID if found in error message, null otherwise
 */
export function extractNodeIdFromError(error: string): string | null {
  // Error format: "CDI not yet retrieved for node 01.02.03.04.05.06"
  // or: "Node 01.02.03.04.05.06 not found"
  const match = error.match(/node ([0-9A-Fa-f.]+)/);
  return match ? match[1] : null;
}

// ============================================================================
// Constants
// ============================================================================

/**
 * Maximum CDI XML size supported (10MB per spec)
 */
export const MAX_CDI_SIZE_BYTES = 10 * 1024 * 1024;

/**
 * Threshold for displaying performance warning (1MB)
 */
export const CDI_SIZE_WARNING_THRESHOLD = 1 * 1024 * 1024;

/**
 * User-friendly error messages for CDI error types
 */
export const CDI_ERROR_MESSAGES: Record<CdiErrorType, string> = {
  CdiNotRetrieved: 'CDI data has not been retrieved for this node. Retrieve configuration first.',
  CdiUnavailable: 'This node does not provide CDI (Configuration Description Information).',
  RetrievalFailed: 'CDI retrieval failed. Check node connection and try again.',
  InvalidXml: 'XML parsing failed. Raw content will be displayed.',
  NodeNotFound: 'Node not found. Refresh the node list and try again.',
};

/**
 * Get user-friendly error message from Tauri error
 * 
 * @param error - Error from Tauri command (Promise rejection)
 * @returns User-friendly error message
 */
export function getCdiErrorMessage(error: unknown): string {
  if (typeof error !== 'string') {
    return 'An unexpected error occurred while retrieving CDI.';
  }
  
  // Check each error type
  for (const [type, message] of Object.entries(CDI_ERROR_MESSAGES)) {
    if (error.includes(type)) {
      return message;
    }
  }
  
  // Fallback: return the error as-is
  return error;
}
