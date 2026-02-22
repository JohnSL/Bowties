# Technical Implementation Context: Read Node Configuration

**Feature**: 004-read-node-config  
**Purpose**: Technical reference for implementation planning  
**Generated**: February 19, 2026  
**Last Updated**: February 22, 2026 (corrected protocol format details, address calculation; see `plan-fixLccMemoryConfigReading.md`)

## Executive Summary

All required infrastructure exists. Memory configuration protocol is fully implemented in `lcc-rs`. Node refresh, CDI navigation, and UI patterns are established. Integration points are clear.

---

## Backend Infrastructure (Rust)

### Memory Configuration Protocol - Already Implemented

**Location**: `lcc-rs/src/protocol/memory_config.rs`

**Key Methods Available**:
- `MemoryConfigCmd::build_read(source_alias, dest_alias, space, address, count)` (Lines 55-81)
  - Builds memory read command datagram
  - Can read 1-64 bytes per request
  - Returns datagram ready to send
  
- `MemoryConfigCmd::parse_read_reply(data)` (Lines 90-150)
  - Parses response datagram
  - Returns `ReadReply::Success { data, address, space }` or `ReadReply::Failed`
  - Uses deterministic bit-rule: `cmd & 0x03 != 0` → embedded reply, data at `[6..]`; `cmd & 0x03 == 0` → generic reply, space byte at `[6]`, data at `[7..]`
  - Note: `address_space` parameter was removed — the space is derived from the reply bytes

**Address Spaces** (Lines 10-17):
```rust
pub enum AddressSpace {
    Configuration = 0xFD,  // ← Use this for config values
    Cdi = 0xFF,            // Already used for CDI reading
    // ... others
}
```

**⚠️ Read Format — Embedded vs Generic**: The protocol has two request formats. Use the `AddressSpace::command_flag()` result to select:
- Spaces `>= 0xFD` (`Configuration 0xFD`, `AllMemory 0xFE`, `Cdi 0xFF`): **embedded format** — command byte encodes the space in bits 0–1 (`0x41`/`0x42`/`0x43`); 7-byte payload; **no separate space byte**; replies are `0x51`/`0x52`/`0x53` with data at `[6..]`
- Spaces `< 0xFD` (e.g. `AcdiUser 0xFB`, `AcdiManufacturer 0xFC`): **generic format** — command byte `0x40`; space byte at payload `[6]`; 8-byte payload; replies are `0x50` with space byte at `[6]` and data at `[7..]`

Reference: `OpenLCB_Java MemoryConfigurationService.fillRequest()` / `getPayloadOffset(data)`. This is the same implementation JMRI uses and has been verified against live traffic captures in `specs/004-read-node-config/traffic/`:
- `traffic/JMRI_async_blink.txt` — JMRI reference traffic (correct requests: `0x41` for config, `0x40` for ACDI)
- `traffic/Bowties_async_blink.txt` — Bowties post-fix traffic (confirmed matching JMRI pattern)

**Complete Working Example**: `lcc-rs/src/discovery.rs` Lines 471-620 (`read_cdi()` method)
- Shows full pattern: build request → send datagram → wait for reply → parse response
- Handle multi-chunk reads for large data
- Error handling pattern

### CDI Element Navigation - Already Implemented

**Location**: `app/src-tauri/src/commands/cdi.rs`

**Memory Addresses Already Exposed**:
- Line 737: `ElementDetailsResponse { memory_address: i32, ... }`
- Frontend already receives memory address for every element
- Currently displayed in Details Panel (Line 106 of DetailsPanel.svelte)

**Element Navigation Pattern** (Lines 754-770):
```rust
fn navigate_to_element(segments, element_path) -> Result<Element>
```
- Takes path like `["segment1", "group2", "element3"]`
- Returns the CDI Element with all metadata
- Can extract element type, size, offset from here

**Element Types with Memory Info** (`lcc-rs/src/cdi/mod.rs` Lines 86-210):
- `IntElement { offset: i32, size: u8, ... }` (Lines 91-95)
- `EventIdElement { offset: i32, ... }` (Lines 137-141) - always 8 bytes
- `StringElement { offset: i32, size: u32, ... }` (Lines 162-166)
- `FloatElement { offset: i32, ... }` (Lines 178-182) - always 4 bytes
- `Segment { origin: u32, space: u8, ... }` (Lines 86-90)
- `Group { offset: i32, replication: u16, ... }` (Lines 110-125) - can have name element

### Connection Management - Already Established

**Location**: `app/src-tauri/src/state.rs` Lines 10-27

**AppState Structure**:
```rust
pub struct AppState {
    pub connection: Arc<RwLock<Option<Arc<Mutex<LccConnection>>>>>,
    pub nodes: Arc<RwLock<Vec<DiscoveredNode>>>,
    // ... other fields
}
```

**Access Pattern** (from `commands/discovery.rs` Lines 220-227):
```rust
let conn_lock = state.connection.read().await;
let connection = conn_lock.as_ref().ok_or("Not connected")?;
let connection = connection.lock().await;
// Now use connection methods
```

