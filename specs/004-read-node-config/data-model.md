# Data Model: Read Node Configuration

**Feature**: 004-read-node-config  
**Generated**: February 19, 2026  
**Purpose**: Define data structures for configuration value reading, caching, and progress tracking

---

## Core Entities

### 1. Configuration Value

Represents a typed configuration value read from a node's memory, including metadata for validation and display.

**Rust Backend Structure** (`app/src-tauri/src/commands/cdi.rs`):
```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ConfigValue {
    Int { value: i64, size_bytes: u8 },           // Covers 1, 2, 4, 8 byte integers
    String { value: String, size_bytes: u32 },     // Variable-length UTF-8 string
    EventId { value: [u8; 8] },                    // 8-byte event ID
    Float { value: f32 },                          // 4-byte IEEE 754 float
    Invalid { error: String },                     // Read succeeded but parsing failed
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigValueWithMetadata {
    pub value: ConfigValue,
    pub memory_address: u32,        // Absolute address (segment.origin + offset)
    pub address_space: u8,          // Always 0xFD for configuration
    pub element_path: Vec<String>,  // Path in CDI tree: ["segment", "group", "element"]
    pub timestamp: String,          // ISO 8601 timestamp when read
}
```

**TypeScript Frontend Types** (`app/src/lib/api/types.ts`):
```typescript
export type ConfigValue = 
    | { type: 'Int'; value: number; size_bytes: number }
    | { type: 'String'; value: string; size_bytes: number }
    | { type: 'EventId'; value: number[] }  // Array of 8 bytes
    | { type: 'Float'; value: number }
    | { type: 'Invalid'; error: string };

export interface ConfigValueWithMetadata {
    value: ConfigValue;
    memory_address: number;
    address_space: number;
    element_path: string[];
    timestamp: string;
}

export type ConfigValueMap = Map<string, ConfigValueWithMetadata>;  // Key: node_id:element_path
```

**Field Descriptions**:
- `value`: The typed configuration value (discriminated union)
- `memory_address`: Absolute memory address calculated from CDI (segment.origin + element.offset)
- `address_space`: LCC address space identifier (0xFD = Configuration per TN-9.7.4.2)
- `element_path`: CDI navigation path (e.g., `["Settings", "Network", "Node Name"]`)
- `timestamp`: When the value was read (for staleness detection)
- `size_bytes`: Size of the value in memory (for validation and display)

**Validation Rules**:
- Int values must have size_bytes ∈ {1, 2, 4, 8}
- String values must be valid UTF-8
- EventId values must be exactly 8 bytes
- Float values must be exactly 4 bytes (IEEE 754)
- memory_address must match CDI-specified address
- address_space must be 0xFD for all configuration values

**State Transitions**:
```
[Unread] → (read request) → [Reading] → (success) → [Valid]
                                      → (error) → [Invalid]
[Valid] → (refresh request) → [Reading] → ...
```

---

### 2. Read Progress State

Tracks the state of batch configuration reading operation across multiple nodes, enabling progress indication and cancellation.

**Rust Backend Structure** (`app/src-tauri/src/commands/cdi.rs`):
```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadProgressUpdate {
    pub total_nodes: usize,
    pub current_node_index: usize,        // 0-based index
    pub current_node_name: String,        // From SNIP data (priority cascade)
    pub current_node_id: String,          // NodeID as string
    pub total_elements: usize,            // Total across all nodes
    pub elements_read: usize,             // Count of successfully read elements
    pub elements_failed: usize,           // Count of failed reads
    pub percentage: u8,                   // 0-100
    pub status: ProgressStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ProgressStatus {
    Starting,                             // Initial state
    ReadingNode { node_name: String },    // Currently reading from a specific node
    NodeComplete { node_name: String, success: bool },  // Node finished (success/fail)
    Cancelled,                            // User cancelled operation
    Complete { success_count: usize, fail_count: usize },  // All nodes processed
}
```

**TypeScript Frontend Types** (`app/src/lib/stores/millerColumns.ts`):
```typescript
export interface ReadProgressState {
    totalNodes: number;
    currentNodeIndex: number;
    currentNodeName: string;
    currentNodeId: string;
    totalElements: number;
    elementsRead: number;
    elementsFailed: number;
    percentage: number;  // 0-100
    status: ProgressStatus;
}

export type ProgressStatus = 
    | { type: 'Starting' }
    | { type: 'ReadingNode'; node_name: string }
    | { type: 'NodeComplete'; node_name: string; success: boolean }
    | { type: 'Cancelled' }
    | { type: 'Complete'; success_count: number; fail_count: number };
```

