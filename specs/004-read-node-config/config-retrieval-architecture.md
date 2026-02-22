# Configuration Data Retrieval Architecture

This document describes the full pipeline for reading LCC node configuration values, the current known issues, and the files a future session needs to make improvements.

## Pipeline Overview

```
User selects an element in the Miller columns UI
        │
        ▼
read_config_value (Tauri command) OR read_all_config_values
  app/src-tauri/src/commands/cdi.rs
        │
        ├── get_cdi_from_cache() — get cached parsed CDI
        ├── navigate_to_element() — find element in CDI tree by path
        ├── extract_address_info() — get segment origin + element offset
        │
        ▼
LccConnection::read_memory() — lcc-rs/src/discovery.rs
  Sends Memory Config Protocol datagram to node
  Waits for datagram reply
  Assembles multi-frame datagrams
  Returns raw bytes
        │
        ▼
parse_config_value() — interprets bytes as typed value
  Returns ConfigValue::Int / String / EventId / Float / Invalid
        │
        ▼
ConfigValueWithMetadata — returned to frontend
  Includes typed value, address, address_space, path, timestamp
```

## Key Files

| File | Purpose |
|------|---------|
| `app/src-tauri/src/commands/cdi.rs` | All Tauri commands: CDI download, Miller columns navigation, config read |
| `lcc-rs/src/discovery.rs` | `LccConnection::read_memory()`, `read_cdi()`, datagram assembly |
| `lcc-rs/src/cdi/mod.rs` | CDI data model: `Cdi`, `Segment`, `DataElement`, `Group`, `IntElement` etc. |
| `lcc-rs/src/cdi/parser.rs` | `parse_cdi(xml)` — parses CDI XML into the `Cdi` struct |
| `lcc-rs/src/cdi/hierarchy.rs` | `navigate_to_path()`, `NavigationResult`, `calculate_max_depth()` |
| `lcc-rs/src/protocol/` | `MemoryConfigCmd`, `DatagramAssembler`, `GridConnectFrame`, `MTI` |
| `app/src/lib/api/types.ts` | `ConfigValue`, `ConfigValueWithMetadata`, `ReadProgressState` TypeScript types |
| `specs/004-read-node-config/traffic/Bowties_async_blink.txt` | Captured traffic for a full discover + SNIP + CDI + config read sequence |

## Data Model

### CDI Structure (lcc-rs/src/cdi/mod.rs)

```rust
pub struct Cdi {
    pub segments: Vec<Segment>,
}

pub struct Segment {
    pub name: Option<String>,
    pub description: Option<String>,
    pub space: u8,      // Address space: 0xFD = config, 0xFE = all memory, 0xFF = CDI
    pub origin: i32,    // Starting address within the space
    pub elements: Vec<DataElement>,
}

pub enum DataElement {
    Group(Group),
    Int(IntElement),
    String(StringElement),
    EventId(EventIdElement),
    Float(FloatElement),
    Action(ActionElement),
    Blob(BlobElement),
}

pub struct Group {
    pub name: Option<String>,
    pub offset: i32,       // Byte offset from segment origin
    pub replication: u32,  // Number of instances (default 1)
    pub repname: Vec<String>,
    pub elements: Vec<DataElement>,
}

pub struct IntElement {
    pub name: Option<String>,
    pub offset: i32,     // Byte offset from containing group/segment
    pub size: u8,        // 1, 2, 4, or 8 bytes
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub default: Option<i64>,
    pub map: Option<MapValues>,  // Named values for enum-like fields
}
```

### Memory Address Calculation

```
absolute_address = segment.origin + element.offset
address_space    = segment.space   (NOT always 0xFD!)
```

For replicated groups, each instance `i` (0-indexed) starts at:
```
instance_address = group.offset + (i * group_stride)
```
where `group_stride` must be computed from the total size of the group's elements.

## Tauri Commands

All commands are in `app/src-tauri/src/commands/cdi.rs` and registered in `app/src-tauri/src/commands/mod.rs`.

| Command | Purpose |
|---------|---------|
| `download_cdi` | Downloads CDI XML from node via Memory Config Protocol, caches to disk + memory |
| `get_cdi_xml` | Returns cached CDI XML (does NOT download if missing) |
| `get_cdi_structure` | Returns segments list for Miller column level 2 |
| `get_column_items` | Returns groups or primitive elements for a given path, for Miller columns |
| `get_element_details` | Returns element metadata (type, constraints, address, description) |
| `read_config_value` | Reads a single element's current value from node memory |
| `read_all_config_values` | Reads all elements, emits `config-read-progress` events |
| `cancel_config_reading` | Sets atomic cancellation flag |

