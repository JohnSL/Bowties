# Tauri API Reference

This document describes all Tauri commands available to the frontend application, including connection management, node discovery, and SNIP data retrieval.

## Table of Contents

1. [Connection Commands](#connection-commands)
2. [Discovery Commands](#discovery-commands)
3. [CDI Commands](#cdi-commands)
4. [TypeScript Type Definitions](#typescript-type-definitions)
5. [Error Handling](#error-handling)

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
- [ ] Add configuration memory read/write commands
- [ ] Add event producer/consumer commands

---

*Document generated: February 16, 2026*  
*Last updated: February 16, 2026*
