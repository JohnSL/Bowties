# Research: 007-edit-node-config

**Date**: 2026-02-28  
**Status**: Complete  
**Reference Implementation**: `OpenLCB_Java` (workspace folder)

## R1: Memory Configuration Protocol Write Commands

**Decision**: Mirror the existing `build_read()` pattern exactly, with write command bytes offset by -0x40 from read command bytes.

**Rationale**: The OpenLCB_Java reference implementation (`MemoryConfigurationService.java`) uses identical address/space encoding for writes and reads. The only difference is the command byte's high bits (read = `0b01`, write = `0b00`). Our existing `build_read()` already implements the correct address and space encoding, so `build_write()` can follow the same structure.

**Alternatives considered**:
- Building a generalized command builder abstracting read/write: Rejected — the payloads differ (read has a count byte, write has data bytes), making a shared builder more complex than two simple methods.

### Write Command Byte Layout (per S-9.7.4.2 §4.8)

| Cmd byte | Space | Format |
|----------|-------|--------|
| `0x00` | Generic (space in byte 6) | For spaces < 0xFD (e.g., 0xFB ACDI User) |
| `0x01` | 0xFD (Configuration) | Embedded — no space byte |
| `0x02` | 0xFE (All Memory) | Embedded — no space byte |
| `0x03` | 0xFF (CDI) | Embedded — no space byte |

Relationship: `write_cmd = read_cmd - 0x40` (read 0x41 → write 0x01, etc.)

### Write Datagram Structure

**Embedded format** (spaces ≥ 0xFD):
```
[0x20] [cmd: 0x01/0x02/0x03] [addr_hi] [addr_mid_hi] [addr_mid_lo] [addr_lo] [data: 1-64 bytes]
```

**Generic format** (spaces < 0xFD, e.g., 0xFB):
```
[0x20] [0x00] [addr_hi] [addr_mid_hi] [addr_mid_lo] [addr_lo] [space_byte] [data: 1-64 bytes]
```

Reference: `MemoryConfigurationService.fillRequest()` at line 348 and `McsWriteMemo.fillPayload()` at line 641.

### Write Acknowledgment Flow

**Decision**: Implement the simple path (Datagram Received OK without Reply Pending = success). Do NOT wait for Write Reply datagrams initially.

**Rationale**: The OpenLCB_Java reference implementation uses `RequestWithNoReply` for writes. The old write-reply handling code is commented out (lines 119-137). The `McsWriteMemo` class explicitly implements `RequestWithNoReply`, meaning the Datagram Acknowledged message is the only confirmation expected. This is sufficient for all current LCC hardware.

**Flow**:
```
Sender                              Remote Node
  |-- Write Datagram ----------------->|
  |<-- Datagram Received OK (flags) ---|
  |                                    |
  If Reply Pending NOT set → success (most common)
  If Reply Pending SET → still treat as success (per Java reference)
```

**Alternatives considered**:
- Implementing full Write Reply datagram handling: Deferred — the Java reference doesn't use it, and it adds complexity for a path that current hardware doesn't exercise.

### Update Complete Command

**Decision**: Send `[0x20, 0xA8]` as a fire-and-forget datagram after all writes in a save batch complete.

**Rationale**: Per S-9.7.4.2 §4.23 and the Java reference `CdiPanel.runUpdateComplete()`, this is a 2-byte datagram. The Java implementation sends it via `sendData()` without a callback — fire-and-forget. Nodes use it to trigger persisting changes to non-volatile storage.

**Byte sequence**: `[0x20, 0xA8]`

### Chunking for Large Writes

**Decision**: Chunk writes at 64 bytes per datagram, sequential with ack between each chunk.

**Rationale**: Same 64-byte data limit as reads (per S-9.7.4.2 §4.8). The Java reference `MemorySpaceCache.RepeatedWrite` sends chunks sequentially, waiting for each `handleSuccess()` before sending the next. Address advances by chunk offset.

