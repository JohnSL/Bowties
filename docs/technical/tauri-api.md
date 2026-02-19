# Tauri API Reference

This document describes all Tauri commands available to the frontend application, including connection management, node discovery, and SNIP data retrieval.

## Table of Contents

1. [Connection Commands](#connection-commands)
2. [Discovery Commands](#discovery-commands)
3. [CDI Commands](#cdi-commands)
4. [Events](#events)
5. [TypeScript Type Definitions](#typescript-type-definitions)
6. [Error Handling](#error-handling)

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
- [ ] Add configuration memory read/write commands (value retrieval and editing)
- [ ] Add event producer/consumer commands

---

*Document generated: February 16, 2026*  
*Last updated: February 18, 2026*