### Existing Command Pattern to Follow

**Location**: `app/src-tauri/src/commands/discovery.rs` Lines 207-264

**`refresh_all_nodes` command**:
- Gets nodes from AppState
- Iterates through each node
- Calls `connection.verify_node(alias, timeout)`
- Updates state for each node
- Returns Vec<DiscoveredNode>

**Registration** (`app/src-tauri/src/lib.rs` Line 91):
```rust
.invoke_handler(tauri::generate_handler![
    refresh_all_nodes,  // ← Add new command here
    // ... other commands
])
```

---

## Frontend Infrastructure (TypeScript/Svelte)

### Node Refresh Integration Point

**Location**: `app/src/routes/+page.svelte` Lines 70-84

**Current Refresh Flow**:
```typescript
async function handleRefreshNodes() {
    isRefreshing = true;
    try {
        nodes = await refreshAllNodes(timeoutMs);
        if (millerColumnsNav) {
            millerColumnsNav.refreshNodes();  // ← Insert config read HERE (after L77)
        }
    } finally {
        isRefreshing = false;
    }
}
```

**Integration Strategy**:
1. After `refreshAllNodes()` completes (Line 75)
2. Before `millerColumnsNav.refreshNodes()` (Line 77)
3. Add: `await readAllConfigValues(nodes)` with progress callback

### Progress UI Patterns - Established

**Loading States** (`app/src/lib/components/MillerColumns/NodesColumn.svelte` Lines 7-97):
```svelte
let loading = false;
loading = true;  // Start loading
// ... do work
loading = false; // End loading

{#if loading}
  <div class="loading">Loading nodes...</div>
{/if}
```

**Status Messages** (`app/src/routes/+page.svelte` Lines 213-215):
```svelte
<p class="status">Scanning network for nodes...</p>
<p class="status">Loading node information...</p>
```

**Spinner Animation** (`app/src/lib/components/CdiXmlViewer.svelte` Lines 266-280):
```css
.spinner {
  border: 4px solid #f3f3f3;
  border-top: 4px solid #667eea;
  animation: spin 1s linear infinite;
}
```

### Miller Columns Store - State Management

**Location**: `app/src/lib/stores/millerColumns.ts`

**Current State** (Lines 93-99):
```typescript
interface MillerColumnsState {
    selectedNode: DiscoveredNode | null;
    selectedElementDetails: ElementDetails | null;
    isLoading: boolean;
    error: string | null;
    // ... columns, breadcrumb
}
```

**Available Methods** (Lines 241-257):
- `setLoading(isLoading: boolean)` - Line 241
- `setError(error: string | null)` - Line 249
- Store is reactive - components auto-update

**Extend This Store For Config Values**:
```typescript
// Add to state:
configValues: Map<string, ConfigValue>; // Key: "nodeId:elementPath"
readProgress: { current: number, total: number, nodeName: string } | null;
```

### Details Panel - Value Display Location

**Location**: `app/src/lib/components/MillerColumns/DetailsPanel.svelte`

**Current Display** (Lines 106-109):
```svelte
<div class="detail-row">
    <div class="detail-label">Memory Address:</div>
    <div class="detail-value">{formatMemoryAddress(details.memoryAddress)}</div>
</div>
```

**Add After This**:
```svelte
<div class="detail-row">
    <div class="detail-label">Current Value:</div>
    <div class="detail-value">{formatConfigValue(value)}</div>
</div>
```

**Helper Functions Already Present** (Line 32):
- `formatMemoryAddress(addr)` - converts to hex
- Can add similar formatters for values

### API Pattern - Tauri Command Invocation

**Location**: `app/src/lib/api/tauri.ts`

**Existing Pattern** (Lines 117-119):
```typescript
export async function refreshAllNodes(timeout_ms?: number): Promise<DiscoveredNode[]> {
    return await invoke<DiscoveredNode[]>('refresh_all_nodes', { timeout_ms });
}
```

**Add New Command**:
```typescript
export async function readConfigValues(
    node_id: string,
    onProgress?: (current: number, total: number, nodeName: string) => void
): Promise<ConfigValueMap> {
    return await invoke<ConfigValueMap>('read_config_values', { node_id });
}
```

---

## Data Structures Reference

### DiscoveredNode (Frontend)

**Location**: `app/src/lib/api/types.ts`

```typescript
interface DiscoveredNode {
    node_id: NodeID;
    alias: number;
    snip_data: SNIPData | null;  // ← Contains user_name, user_description, model_name
    snip_status: SNIPStatus;
    connection_status: ConnectionStatus;
    last_verified: string | null;
    last_seen: string;
}

interface SNIPData {
    manufacturer_name: string;
    model_name: string;
    hardware_version: string;
    software_version: string;
    user_name: string;          // ← Priority 1 for display
    user_description: string;   // ← Priority 2 for display
}
```

### CDI Segment Structure

**Location**: `lcc-rs/src/cdi/mod.rs` Lines 86-90

```rust
pub struct Segment {
    pub origin: u32,     // Base address for this segment
    pub space: u8,       // Address space (0xFD = Configuration)
    pub name: Option<String>,
    pub description: Option<String>,
    pub elements: Vec<Element>,
}
```