**Pattern**:
1. Split data into ≤64-byte chunks
2. For each chunk: send write datagram at `base_address + offset`
3. Wait for Datagram Received OK
4. Advance offset, repeat
5. After all chunks: send Update Complete

## R2: Value Serialization Per Type

**Decision**: Follow OpenLCB_Java `ConfigRepresentation.java` serialization exactly.

**Rationale**: The Java reference is the canonical implementation. Byte-level compatibility is essential for interoperability with production LCC nodes.

### Integer (big-endian)
```
byte[] b = new byte[size];  // size from CDI: 1, 2, 4, or 8
for i in (size-1)..=0: b[i] = value & 0xFF; value >>= 8;
```
Example: value 258 with size 2 → `[0x01, 0x02]`

### String (UTF-8 + null terminator, NOT full-padded)
```
bytes = value.as_bytes();                           // UTF-8
output = new byte[min(field_size, bytes.len + 1)];  // +1 for null
copy bytes into output[0..];                        // trailing byte is 0x00
```
**Key detail**: Only writes `string_length + 1` bytes, NOT the full field width. This matches Java `StringEntry.setValue()` (lines 839-845). The null terminator is the last byte written.

**Alternatives considered**:
- Full null-padding to field width: Rejected — Java reference doesn't do this, and writing fewer bytes is more efficient (smaller datagrams, less node flash wear).

### Event ID (8 raw bytes)
```
bytes = event_id.to_bytes();  // always exactly 8 bytes
```
Parsed from dotted-hex: `"05.01.01.01.22.00.00.FF"` → `[0x05, 0x01, 0x01, 0x01, 0x22, 0x00, 0x00, 0xFF]`

### Float (IEEE 754, big-endian)
- 4-byte: IEEE 754 single-precision (`f32.to_be_bytes()`)
- 8-byte: IEEE 754 double-precision (`f64.to_be_bytes()`)
- 2-byte: Half-precision float (custom, rare — defer if no CDI uses it)

Reference: `FloatEntry.setValue()` uses `ByteBuffer` in network byte order (big-endian).

## R3: Error Handling & Retry Strategy

**Decision**: 3 retries with 3-second timeout per attempt, matching the Java reference.

**Rationale**: `MemoryConfigurationService` uses `MAX_TRIES = 3` and `TIMEOUT = 3000ms`. The `DatagramMeteringBuffer` adds a lower-level 3000ms timeout with fabricated error `0x0100`. This dual-layer retry provides robust handling for real-world LCC networks where timing issues are common.

**Error handling layers**:
1. Datagram level: If `DatagramRejected` with `canResend()` → auto-retry at transport layer
2. Memory Config level: If no Datagram Received OK within 3s → retry up to 3 times
3. Application level: After 3 retries exhausted → mark field as error state, allow user to retry

**Alternatives considered**:
- Exponential backoff: Rejected — Java reference uses fixed timeout, and LCC nodes are simple microcontrollers that don't benefit from backoff.
- Parallel writes: Rejected — Java reference queues writes sequentially (one pending request per type). LCC nodes typically have limited datagram buffers.

## R4: Frontend Edit State Architecture

**Decision**: Edit state lives entirely in a frontend Svelte store (`PendingEditsStore`), not in the Tauri backend.

**Rationale**: Edit state is transient UI state — it exists only while the user is editing and disappears on save/discard. Keeping it in the frontend avoids IPC overhead for every keystroke and follows established Svelte store patterns in the project (`nodeTree.svelte.ts`, `bowties.svelte.ts`).

**Store design**:
- Class-based singleton with Svelte 5 `$state` runes (matching `NodeTreeStore` pattern)
- Map keyed by `"${nodeId}:${segmentOrigin}:${address}"` for uniqueness
- Tracks per-field: original value, pending value, validation state, write state
- Derived getters for: dirty count, has-invalid, per-node dirty check, per-segment dirty check