**Field Descriptions**:
- `total_nodes`: Total number of nodes to read from
- `current_node_index`: Zero-based index of current node (for display: add 1)
- `current_node_name`: Human-readable node identifier (SNIP priority cascade)
- `current_node_id`: LCC NodeID (fallback identifier)
- `total_elements`: Total configuration elements across all nodes
- `elements_read`: Successfully read elements (increments after each read)
- `elements_failed`: Failed element reads (node timeout, parse error)
- `percentage`: Overall completion percentage (0-100)
- `status`: Current operation status (discriminated union)

**SNIP Data Priority Cascade** (for `current_node_name`):
1. user_name (if non-empty)
2. user_description (if non-empty)
3. model_name (if non-empty)
4. node_id (fallback, always available)

This cascade logic is implemented in backend and ensures consistent node identification across UI.

**State Transitions**:
```
[Starting] → [ReadingNode] → [NodeComplete] → [ReadingNode (next)] → ... 
                                             → [Complete]
          → [Cancelled] (at any point)
```

---

### 3. Value Cache

In-memory storage of configuration values to avoid redundant network reads during UI navigation. Cleared on node refresh.

**Rust Backend Structure** (`app/src-tauri/src/state.rs` - extend AppState):
```rust
use std::collections::HashMap;

pub type NodeConfigCache = HashMap<String, HashMap<String, ConfigValueWithMetadata>>;
// Outer key: node_id (as string)
// Inner key: element_path joined with "/" (e.g., "Settings/Network/Node Name")

// Add to AppState:
pub struct AppState {
    // ... existing fields ...
    pub config_cache: Arc<RwLock<NodeConfigCache>>,
}
```

**TypeScript Frontend Store** (`app/src/lib/stores/millerColumns.ts`):
```typescript
interface MillerColumnsState {
    // ... existing fields ...
    configValues: Map<string, ConfigValueWithMetadata>;  // Key: "nodeId:elementPath"
    readProgress: ReadProgressState | null;
}

// Helper functions:
function getCacheKey(nodeId: string, elementPath: string[]): string {
    return `${nodeId}:${elementPath.join('/')}`;
}

function getConfigValue(nodeId: string, elementPath: string[]): ConfigValueWithMetadata | null {
    return get(millerColumnsStore).configValues.get(getCacheKey(nodeId, elementPath)) ?? null;
}
```

**Cache Operations**:
- **Store**: `cache.insert(cache_key, value)` after successful read
- **Retrieve**: `cache.get(cache_key)` when displaying element details
- **Invalidate**: `cache.clear()` on node refresh or manual refresh
- **Update**: `cache.insert(cache_key, new_value)` on manual element refresh

**Cache Key Format**: 
- Backend: `"{node_id}:{element_path_joined}"` (e.g., `"05.01.01.01.03.01:Settings/Network/Node Name"`)
- Frontend: `"{nodeId}:{elementPath}"` (e.g., `"05.01.01.01.03.01:Settings/Network/Node Name"`)

**Note**: Both backend and frontend use colon (`:`) separator to ensure cache key consistency.

**Eviction Policy**: 
- No automatic eviction (in-memory only, cleared on app restart)
- Manual refresh overwrites cached values
- Node refresh clears entire cache

---

## Relationships

```
DiscoveredNode (1) ──< (many) ConfigValueWithMetadata
    │                           │
    │                           └── references via element_path
    │
    └── has SNIP data used for ReadProgressState.current_node_name

ReadProgressState (1) ── tracks ──> (many) ConfigValueWithMetadata
    │
    └── current_node_name derived from DiscoveredNode.snip_data

ValueCache (1) ──> (many) ConfigValueWithMetadata
    │
    └── indexed by cache_key = f"{node_id}/{element_path}"
```

**Navigation Flow**:
1. User refreshes nodes → triggers `read_all_config_values` for each node
2. Backend reads CDI structure → extracts all elements with memory addresses
3. For each element → read from address space 0xFD → parse typed value
4. Store in cache with metadata (address, path, timestamp)
5. Emit progress updates → frontend updates progress UI
6. User selects element in Miller Columns → frontend retrieves from cache
7. Display value in DetailsPanel alongside existing element metadata

