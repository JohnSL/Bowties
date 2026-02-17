# Data Model: Enhanced Node Discovery with SNIP Data

**Feature**: 001-node-snip-data  
**Date**: 2026-02-16  
**Status**: Phase 1 Design

## Overview

This document defines the data entities, their relationships, validation rules, and state transitions for the Enhanced Node Discovery with SNIP Data feature.

## Core Entities

### 1. Node

Represents a physical or virtual LCC device discovered on the network.

#### Fields

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `node_id` | `NodeID` | Required, unique, 6 bytes | 48-bit globally unique node identifier |
| `alias` | `NodeAlias` | Required, 12 bits (0x001-0xFFF) | Dynamic short identifier for CAN addressing |
| `snip_data` | `Option<SNIPData>` | Optional | SNIP information if retrieved successfully |
| `snip_status` | `SNIPStatus` | Required, default: `Unknown` | Status of SNIP data retrieval |
| `connection_status` | `ConnectionStatus` | Required, default: `Unknown` | Current reachability status |
| `last_verified` | `Option<DateTime>` | Optional | Timestamp of last successful verification |
| `last_seen` | `DateTime` | Required | Timestamp when node last observed on network |

#### Rust Type Definition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_id: NodeID,
    pub alias: NodeAlias,
    pub snip_data: Option<SNIPData>,
    pub snip_status: SNIPStatus,
    pub connection_status: ConnectionStatus,
    pub last_verified: Option<DateTime<Utc>>,
    pub last_seen: DateTime<Utc>,
}
```

#### Validation Rules

- `node_id`: Must be exactly 6 bytes, all bytes 0x00 is invalid
- `alias`: Must be in range 0x001-0xFFF (0x000 is reserved)
- `snip_data`: If present, `snip_status` must be `Complete` or `Partial`
- `last_verified`: Must not be in the future
- `last_seen`: Must not be in the future

#### Business Rules

- **Uniqueness**: `node_id` is globally unique across all nodes
- **Alias reuse**: Same alias may be used by different nodes at different times (after network reset)
- **SNIP caching**: Once `snip_data` retrieved with `Complete` status, cache until node reinitializes
- **Stale detection**: If `last_seen` is >60 seconds ago, mark `connection_status` as `Unknown`

---

### 2. NodeID

48-bit globally unique identifier for LCC nodes.

#### Fields

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `bytes` | `[u8; 6]` | Required, not all zeros | 6-byte array representing the Node ID |

#### Rust Type Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeID([u8; 6]);

impl NodeID {
    pub fn new(bytes: [u8; 6]) -> Result<Self> {
        if bytes == [0; 6] {
            return Err(Error::InvalidNodeID);
        }
        Ok(NodeID(bytes))
    }
    
    pub fn from_u64(value: u64) -> Result<Self> {
        // Extract 48 bits (bytes 5-0 of u64)
        let bytes = [
            ((value >> 40) & 0xFF) as u8,
            ((value >> 32) & 0xFF) as u8,
            ((value >> 24) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        ];
        Self::new(bytes)
    }
    
    pub fn to_u64(&self) -> u64 {
        ((self.0[0] as u64) << 40)
            | ((self.0[1] as u64) << 32)
            | ((self.0[2] as u64) << 24)
            | ((self.0[3] as u64) << 16)
            | ((self.0[4] as u64) << 8)
            | (self.0[5] as u64)
    }
}

impl Display for NodeID {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:02X}.{:02X}.{:02X}.{:02X}.{:02X}.{:02X}",
            self.0[0], self.0[1], self.0[2],
            self.0[3], self.0[4], self.0[5])
    }
}
```

#### Display Format

- **Hex dot-notation**: `05.02.01.02.00.03`
- **Compact**: `05020102000` (omit dots for copy/paste)

---

### 3. NodeAlias

12-bit short identifier used for CAN addressing.

#### Fields

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `value` | `u16` | Required, range: 0x001-0xFFF | 12-bit alias value |

#### Rust Type Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeAlias(u16);

impl NodeAlias {
    pub fn new(value: u16) -> Result<Self> {
        if value == 0 || value > 0xFFF {
            return Err(Error::InvalidAlias);
        }
        Ok(NodeAlias(value))
    }
}

