# Tauri API Reference

This document describes all Tauri commands available to the frontend application, including connection management, node discovery, and SNIP data retrieval.

## Table of Contents

1. [Connection Commands](#connection-commands)
2. [Discovery Commands](#discovery-commands)
3. [CDI Commands](#cdi-commands)
4. [Bowties Commands](#bowties-commands)
5. [Events](#events)
6. [TypeScript Type Definitions](#typescript-type-definitions)
7. [Frontend Stores & Utilities](#frontend-stores--utilities)
8. [Error Handling](#error-handling)

---

## Connection Commands

### `connect_lcc`

Establish a connection to an LCC network via GridConnect TCP.

**Parameters:**
- `host: string` - Hostname or IP address of the LCC gateway (e.g., "localhost", "192.168.1.100")
- `port: number` - TCP port number (typically 12021)

**Returns:** `Promise<ConnectionInfo>`

```typescript
interface ConnectionInfo {
  host: string;
  port: number;
  connected: boolean;
}
```

**Usage Example:**
```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const result = await invoke("connect_lcc", { 
    host: "localhost", 
    port: 12021 
  });
  console.log("Connected:", result.connected);
} catch (error) {
  console.error("Connection failed:", error);
}
```

**Error Conditions:**
- Network connection failure
- Invalid host/port
- GridConnect protocol errors

---

### `disconnect_lcc`

Disconnect from the current LCC network.

**Parameters:** None

**Returns:** `Promise<void>`

**Usage Example:**
```typescript
try {
  await invoke("disconnect_lcc");
  console.log("Disconnected successfully");
} catch (error) {
  console.error("Disconnect failed:", error);
}
```

---

### `get_connection_status`

Retrieve current connection status and parameters.

**Parameters:** None

**Returns:** `Promise<ConnectionInfo>`

**Usage Example:**
```typescript
const status = await invoke("get_connection_status");
console.log(`Connected to ${status.host}:${status.port}: ${status.connected}`);
```

---

## Discovery Commands

### `discover_nodes`

Discover all nodes on the LCC network using the Verified Node ID Number protocol.

**Parameters:**
- `timeout_ms?: number` - Maximum time to wait for responses in milliseconds (default: 250)

**Returns:** `Promise<DiscoveredNode[]>`

**Rust Implementation:**
```rust
#[tauri::command]
pub async fn discover_nodes(
    timeout_ms: Option<u64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DiscoveredNode>, String>
```

**TypeScript Wrapper:**
```typescript
export async function discoverNodes(timeout_ms?: number): Promise<DiscoveredNode[]> {
  return invoke<DiscoveredNode[]>('discover_nodes', { timeout_ms });
}
```

**Usage Example:**
```typescript
import { discoverNodes } from '$lib/api/tauri';

// Use default 250ms timeout
const nodes = await discoverNodes();

// Custom timeout
const nodesWithLongerTimeout = await discoverNodes(500);
console.log(`Found ${nodes.length} nodes`);
```

**Behavior:**
- Sends Verify Node ID Number (global) message
- Collects all Verified Node ID Number replies within timeout
- Updates internal node cache
- Returns cached nodes with discovery metadata

---

### `query_snip_single`

Query SNIP (Simple Node Identification Protocol) data for a specific node.

**Parameters:**
- `alias: number` - Destination node alias (1-4095)
- `timeout_ms?: number` - Timeout for SNIP request in milliseconds (default: 5000)

**Returns:** `Promise<QuerySnipResponse>`

**Rust Implementation:**
```rust
#[tauri::command]
pub async fn query_snip_single(
    alias: u16,
    state: tauri::State<'_, AppState>,
) -> Result<QuerySnipResponse, String>
```

**TypeScript Wrapper:**
```typescript
export async function querySnip(
  alias: number, 
  timeout_ms?: number
): Promise<QuerySnipResponse> {
  return invoke<QuerySnipResponse>('query_snip_single', { alias, timeout_ms });
}
```

**Usage Example:**
```typescript
import { querySnip } from '$lib/api/tauri';

const result = await querySnip(0x123);
if (result.status === 'Complete') {
  console.log(`Manufacturer: ${result.snip_data?.manufacturer}`);
  console.log(`Model: ${result.snip_data?.model}`);
}
```

**SNIP Fields Retrieved:**
- Manufacturer name
- Model name
- Hardware version
- Software version
- User-assigned name
- User description

---

### `query_snip_batch`

Query SNIP data for multiple nodes concurrently.

**Parameters:**
- `aliases: number[]` - Array of destination node aliases
- `timeout_ms?: number` - Timeout per node in milliseconds (default: 5000)

**Returns:** `Promise<QuerySnipResponse[]>`

**Rust Implementation:**
```rust
#[tauri::command]
pub async fn query_snip_batch(
    aliases: Vec<u16>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<QuerySnipResponse>, String>
```

**TypeScript Wrapper:**
```typescript
export async function querySnipBatch(
  aliases: number[], 
  timeout_ms?: number
): Promise<QuerySnipResponse[]> {
  return invoke<QuerySnipResponse[]>('query_snip_batch', { aliases, timeout_ms });
}
```

**Usage Example:**
```typescript
import { querySnipBatch } from '$lib/api/tauri';

const aliases = [0x123, 0x456, 0x789];
const results = await querySnipBatch(aliases);

results.forEach(result => {
  console.log(`Alias ${result.alias}: ${result.status}`);
});
```

**Concurrency:**
- Maximum 5 concurrent SNIP requests (controlled by semaphore)
- Sequential execution due to mutable borrow constraints (may be refactored)
- Automatically updates node cache with retrieved data

---

### `verify_node_status`

Verify if a specific node is online and responding.

**Parameters:**
- `alias: number` - Destination node alias (1-4095)
- `timeout_ms?: number` - Timeout for verification in milliseconds (default: 500)

**Returns:** `Promise<boolean>`

**Rust Implementation:**
```rust
#[tauri::command]
pub async fn verify_node_status(
    alias: u16,
    timeout_ms: Option<u64>,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String>
```

**TypeScript Wrapper:**
```typescript
export async function verifyNodeStatus(
  alias: number,
  timeout_ms?: number
): Promise<boolean> {
  return invoke<boolean>('verify_node_status', { alias, timeout_ms });
}
```

**Usage Example:**
```typescript
import { verifyNodeStatus } from '$lib/api/tauri';

const isOnline = await verifyNodeStatus(0x123);
if (isOnline) {
  console.log('Node is responding');
} else {
  console.log('Node is offline or not responding');
}
```

**Side Effects:**
- Updates cached node's `connection_status` field
- Updates `last_verified` and `last_seen` timestamps for online nodes
- Marks offline nodes as `NotResponding`

---

### `refresh_all_nodes`

Refresh the status of all previously discovered nodes.

**Parameters:**
- `timeout_ms?: number` - Timeout per node in milliseconds (default: 500)

**Returns:** `Promise<DiscoveredNode[]>`

**Rust Implementation:**
```rust
#[tauri::command]
pub async fn refresh_all_nodes(
    timeout_ms: Option<u64>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DiscoveredNode>, String>
```

**TypeScript Wrapper:**
```typescript
export async function refreshAllNodes(timeout_ms?: number): Promise<DiscoveredNode[]> {
  return invoke<DiscoveredNode[]>('refresh_all_nodes', { timeout_ms });
}
```

**Usage Example:**
```typescript
import { refreshAllNodes } from '$lib/api/tauri';

// Refresh all nodes with default timeout
const updatedNodes = await refreshAllNodes();

// Custom timeout for slower networks
const nodes = await refreshAllNodes(1000);
console.log(`${nodes.filter(n => n.connection_status === 'Connected').length} nodes online`);
```

**Behavior:**
- Verifies each cached node sequentially
- Updates `connection_status` for each node
- Updates timestamps for responding nodes
- Returns complete updated node list

---

## CDI Commands

### `get_cdi_xml`

Retrieve CDI (Configuration Description Information) XML for a specific node from cache.

**Parameters:**
- `node_id: string` - Node ID in hex format (e.g., "01.02.03.04.05.06")

**Returns:** `Promise<GetCdiXmlResponse>`

```typescript
interface GetCdiXmlResponse {
  xmlContent: string | null;       // CDI XML content (null if not available)
  sizeBytes: number | null;         // Size of XML in bytes
  retrievedAt: string | null;       // ISO 8601 timestamp of retrieval
}
```

**Cache Strategy:**
1. Check memory cache (node.cdi) first
2. If not in memory, check file cache (cdi_cache directory)
3. If found in file cache, load into memory cache
4. If not found in either, throw `CdiNotRetrieved` error

**File Cache Location:**
- **Windows:** `%APPDATA%\com.bowtiesapp.bowties\cdi_cache\`
- **macOS:** `~/Library/Application Support/com.bowtiesapp.bowties/cdi_cache/`
- **Linux:** `~/.local/share/com.bowtiesapp.bowties/cdi_cache/`

**Cache Filename Format:** `{manufacturer}_{model}_{software_version}.cdi.xml`

**Usage Example:**
```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  const cdi = await invoke("get_cdi_xml", { 
    nodeId: "01.02.03.04.05.06" 
  });
  
  if (cdi.xmlContent) {
    console.log(`CDI size: ${cdi.sizeBytes} bytes`);
    console.log(`Retrieved: ${cdi.retrievedAt}`);
    // Display or parse XML content
    displayCdiXml(cdi.xmlContent);
  }
} catch (error) {
  if (error.includes("CdiNotRetrieved")) {
    console.log("CDI not yet downloaded for this node");
    // Trigger download...
  } else {
    console.error("Error retrieving CDI:", error);
  }
}
```

**Error Conditions:**
- `CdiNotRetrieved` - CDI has not been downloaded for this node
- `NodeNotFound` - Node ID not found in discovered nodes
- `InvalidNodeId` - Malformed node ID string
- `IoError` - File system error accessing cache

---

### `download_cdi`

Download CDI XML from a node over the network and cache it.

**Parameters:**
- `node_id: string` - Node ID in hex format (e.g., "01.02.03.04.05.06")

**Returns:** `Promise<GetCdiXmlResponse>`

**Process:**
1. Looks up node alias from discovered nodes
2. Downloads CDI using Memory Configuration Protocol (address space 0xFF)
3. Saves to memory cache (node.cdi)
4. Saves to file cache (if SNIP data available for filename)
5. Returns CDI XML content and metadata

**Timeout:** 5 seconds per chunk (accommodates slower nodes)

**Usage Example:**
```typescript
import { invoke } from '@tauri-apps/api/core';

async function retrieveCdi(nodeId: string) {
  try {
    console.log("Downloading CDI from network...");
    const cdi = await invoke("download_cdi", { nodeId });
    
    console.log(`CDI downloaded: ${cdi.sizeBytes} bytes`);
    console.log(`Retrieved at: ${cdi.retrievedAt}`);
    
    // CDI is now cached for future get_cdi_xml calls
    return cdi.xmlContent;
  } catch (error) {
    console.error("CDI download failed:", error);
    throw error;
  }
}

// Later, can retrieve from cache
const cachedCdi = await invoke("get_cdi_xml", { nodeId });
```

**With Progress Indication:**
```typescript
import { invoke } from '@tauri-apps/api/core';

async function downloadCdiWithProgress(nodeId: string) {
  // Could show loading spinner
  setLoading(true);
  
  try {
    const cdi = await invoke("download_cdi", { nodeId });
    setLoading(false);
    
    // Show success
    showNotification(`CDI retrieved: ${cdi.sizeBytes} bytes`);
    return cdi;
  } catch (error) {
    setLoading(false);
    
    if (error.includes("RetrievalFailed")) {
      showError("Failed to download CDI. Node may not support CDI.");
    } else if (error.includes("Not connected")) {
      showError("Not connected to LCC network");
    } else {
      showError(`CDI download error: ${error}`);
    }
  }
}
```

**Error Conditions:**
- `RetrievalFailed` - Network retrieval failed (node may not support CDI)
- `NodeNotFound` - Node ID not found in discovered nodes
- `InvalidNodeId` - Malformed node ID string
- `Not connected to LCC network` - No active connection
- `IoError` - File system error writing cache

**Performance Characteristics:**
- Small CDI (~10-20KB): 1-2 seconds
- Large CDI (100KB+): 5-10 seconds
- Network timeout: 5 seconds per chunk

---

## Bowties Commands

### `get_bowties`

Return the most recently built bowtie catalog from `AppState`. The catalog is built automatically at the end of a full CDI read cycle and also emitted via the `cdi-read-complete` event. Call this command if you need the catalog on-demand (e.g., on component mount).

**Parameters:** None

**Returns:** `Promise<BowtieCatalog | null>`

Returns `null` if no CDI read has been completed yet. Returns the `BowtieCatalog` once the Identify Events exchange and catalog build have finished.

**Rust Implementation:**
```rust
#[tauri::command]
pub async fn get_bowties(
    state: tauri::State<'_, AppState>,
) -> Result<Option<BowtieCatalog>, String>
```

**TypeScript Wrapper:**
```typescript
export async function getBowties(): Promise<BowtieCatalog | null> {
  return invoke<BowtieCatalog | null>('get_bowties');
}
```

**Usage Example:**
```typescript
import { getBowties } from '$lib/api/tauri';
import { bowtieCatalogStore } from '$lib/stores/bowties.svelte';

// On component mount — repopulate store from backend if page reloaded
const catalog = await getBowties();
if (catalog) {
  bowtieCatalogStore.setCatalog(catalog);
}
```

**When the catalog is built:**
1. `read_all_config_values` completes for the last discovered node
2. `query_event_roles` sends `IdentifyEventsAddressed` to every node (125 ms apart)
3. `ProducerIdentified` / `ConsumerIdentified` replies collected for 500 ms after last send
4. `build_bowtie_catalog` groups results into `BowtieCard` entries
5. Catalog stored in `AppState.bowties_catalog`
6. `cdi-read-complete` event emitted with full catalog payload

**Performance Goals:**
- SC-001: catalog built within 5 s of last CDI read completing on a typical network
- SC-004: empty-state visible within 1 s of tab enable

**Error Conditions:**
- `Not connected to LCC network` - No active connection when building catalog

---

### `load_layout`

Open a YAML layout file via a native OS file-open dialog and merge its metadata into the running catalog. Emits `layout-loaded` on success.

**Parameters:**
- `layout_path?: string` — Optional absolute path; if omitted a native open-file dialog is shown.

**Returns:** `Promise<LayoutFile>`

```typescript
// Rust type mirrored to TypeScript via serde
export interface LayoutFile {
  version: number;             // Schema version (currently 1)
  bowties: Record<string, BowtieMetadata>; // Keyed by dotted-hex event ID
  role_classifications: Record<string, 'Producer' | 'Consumer'>; // "{nodeId}:{elementPath}"
}
export interface BowtieMetadata {
  name: string | null;
  tags: string[];
}
```

**Tauri Event emitted:** `layout-loaded` with the loaded `LayoutFile` payload.

**Error Conditions:**
- Dialog cancelled (returns `null` rather than throwing)
- `IoError` — file not readable
- `ParseError` — YAML schema validation failed; returns degraded mode (empty `LayoutFile`) rather than erroring

---

### `save_layout`

Atomically write the current layout file to disk (temp file → flush → rename). Uses the current layout path; if none is set, falls back to a native save-file dialog.

**Parameters:**
- `layout: LayoutFile` — Full layout to write.
- `path?: string` — Absolute path to write to. If omitted, uses the current layout path or shows dialog.

**Returns:** `Promise<string>` — The path the file was saved to.

**Tauri Event emitted:** `layout-save-error` on failure with error string payload.

**Error Conditions:**
- `IoError` — temp write, flush, or rename failed

---

### `get_recent_layout`

Return the path of the most recently opened layout file, or `null` if none.

**Parameters:** None

**Returns:** `Promise<string | null>`

**Persistence:** Stored in `{app_data_dir}/recent-layout.json`.

---

### `set_recent_layout`

Persist a layout path as the most recently used.

**Parameters:**
- `path: string` — Absolute path to the layout file.

**Returns:** `Promise<void>`

---

## Events

The application emits Tauri events for real-time updates when network changes occur. These events enable the frontend to stay synchronized with the LCC network without polling.

### Event Initialization

Events are emitted automatically when using `connect_lcc` with the event-driven architecture (Phase 4). The EventRouter spawns a background task that monitors all LCC messages and emits relevant events to the frontend.

**Automatic Startup:**
- EventRouter starts when connection is established via `connect_lcc`
- Background task continuously monitors messages from MessageDispatcher
- Events are emitted to all listening frontend components
- No manual initialization required

---

### `lcc-node-discovered`

Emitted when a new node is discovered on the network or an existing node's status changes.

**Event Payload:**
```typescript
interface NodeDiscoveredEvent {
  nodeId: string;              // Node ID in hex format (e.g., "01.02.03.04.05.06")
  alias: number;               // Node alias (1-4095)
  connectionStatus: string;    // "Connected" | "NotResponding"
  lastSeen: string;           // ISO 8601 timestamp
}
```

**Usage Example:**
```typescript
import { listen } from '@tauri-apps/api/event';

// Listen for node discovery events
const unlisten = await listen<NodeDiscoveredEvent>('lcc-node-discovered', (event) => {
  console.log(`Node discovered: ${event.payload.nodeId}`);
  console.log(`Alias: ${event.payload.alias}`);
  console.log(`Status: ${event.payload.connectionStatus}`);
  
  // Update UI with new node
  addNodeToList(event.payload);
});

// Clean up listener when component unmounts
onDestroy(() => {
  unlisten();
});
```

**Svelte Store Integration:**
```typescript
import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';

// Create a store for discovered nodes
export const discoveredNodes = writable<Map<string, NodeDiscoveredEvent>>(new Map());

// Listen for node discovery events and update store
listen<NodeDiscoveredEvent>('lcc-node-discovered', (event) => {
  discoveredNodes.update(nodes => {
    nodes.set(event.payload.nodeId, event.payload);
    return nodes;
  });
});
```

**When Emitted:**
- New node responds to Verify Node ID Global message
- Existing node's connection status changes
- Node comes back online after being offline

---

### `cdi-read-complete`

Emitted when the CDI read cycle for all discovered nodes has finished **and** the bowtie catalog has been built. This is the primary trigger for enabling the Bowties tab and populating `bowtieCatalogStore`.

**Event Payload:**
```typescript
interface CdiReadCompletePayload {
  catalog: BowtieCatalog;   // Fully built bowtie catalog
  node_count: number;       // Number of nodes included in the catalog build
}
```

**Usage Example:**
```typescript
import { listen } from '@tauri-apps/api/event';
import type { CdiReadCompletePayload } from '$lib/api/tauri';
import { bowtieCatalogStore, cdiReadCompleteStore } from '$lib/stores/bowties.svelte';

const unlisten = await listen<CdiReadCompletePayload>('cdi-read-complete', (event) => {
  bowtieCatalogStore.setCatalog(event.payload.catalog);
  cdiReadCompleteStore.set(true);
});
```

**When Emitted:**
- At the end of `read_all_config_values` when `node_index + 1 === total_nodes`
- After `query_event_roles` and `build_bowtie_catalog` have completed
- Not emitted if no nodes are discovered

---

### `config-read-progress`

Emitted during `read_all_config_values` to report incremental progress for each node's config memory read.

**Event Payload:**
```typescript
interface ReadProgressUpdate {
  node_id: string;          // Node ID in hex format
  node_name: string;        // SNIP user name or fallback
  node_index: number;       // 0-based index of current node
  total_nodes: number;      // Total nodes to read
  bytes_read: number;       // Bytes read so far for this node
  total_bytes: number | null; // Total expected bytes (null if unknown)
}
```

**Usage Example:**
```typescript
import { listen } from '@tauri-apps/api/event';
import type { ReadProgressUpdate } from '$lib/api/tauri';

const unlisten = await listen<ReadProgressUpdate>('config-read-progress', (event) => {
  const p = event.payload;
  console.log(`Reading ${p.node_name}: node ${p.node_index + 1}/${p.total_nodes}`);
});
```

**When Emitted:**
- Once per node during the bulk config read cycle

---

### `lcc-message-received`

Emitted for all LCC messages received from the network (useful for monitoring/debugging).

**Event Payload:**
```typescript
interface MessageReceivedEvent {
  mti: string;                 // Message Type Identifier in hex (e.g., "0x0490")
  sourceAlias: number;         // Source node alias
  destinationAlias: number | null;  // Destination alias (null for global messages)
  payload: number[];           // Message payload bytes
  timestamp: string;           // ISO 8601 timestamp when received
}
```

**Usage Example:**
```typescript
import { listen } from '@tauri-apps/api/event';

// Monitor all LCC messages
const unlisten = await listen<MessageReceivedEvent>('lcc-message-received', (event) => {
  console.log(`Message received: MTI=${event.payload.mti}, Source=${event.payload.sourceAlias}`);
  
  // Filter for specific message types
  if (event.payload.mti === '0x05B4') {  // CDI retrieval complete
    console.log('CDI retrieval completed');
  }
});
```

**Message Monitor Component:**
```svelte
<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  
  let messages: MessageReceivedEvent[] = [];
  let unlisten: (() => void) | null = null;
  
  onMount(async () => {
    unlisten = await listen<MessageReceivedEvent>('lcc-message-received', (event) => {
      // Add to message list (keep last 100)
      messages = [...messages, event.payload].slice(-100);
    });
  });
  
  onDestroy(() => {
    if (unlisten) unlisten();
  });
</script>

<div class="message-monitor">
  <h2>LCC Message Monitor</h2>
  {#each messages as msg}
    <div class="message">
      <span class="timestamp">{msg.timestamp}</span>
      <span class="mti">{msg.mti}</span>
      <span class="alias">Alias: {msg.sourceAlias}</span>
    </div>
  {/each}
</div>
```

**Common MTI Values:**
- `0x0490` - Verify Node ID Global
- `0x0170` - Verified Node ID Number
- `0x0DE8` - Simple Node Ident Info Request
- `0x0A08` - Simple Node Ident Info Reply
- `0x0A28` - Datagram Received OK
- `0x0A48` - Datagram Rejected
- `0x1A28` - Write Stream Reply
- `0x1C48` - Read Stream Reply

**Performance Considerations:**
- High-traffic networks may generate many events per second
- Consider throttling or filtering messages in production
- Use debouncing for UI updates triggered by events
- Limit message history size to prevent memory growth

---

### Event Lifecycle

**Connection Established:**
1. `connect_lcc` command called
2. MessageDispatcher created and started
3. EventRouter spawned with message subscribers
4. Background tasks begin monitoring network
5. Events start flowing to frontend

**During Operation:**
- Continuous message monitoring (100ms polling cycle)
- Events emitted immediately when messages received
- No frontend polling required
- Automatic UI updates via event handlers

**Disconnection:**
1. `disconnect_lcc` command called
2. EventRouter shutdown signal sent
3. Background tasks terminate gracefully
4. No more events emitted
5. Cleanup complete

**Reconnection:**
- EventRouter automatically restarted on reconnect
- Subscribers automatically reconnected
- No manual re-initialization required

---

## TypeScript Type Definitions

All types are defined in `app/src/lib/api/tauri.ts`.

---

### `ElementEntry`

```typescript
/**
 * One producer or consumer slot entry inside a BowtieCard.
 * Role is always Producer or Consumer — Ambiguous entries are in
 * BowtieCard.ambiguous_entries instead.
 */
export interface EventSlotEntry {
  node_id: string;            // "02.01.57.00.00.01"
  node_name: string;          // SNIP user_name, or "{mfg} — {model}", or node_id fallback
  element_path: string[];     // ["seg:0", "elem:3", "elem:2"]
  element_label?: string;     // Computed by the frontend from the live tree (not sent by Rust)
  event_id: number[];         // 8-byte event ID array
  role: EventRole;            // "Producer" | "Consumer"
}
```

---

### `BowtieCard`

```typescript
/**
 * One bowtie — a shared event ID with confirmed producers and consumers.
 * Emitted only when producers.length ≥ 1 AND consumers.length ≥ 1.
 */
export interface BowtieCard {
  event_id_hex: string;         // Dotted hex, e.g. "02.01.57.00.00.01.00.1E"
  event_id_bytes: number[];     // Raw 8-byte event ID
  producers: EventSlotEntry[];  // Confirmed producer slots
  consumers: EventSlotEntry[];  // Confirmed consumer slots
  ambiguous_entries: EventSlotEntry[]; // Same-node, heuristic unresolved
  name: string | null;          // User-assigned bowtie name (from layout file metadata)
  tags: string[];               // User-assigned tags (from layout file metadata)
  state: 'Active' | 'Incomplete' | 'Planning'; // Active = both sides present; Incomplete = one side empty; Planning = metadata-only, no live event
}
```

---

### `BowtieCatalog`

```typescript
/**
 * Complete set of discovered bowties from the most recent CDI + Identify Events cycle.
 */
export interface BowtieCatalog {
  bowties: BowtieCard[];        // Sorted by event_id_bytes
  built_at: string;             // ISO 8601 timestamp
  source_node_count: number;    // Nodes included in the build
  total_slots_scanned: number;  // Total event ID slots walked across all nodes
}
```

---

### `EventRole`

```typescript
/**
 * Role of an event slot, as determined by protocol reply and/or CDI heuristic.
 * "Ambiguous" means the node replied with both ProducerIdentified and ConsumerIdentified
 * for the same event ID and the CDI heuristic could not resolve which role applies.
 */
export type EventRole = 'Producer' | 'Consumer' | 'Ambiguous';
```

---

### `CdiReadCompletePayload`

```typescript
export interface CdiReadCompletePayload {
  catalog: BowtieCatalog;
  node_count: number;
}
```

---

### `ReadProgressUpdate`

```typescript
export interface ReadProgressUpdate {
  node_id: string;
  node_name: string;
  node_index: number;
  total_nodes: number;
  bytes_read: number;
  total_bytes: number | null;
}
```

---

### `NodeID`

```typescript
/**
 * Node ID represented as 6-byte array (serialized directly as array from Rust)
 */
export type NodeID = number[];
```

**Example:** `[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]`

---

### `SNIPData`

```typescript
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
```

**Example:**
```typescript
{
  manufacturer: "Acme Co.",
  model: "LCC-1000",
  hardware_version: "1.0",
  software_version: "2.3.4",
  user_name: "Living Room Light",
  user_description: "Main ceiling fixture"
}
```

---

### `SNIPStatus`

```typescript
/**
 * Status of SNIP data retrieval operation
 */
export type SNIPStatus = 
  | 'Unknown'       // Not yet queried
  | 'InProgress'    // Request in progress
  | 'Complete'      // All data retrieved
  | 'Partial'       // Some fields missing
  | 'NotSupported'  // Node doesn't support SNIP
  | 'Timeout'       // Request timed out
  | 'Error';        // Error occurred
```

---

### `ConnectionStatus`

```typescript
/**
 * Connection status of a node
 */
export type ConnectionStatus = 
  | 'Unknown'        // Status unknown
  | 'Verifying'      // Verification in progress
  | 'Connected'      // Node responding
  | 'NotResponding'; // Node not responding
```

---

### `DiscoveredNode`

```typescript
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
}
```

**Example:**
```typescript
{
  node_id: [0x09, 0x00, 0x99, 0xFF, 0x01, 0x23],
  alias: 0x123,
  snip_data: {
    manufacturer: "Example Mfg",
    model: "Node-100",
    // ... other fields
  },
  snip_status: "Complete",
  connection_status: "Connected",
  last_verified: "2026-02-16T10:30:00Z",
  last_seen: "2026-02-16T10:30:00Z"
}
```

---

### `QuerySnipResponse`

```typescript
/**
 * Response from query_snip command
 */
export interface QuerySnipResponse {
  alias: number;
  snip_data: SNIPData | null;
  status: SNIPStatus;
}
```

**Example:**
```typescript
{
  alias: 0x123,
  snip_data: { /* ... */ },
  status: "Complete"
}
```

---

### `GetCdiXmlResponse`

```typescript
/**
 * Response from CDI retrieval commands (get_cdi_xml, download_cdi)
 */
export interface GetCdiXmlResponse {
  xmlContent: string | null;    // CDI XML content (null if not available)
  sizeBytes: number | null;      // Size of XML in bytes
  retrievedAt: string | null;    // ISO 8601 timestamp when retrieved
}
```

**Example:**
```typescript
{
  xmlContent: "<?xml version=\"1.0\"?>\n<cdi>...</cdi>",
  sizeBytes: 18432,
  retrievedAt: "2026-02-17T14:23:15Z"
}
```

**Null Response (CDI not available):**
```typescript
{
  xmlContent: null,
  sizeBytes: null,
  retrievedAt: null
}
```

---

## Miller Columns CDI Navigation Commands

### `get_discovered_nodes`

Retrieve the list of discovered nodes with their CDI availability status.

**Parameters:** None

**Returns:** `Promise<GetDiscoveredNodesResponse>`

```typescript
interface GetDiscoveredNodesResponse {
  nodes: DiscoveredNode[];  // With additional hasCdi field
}

interface DiscoveredNode {
  nodeId: string;         // Formatted as "01.02.03.04.05.06"
  alias: number;
  nodeName: string;       // User name or SNIP display name
  hasCdi: boolean;        // True if CDI is cached
  // ... other fields
}
```

**Usage Example:**
```typescript
import { getDiscoveredNodes } from '$lib/api/cdi';

const response = await getDiscoveredNodes();
console.log(`Found ${response.nodes.length} nodes`);

// Filter nodes with CDI available
const nodesWithCdi = response.nodes.filter(n => n.hasCdi);
console.log(`${nodesWithCdi.length} nodes have CDI available`);
```

---

### `get_cdi_structure`

Parse CDI XML and return the top-level segment structure.

**Parameters:**
- `node_id: string` - Node ID in hex format (e.g., "01.02.03.04.05.06")

**Returns:** `Promise<CdiStructureResponse>`

```typescript
interface CdiStructureResponse {
  segments: SegmentInfo[];
  maxDepth: number;         // Maximum nesting depth in CDI
}

interface SegmentInfo {
  id: string;               // UUID for UI rendering
  name: string;             // Segment name from CDI
  description: string | null;
  space: number;            // Address space number
  hasGroups: boolean;       // Contains group elements
  hasElements: boolean;     // Contains primitive elements
  metadata: {
    pathId: string;         // Navigation path (e.g., "seg:0")
    space: number;
  };
}
```

**pathId Format:** `seg:N` where N is the 0-based segment index

**Usage Example:**
```typescript
import { getCdiStructure } from '$lib/api/cdi';

const structure = await getCdiStructure("01.02.03.04.05.06");
console.log(`CDI has ${structure.segments.length} segments`);
console.log(`Maximum depth: ${structure.maxDepth} levels`);

// Display segments
structure.segments.forEach(seg => {
  console.log(`${seg.name} (space ${seg.space})`);
  console.log(`  Path: ${seg.metadata.pathId}`);
});
```

---

### `get_column_items`

Navigate to a specific path in the CDI hierarchy and return child items.

**Parameters:**
- `node_id: string` - Node ID in hex format
- `parent_path: string[]` - Array of pathIds representing the navigation path
- `depth: number` - Current depth level (for context)

**Returns:** `Promise<GetColumnItemsResponse>`

```typescript
interface GetColumnItemsResponse {
  items: ColumnItem[];
}

interface ColumnItem {
  id: string;               // UUID for UI rendering
  name: string;             // Display name
  fullName: string | null;  // Full description
  itemType: string;         // "group" | "int" | "string" | "eventid" | etc.
  hasChildren: boolean;     // True if contains nested elements
  metadata: {
    pathId: string;         // Navigation path (e.g., "elem:2" or "elem:0#5")
    replicated?: boolean;   // True if from replicated group
    instanceIndex?: number; // 0-based instance index
    instanceNumber?: number;// 1-based instance number for display
    replication?: number;   // Total replication count
    // ... element-specific fields
  };
}
```

**pathId Format:**
- Non-replicated element: `elem:N` where N is the 0-based element index
- Replicated group instance: `elem:N#I` where N is element index, I is 1-based instance number

**Usage Example:**
```typescript
import { getColumnItems } from '$lib/api/cdi';

// Navigate to segment 0
const segmentItems = await getColumnItems(
  "01.02.03.04.05.06",
  ["seg:0"],
  1
);

console.log(`Segment contains ${segmentItems.items.length} items`);

// Navigate deeper to a replicated group instance
const groupItems = await getColumnItems(
  "01.02.03.04.05.06",
  ["seg:0", "elem:0#12"],  // Logic group, instance #12
  2
);

groupItems.items.forEach(item => {
  console.log(`${item.name} (${item.itemType})`);
  if (item.metadata.replicated) {
    console.log(`  Instance ${item.metadata.instanceNumber} of ${item.metadata.replication}`);
  }
});
```

**Why pathId System:**
- Eliminates ambiguity with CDI element names containing special characters (e.g., "Variable #1")
- Provides stable references independent of name changes
- Enables efficient O(1) path resolution via array indexing
- Separates UI identifiers (UUIDs) from navigation paths (pathIds)

---

### `get_element_details`

Retrieve detailed metadata for a specific element.

**Parameters:**
- `node_id: string` - Node ID in hex format
- `element_path: string[]` - Array of pathIds to the target element

**Returns:** `Promise<ElementDetailsResponse>`

```typescript
interface ElementDetailsResponse {
  name: string;
  description: string | null;
  dataType: string;         // "Event ID (8 bytes)", "Integer (2 bytes)", etc.
  size: number;             // Size in bytes
  offset: number;           // Memory offset
  constraints: Constraint[] | null;
  defaultValue: string | null;
  memoryAddress: string;    // "Space 253, offset 0x0010"
}

interface Constraint {
  constraintType: string;   // "min" | "max" | "map"
  value: number | string;
  label?: string;           // For map values
}
```

**Usage Example:**
```typescript
import { getElementDetails } from '$lib/api/cdi';

// Get details for an Event ID element
const details = await getElementDetails(
  "01.02.03.04.05.06",
  ["seg:0", "elem:0#5", "elem:2", "elem:0"]
);

console.log(`Element: ${details.name}`);
console.log(`Type: ${details.dataType}`);
console.log(`Description: ${details.description}`);
console.log(`Memory: ${details.memoryAddress}`);

if (details.constraints) {
  console.log('Constraints:');
  details.constraints.forEach(c => {
    console.log(`  ${c.constraintType}: ${c.value} ${c.label || ''}`);
  });
}

if (details.defaultValue) {
  console.log(`Default: ${details.defaultValue}`);
}
```

---

### `expand_replicated_group`

Expand a replicated group to return all instances.

**Parameters:**
- `node_id: string` - Node ID in hex format
- `group_path: string[]` - Path to the replicated group

**Returns:** `Promise<ExpandReplicatedGroupResponse>`

```typescript
interface ExpandReplicatedGroupResponse {
  instances: GroupInstance[];
}

interface GroupInstance {
  index: number;            // 0-based instance index
  name: string;             // Computed instance name (e.g., "Logic 12")
}
```

**Usage Example:**
```typescript
import { expandReplicatedGroup } from '$lib/api/cdi';

// Expand "Logic" group with replication=32
const expansion = await expandReplicatedGroup(
  "01.02.03.04.05.06",
  ["seg:0", "elem:0"]
);

console.log(`Group has ${expansion.instances.length} instances`);
expansion.instances.forEach(inst => {
  console.log(`[${inst.index}] ${inst.name}`);
});

// Output:
// [0] Logic 1
// [1] Logic 2
// ...
// [31] Logic 32
```

---

## Miller Columns Type Definitions

### `SegmentInfo`

```typescript
interface SegmentInfo {
  id: string;               // UUID for UI rendering (React/Svelte key)
  name: string;             // Segment name from CDI
  description: string | null;
  space: number;            // Address space number (0-255)
  hasGroups: boolean;       // True if contains <group> elements
  hasElements: boolean;     // True if contains any elements
  metadata: {
    pathId: string;         // Navigation identifier (e.g., "seg:0")
    space: number;
  };
}
```

---

### `ColumnItem`

```typescript
interface ColumnItem {
  id: string;               // UUID for UI rendering (unique per render)
  name: string;             // Display name
  fullName: string | null;  // Full description from CDI
  itemType: string;         // "group" | "int" | "string" | "eventid" | "float" | "action" | "blob"
  hasChildren: boolean;     // True if navigable (groups with children)
  metadata: {
    pathId: string;         // Navigation identifier (e.g., "elem:2" or "elem:0#5")
    replicated?: boolean;   // True if from replicated group
    instanceIndex?: number; // 0-based instance index
    instanceNumber?: number;// 1-based instance number (for display)
    replication?: number;   // Total instance count
  };
}
```

---

### `ElementDetails`

```typescript
interface ElementDetailsResponse {
  name: string;             // Element name
  description: string | null;
  dataType: string;         // Human-readable type (e.g., "Event ID (8 bytes)")
  size: number;             // Size in bytes
  offset: number;           // Memory offset within segment
  constraints: Constraint[] | null;
  defaultValue: string | null;
  memoryAddress: string;    // Formatted address (e.g., "Space 253, offset 0x0010")
}

interface Constraint {
  constraintType: string;   // "min" | "max" | "map"
  value: number | string;
  label?: string;           // For map values (key-value pairs)
}
```

---

## Frontend Stores & Utilities

These are TypeScript/Svelte modules that live entirely in the frontend. They are not Tauri commands but are key architectural pieces referenced by components.

### `bowtieMetadata.svelte.ts` — `BowtieMetadataStore`

Singleton store (Svelte 5 `$state` runes) holding all user-authored bowtie metadata: names, tags, and role classifications. Changes here drive the `EditableBowtiePreviewStore` derived computation.

**Key mutations:**
| Method | Description |
|--------|-------------|
| `createBowtie(eventIdHex, name?)` | Register a new bowtie in the metadata map |
| `deleteBowtie(eventIdHex)` | Remove all metadata for a bowtie |
| `renameBowtie(eventIdHex, name)` | Update display name |
| `addTag(eventIdHex, tag)` | Add a tag |
| `removeTag(eventIdHex, tag)` | Remove a tag |
| `classifyRole(key, role)` | Persist `'Producer' \| 'Consumer'` for `"${nodeId}:${elementPath.join('/')}"` |
| `clearAll()` | Reset to empty (used on discard) |

**Key queries:** `isDirty`, `getMetadata(hex)`, `getRoleClassification(key)`, `getAllTags()`

---

### `layout.svelte.ts` — `LayoutStore`

Manages the current layout file path, dirty state, and open/save operations via the native OS dialog plugin.

**Methods:** `loadLayout(path?)`, `saveLayout()`, `saveLayoutAs()`

**State:** `layoutPath: string | null`, `isDirty: boolean`, `currentLayout: LayoutFile | null`

---

### `bowties.svelte.ts` — `EditableBowtiePreviewStore`

Derives `EditableBowtiePreview` by merging:
1. Live `BowtieCatalog` from the backend
2. Pending event ID edits from `pendingEditsStore`
3. Name/tag/role metadata from `BowtieMetadataStore`

Each card in the preview carries `isDirty`, `dirtyFields`, and `newEntryKeys` for UI indicators. `enrichEntryLabel()` computes `element_label` from the live node tree (reflecting pending string edits and `getInstanceDisplayName()`). `isEntryStillActive()` filters catalog entries whose slot event ID has already been reassigned.

---

### `connectionRequest.svelte.ts` — `ConnectionRequestStore`

Singleton for config-first connection requests. `TreeLeafRow` calls `requestConnection(selection, role)` when the user clicks **→ New Connection**; `+page.svelte` watches `pendingRequest` and switches to the Bowties tab; `BowtieCatalogPanel` reads the request, pre-fills `NewConnectionDialog`, then calls `clearRequest()`.

---

### `pillSelection.ts` — `pillSelections`

Svelte writable store (`Map<string, number>`) persisting the selected pill (instance) index for replicated `TreeGroupAccordion` groups across view switches (e.g. Config ↔ Bowties). Key format: `"${nodeId}:${siblings[0].path.join('/')}"`.

**Exports:** `pillSelections: Writable<Map<string, number>>`, `setPillSelection(key, index)`

---

### `app/src/lib/utils/eventIds.ts`

**`generateFreshEventIdForNode(nodeId, tree)`** — Generates a unique event ID for a node that does not conflict with any existing event IDs already assigned in that node's config tree. Algorithm: parse node ID as 6 bytes, collect all 16-bit counters (bytes 6–7) of existing IDs whose first 6 bytes match, return `max+1` (or first gap if counter overflows).

---

### `app/src/lib/utils/formatters.ts`

**`isWellKnownEvent(hex)`** — Returns `true` when the given dotted-hex event ID is an LCC well-known event (Emergency Off, Emergency Stop, Duplicate Node ID, Is Train, etc.). Used in `BowtieCard` to suppress "No producers / No consumers" hints for global protocol events.

---

## Error Handling

All Tauri commands return `Result<T, String>` from Rust, which translates to rejected promises in TypeScript.

### Common Error Patterns

**Connection Errors:**
```typescript
try {
  await invoke("connect_lcc", { host, port });
} catch (error) {
  // error is a string like "Failed to connect: Connection refused"
  console.error(error);
}
```

**Not Connected:**
```typescript
try {
  const nodes = await discoverNodes();
} catch (error) {
  if (error === "Not connected to LCC network") {
    // Prompt user to connect first
  }
}
```

**Invalid Parameters:**
```typescript
try {
  await querySnip(0x1000); // Alias too large (>4095)
} catch (error) {
  // error: "Invalid alias: Alias must be 12-bit (<=0xFFF), got 0x1000"
}
```

### Best Practice Error Handling

```typescript
async function safeDiscover() {
  try {
    // Check connection first
    const status = await invoke<ConnectionInfo>("get_connection_status");
    if (!status.connected) {
      throw new Error("Not connected to LCC network");
    }
    
    // Perform discovery
    const nodes = await discoverNodes();
    return { success: true, nodes };
    
  } catch (error) {
    return { 
      success: false, 
      error: error instanceof Error ? error.message : String(error) 
    };
  }
}
```

---

## Implementation Notes

### State Management

All commands interact with shared `AppState`:
- `connection: Arc<RwLock<Option<LccConnection>>>` - Current LCC connection
- `nodes: Arc<RwLock<Vec<DiscoveredNode>>>` - Cached discovered nodes (includes CDI data)
- `host: Arc<RwLock<String>>` - Connection host
- `port: Arc<RwLock<u16>>` - Connection port

### CDI Caching

**Memory Cache:**
- CDI data stored in `DiscoveredNode.cdi` field
- Persists for lifetime of application session
- Faster access for repeated requests

**File Cache:**
- Platform-specific app data directory
- Keyed by `{manufacturer}_{model}_{software_version}.cdi.xml`
- Survives application restarts
- Shared across all nodes with same hardware/software

### Concurrency

- Discovery and SNIP commands temporarily take ownership of the connection
- CDI download commands hold connection lock during retrieval
- RwLock ensures thread-safe access
- Only one command can modify the connection at a time
- Node cache updates are atomic

### Timeouts

Default timeouts are designed for local networks:
- Discovery: 250ms (sufficient for ~100 nodes on LAN)
- SNIP queries: 5000ms (datagram protocol may need retries)
- CDI download: 5000ms per chunk (accommodates slower nodes)
- Node verification: 500ms (quick ping-like check)

Adjust timeouts for slower networks or long-distance connections.

---

## Command Registration

All commands are registered in `app/src-tauri/src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    connect_lcc,
    disconnect_lcc,
    get_connection_status,
    commands::discover_nodes,
    commands::query_snip_single,
    commands::query_snip_batch,
    commands::verify_node_status,
    commands::refresh_all_nodes,
    commands::get_cdi_xml,
    commands::download_cdi,
    // Miller Columns CDI Navigation
    commands::get_discovered_nodes,
    commands::get_cdi_structure,
    commands::get_column_items,
    commands::get_element_details,
    commands::expand_replicated_group,
    // Bowties
    commands::get_bowties,
])
```

---

## Complete Usage Example

```typescript
import { invoke } from '@tauri-apps/api/core';
import { 
  discoverNodes, 
  querySnipBatch, 
  refreshAllNodes,
  type DiscoveredNode 
} from '$lib/api/tauri';

async function initializeLCC() {
  try {
    // 1. Connect to LCC network
    await invoke("connect_lcc", { 
      host: "localhost", 
      port: 12021 
    });
    
    // 2. Discover all nodes
    const nodes = await discoverNodes(250);
    console.log(`Found ${nodes.length} nodes`);
    
    // 3. Query SNIP for all discovered nodes
    const aliases = nodes.map(n => n.alias);
    const snipResults = await querySnipBatch(aliases);
    
    // 4. Display results
    snipResults.forEach(result => {
      if (result.status === 'Complete' && result.snip_data) {
        console.log(`${result.snip_data.manufacturer} ${result.snip_data.model}`);
      }
    });
    
    // 5. Download CDI for first node (example)
    if (nodes.length > 0) {
      const firstNode = nodes[0];
      const nodeId = firstNode.node_id.map(b => 
        b.toString(16).padStart(2, '0')
      ).join('.');
      
      try {
        const cdi = await invoke("download_cdi", { nodeId });
        console.log(`CDI retrieved: ${cdi.sizeBytes} bytes`);
        
        // Later, can retrieve from cache
        const cachedCdi = await invoke("get_cdi_xml", { nodeId });
        console.log(`CDI from cache: ${cachedCdi.sizeBytes} bytes`);
      } catch (error) {
        console.log(`CDI not available for ${nodeId}`);
      }
    }
    
    // 6. Periodically refresh node status
    setInterval(async () => {
      const updated = await refreshAllNodes();
      const online = updated.filter(n => n.connection_status === 'Connected');
      console.log(`${online.length}/${updated.length} nodes online`);
    }, 10000);
    
  } catch (error) {
    console.error("LCC initialization failed:", error);
  }
}
```

---

## API Limitations & Future Enhancements

### Current Limitations

1. **Sequential SNIP Batch Queries**: `query_snip_batch` executes sequentially due to mutable borrow constraints. May be refactored to use `Arc<Mutex<Transport>>` for true concurrency.

2. **Connection Singleton**: Only one LCC connection supported per application instance.

3. **No Event Streaming**: Commands use request-response pattern. No real-time event subscription yet.

### Planned Enhancements

- [ ] Add WebSocket/event streaming for real-time node status updates
- [ ] Support concurrent SNIP queries with Arc<Mutex> refactoring
- [x] Add node verification command
- [x] Add batch SNIP query command
- [x] Add CDI structure parsing and navigation commands
- [x] Add Miller Columns support (get_cdi_structure, get_column_items, get_element_details)
- [x] Add bowtie catalog command (get_bowties) and cdi-read-complete event (Feature 006)
- [ ] Add configuration memory read/write commands (value retrieval and editing)
- [ ] Add event producer/consumer commands

---

*Document generated: February 16, 2026*  
*Last updated: February 22, 2026*