---

## Data Flow Diagram

```
[Node Refresh] 
    ↓
[For each DiscoveredNode]
    ↓
[read_all_config_values(node_id)] ← Tauri command
    ↓
[Get CDI structure for node]
    ↓
[Extract all elements with memory addresses]
    ↓
[For each element]
    ├─→ [Calculate absolute address: segment.origin + element.offset]
    ├─→ [Send Memory Config Read request to address space 0xFD]
    ├─→ [Wait for datagram response (timeout 2s)]
    ├─→ [Parse response bytes based on element type]
    ├─→ [Create ConfigValueWithMetadata]
    ├─→ [Store in NodeConfigCache]
    └─→ [Emit ReadProgressUpdate event]
    ↓
[Return Map<element_path, ConfigValue>]
    ↓
[Frontend receives results]
    ↓
[Update millerColumns store: configValues cache]
    ↓
[User selects element in Miller Columns]
    ↓
[Lookup in configValues cache by cache_key]
    ↓
[Display in DetailsPanel: formatConfigValue(value)]
```

---

## Example Data Instances

### Configuration Value - String Type
```rust
ConfigValueWithMetadata {
    value: ConfigValue::String {
        value: "Tower LCC".to_string(),
        size_bytes: 32,
    },
    memory_address: 0x0000_0010,  // segment.origin (0) + element.offset (16)
    address_space: 0xFD,
    element_path: vec!["Settings".to_string(), "Identification".to_string(), "User Name".to_string()],
    timestamp: "2026-02-19T14:32:00Z".to_string(),
}
```

### Configuration Value - Event ID Type
```rust
ConfigValueWithMetadata {
    value: ConfigValue::EventId {
        value: [0x05, 0x01, 0x01, 0x01, 0x03, 0x01, 0x00, 0x00],
    },
    memory_address: 0x0000_0100,
    address_space: 0xFD,
    element_path: vec!["Events".to_string(), "Producers".to_string(), "Output 1 Active".to_string()],
    timestamp: "2026-02-19T14:32:05Z".to_string(),
}
```

### Read Progress State - Mid-Operation
```rust
ReadProgressUpdate {
    total_nodes: 3,
    current_node_index: 1,  // Second node (0-indexed)
    current_node_name: "Tower LCC".to_string(),  // From SNIP user_name
    current_node_id: "05.01.01.01.03.01".to_string(),
    total_elements: 127,
    elements_read: 48,
    elements_failed: 2,
    percentage: 38,  // (48+2) / 127 * 100
    status: ProgressStatus::ReadingNode { node_name: "Tower LCC".to_string() },
}
```

---

## Memory and Performance Considerations

### Cache Size Estimation
- Single ConfigValue: ~100-200 bytes (includes metadata)
- 100 elements per node × 10 nodes = 1000 elements
- Total cache size: ~100-200 KB (negligible for desktop application)

### Network Bandwidth
- Single memory read: ~20 bytes request + ~80 bytes response = 100 bytes
- 100 elements: ~10 KB network traffic per node
- 10 nodes: ~100 KB total (bandwidth impact minimal)

### Latency Budget
- Single element read: 2s timeout (includes network RTT + node processing)
- 50 elements per node × 2s = ~100s worst case (serial reading)
- **Optimization**: Pipeline reads where possible (send next request before response)
- Target: Complete node in 5-10s typical, 15s for 3-node network

### Concurrency Model
- Read nodes sequentially (one at a time) to avoid overwhelming network
- Read elements within a node serially (maintain datagram protocol ordering)
- Progress updates emitted after each element/node completion
- UI remains responsive via async/await (non-blocking Tauri commands)

---

## Notes

- All configuration data is stored in **address space 0xFD** per **TN-9.7.4.2 §2.1**
- Memory addresses are **absolute** (segment.origin + element.offset) per **TN-9.7.4.1 §3.2**
- Values are cached to avoid redundant network reads during UI navigation
- Cache is cleared on node refresh to ensure current data
- Progress indication uses SNIP data priority cascade for human-readable node names
- Invalid/unparseable values are stored as `ConfigValue::Invalid` to distinguish from read failures
- Timestamps enable future features like staleness indicators and auto-refresh policies
