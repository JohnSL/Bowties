/**
 * CDI XML Viewer Type Definitions
 * 
 * TypeScript interfaces for CDI (Configuration Description Information)
 * viewer state management and Tauri command interactions.
 */

/**
 * Status of the CDI viewer operation
 */
export type ViewerStatus = 'idle' | 'loading' | 'success' | 'error';

/**
 * State of the CDI XML viewer modal
 */
export interface CdiViewerState {
  /** Whether the modal is currently displayed */
  visible: boolean;
  
  /** The node ID whose CDI is being viewed (null if modal closed) */
  nodeId: string | null;
  
  /** The raw CDI XML content (null if not loaded or error) */
  xmlContent: string | null;
  
  /** The formatted/indented XML for display */
  formattedXml: string | null;
  
  /** Current status of the viewer */
  status: ViewerStatus;
  
  /** Error message if status is error */
  errorMessage: string | null;
}

/**
 * Response from the get_cdi_xml Tauri command
 */
export interface GetCdiXmlResponse {
  /** Raw CDI XML content as string (null if not available) */
  xmlContent: string | null;
  
  /** Size of XML content in bytes (null if xmlContent is null) */
  sizeBytes: number | null;
  
  /** Timestamp when CDI was retrieved (ISO 8601 format, null if not available) */
  retrievedAt: string | null;
}

/**
 * Error types that can be returned from CDI operations
 */
export type CdiErrorType =
  | 'CdiNotRetrieved'   // CDI not yet fetched from node
  | 'CdiUnavailable'    // Node doesn't provide CDI
  | 'RetrievalFailed'   // CDI fetch operation failed
  | 'InvalidXml'        // XML parsing failed
  | 'NodeNotFound';     // Node ID not in cache

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

/**
 * Check if error message matches a specific CDI error type
 */
export function isCdiError(error: unknown, type: CdiErrorType): boolean {
  if (typeof error !== 'string') return false;
  return error.includes(type);
}