**Alternatives considered**:
- Backend-managed edit state with Tauri events: Rejected — adds unnecessary IPC latency and complexity for what is fundamentally UI state.
- Svelte 4 writable stores: Possible but inconsistent with the project's migration toward Svelte 5 runes in new code.

## R5: Editable Component Rendering Strategy

**Decision**: Modify `TreeLeafRow.svelte` to conditionally render editable inputs based on field type, keeping the same Svelte 4 syntax for consistency with the existing component.

**Rationale**: `TreeLeafRow` already has access to `leaf.elementType`, `leaf.constraints`, and `leaf.value`. Adding editable controls inline (rather than creating a separate component) minimizes disruption and keeps the field label + value + description layout intact.

**Input mapping**:
| `leaf.elementType` | Has `mapEntries`? | Control | Validation |
|----|----|----|-----|
| `int` | Yes | `<select>` dropdown | Constrained to map values |
| `int` | No | `<input type="number">` | `min`/`max` from constraints |
| `string` | — | `<input type="text">` | `maxlength` from `leaf.size - 1` (null terminator) |
| `eventId` | — | `<input type="text">` | Regex: `^[0-9A-Fa-f]{2}(\.[0-9A-Fa-f]{2}){7}$` |
| `float` | — | `<input type="number" step="any">` | `min`/`max` from constraints |
| `action` | — | Read-only (excluded per spec) | N/A |
| `blob` | — | Read-only (excluded per spec) | N/A |

**Alternatives considered**:
- Separate `EditableField.svelte` component: Could be useful for testing isolation but adds another component layer. Can be extracted later if TreeLeafRow grows too complex.

## R6: Save/Discard UX Pattern

**Decision**: Place Save/Discard controls in a sticky toolbar at the top of `SegmentView.svelte`, visible only when pending edits exist for the current segment or node.

**Rationale**: The toolbar should be always-visible (sticky) when changes exist so the user doesn't need to scroll to find the Save button. The Java reference (`CdiPanel`) places save/update-complete buttons in a fixed toolbar — Bowties should follow a similar pattern but with modern UX (progress indicator, field-by-field transition).

**Save workflow**:
1. User clicks Save → Save button shows progress ("Writing 1 of N...")
2. For each dirty field: invoke Tauri `write_config_value` command
3. As each field succeeds: clear its dirty indicator, update progress
4. After all fields: send Update Complete command (`[0x20, 0xA8]`)
5. If any field fails after retries: mark it with error state, continue with remaining fields

**Alternatives considered**:
- Per-field save buttons: Rejected — clutters UI and doesn't match the batch-save-then-update-complete pattern required by the protocol.
- Save at node level instead of segment level: The Save button should save ALL pending edits for the current node (not just the visible segment), since Update Complete is per-node.

## R7: Navigation Guard Strategy

**Decision**: Use SvelteKit `beforeNavigate` for page-level guards, plus custom logic for node/segment switching within the config page.

**Rationale**: SvelteKit's `beforeNavigate` handles page navigation (Config → Bowties, browser back). For within-page navigation (selecting different node or segment in sidebar), custom guard logic in the sidebar event handlers checks the `PendingEditsStore`.

**Guard behavior**:
- Prompt: "You have N unsaved changes. Save, Discard, or Cancel?"
- Save: trigger save workflow, then navigate after completion
- Discard: clear pending edits, navigate
- Cancel: abort navigation

## R8: ACDI User Space (0xFB) Write Handling

**Decision**: No special handling needed. Use generic format (cmd byte `0x00`, space byte `0xFB` at position 6).

**Rationale**: Per S-9.7.4.2 §4.14 and the existing `AddressSpace::command_flag()` implementation, ACDI User space uses the generic format (same as `AcdiManufacturer`). The write mechanism is identical to Configuration space — only the command byte and address encoding differ, which the existing `command_flag()` method already handles correctly.

ACDI User space layout:
- Offset 0: Version byte (1 byte, read-only)
- Offset 1: User Name (null-terminated, typically 63 bytes max)
- Offset 64: User Description (null-terminated, typically 64 bytes max)