**Calculating Absolute Address**:

⚠️ **The CDI `offset` attribute is a relative skip (gap), not an absolute address.** It specifies how many bytes to skip *from the end of the previous element* before this element begins. For a sequential group of elements with no explicit offsets, the address of element N is the sum of the origin and the sizes of all preceding elements.

The correct calculation uses a running cursor (see `process_elements()` in `app/src-tauri/src/commands/cdi.rs`):
```
cursor += element.offset    // skip any explicit gap before this element
absolute_address = segment.origin + base_offset + cursor
cursor += element.size      // advance past this element's bytes
```
For nested groups: `base_offset = group_start + instance_index * group.calculate_size()`

### Element Path Structure

**Used Throughout Frontend**:
```typescript
type ElementPath = string[];  // e.g., ["segment1", "group2", "element3"]
```

**Used in Backend**:
```rust
element_path: Vec<String>
```

---

## Performance Considerations

### Memory Read Constraints

- **Max 64 bytes per datagram** (memory_config.rs Line 60)
- For larger values: need multiple reads
- Most config elements are ≤ 64 bytes

### Parallel Reading Opportunities

- Can read from different nodes in parallel using async/await
- Can read multiple elements from same node in sequence
- Datagram protocol handles concurrent operations

### Timeout Recommendations

- Single element read: 2 seconds (per assumptions)
- Node with 50 elements: ~5 seconds (per SC-004)
- 3-node network: ~15 seconds total (per SC-004)

---

## Implementation Checklist

### Backend (Rust)

- [ ] Create `read_config_value(node_id, element_path)` command in `commands/cdi.rs`
  - Navigate to element using existing `get_element_details` logic
  - Calculate absolute address: segment.origin + element.offset
  - Use `MemoryConfigCmd::build_read()` with AddressSpace::Configuration
  - Send datagram via connection
  - Parse reply with `parse_read_reply()`
  - Format value based on element type (Int/String/EventId/Float)
  - Return typed value
  
- [ ] Create `read_all_config_values(node_id)` command
  - Get CDI structure for node
  - Iterate all elements with memory addresses (including groups, segments)
  - Handle replicated groups (expand all instances)
  - Read each element's value
  - Return Map of path → value
  - Emit progress events for frontend

- [ ] Register commands in `lib.rs` invoke_handler

### Frontend (TypeScript/Svelte)

- [ ] Add API wrappers in `lib/api/cdi.ts`
  - `readConfigValue(nodeId, elementPath)`
  - `readAllConfigValues(nodeId, onProgress)`

- [ ] Extend Miller Columns store (`lib/stores/millerColumns.ts`)
  - Add `configValues` map
  - Add `readProgress` state
  - Add actions: `setConfigValue()`, `updateProgress()`, `clearConfigValues()`

- [ ] Update refresh flow (`routes/+page.svelte`)
  - After `refreshAllNodes()` completes
  - Loop through nodes
  - Call `readAllConfigValues()` for each
  - Update progress display
  - Store values in Miller Columns store

- [ ] Add progress indicator component or extend existing status display
  - Show "Reading [Node Name] config... X%"
  - Use SNIP data priority: user_name > user_description > model_name > node_id

- [ ] Update Details Panel (`components/MillerColumns/DetailsPanel.svelte`)
  - Subscribe to config values from store
  - Format and display value for selected element
  - Add "Refresh Value" button

---

## Key Files Summary

### Must Modify:
1. `app/src-tauri/src/commands/cdi.rs` - Add new commands
2. `app/src-tauri/src/lib.rs` - Register commands
3. `app/src/lib/api/cdi.ts` - Add API wrappers
4. `app/src/lib/stores/millerColumns.ts` - Extend state
5. `app/src/routes/+page.svelte` - Update refresh flow
6. `app/src/lib/components/MillerColumns/DetailsPanel.svelte` - Display values

### Reference (Don't Modify):
1. `lcc-rs/src/protocol/memory_config.rs` - Use existing protocol
2. `lcc-rs/src/cdi/mod.rs` - Reference element types
3. `lcc-rs/src/discovery.rs` - Follow `read_cdi()` pattern
4. `app/src/lib/api/types.ts` - Use existing types

---

## Testing Strategy

### Manual Testing Setup
1. Need at least 2 LCC nodes on network
2. Nodes should have SNIP data with different user names
3. Nodes should have varied config elements (strings, ints, event IDs)
4. At least one node should have replicated groups

### Validation Points
- Progress updates smoothly with correct node names
- All element values display correctly in Details Panel
- Handle node timeout gracefully (continue with next node)
- Cancel operation works without breaking state
- Values persist when navigating between elements
- Manual refresh updates displayed value

### Success Metrics (from SC-001 to SC-008)
- View value within 2 seconds of selection
- Progress updates smoothly showing percentage
- Support 100+ config elements per node
- Complete 3-node network in ≤15 seconds
- <5% error rate in normal conditions
- UI remains responsive (can cancel)
- 90% of values displayed in readable format