### Progress Events

`read_all_config_values` emits `"config-read-progress"` Tauri events with `ReadProgressUpdate` payload:

```rust
pub struct ReadProgressUpdate {
    pub total_nodes: usize,
    pub current_node_index: usize,
    pub current_node_name: String,
    pub current_node_id: String,
    pub total_elements: usize,
    pub elements_read: usize,
    pub elements_failed: usize,
    pub percentage: u8,
    pub status: ProgressStatus,   // Starting | ReadingNode | NodeComplete | Cancelled | Complete
}
```

## Caching Strategy

Two levels of CDI caching:

1. **Disk cache** — `{app_data_dir}/cdi_cache/{manufacturer}_{model}_{version}.cdi.xml`
   - Populated by `download_cdi`
   - Read by `get_cdi_xml`
   - Shared across sessions — a given hardware model only needs one download

2. **Memory parse cache** — `CDI_PARSE_CACHE: Arc<RwLock<HashMap<String, Cdi>>>`
   - Populated on first `get_cdi_from_cache()` call
   - Keyed by node ID string (hex)
   - Avoids re-parsing XML on every read

## Known Issues

### 1. Address space hardcoded to 0xFD in read commands

**Location:** `read_config_value` (line ~1450) and `read_all_config_values` (line ~1610)

**Problem:** Both commands always pass `address_space: 0xFD` to `read_memory()`. The CDI `Segment` struct has a `space: u8` field that specifies the actual address space. Some nodes have configuration segments in address spaces other than `0xFD` (e.g., `0xFE`). Reads will silently return wrong data or fail.

**Fix:** `extract_all_elements_with_addresses` should also return the `segment.space`, and `read_config_value`/`read_all_config_values` should pass it to `read_memory` instead of the hardcoded `0xFD`.

**Change needed in:**
- `extract_all_elements_with_addresses` return type: add `u8` for space
- `read_config_value`: use `segment.space` not `0xFD`
- `read_all_config_values`: use `segment.space` not `0xFD`

---

### 2. Group replication address offsets not applied

**Location:** `extract_all_elements_with_addresses` (line ~1290), inner `process_elements` function

