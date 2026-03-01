# Data Model: 007-edit-node-config

**Date**: 2026-02-28  
**Spec**: [spec.md](spec.md) | **Research**: [research.md](research.md)

## Entity Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Frontend (Svelte)                     │
│  ┌─────────────────┐    ┌────────────────────────────┐  │
│  │ PendingEditsStore│───▶│  PendingEdit (per field)   │  │
│  │  (Map by key)   │    │  - originalValue           │  │
│  │                 │    │  - pendingValue             │  │
│  │  Derived:       │    │  - validationState          │  │
│  │  - dirtyCount   │    │  - writeState               │  │
│  │  - hasInvalid   │    └────────────────────────────┘  │
│  │  - perNode      │                                    │
│  │  - perSegment   │    ┌────────────────────────────┐  │
│  └─────────────────┘    │  WriteResult (per write)   │  │
│                         │  - success/failure          │  │
│                         │  - errorMessage             │  │
│                         └────────────────────────────┘  │
│                         ┌────────────────────────────┐  │
│                         │  SaveProgress              │  │
│                         │  - total / completed        │  │
│                         │  - currentField             │  │
│                         │  - state (idle/saving/done) │  │
│                         └────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                          │ Tauri invoke()
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  Backend (Rust/Tauri)                    │
│  ┌─────────────────┐    ┌────────────────────────────┐  │
│  │ WriteRequest     │───▶│  lcc-rs write_memory()    │  │
│  │  - nodeId        │    │  - build_write()           │  │
│  │  - address       │    │  - send datagram           │  │
│  │  - space         │    │  - await ack               │  │
│  │  - data (bytes)  │    │  - retry (3x / 3s)         │  │
│  └─────────────────┘    └────────────────────────────┘  │
│                                                         │
│  ┌─────────────────────────────────────────────────────┐│
│  │ Memory Config Commands (lcc-rs)                     ││
│  │  build_write() → Vec<GridConnectFrame>              ││
│  │  build_update_complete() → Vec<GridConnectFrame>    ││
│  │  parse_write_reply() → WriteReply (future use)      ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

## Frontend Entities

### PendingEdit

Represents a single field that has been modified but not yet saved.

| Field | Type | Description |
|-------|------|-------------|
| `key` | `string` | Unique key: `"${nodeId}:${space}:${address}"` |
| `nodeId` | `string` | Node ID in dotted-hex (e.g., `"05.01.01.01.03.00"`) |
| `segmentOrigin` | `number` | Segment origin address (for per-segment queries) |
| `segmentName` | `string` | Segment name (for display in progress) |
| `address` | `number` | Memory address of the field |
| `space` | `number` | Address space byte (0xFB or 0xFD) |
| `size` | `number` | Field size in bytes (from CDI) |
| `elementType` | `LeafType` | `'int' \| 'string' \| 'eventId' \| 'float'` |
| `fieldPath` | `string[]` | Path in the config tree (for display) |
| `fieldLabel` | `string` | Human-readable field name |
| `originalValue` | `TreeConfigValue` | Value last read from the node |
| `pendingValue` | `TreeConfigValue` | Current user-entered value |
| `validationState` | `ValidationState` | `'valid' \| 'invalid'` |
| `validationMessage` | `string \| null` | Error message if invalid |
| `writeState` | `WriteState` | `'dirty' \| 'writing' \| 'error' \| 'clean'` |
| `writeError` | `string \| null` | Error message if write failed |
| `constraints` | `LeafConstraints \| null` | CDI-defined constraints (min, max, mapEntries) |

**State transitions**:
```
User edits field → dirty
User reverts to original → removed from store
User clicks Save → writing
Write succeeds → clean (removed from store)
Write fails after retries → error
User edits error field → dirty (clears error)
User clicks Discard → removed from store
```

**Validation rules**:
| Type | Rule | Message |
|------|------|---------|
| `int` (no map) | `min <= value <= max` (if constraints exist) | "Value must be between {min} and {max}" |
| `int` (with map) | Value must be one of map entry values | "Invalid selection" |
| `string` | `length <= size - 1` (null terminator) | "Text too long (max {size-1} characters)" |
| `eventId` | Regex: `^[0-9A-Fa-f]{2}(\.[0-9A-Fa-f]{2}){7}$` | "Must be 8 bytes in dotted hex (e.g., 05.01.01.01.22.00.00.FF)" |
| `float` | Must be a valid number, `min <= value <= max` if constraints | "Must be a valid number" / "Value must be between {min} and {max}" |

### WriteResult

