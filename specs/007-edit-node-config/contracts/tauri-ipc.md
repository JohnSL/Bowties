# Tauri IPC Contracts: 007-edit-node-config

**Date**: 2026-02-28  
**Transport**: Tauri 2 `invoke()` / `listen()` IPC

This project uses Tauri IPC (not REST/GraphQL). Contracts are defined as Tauri command signatures (Rust) with their TypeScript invoke wrappers.

---

## Commands

### `write_config_value`

Writes a single serialized value to a node's memory at the specified address and space. Handles chunking for data > 64 bytes, retries up to 3 times with 3-second timeout per attempt.

**Rust signature** (in `app/src-tauri/src/commands/cdi.rs`):

```rust
#[tauri::command]
pub async fn write_config_value(
    state: tauri::State<'_, AppState>,
    node_id: String,     // dotted-hex node ID, e.g. "05.01.01.01.03.00"
    address: u32,        // memory address to write
    space: u8,           // address space (0xFB or 0xFD)
    data: Vec<u8>,       // serialized value bytes (pre-serialized by frontend)
) -> Result<WriteResponse, String>
```

**TypeScript invoke wrapper** (in `app/src/lib/api/config.ts`):

```typescript
export interface WriteResponse {
  address: number;
  space: number;
  success: boolean;
  error_code: number | null;
  error_message: string | null;
  retry_count: number;
}

export async function writeConfigValue(
  nodeId: string,
  address: number,
  space: number,
  data: number[]   // byte array
): Promise<WriteResponse> {
  return invoke<WriteResponse>('write_config_value', {
    nodeId,
    address,
    space,
    data,
  });
}
```

**Errors**:
| Error | Condition |
|-------|-----------|
| `"Not connected"` | No active LCC connection |
| `"Node not found: {nodeId}"` | Node alias not found in discovered nodes |
| `"Write failed after 3 retries: {details}"` | All retry attempts exhausted |
| `"Write rejected: {error_code}"` | Node rejected the datagram permanently |

---

### `send_update_complete`

Sends the Update Complete command (`[0x20, 0xA8]`) to a node, signaling it to persist configuration changes to non-volatile storage. Fire-and-forget — waits for Datagram Received OK but does not check for errors beyond that.

**Rust signature**:

```rust
#[tauri::command]
pub async fn send_update_complete(
    state: tauri::State<'_, AppState>,
    node_id: String,     // dotted-hex node ID
) -> Result<(), String>
```

**TypeScript invoke wrapper**:

```typescript
export async function sendUpdateComplete(nodeId: string): Promise<void> {
  return invoke<void>('send_update_complete', { nodeId });
}
```

**Errors**:
| Error | Condition |
|-------|-----------|
| `"Not connected"` | No active LCC connection |
| `"Node not found: {nodeId}"` | Node alias not found |

---

## Events (Backend → Frontend)

No new Tauri events are required for this feature. The save workflow is synchronous from the frontend's perspective — each `write_config_value` call returns a `WriteResponse`, and the frontend tracks progress by iterating through pending edits sequentially.

The existing `node-tree-updated` event (already used for config reads) may be emitted if the backend updates its cached `NodeConfigTree` after writes, but this is optional since the frontend already knows the new values.

---

## lcc-rs Library API Additions

These are internal library functions, not Tauri commands, but form the protocol contract.

### `MemoryConfigCmd::build_write()`

```rust
/// Build a write command datagram.
///
/// Mirrors `build_read()` but uses write command bytes (0x00-0x03 instead of 0x40-0x43)
/// and includes data payload instead of read count.
///
/// Per OpenLCB_Java `MemoryConfigurationService.McsWriteMemo.fillRequest()`.
///
/// # Arguments
/// * `source_alias` - Our node alias
/// * `dest_alias` - Target node alias  
/// * `space` - Address space to write to
/// * `address` - Starting address (32-bit)
/// * `payload` - Data bytes to write (1-64 bytes)
///
/// # Returns
/// Vector of GridConnect frames to send
pub fn build_write(
    source_alias: u16,
    dest_alias: u16,
    space: AddressSpace,
    address: u32,
    payload: &[u8],
) -> Result<Vec<GridConnectFrame>>
```

### `MemoryConfigCmd::build_update_complete()`

```rust
/// Build an Update Complete command datagram.
///
/// Sends [0x20, 0xA8] to signal the node to persist configuration changes.
/// Per S-9.7.4.2 §4.23.
///
/// # Arguments
/// * `source_alias` - Our node alias
/// * `dest_alias` - Target node alias
///
/// # Returns
/// Vector of GridConnect frames to send
pub fn build_update_complete(
    source_alias: u16,
    dest_alias: u16,
) -> Result<Vec<GridConnectFrame>>
```

### `LccConnection::write_memory()`

```rust
/// Write data to a node's memory at the specified address and space.
///
/// Handles: datagram framing, send, wait for Datagram Received OK,
/// retry up to 3 times with 3-second timeout.
///
/// For data > 64 bytes, automatically chunks into sequential writes.
///
/// Per OpenLCB_Java `MemoryConfigurationService.sendRequest()` with
/// `RequestWithNoReply` pattern — Datagram Received OK (without Reply Pending)
/// is sufficient confirmation.
///
/// # Arguments
/// * `dest_node` - Target node ID
/// * `space` - Address space
/// * `address` - Starting address
/// * `data` - Bytes to write
///
/// # Returns
/// Ok(()) on success, Err on failure after retries
pub async fn write_memory(
    &mut self,
    dest_node: &NodeID,
    space: AddressSpace,
    address: u32,
    data: &[u8],
) -> Result<()>
```

### `LccConnection::send_update_complete()`

```rust
/// Send Update Complete command to a node.
///
/// Per OpenLCB_Java `CdiPanel.runUpdateComplete()` — fire-and-forget
/// 2-byte datagram [0x20, 0xA8].
///
/// # Arguments
/// * `dest_node` - Target node ID
///
/// # Returns
/// Ok(()) on successful datagram send and ack
pub async fn send_update_complete(
    &mut self,
    dest_node: &NodeID,
) -> Result<()>
```

---

## Value Serialization Contract

Value serialization happens in the **frontend** before invoking `write_config_value`. The frontend converts typed values to byte arrays matching the OpenLCB_Java `ConfigRepresentation` patterns.

### `serializeConfigValue()` (TypeScript)

```typescript
/**
 * Serialize a typed config value to bytes for writing to node memory.
 * 
 * Per OpenLCB_Java ConfigRepresentation value serialization:
 * - Int: big-endian bytes of CDI-defined size
 * - String: UTF-8 + null terminator (NOT full-padded)
 * - EventId: 8 raw bytes from dotted-hex
 * - Float: IEEE 754 big-endian (4 or 8 bytes)
 */
export function serializeConfigValue(
  value: TreeConfigValue,
  elementType: LeafType,
  size: number,
): number[] // byte array
```

| Type | Input | Size | Output |
|------|-------|------|--------|
| `int` | `{ type: 'int', value: 258 }` | 2 | `[0x01, 0x02]` |
| `int` | `{ type: 'int', value: 1 }` | 1 | `[0x01]` |
| `string` | `{ type: 'string', value: "Hello" }` | 64 | `[0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x00]` (6 bytes, not 64) |
| `eventId` | `{ type: 'eventId', hex: "05.01.01.01.22.00.00.FF" }` | 8 | `[0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xFF]` |
| `float` | `{ type: 'float', value: 3.14 }` | 4 | `[0x40, 0x48, 0xF5, 0xC3]` (IEEE 754 single) |