**Problem:** When `g.replication > 1`, the code iterates `for instance in 0..g.replication` and gives each a path like `GroupName[0]`, `GroupName[1]`, etc. — but it does **not** offset element addresses by the instance stride. All instances will read from the same memory address (instance 0's address), which is wrong.

The stride for instance `i` should be `i * total_group_size_bytes`, where `total_group_size_bytes` is the sum of all element sizes within the group.

**Fix:** Compute stride from group element sizes and add `instance * stride` to `segment_origin` (or pass it alongside the offset) for each replicated instance.

---

### 3. `read_memory_impl` does not filter datagrams by source/destination

**Location:** `read_memory_impl` in `lcc-rs/src/discovery.rs` (line ~760)

**Problem:** The assembler receives **every** frame, not just datagrams from the target node:

```rust
// Current (wrong):
if let Ok(Some(datagram_data)) = assembler.handle_frame(&frame) { ... }

// Should be (correct, as done in read_cdi_impl):
if let Ok((mti, source, dest)) = MTI::from_datagram_header(frame.header) {
    if source == dest_alias && dest == our_alias && matches!(mti, ...) {
        if let Some(complete_payload) = assembler.handle_frame(&frame)? { ... }
    }
}
```

On a busy network with multiple nodes, a datagram from a different node could corrupt the assembly state or cause the wrong data to be returned.

---

### 4. `navigate_to_element` uses names; Miller columns use `pathId` strings

**Location:** `navigate_to_element` (line ~1095), `get_column_items` (line ~480)

**Problem:** `get_column_items` assigns `pathId` values to each item's `metadata` (e.g., `"seg:0"`, `"elem:2"`, `"elem:5#1"`). These are positional (index-based) identifiers. `navigate_to_element`, used by `read_config_value`, navigates by **name** (e.g., `["Configuration", "Channel", "Delay"]`). 

If the frontend passes a `pathId`-based path to `read_config_value`, navigation will fail because `navigate_to_element` tries to match `"elem:2"` as an element name.

There are two path representations in use:
- **Name-based**: `["Segment Name", "Group Name", "Element Name"]` — used by `navigate_to_element` and `get_element_details`
- **Index-based pathId**: `"seg:0"`, `"elem:2"`, `"elem:5#1"` — stored in `metadata.pathId` by `get_column_items`

These need to be unified. The recommended approach is to always use name-based paths end-to-end, since element names are stable and human-readable.

---

### 5. `get_element_details` does not use CDI parse cache

**Location:** `get_element_details` (line ~740)

**Problem:** It calls `lcc_rs::cdi::parser::parse_cdi` directly instead of `get_cdi_from_cache`. For large CDIs, this re-parses the XML on every call to get element metadata.

**Fix:** Replace the direct parse call with `get_cdi_from_cache(&node_id, &app_handle, &state).await?`.

---

### 6. Progress events emitted sparsely (every 10 elements)

**Location:** `read_all_config_values` (line ~1570)

**Problem:** Progress is only emitted every 10 elements (`if index % 10 == 0 || index == total_count - 1`). For a node with 12 elements, you get only 2 progress updates (at 0 and 11). The UI progress bar barely moves.

**Fix:** Emit on every element, or at least every element (the cost is minimal — it's just a Tauri IPC message).

---

### 7. `read_all_config_values` reads elements sequentially and holds the connection lock per read

**Location:** `read_all_config_values` (line ~1550)

**Problem:** Each element is read with:
```rust
let mut conn = connection.lock().await;
let response_data = conn.read_memory(...).await?;
drop(conn);
```

The lock is held for the full duration of the datagram round-trip (~100ms per element based on the traffic capture). For a node with 30 elements, this means ~3 seconds of sequential, single-threaded reads. 

This is the correct approach for LCC (the protocol is inherently request-response per datagram), but the 2-second per-element timeout is very conservative and causes unnecessary delay when a node is slow to respond. Consider:
- Using a shorter timeout (500ms) with retry logic
- Batching multiple reads if the node supports it (only possible if a future `MemoryConfigCmd::build_read_batch` is added)

## Protocol Reference (Memory Configuration)

Each config read is a Memory Configuration Protocol transaction:

1. **Request datagram** sent by us:
   - Payload: `[0x20, 0x40/0x41/0x42, addr_high, addr_mid, addr_low, count]`
   - `0x40` = read from config space (0xFD), `0x41` = read from all memory (0xFE), `0x42` = read next (relative)
   - Single or multi-frame datagram depending on payload size

2. **DatagramReceivedOK** sent by node (acknowledges our request)

3. **Reply datagram** sent by node:
   - Payload: `[0x20, 0x50/0x51/0x52, addr_high, addr_mid, addr_low, ...data...]`
   - `0x50` = read reply from config space

4. **DatagramReceivedOK** sent by us (acknowledges node's reply)

See the traffic capture in `specs/004-read-node-config/traffic/Bowties_async_blink.txt` for full annotated examples.

## Common Change Scenarios

### Fix address space for non-0xFD segments

In `extract_all_elements_with_addresses`, change the return type from:
```rust
Vec<(Vec<String>, u32, u32, String, &DataElement)>   // path, origin, offset, name, element
```
to:
```rust
Vec<(Vec<String>, u8, u32, u32, String, &DataElement)>  // path, space, origin, offset, name, element
```

Then thread `segment.space` through to the `read_memory(alias, space, absolute_address, ...)` call.

### Fix replicated group address calculation

In `process_elements` inside `extract_all_elements_with_addresses`, after computing the group total size, add:
```rust
let instance_offset = group.offset as u32 + (instance as u32 * group_stride);
// pass instance_offset as an additional offset parameter to recursive calls
```

This requires computing `group_stride` = sum of all element sizes (recursively for nested groups).

### Unify path representation

Option A: Always use name-based paths. Change `get_column_items` `metadata.pathId` to store the full name-based path array (JSON array) instead of `"seg:0"` / `"elem:2"` strings. Frontend stores and passes this array to `read_config_value`.

Option B: Use index-based paths everywhere. Rewrite `navigate_to_element` to navigate by `"seg:N"` / `"elem:N"` indices instead of names. This is more robust when elements share names.

### Add source/dest filtering to read_memory_impl

Replace the unfiltered `assembler.handle_frame` call with the same pattern used in `read_cdi_impl`:
```rust
if let Ok((mti, source, dest)) = MTI::from_datagram_header(frame.header) {
    if source == dest_alias && dest == our_alias && 
       matches!(mti, MTI::DatagramOnly | MTI::DatagramFirst | MTI::DatagramMiddle | MTI::DatagramFinal) {
        if let Some(complete_payload) = assembler.handle_frame(&frame)? {
            // send ack, parse reply ...
        }
    }
}
```
