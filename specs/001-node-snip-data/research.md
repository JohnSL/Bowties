# Research: Enhanced Node Discovery with SNIP Data

**Feature**: 001-node-snip-data  
**Date**: 2026-02-16  
**Status**: Complete

## Research Summary

This document consolidates findings from the Python POC implementation, OpenLCB_Python library, and LCC standards documentation to support implementation of SNIP (Simple Node Identification Protocol) in the Rust/Tauri Bowties application.

## 1. SNIP Protocol Specification

### 1.1 MTI Values

**Decision**: Use the following MTI constants for SNIP communication

| Message Type | MTI Value | Header Bits | Description |
|--------------|-----------|-------------|-------------|
| SNIP Request | 0x19DE8 | Addressed message | Request node to send its identification data |
| SNIP Response | 0x19A08 | Addressed message | Response containing SNIP data (via datagram) |

**Rationale**: These values are specified in TN-9.7.4.3 (Simple Node Information) and confirmed in the Python implementation (`simpleNodeIdentificationInformation.py` line 12).

**Alternatives considered**: None - these are protocol constants defined by the LCC standard.

**Source**: 
- `OpenLCB_Python/simpleNodeIdentificationInformation.py` 
- `markdown/standards/TN-9.7.4.3-SimpleNodeInformation-2024-07-22.md`

### 1.2 Request Format

**Decision**: SNIP request is an addressed message containing 2-byte destination alias payload

```
Frame format: :X19DE8[source_alias]N[dest_alias_hi][dest_alias_lo];
Example: :X19DE8123N03AE; (source=0x123, dest=0x3AE)
```

**Rationale**: Addressed messages allow a specific node to be queried without global broadcast. The 2-byte payload identifies the destination node by its alias.

**Source**: `bowtie/webapp/app/node_discovery.py` lines 20-23, Python implementation in `simpleNodeIdentificationInformation.py`

### 1.3 Response Format

**Decision**: SNIP response is delivered via LCC datagram protocol (MTI 0x19A08)

Datagrams can be single-frame or multi-frame:
- **Single frame**: MTI 0x1A000 (DatagramOnly) - up to 8 bytes
- **Multi-frame**: 
  - First frame: MTI 0x1B000 (DatagramFirst) - 8 bytes
  - Middle frames: MTI 0x1C000 (DatagramMiddle) - 8 bytes each
  - Final frame: MTI 0x1D000 (DatagramFinal) - remaining bytes (≤8)

Each frame contains:
- Bytes 0-1: Source/destination addressing info
- Bytes 2-7: Actual SNIP payload data (6 bytes per frame)

**Rationale**: SNIP data (6 string fields) typically requires 20-100 bytes, exceeding single CAN frame capacity (8 bytes). The datagram protocol provides reliable multi-frame delivery.

**Alternatives considered**: The standard also allows retrieval via Memory Configuration Protocol (space 0xFC + 0xFB), but SNIP is simpler and optimized for discovery use cases.

**Source**: 
- `OpenLCB_Python/datagram.py` lines 9-20
- `markdown/standards/TN-9.7.3.2-DatagramTransport-2021-04-25.md`
- `bowtie/webapp/app/node_discovery.py` lines 222-241

### 1.4 SNIP Data Structure

**Decision**: SNIP payload consists of two sections with null-terminated strings

```
Section 1 (Manufacturer ACDI - read-only):
  Byte 0: Version byte (0x04 = 4 fields)
  String 1: Manufacturer name (null-terminated)
  String 2: Model name (null-terminated)
  String 3: Hardware version (null-terminated)
  String 4: Software version (null-terminated)

Section 2 (User ACDI - user-configurable):
  Byte N: Version byte (0x02 = 2 fields)
  String 5: User name (null-terminated)
  String 6: User description/comment (null-terminated)
```

**Rationale**: Version bytes double as field counts, allowing forward compatibility. Receivers count null terminators to parse unknown future fields. Section 2 may be omitted if not supported by the node.