impl Display for NodeAlias {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:03X}", self.0)
    }
}
```

#### Display Format

- **Hex**: `3AE` (always 3 hex digits)

---

### 4. SNIPData

Simple Node Identification Protocol data containing manufacturer and user information.

#### Fields

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `manufacturer` | `String` | Max 64 chars, sanitized | Manufacturer name (e.g., "RR-CirKits") |
| `model` | `String` | Max 64 chars, sanitized | Model name (e.g., "Tower-LCC") |
| `hardware_version` | `String` | Max 32 chars, sanitized | Hardware version string |
| `software_version` | `String` | Max 32 chars, sanitized | Software/firmware version |
| `user_name` | `String` | Max 64 chars, sanitized | User-assigned name (e.g., "East Panel Controller") |
| `user_description` | `String` | Max 128 chars, sanitized | User-assigned description |

#### Rust Type Definition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SNIPData {
    pub manufacturer: String,
    pub model: String,
    pub hardware_version: String,
    pub software_version: String,
    pub user_name: String,
    pub user_description: String,
}

impl SNIPData {
    /// Sanitize string by replacing invalid UTF-8 and control characters
    fn sanitize(s: &str, max_len: usize) -> String {
        s.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
            .take(max_len)
            .collect()
    }
    
    pub fn new(
        manufacturer: String,
        model: String,
        hardware_version: String,
        software_version: String,
        user_name: String,
        user_description: String,
    ) -> Self {
        Self {
            manufacturer: Self::sanitize(&manufacturer, 64),
            model: Self::sanitize(&model, 64),
            hardware_version: Self::sanitize(&hardware_version, 32),
            software_version: Self::sanitize(&software_version, 32),
            user_name: Self::sanitize(&user_name, 64),
            user_description: Self::sanitize(&user_description, 128),
        }
    }
    
    /// Get friendly display name for the node
    pub fn friendly_name(&self) -> String {
        if !self.user_name.is_empty() {
            self.user_name.clone()
        } else if !self.manufacturer.is_empty() || !self.model.is_empty() {
            format!("{} {}", self.manufacturer, self.model).trim().to_string()
        } else {
            "Unknown Node".to_string()
        }
    }
    
    /// Get secondary info line for display
    pub fn secondary_info(&self) -> String {
        if !self.user_name.is_empty() {
            // Show manufacturer/model when user name is primary
            format!("{} {}", self.manufacturer, self.model).trim().to_string()
        } else {
            // Show software version when manufacturer/model is primary
            self.software_version.clone()
        }
    }
}
```

#### Validation Rules

- All strings must be valid UTF-8
- Control characters (except \n, \r, \t) are removed during sanitization
- Empty strings are valid (represent missing/unconfigured fields)
- Maximum lengths enforced to prevent UI overflow

---

### 5. SNIPStatus

Enum representing the status of SNIP data retrieval.

#### Values

| Status | Description | UI Display |
|--------|-------------|------------|
| `Unknown` | SNIP not yet requested | Gray indicator, "Not verified" |
| `InProgress` | SNIP request sent, awaiting response | Spinner, "Loading..." |
| `Complete` | All SNIP fields retrieved successfully | Green indicator |
| `Partial` | Some SNIP data received, but incomplete | Yellow indicator, "Partial data" |
| `NotSupported` | Node does not support SNIP protocol | Gray indicator, "SNIP not supported" |
| `Timeout` | SNIP request timed out | Red indicator, "Timeout" |
| `Error` | SNIP request failed with error | Red indicator, "Error" |

#### Rust Type Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SNIPStatus {
    Unknown,
    InProgress,
    Complete,
    Partial,
    NotSupported,
    Timeout,
    Error,
}

impl SNIPStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, 
            SNIPStatus::Complete 
            | SNIPStatus::Partial 
            | SNIPStatus::NotSupported 
            | SNIPStatus::Timeout 
            | SNIPStatus::Error
        )
    }
    
    pub fn should_retry(&self) -> bool {
        matches!(self, SNIPStatus::Timeout | SNIPStatus::Error)
    }
}
```

---

### 6. ConnectionStatus

Enum representing the current reachability of a node.

#### Values

| Status | Description | UI Display |
|--------|-------------|------------|
| `Unknown` | Status not yet verified | Gray dot |
| `Verifying` | Verification in progress | Pulsing gray dot |
| `Connected` | Node responded to recent verification | Green dot |
| `NotResponding` | Node failed to respond to verification | Red dot |

#### Rust Type Definition

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Unknown,
    Verifying,
    Connected,
    NotResponding,
}

impl ConnectionStatus {
    pub fn icon_color(&self) -> &'static str {
        match self {
            ConnectionStatus::Unknown => "gray",
            ConnectionStatus::Verifying => "gray-pulse",
            ConnectionStatus::Connected => "green",
            ConnectionStatus::NotResponding => "red",
        }
    }
}
```