Outcome of a write operation for a single field, returned from the Tauri command.

| Field | Type | Description |
|-------|------|-------------|
| `address` | `number` | Memory address that was written |
| `space` | `number` | Address space that was written |
| `success` | `boolean` | Whether the write succeeded |
| `errorCode` | `number \| null` | Protocol error code if failed |
| `errorMessage` | `string \| null` | Human-readable error message |
| `retryCount` | `number` | Number of attempts made (1-3) |

### SaveProgress

Tracks the overall save operation state in the UI.

| Field | Type | Description |
|-------|------|-------------|
| `state` | `SaveState` | `'idle' \| 'saving' \| 'completed' \| 'partial-failure'` |
| `total` | `number` | Total number of fields to write |
| `completed` | `number` | Number of fields written successfully so far |
| `failed` | `number` | Number of fields that failed |
| `currentFieldLabel` | `string \| null` | Label of the field currently being written |

### ValidationState (enum)

```typescript
type ValidationState = 'valid' | 'invalid';
```

### WriteState (enum)

```typescript
type WriteState = 'dirty' | 'writing' | 'error' | 'clean';
```

### SaveState (enum)

```typescript
type SaveState = 'idle' | 'saving' | 'completed' | 'partial-failure';
```

## Backend Entities (Rust)

### WriteRequest (Tauri command input)

Received from the frontend via Tauri `invoke()`.

| Field | Type (Rust) | Description |
|-------|-------------|-------------|
| `node_id` | `String` | Target node ID (dotted-hex) |
| `address` | `u32` | Memory address to write |
| `space` | `u8` | Address space (0xFB or 0xFD) |
| `data` | `Vec<u8>` | Serialized value bytes |

### WriteResponse (Tauri command output)

Returned to the frontend.

| Field | Type (Rust) | Description |
|-------|-------------|-------------|
| `address` | `u32` | Memory address that was written |
| `space` | `u8` | Address space |
| `success` | `bool` | Whether write succeeded |
| `error_code` | `Option<u16>` | Protocol error code |
| `error_message` | `Option<String>` | Error description |
| `retry_count` | `u32` | Number of attempts |

### WriteReply (lcc-rs protocol type)

Result of parsing a Memory Configuration write reply datagram (future use — currently writes use Datagram Received OK only).

| Field | Type (Rust) | Description |
|-------|-------------|-------------|
| variant | `enum` | `Ok { address, space }` or `Fail { address, space, error_code, error_message }` |

### AddressSpace (existing, no changes)

Already exists in `lcc-rs/src/protocol/memory_config.rs`. The `command_flag()` method returns read command bytes (0x40-0x43). For writes, compute `write_cmd = command_flag() - 0x40` to get 0x00-0x03.

## Value Serialization (Rust → bytes)

### serialize_config_value()

Converts a typed config value to bytes for writing to node memory.

| Input Type | CDI Size | Output Bytes | Method |
|------------|----------|--------------|--------|
| `Int(value)` | 1 | `[v as u8]` | Truncate to size |
| `Int(value)` | 2 | `(v as u16).to_be_bytes()` | Big-endian |
| `Int(value)` | 4 | `(v as u32).to_be_bytes()` | Big-endian |
| `Int(value)` | 8 | `(v as u64).to_be_bytes()` | Big-endian |
| `String(value)` | N | `value.as_bytes()[..min(len, N-1)] + [0x00]` | UTF-8, null-terminated, NOT padded |
| `EventId(bytes)` | 8 | `bytes[0..8]` | Raw 8 bytes |
| `Float(value)` | 4 | `(v as f32).to_be_bytes()` | IEEE 754 single |
| `Float(value)` | 8 | `(v as f64).to_be_bytes()` | IEEE 754 double |

**Per OpenLCB_Java reference**: String writes only include `string_length + 1` bytes (not full field width). This minimizes wire traffic and flash wear on nodes.

## Relationships

```
NodeConfigTree (existing)
  └── SegmentNode[] (existing)
       └── ConfigNode[] (existing)
            └── LeafNode (existing) ──address,space──▶ PendingEdit.key
                                        │
                                        ▼
                              PendingEditsStore (new)
                                        │
                                        ▼ on Save
                              WriteRequest (new)
                                        │
                                        ▼ Tauri invoke
                              write_memory() (new in lcc-rs)
                                        │
                                        ▼
                              build_write() (new in lcc-rs)
                                        │
                                        ▼
                              GridConnectFrame (existing)
                                        │
                                        ▼
                              TcpTransport.send() (existing)
```