**Source**: 
- `markdown/standards/TN-9.7.4.3-SimpleNodeInformation-2024-07-22.md` sections 2.5.1, 2.5.2
- `bowtie/webapp/app/node_discovery.py` lines 270-340

### 1.5 String Encoding

**Decision**: Strings are ASCII/UTF-8 encoded, null-terminated (0x00)

**Rationale**: LCC standard specifies null-terminated strings. While ASCII is baseline, UTF-8 is compatible and handles international characters (important for user names/descriptions).

**Alternatives considered**: 
- Length-prefixed strings: Rejected - not supported by LCC standard
- Fixed-width fields: Rejected - wasteful of network bandwidth

**Edge cases**:
- Invalid UTF-8 bytes: Replace with '?' character to prevent crashes
- Missing null terminators: Treat end of datagram as implicit terminator
- Empty strings: Valid - two consecutive null bytes

**Source**: `bowtie/webapp/app/node_discovery.py` lines 301-304, 325-328

## 2. Datagram Assembly and Concurrency

### 2.1 Multi-Frame Datagram Assembly

**Decision**: Implement stateful datagram receiver to reassemble multi-frame messages

Algorithm:
1. Detect first frame (MTI 0x1B000) or single frame (MTI 0x1A000)
2. For multi-frame: buffer all middle frames (MTI 0x1C000)
3. Complete on final frame (MTI 0x1D000) or single frame
4. Extract bytes 2-7 from each frame (skip addressing bytes 0-1)
5. Concatenate into complete SNIP payload

**Rationale**: CAN frames limited to 8 bytes; typical SNIP response is 40-100 bytes requiring 7-15 frames.

**Source**: 
- `OpenLCB_Python/datagram.py` lines 61-76
- `bowtie/webapp/app/node_discovery.py` lines 227-241

### 2.2 Concurrency and Rate Limiting

**Decision**: Queue SNIP requests with maximum 5 concurrent in-flight requests

**Rationale**: 
- Prevents network flooding during initial discovery with many nodes
- Allows some parallelism for improved perceived performance
- Python POC showed good results with this approach on 20-node networks

**Implementation approach**:
- Use async/await in Rust with tokio semaphore (capacity=5)
- Queue requests, await semaphore permit before sending
- Release permit when response received or timeout occurs

**Alternatives considered**:
- Sequential (1 at a time): Too slow for 20+ nodes
- Unlimited parallel: Risk overwhelming smaller nodes and network segments

**Source**: Feature spec FR-009, informed by Python POC testing experience

### 2.3 Timeout Strategy

**Decision**: Multi-level timeout strategy for SNIP requests

| Timeout Type | Duration | Purpose |
|--------------|----------|---------|
| Initial response | 50ms | Wait for first SNIP frame after request sent |
| Silence timeout | 25ms | Stop waiting after no frames received for 25ms |
| Maximum request | 5 seconds | Hard limit per spec, handles slow/busy nodes |

**Rationale**: 
- Most nodes respond within 50-200ms based on Python POC testing
- Silence timeout (25ms) allows early termination when response complete
- 5-second maximum handles edge cases (slow controllers, network congestion)

**Error handling**:
- Timeout → Mark node with "Partial data" status, display what was received
- No response → Node marked as "SNIP not supported", fall back to Node ID/alias display

**Source**: 
- `bowtie/webapp/app/node_discovery.py` lines 15-18 (timeout constants)
- Feature spec FR-010

## 3. Node Discovery Integration

### 3.1 Automatic Detection of New Nodes

**Decision**: Listen for Verified Node ID broadcasts (MTI 0x19170) to detect newly joined nodes

**Rationale**: When nodes join the network or complete initialization, they broadcast Verified Node ID messages. This allows automatic discovery without continuous polling.

**Implementation**:
- Background listener task monitors all network traffic
- Filter for MTI 0x19170 frames
- Extract Node ID (6 bytes) and alias (from header) from frame
- Trigger SNIP retrieval for new nodes not in cache
- Update UI reactively when new node discovered

**Source**: 
- `bowtie/webapp/app/node_discovery.py` lines 352-377
- Feature spec FR-007