---

## Entity Relationships

### Node → SNIPData

- **Type**: One-to-Optional-One
- **Description**: Each Node may have associated SNIP data
- **Cascade**: Deleting a Node removes its SNIPData
- **Constraint**: SNIP data can only exist if `snip_status` is `Complete` or `Partial`

### Node → NodeID

- **Type**: One-to-One (composition)
- **Description**: Every Node has exactly one NodeID
- **Identity**: NodeID uniquely identifies the Node

### Node → NodeAlias

- **Type**: One-to-One (composition)
- **Description**: Every Node has exactly one alias at any given time
- **Volatility**: Alias can change after network reset or node power cycle

---

## State Transitions

### Node Discovery Lifecycle

```
                     ┌─────────────┐
                     │   Unknown   │ (Node not yet discovered)
                     └──────┬──────┘
                            │
                ┌───────────▼───────────┐
                │ Verified Node ID msg  │
                │    received           │
                └───────────┬───────────┘
                            │
                     ┌──────▼──────┐
                     │  Discovered  │
                     │ (status:     │
                     │  Unknown)    │
                     └──────┬──────┘
                            │
                ┌───────────▼───────────┐
                │  Query SNIP data      │
                └───────────┬───────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
  ┌─────▼─────┐      ┌──────▼──────┐     ┌─────▼─────┐
  │ Complete  │      │   Partial   │     │ Timeout / │
  │           │      │             │     │ Not Supp. │
  └───────────┘      └─────────────┘     └───────────┘
        │                   │                   │
        └───────────────────┴───────────────────┘
                            │
                     ┌──────▼──────┐
                     │   Active    │ (shown in node list)
                     │             │
                     └──────┬──────┘
                            │
                ┌───────────┴───────────┐
                │                       │
         ┌──────▼──────┐         ┌─────▼─────┐
         │ Node Init   │         │ Network   │
         │ Complete    │         │ timeout   │
         │ received    │         │ (60s)     │
         └──────┬──────┘         └─────┬─────┘
                │                      │
         ┌──────▼──────┐         ┌─────▼─────┐
         │ Re-query    │         │  Stale    │
         │ SNIP        │         │ (grayed)  │
         └─────────────┘         └───────────┘
```

### SNIP Status State Machine

```
  ┌─────────┐
  │ Unknown │ (initial state)
  └────┬────┘
       │
       │ SNIP request sent
       ▼
  ┌──────────────┐
  │  InProgress  │
  └──────┬───────┘
         │
    ┌────┴─────────────────────┐
    │                          │
    │ Response frames received │ Timeout (5s)
    ▼                          ▼
┌──────────┐              ┌─────────┐
│ Complete │              │ Timeout │
│   or     │              └─────────┘
│ Partial  │
└──────────┘
    │ Optional Interaction
    │ Rejected received
    ▼
┌──────────────┐
│ NotSupported │
└──────────────┘
```

### Connection Status State Machine

```
  ┌─────────┐
  │ Unknown │ (initial state)
  └────┬────┘
       │
       │ Verify request sent
       ▼
  ┌───────────┐
  │ Verifying │
  └─────┬─────┘
        │
    ┌───┴────────────────┐
    │                    │
    │ Response received  │ Timeout
    ▼                    ▼
┌───────────┐      ┌────────────────┐
│ Connected │      │ NotResponding  │
└─────┬─────┘      └────────┬───────┘
      │                     │
      │                     │
      │ Manual refresh      │ Manual refresh
      └─────────┬───────────┘
                ▼
           ┌─────────┐
           │ Verifying│ (cycle repeats)
           └──────────┘
```

## Derived Data

### Display Name

**Computed from**: `Node.snip_data`

