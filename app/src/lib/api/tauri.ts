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
// ─── Feature 006: Bowties — Discover Existing Connections ─────────────────

/** The role of an event slot as determined by the classification pipeline. */
export type EventRole = 'Producer' | 'Consumer' | 'Ambiguous';

/**
 * A single classified event ID field from one node.
 * Entries in `producers` always have role 'Producer';
 * entries in `consumers` always have role 'Consumer';
 * 'Ambiguous' slots appear only in `ambiguous_entries`.
 */
export interface EventSlotEntry {
  /** Node identifier in dotted-hex, e.g. "02.01.57.00.00.01" */
  node_id: string;
  /** Human-readable node name */
  node_name: string;
  /** CDI path from segment root to this element */
  element_path: string[];
  /** Display label (CDI name → description first sentence → slash-joined path) */
  element_label: string;
  /** Full CDI <description> text (null when absent). Shown in the Unknown role section
   *  so users can read the raw firmware description and classify the slot manually. */
  element_description: string | null;
  /** The 8-byte event ID stored in this slot (as u8 array) */
  event_id: number[];
  /** Classified role (only Producer/Consumer here; Ambiguous in ambiguous_entries only) */
  role: EventRole;
}

/**
 * A bowtie card — one shared event ID with ≥1 producer and ≥1 consumer.
 *
 * Invariants: producers.length ≥ 1, consumers.length ≥ 1.
 */
export interface BowtieCard {
  /** Dotted-hex event ID, e.g. "05.02.01.02.03.00.00.01" — unique key */
  event_id_hex: string;
  /** Raw 8-byte event ID (for sorting / equality checks) */
  event_id_bytes: number[];
  /** Confirmed producer slots (≥1) */
  producers: EventSlotEntry[];
  /** Confirmed consumer slots (≥1) */
  consumers: EventSlotEntry[];
  /** Slots whose role could not be determined */
  ambiguous_entries: EventSlotEntry[];
  /** User-assigned name (null = unnamed, show event_id_hex as header per FR-014) */
  name: string | null;
}

/**
 * Complete bowtie catalog for the current session.
 * Rebuilt atomically after each full CDI + Identify Events refresh.
 */
export interface BowtieCatalog {
  /** All bowties sorted by event_id_bytes (lexicographic) */
  bowties: BowtieCard[];
  /** ISO 8601 timestamp of last rebuild */
  built_at: string;
  /** Number of nodes included */
  source_node_count: number;
  /** Total event slots scanned */
  total_slots_scanned: number;
}

/**
 * Payload of the `cdi-read-complete` Tauri event.
 * Emitted after all CDI reads complete and the catalog has been rebuilt.
 */
export interface CdiReadCompletePayload {
  catalog: BowtieCatalog;
  node_count: number;
}

/** Derived display name for a BowtieCard (FR-014). */
export function bowtieName(card: BowtieCard): string {
  return card.name ?? card.event_id_hex;
}

/**
 * Get the current BowtieCatalog from AppState.
 * Returns null if CDI reads have not yet completed.
 */
export async function getBowties(): Promise<BowtieCatalog | null> {
  return invoke<BowtieCatalog | null>('get_bowties');
}