### 3.2 Manual Refresh Strategy

**Decision**: Manual "Refresh" button re-sends global Verify Node (MTI 0x19490) and re-queries SNIP for all nodes

**Rationale**: 
- Allows users to verify current network state on-demand
- Detects nodes that disconnected/reconnected
- Updates status indicators (responding/not responding)

**Behavior**:
1. Cancel any in-progress refresh operation
2. Send global Verify Node ID message (alias=0)
3. Collect Verified Node responses for 250ms (with 25ms silence timeout)
4. Queue SNIP requests for all responding nodes (5 concurrent max)
5. Update node list with refreshed data and timestamps

**Source**: Feature spec FR-005, FR-014, based on `discoverAllNodes()` in `node_discovery.py` lines 349-384

## 4. Display and UX Patterns

### 4.1 Friendly Name Formatting

**Decision**: Display nodes using hierarchical name strategy

Priority order:
1. If `user_name` set: **"[User Name]"** with tooltip showing manufacturer/model
2. If only manufacturer data: **"[Manufacturer] [Model]"**
3. If neither: **"Node [Node ID]"** (hex formatted as XX.XX.XX.XX.XX.XX)

Secondary info (always available in tooltip):
- Full Node ID
- Alias
- Software version
- Hardware version (if present)
- User description (if present)

**Rationale**: User-assigned names are most meaningful for identification; manufacturer/model provides context when user hasn't customized; Node ID is guaranteed fallback.

**Source**: Feature spec FR-002, FR-003

### 4.2 Duplicate Name Handling

**Decision**: Disambiguate duplicate user names by appending abbreviated Node ID

```
Format: "[User Name] (XX.XX...)"
Example:
  - East Panel (05.02.01...)
  - East Panel (05.02.02...)
```

**Rationale**: Users may assign same name to multiple nodes (e.g., multiple "Button Board" devices). Node ID provides guaranteed uniqueness.

**Source**: Feature spec FR-012, edge cases section

### 4.3 Status Indicators

**Decision**: Three-state visual system with timestamp

| Status | Icon/Color | Meaning | When Set |
|--------|------------|---------|----------|
| Connected | Green dot | Node responded recently | Successful SNIP or verify within last refresh |
| Not Responding | Red dot | Node failed to respond | Timeout on SNIP request or verify |
| Unknown | Gray dot | Status not yet checked | Initial state before first verification |

**Additional indicator**: "Last verified: [time ago]" (e.g., "2 minutes ago")

**Rationale**: Clear visual feedback on node availability; timestamp helps users assess data freshness.

**Source**: Feature spec FR-004, FR-015

## 5. Error Handling and Edge Cases

### 5.1 Malformed SNIP Data

**Decision**: Gracefully handle invalid or incomplete SNIP responses

| Error Condition | Handling |
|-----------------|----------|
| Invalid UTF-8 byte | Replace with '?' character |
| Missing version bytes | Attempt to parse as null-terminated strings |
| Truncated datagram | Use partial data received, mark as "Partial" |
| Missing Section 2 | Display only manufacturer data (valid per spec) |
| Empty strings | Valid - display as empty (omit from UI where appropriate) |

**Rationale**: Nodes in the field may have firmware bugs or incomplete implementations. Displaying partial data is better than showing nothing.

**Source**: Feature spec FR-008, FR-011

### 5.2 Nodes Without SNIP Support

**Decision**: Fall back to Node ID + alias display with note "SNIP not supported"

Detection:
- Timeout after 5 seconds with no response
- Optional Interaction Rejected message received (MTI 0x19068)

Display:
```
Node 05.02.01.02.03.04 (alias: 3AE)
Note: SNIP not supported
```

**Rationale**: Older or minimal LCC nodes may not implement SNIP. They should still be visible and usable.

**Source**: Feature spec edge cases, TN-9.7.4.3 section 2.6

### 5.3 Slow or Busy Nodes

**Decision**: Progressive loading with visual feedback