**Logic**:
```rust
impl Node {
    pub fn display_name(&self) -> String {
        match &self.snip_data {
            Some(snip) => snip.friendly_name(),
            None => format!("Node {}", self.node_id),
        }
    }
    
    pub fn display_name_with_disambiguation(&self, other_nodes: &[Node]) -> String {
        let base_name = self.display_name();
        
        // Check for duplicate names
        let duplicates: Vec<_> = other_nodes.iter()
            .filter(|n| n.display_name() == base_name && n.node_id != self.node_id)
            .collect();
        
        if duplicates.is_empty() {
            base_name
        } else {
            // Append abbreviated Node ID for disambiguation
            let short_id = format!("{:02X}.{:02X}.{:02X}...",
                self.node_id.0[0], self.node_id.0[1], self.node_id.0[2]);
            format!("{} ({})", base_name, short_id)
        }
    }
}
```

### Last Verified Text

**Computed from**: `Node.last_verified`

**Logic**:
```rust
impl Node {
    pub fn last_verified_text(&self) -> String {
        match self.last_verified {
            Some(timestamp) => {
                let elapsed = Utc::now() - timestamp;
                if elapsed.num_seconds() < 60 {
                    "Just now".to_string()
                } else if elapsed.num_minutes() < 60 {
                    format!("{} min ago", elapsed.num_minutes())
                } else if elapsed.num_hours() < 24 {
                    format!("{} hr ago", elapsed.num_hours())
                } else {
                    format!("{} days ago", elapsed.num_days())
                }
            }
            None => "Never".to_string(),
        }
    }
}
```

## Invariants

1. **Node ID Uniqueness**: No two nodes with same `node_id` can exist simultaneously
2. **Valid Alias**: `alias` must always be in range 0x001-0xFFF
3. **SNIP Status Consistency**: If `snip_data.is_some()`, then `snip_status` must be `Complete` or `Partial`
4. **Timestamp Validity**: `last_verified` and `last_seen` must not be future timestamps
5. **Connection Status Logic**: If `last_seen` is >60s ago, `connection_status` should be `Unknown` or `NotResponding`

## Persistence Strategy

### For This Feature (Phase 1)

- **In-memory only**: All `Node` entities stored in memory (Rust backend)
- **No database**: Nodes cleared on application restart
- **Rationale**: Simplifies initial implementation; SNIP data can be re-queried quickly

### Future Enhancement

- **SQLite cache**: Store nodes with SNIP data to speed up startup
- **Cache key**: `node_id` (primary key)
- **Invalidation**: Clear cache entry when receiving Node Initialization Complete message
- **Benefits**: Instant node list display on app launch, reduced network traffic

## TypeScript Frontend Types

```typescript
// Mirrors Rust types for Tauri IPC

export interface NodeID {
    bytes: number[];  // 6-element array
}

export type NodeAlias = number;  // 12-bit value (1-4095)

export interface SNIPData {
    manufacturer: string;
    model: string;
    hardware_version: string;
    software_version: string;
    user_name: string;
    user_description: string;
}

export type SNIPStatus = 
    | 'Unknown'
    | 'InProgress'
    | 'Complete'
    | 'Partial'
    | 'NotSupported'
    | 'Timeout'
    | 'Error';

export type ConnectionStatus =
    | 'Unknown'
    | 'Verifying'
    | 'Connected'
    | 'NotResponding';

export interface DiscoveredNode {
    node_id: NodeID;
    alias: NodeAlias;
    snip_data?: SNIPData;
    snip_status: SNIPStatus;
    connection_status: ConnectionStatus;
    last_verified?: string;  // ISO 8601 timestamp
    last_seen: string;       // ISO 8601 timestamp
}

// Helper functions in TypeScript
export function formatNodeID(node_id: NodeID): string {
    return node_id.bytes.map(b => b.toString(16).toUpperCase().padStart(2, '0')).join('.');
}

export function friendlyName(node: DiscoveredNode): string {
    if (node.snip_data?.user_name) {
        return node.snip_data.user_name;
    }
    if (node.snip_data?.manufacturer || node.snip_data?.model) {
        return `${node.snip_data.manufacturer} ${node.snip_data.model}`.trim();
    }
    return `Node ${formatNodeID(node.node_id)}`;
}
```

## Summary

This data model provides:
- **Type-safe entities** in Rust with validation
- **Clear state machines** for SNIP and connection status
- **Friendly name logic** with duplicate handling
- **TypeScript types** for frontend integration
- **Immutability-friendly** design (all fields can be cloned)
- **Testable** entities with clear invariants

Ready for implementation in `lcc-rs/src/types.rs` and frontend stores.