1. Show node immediately with "Loading..." placeholder
2. Display SNIP data as it arrives
3. If timeout at 5 seconds, show "Partial data" warning
4. Allow manual retry via context menu

**Rationale**: Maintains responsive UI even when nodes are slow; keeps user informed of progress.

**Source**: Feature spec edge cases

## 6. Testing Strategy

### 6.1 Protocol Correctness Tests

**Approach**: Property-based testing for datagram assembly using proptest

Test cases:
- Single-frame datagrams (≤8 bytes payload)
- Multi-frame datagrams (9-253 bytes)
- Frame order verification
- Byte extraction (skip addressing bytes 0-1)
- Invalid frame sequences (missing middle frame, duplicate final)

**Source**: Rust testing best practices, `lcc-rs/Cargo.toml` dev-dependencies

### 6.2 Integration Tests

**Approach**: Test against real LCC hardware or Python simulator

Test scenarios:
- Query known node with complete SNIP support
- Query node without SNIP support (timeout handling)
- Query multiple nodes concurrently
- Parse SNIP data with international characters (UTF-8)
- Handle slow node (artificial delay in simulator)

**Validation**: Compare byte-for-byte with Python POC output

**Source**: Feature spec success criteria SC-002 through SC-007

## 7. Implementation Recommendations

### 7.1 Rust Code Organization

**Recommended structure**:

```
lcc-rs/src/
├── protocol/
│   ├── datagram.rs    # NEW: Multi-frame datagram assembly
│   ├── mti.rs         # UPDATE: Add SNIP MTIs
│   └── snip.rs        # NEW: SNIP request/response handling
├── snip.rs            # NEW: High-level SNIP query API
└── types.rs           # UPDATE: SNIPData struct (already exists)
```

**Key types**:
```rust
pub struct SNIPData {
    pub manufacturer: String,
    pub model: String,
    pub hardware_version: String,
    pub software_version: String,
    pub user_name: String,
    pub user_description: String,
}

pub enum SNIPStatus {
    Complete,
    Partial,
    NotSupported,
    Timeout,
}
```

### 7.2 Async API Pattern

**Recommended API**:

```rust
pub async fn query_snip(
    connection: &LccConnection,
    source_alias: NodeAlias,
    dest_alias: NodeAlias,
    timeout: Duration,
) -> Result<(SNIPData, SNIPStatus)>
```

**Concurrency**:
```rust
// In discovery module
let semaphore = Arc::new(Semaphore::new(5));
let tasks: Vec<_> = nodes.iter().map(|node| {
    let permit = semaphore.clone().acquire_owned().await;
    tokio::spawn(async move {
        let result = query_snip(...).await;
        drop(permit);
        result
    })
}).collect();
```

### 7.3 Frontend Integration

**Svelte store pattern**:

```typescript
// stores/nodes.ts
interface DiscoveredNode {
    nodeId: string;
    alias: number;
    status: 'connected' | 'not-responding' | 'unknown';
    lastVerified: Date;
    snip?: SNIPData;
    snipStatus?: 'complete' | 'partial' | 'not-supported';
}

export const nodes = writable<DiscoveredNode[]>([]);
```

**Tauri command**:
```rust
#[tauri::command]
async fn refresh_nodes(state: State<'_, AppState>) -> Result<Vec<DiscoveredNode>, String>
```

## 8. Performance Targets

Based on Python POC testing and feature requirements:

| Metric | Target | Test Scenario |
|--------|--------|---------------|
| SNIP retrieval time | <3s for 95% of nodes | 20-node network, concurrent requests |
| Manual refresh | <5s complete | 20 nodes, full SNIP re-query |
| New node detection | <10s | Node joins network, appears in UI |
| UI responsiveness | No lag | 50 nodes in list, scrolling/interaction |

**Measurement approach**: Integration tests with timestamps; user acceptance testing

## Next Steps

This research phase is now complete. All technical unknowns have been resolved through examination of:
- Python POC implementation (bowtie/webapp)
- OpenLCB_Python library reference code
- LCC standards documentation (markdown/standards)

Ready to proceed to Phase 1: Data Model and Contract Design.
