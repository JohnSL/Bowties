# LCC-RS API Documentation

**Version:** 0.1.0  
**Last Updated:** February 18, 2026

## Overview

`lcc-rs` is a Rust implementation of the Layout Command Control (LCC/OpenLCB) protocol, commonly used in model railroading for distributed control systems. The library provides asynchronous support for network communication, node discovery, and Simple Node Identification Protocol (SNIP) queries.

## Features

- ✅ GridConnect frame parsing and encoding
- ✅ TCP transport layer (async with Tokio)
- ✅ Node discovery protocol
- ✅ Message Type Identifier (MTI) handling
- ✅ Multi-frame datagram reassembly
- ✅ SNIP (Simple Node Identification Protocol) support
- ✅ Memory Configuration Protocol (CDI retrieval and configuration memory)
- ✅ CDI XML parsing and navigation
- ✅ CDI hierarchy analysis (depth calculation, replication expansion)
- ✅ Index-based pathId navigation system
- ✅ Addressed and global messaging
- ✅ Concurrency control with semaphores

---

## Module Organization

```
lcc_rs
├── types           - Core data types (NodeID, EventID, NodeAlias, etc.)
├── protocol        - Protocol-level structures
│   ├── frame       - GridConnect frame parsing/encoding
│   ├── mti         - Message Type Identifiers
│   ├── datagram    - Multi-frame datagram reassembly
│   └── memory_config - Memory Configuration Protocol
├── cdi             - CDI parsing and navigation
│   ├── mod.rs      - Type definitions (Cdi, Segment, Group, DataElement)
│   ├── parser.rs   - XML parsing (roxmltree-based)
│   └── hierarchy.rs - Navigation helpers (expand, navigate_to_path)
├── transport       - Transport layer implementations
│   └── tcp         - TCP transport (async)
├── discovery       - Node discovery functionality
└── snip            - Simple Node Identification Protocol
```

---

## Core Types (`types` module)

### NodeID

A 48-bit (6-byte) unique node identifier in the LCC network.

```rust
pub struct NodeID(pub [u8; 6]);
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(bytes: [u8; 6]) -> Self` | Create a new NodeID from a 6-byte array |
| `from_slice` | `fn from_slice(slice: &[u8]) -> Result<Self, String>` | Create a NodeID from a slice (must be exactly 6 bytes) |
| `as_bytes` | `fn as_bytes(&self) -> &[u8]` | Convert NodeID to a byte slice |
| `to_hex_string` | `fn to_hex_string(&self) -> String` | Convert to hex string (e.g., "01.02.03.04.05.06") |
| `from_hex_string` | `fn from_hex_string(s: &str) -> Result<Self, String>` | Parse from hex string (supports dots, spaces, dashes) |

#### Example

```rust
use lcc_rs::NodeID;

// Create from bytes
let node_id = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

// Parse from hex string
let node_id = NodeID::from_hex_string("01.02.03.04.05.06").unwrap();
let node_id = NodeID::from_hex_string("010203040506").unwrap(); // Also works

// Display
println!("{}", node_id); // "01.02.03.04.05.06"
```

---

### EventID

A 64-bit (8-byte) event identifier in the LCC network.

```rust
pub struct EventID(pub [u8; 8]);
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(bytes: [u8; 8]) -> Self` | Create a new EventID from an 8-byte array |
| `from_slice` | `fn from_slice(slice: &[u8]) -> Result<Self, String>` | Create from slice (must be exactly 8 bytes) |
| `as_bytes` | `fn as_bytes(&self) -> &[u8]` | Convert to byte slice |
| `to_hex_string` | `fn to_hex_string(&self) -> String` | Convert to hex string |

#### Example

```rust
use lcc_rs::EventID;

let event_id = EventID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
println!("{}", event_id); // "01.02.03.04.05.06.07.08"
```

---

### NodeAlias

A 12-bit node alias used in CAN frames (0x000 - 0xFFF).

```rust
pub struct NodeAlias(pub u16);
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(alias: u16) -> Result<Self, String>` | Create a new NodeAlias (must be ≤ 0xFFF) |
| `value` | `fn value(&self) -> u16` | Get the raw alias value |

#### Example

```rust
use lcc_rs::NodeAlias;

let alias = NodeAlias::new(0xAAA).unwrap();
println!("{}", alias); // "AAA"
```

---

### SNIPData

Information from Simple Node Identification Protocol.

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
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `sanitize` | `fn sanitize(&mut self)` | Clean and validate SNIP string fields (replaces invalid UTF-8 and control characters with '?') |

---

### SNIPStatus

Status of SNIP data retrieval operation.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SNIPStatus {
    Unknown,         // Not yet queried
    InProgress,      // Request in progress
    Complete,        // Data completely retrieved
    Partial,         // Data partially retrieved
    NotSupported,    // Node does not support SNIP
    Timeout,         // Request timed out
    Error,           // Error occurred
}
```

---

### ConnectionStatus

Connection status of a node.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Unknown,         // Status unknown
    Verifying,       // Verifying connection
    Connected,       // Node is connected and responding
    NotResponding,   // Node is not responding
}
```

---

### DiscoveredNode

A discovered node on the LCC network.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredNode {
    pub node_id: NodeID,
    pub alias: NodeAlias,
    pub snip_data: Option<SNIPData>,
    pub snip_status: SNIPStatus,
    pub connection_status: ConnectionStatus,
    pub last_verified: Option<DateTime<Utc>>,
    pub last_seen: DateTime<Utc>,
}
```

---

## Protocol Types (`protocol` module)

### GridConnectFrame

A GridConnect-formatted CAN frame.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridConnectFrame {
    pub header: u32,     // 29-bit CAN header
    pub data: Vec<u8>,   // Data payload (0-8 bytes)
}
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(header: u32, data: Vec<u8>) -> Result<Self>` | Create a new GridConnect frame |
| `from_mti` | `fn from_mti(mti: MTI, source_alias: u16, data: Vec<u8>) -> Result<Self>` | Create a frame from MTI and source alias |
| `from_addressed_mti` | `fn from_addressed_mti(mti: MTI, source_alias: u16, dest_alias: u16, payload: Vec<u8>) -> Result<Self>` | Create an addressed message frame |
| `parse` | `fn parse(s: &str) -> Result<Self>` | Parse from GridConnect string format |
| `to_string` | `fn to_string(&self) -> String` | Encode to GridConnect string format |
| `get_mti` | `fn get_mti(&self) -> Result<(MTI, u16)>` | Get the MTI and source alias from this frame |
| `source_alias` | `fn source_alias(&self) -> u16` | Get the source alias from this frame |
| `get_dest_from_body` | `fn get_dest_from_body(&self) -> Result<(u16, &[u8])>` | Extract destination alias from addressed message body |

#### GridConnect Format

```
:X[8-hex-header]N[0-16-hex-data];
```

Example: `:X19170123N0102030405060708;`

#### Example

```rust
use lcc_rs::protocol::{GridConnectFrame, MTI};

// Parse a frame
let frame = GridConnectFrame::parse(":X19170123N0102030405060708;").unwrap();
assert_eq!(frame.header, 0x19170123);
assert_eq!(frame.data, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);

// Create a frame
let frame = GridConnectFrame::from_mti(
    MTI::VerifyNodeGlobal,
    0xAAA,
    vec![],
).unwrap();
println!("{}", frame.to_string()); // ":X19490AAAN;"

// Create an addressed frame
let frame = GridConnectFrame::from_addressed_mti(
    MTI::VerifyNodeAddressed,
    0xAAA,  // source
    0xDDD,  // destination
    vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
).unwrap();
```

---

### MTI (Message Type Identifier)

Message Type Identifiers define the message type in LCC protocol.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MTI {
    // Node management
    InitializationComplete,
    VerifyNodeGlobal,
    VerifyNodeAddressed,
    VerifiedNode,
    
    // Protocol support
    OptionalInteractionRejected,
    TerminateDueToError,
    ProtocolSupportInquiry,
    ProtocolSupportReply,
    
    // Event handling
    IdentifyConsumers,
    ConsumerIdentifiedValid,
    ConsumerIdentifiedInvalid,
    ConsumerIdentifiedUnknown,
    IdentifyProducers,
    ProducerIdentifiedValid,
    ProducerIdentifiedInvalid,
    ProducerIdentifiedUnknown,
    IdentifyEventsGlobal,
    IdentifyEventsAddressed,
    ConsumerRangeIdentified,
    ProducerRangeIdentified,
    
    // Alias allocation
    CheckID,
    ReserveID,
    AliasMapReset,
    
    // Datagrams
    Datagram,
    DatagramOnly,
    DatagramFirst,
    DatagramMiddle,
    DatagramFinal,
    DatagramReceivedOk,
    DatagramRejected,
    
    // SNIP
    SNIPRequest,
    SNIPResponse,
    
    Unknown(u32),
}
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `value` | `fn value(&self) -> u32` | Get the raw MTI value |
| `from_value` | `fn from_value(value: u32) -> Self` | Create MTI from raw value |
| `from_header` | `fn from_header(header: u32) -> Result<(MTI, u16)>` | Extract MTI and source alias from 29-bit CAN header |
| `to_header` | `fn to_header(&self, source_alias: u16) -> Result<u32>` | Create CAN header from MTI and source alias |
| `from_datagram_header` | `fn from_datagram_header(header: u32) -> Result<(MTI, u16, u16)>` | Extract MTI, source alias, and dest alias from datagram header |

#### Common MTI Values

| MTI | Value | Description |
|-----|-------|-------------|
| `VerifyNodeGlobal` | `0x19490` | Request all nodes to identify themselves |
| `VerifyNodeAddressed` | `0x19488` | Request specific node to identify itself |
| `VerifiedNode` | `0x19170` | Node identification response |
| `SNIPRequest` | `0x19DE8` | Request SNIP data |
| `SNIPResponse` | `0x19A08` | SNIP data response |
| `IdentifyProducers` | `0x19914` | Query event producers |
| `IdentifyConsumers` | `0x198F4` | Query event consumers |

---

### DatagramAssembler

Handles multi-frame datagram reassembly.

```rust
pub struct DatagramAssembler;
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Self` | Create a new datagram assembler |
| `handle_frame` | `fn handle_frame(&mut self, frame: &GridConnectFrame) -> Result<Option<Vec<u8>>>` | Handle an incoming datagram frame; returns `Some(payload)` if complete |

#### DatagramState

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatagramState {
    Idle,        // No datagram in progress
    Receiving,   // Receiving multi-frame datagram
    Complete,    // Datagram complete
    Error,       // Error occurred
}
```

#### Example

```rust
use lcc_rs::protocol::{DatagramAssembler, GridConnectFrame};

let mut assembler = DatagramAssembler::new();

// Handle frames
if let Some(payload) = assembler.handle_frame(&frame)? {
    println!("Complete datagram: {:?}", payload);
}
```

---

## Transport Layer (`transport` module)

### LccTransport (Trait)

Transport trait for sending and receiving frames.

```rust
#[async_trait::async_trait]
pub trait LccTransport: Send + Sync {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()>;
    async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>>;
    async fn close(&mut self) -> Result<()>;
}
```

---

### TcpTransport

TCP transport implementation.

```rust
pub struct TcpTransport;
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `connect` | `async fn connect(host: &str, port: u16) -> Result<Self>` | Connect to an LCC network via TCP |

#### Example

```rust
use lcc_rs::transport::TcpTransport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut transport = TcpTransport::connect("localhost", 12021).await?;
    Ok(())
}
```

---

## Discovery (`discovery` module)

### LccConnection

High-level LCC connection for performing network operations.

```rust
pub struct LccConnection;
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `connect` | `async fn connect(host: &str, port: u16) -> Result<Self>` | Connect to an LCC network via TCP |
| `with_transport` | `fn with_transport(transport: Box<dyn LccTransport>, our_alias: NodeAlias) -> Self` | Create with custom transport (for testing) |
| `discover_nodes` | `async fn discover_nodes(&mut self, timeout_ms: u64) -> Result<Vec<DiscoveredNode>>` | Discover all nodes on the network |
| `verify_node` | `async fn verify_node(&mut self, dest_alias: u16, timeout_ms: u64) -> Result<Option<NodeID>>` | Verify a specific node's presence |
| `query_snip` | `async fn query_snip(&mut self, dest_alias: u16, semaphore: Option<Arc<Semaphore>>) -> Result<(Option<SNIPData>, SNIPStatus)>` | Query SNIP data from a node |
| `close` | `async fn close(self) -> Result<()>` | Close the connection |

#### Example: Basic Discovery

```rust
use lcc_rs::LccConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to LCC network
    let mut connection = LccConnection::connect("localhost", 12021).await?;
    
    // Discover nodes (250ms timeout)
    let nodes = connection.discover_nodes(250).await?;
    
    for node in nodes {
        println!("Found node: {} (alias: {})", node.node_id, node.alias);
    }
    
    Ok(())
}
```

#### Discovery Implementation Details

- Sends **Verify Node ID Global** (MTI `0x0490`)
- Collects **Verified Node** (MTI `0x0170`) responses
- Uses **silence detection**: stops when no frames arrive for 25ms
- Maximum timeout prevents hanging on busy networks
- Returns `Vec<DiscoveredNode>` with Node IDs and aliases

---

## SNIP (`snip` module)

### query_snip

Query SNIP data from a specific node.

```rust
pub async fn query_snip(
    transport: &mut dyn LccTransport,
    source_alias: u16,
    dest_alias: u16,
    semaphore: Arc<Semaphore>,
) -> Result<(Option<SNIPData>, SNIPStatus)>
```

#### Parameters

- `transport` - LCC transport connection (mutable reference)
- `source_alias` - Our alias (source of the request)
- `dest_alias` - Target node's alias
- `semaphore` - Semaphore for concurrency limiting (typically capacity 5)

#### Returns

- `Ok((Some(SNIPData), SNIPStatus::Complete))` - Successfully retrieved SNIP data
- `Ok((None, SNIPStatus::Timeout))` - Request timed out
- `Err(_)` - Network or protocol error

#### Timeouts

- **SNIP request timeout**: 5 seconds
- **Silence detection**: 100ms with no frames = end of response

#### Example

```rust
use lcc_rs::{LccConnection, snip::query_snip};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = LccConnection::connect("localhost", 12021).await?;
    
    // Discover nodes
    let nodes = connection.discover_nodes(250).await?;
    
    // Query SNIP for each node
    let semaphore = Arc::new(Semaphore::new(5)); // Limit to 5 concurrent requests
    
    for node in nodes {
        let (snip_data, status) = connection.query_snip(
            node.alias.value(),
            Some(semaphore.clone())
        ).await?;
        
        if let Some(data) = snip_data {
            println!("Node: {}", node.node_id);
            println!("  Manufacturer: {}", data.manufacturer);
            println!("  Model: {}", data.model);
            println!("  HW Version: {}", data.hardware_version);
            println!("  SW Version: {}", data.software_version);
        }
    }
    
    Ok(())
}
```

---

### parse_snip_payload

Parse SNIP data from raw payload bytes.

```rust
pub fn parse_snip_payload(payload: &[u8]) -> Result<SNIPData>
```

#### SNIP Payload Format

SNIP data is encoded as null-terminated strings in this order:
1. Manufacturer
2. Model
3. Hardware version
4. Software version
5. User name (node 1)
6. User description (node 1)

Each field is terminated by `0x00`. If a field is empty, it's just `0x00`.

---

## Memory Configuration Protocol (`protocol::memory_config` module)

The Memory Configuration Protocol provides read/write access to node memory spaces, including configuration memory and CDI (Configuration Description Information).

### AddressSpace

Memory address space identifiers for the Memory Configuration Protocol.

```rust
pub enum AddressSpace {
    Configuration,  // 0xFD - Configuration space
    AllMemory,      // 0xFE - All memory space
    Cdi,            // 0xFF - CDI space
}
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `value` | `fn value(&self) -> u8` | Get the space byte value (0xFD, 0xFE, or 0xFF) |
| `command_flag` | `fn command_flag(&self) -> u8` | Get the command flag for this space |

### MemoryConfigCmd

Command builder for Memory Configuration Protocol operations.

```rust
pub struct MemoryConfigCmd;
```

#### build_read

Build a read command datagram to retrieve data from a memory space.

```rust
pub fn build_read(
    source_alias: u16,
    dest_alias: u16,
    space: AddressSpace,
    address: u32,
    count: u8,
) -> Result<Vec<GridConnectFrame>>
```

**Parameters:**
- `source_alias` - Our node alias
- `dest_alias` - Target node alias
- `space` - Address space to read from (Configuration, AllMemory, or Cdi)
- `address` - Starting address (32-bit, big-endian)
- `count` - Number of bytes to read (1-64)

**Returns:** Vector of GridConnect frames to send (may be single or multi-frame datagram)

**Example:**
```rust
use lcc_rs::protocol::memory_config::{MemoryConfigCmd, AddressSpace};

// Read first 64 bytes of CDI
let frames = MemoryConfigCmd::build_read(
    0xAAA,              // source alias
    0xBBB,              // dest alias
    AddressSpace::Cdi,  // CDI space
    0,                  // start at address 0
    64                  // read 64 bytes
)?;

// Send frames to transport
for frame in frames {
    transport.send_frame(&frame).await?;
}
```

#### parse_read_reply

Parse a read reply datagram from the target node.

```rust
pub fn parse_read_reply(data: &[u8]) -> Result<ReadReply>
```

**Parameters:**
- `data` - Datagram payload received from target node

**Returns:** `ReadReply` enum indicating success or failure

**Example:**
```rust
use lcc_rs::protocol::memory_config::{MemoryConfigCmd, ReadReply};

// Assume we received a datagram response
let reply = MemoryConfigCmd::parse_read_reply(&datagram_payload)?;

match reply {
    ReadReply::Success { address, space, data } => {
        println!("Read {} bytes from address 0x{:08X}", data.len(), address);
        // Process data...
    }
    ReadReply::Failed { address, space, error_code, message } => {
        eprintln!("Read failed at 0x{:08X}: {} (code: 0x{:04X})", 
                 address, message, error_code);
    }
}
```

### ReadReply

Result type for memory read operations.

```rust
pub enum ReadReply {
    Success {
        address: u32,
        space: AddressSpace,
        data: Vec<u8>,
    },
    Failed {
        address: u32,
        space: AddressSpace,
        error_code: u16,
        message: String,
    },
}
```

#### Variants

**Success:**
- `address` - Address that was read
- `space` - Address space that was accessed
- `data` - Payload data received (0-58 bytes per datagram)

**Failed:**
- `address` - Address where the error occurred
- `space` - Address space that was accessed
- `error_code` - OpenLCB standard error code
- `message` - Optional error message from the node

### Complete CDI Retrieval Example

```rust
use lcc_rs::{LccConnection, protocol::memory_config::{MemoryConfigCmd, AddressSpace, ReadReply}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = LccConnection::connect("localhost", 12021).await?;
    
    // Discover and get a node alias
    let nodes = connection.discover_nodes(250).await?;
    let target_alias = nodes[0].alias.value();
    let source_alias = connection.our_alias()?;
    
    let mut cdi_data = Vec::new();
    let mut address = 0u32;
    let chunk_size = 64u8;
    
    loop {
        // Build read command
        let frames = MemoryConfigCmd::build_read(
            source_alias,
            target_alias,
            AddressSpace::Cdi,
            address,
            chunk_size
        )?;
        
        // Send frames
        for frame in frames {
            connection.send_frame(&frame).await?;
        }
        
        // Wait for datagram response (simplified - actual code needs datagram assembly)
        let datagram_payload = connection.receive_datagram(target_alias).await?;
        
        // Parse reply
        let reply = MemoryConfigCmd::parse_read_reply(&datagram_payload)?;
        
        match reply {
            ReadReply::Success { data, .. } => {
                if data.is_empty() {
                    break; // End of CDI
                }
                cdi_data.extend_from_slice(&data);
                address += data.len() as u32;
            }
            ReadReply::Failed { error_code, message, .. } => {
                eprintln!("CDI read failed: {} (code: 0x{:04X})", message, error_code);
                break;
            }
        }
    }
    
    // CDI is typically XML
    let cdi_xml = String::from_utf8_lossy(&cdi_data);
    println!("Retrieved CDI ({} bytes):\n{}", cdi_data.len(), cdi_xml);
    
    Ok(())
}
```

---

## Error Handling

### Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid frame format: {0}")]
    InvalidFrame(String),
    
    #[error("Invalid MTI: {0}")]
    InvalidMTI(String),
    
    #[error("Transport error: {0}")]
    Transport(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
}
```

### Result Type

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

---

## Common Usage Patterns

### Pattern 1: Connect and Discover Nodes

```rust
use lcc_rs::LccConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = LccConnection::connect("localhost", 12021).await?;
    let nodes = connection.discover_nodes(250).await?;
    
    for node in nodes {
        println!("Found node: {}", node.node_id);
    }
    
    Ok(())
}
```

---

### Pattern 2: Discover Nodes with SNIP Data

```rust
use lcc_rs::LccConnection;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = LccConnection::connect("localhost", 12021).await?;
    
    // Discover nodes
    let nodes = connection.discover_nodes(250).await?;
    println!("Discovered {} nodes", nodes.len());
    
    // Query SNIP for each node (limit to 5 concurrent)
    let semaphore = Arc::new(Semaphore::new(5));
    
    for node in nodes {
        let (snip_data, status) = connection.query_snip(
            node.alias.value(),
            Some(semaphore.clone())
        ).await?;
        
        println!("\nNode: {} (alias: {})", node.node_id, node.alias);
        
        if let Some(data) = snip_data {
            println!("  Manufacturer: {}", data.manufacturer);
            println!("  Model: {}", data.model);
            println!("  HW Version: {}", data.hardware_version);
            println!("  SW Version: {}", data.software_version);
            println!("  User Name: {}", data.user_name);
            println!("  Description: {}", data.user_description);
        } else {
            println!("  SNIP Status: {:?}", status);
        }
    }
    
    connection.close().await?;
    Ok(())
}
```

---

### Pattern 3: Verify Specific Node

```rust
use lcc_rs::LccConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = LccConnection::connect("localhost", 12021).await?;
    
    // Verify a specific node by alias
    let dest_alias = 0xDDD;
    
    if let Some(node_id) = connection.verify_node(dest_alias, 500).await? {
        println!("Node {:03X} has ID: {}", dest_alias, node_id);
    } else {
        println!("Node {:03X} did not respond", dest_alias);
    }
    
    Ok(())
}
```

---

### Pattern 4: Parse GridConnect Frames from Stream

```rust
use lcc_rs::protocol::{GridConnectFrame, MTI};

fn process_frame(line: &str) -> Result<(), Box<dyn std::error::Error>> {
    let frame = GridConnectFrame::parse(line)?;
    let (mti, source_alias) = frame.get_mti()?;
    
    match mti {
        MTI::VerifiedNode => {
            let node_id = lcc_rs::NodeID::from_slice(&frame.data)?;
            println!("Node {:03X} has ID: {}", source_alias, node_id);
        }
        MTI::VerifyNodeGlobal => {
            println!("Node discovery request from {:03X}", source_alias);
        }
        _ => {
            println!("Received {:?} from {:03X}", mti, source_alias);
        }
    }
    
    Ok(())
}
```

---

### Pattern 5: Create and Send Frames

```rust
use lcc_rs::protocol::{GridConnectFrame, MTI};
use lcc_rs::transport::{TcpTransport, LccTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut transport = TcpTransport::connect("localhost", 12021).await?;
    
    // Send a global verify request
    let frame = GridConnectFrame::from_mti(
        MTI::VerifyNodeGlobal,
        0xAAA,
        vec![],
    )?;
    
    transport.send(&frame).await?;
    
    // Wait for responses
    while let Some(response) = transport.receive(1000).await? {
        let (mti, alias) = response.get_mti()?;
        if mti == MTI::VerifiedNode {
            println!("Found node with alias {:03X}", alias);
        }
    }
    
    Ok(())
}
```

---

### Pattern 6: Handle Multi-frame Datagrams

```rust
use lcc_rs::protocol::{DatagramAssembler, GridConnectFrame};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut assembler = DatagramAssembler::new();
    
    // Process incoming frames
    loop {
        // Receive frame from transport...
        let frame: GridConnectFrame = receive_frame().await?;
        
        // Try to assemble datagram
        if let Some(payload) = assembler.handle_frame(&frame)? {
            println!("Complete datagram received: {} bytes", payload.len());
            process_datagram(&payload)?;
        }
    }
    
    Ok(())
}

// Stub functions
async fn receive_frame() -> Result<GridConnectFrame, Box<dyn std::error::Error>> {
    unimplemented!()
}

fn process_datagram(payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    unimplemented!()
}
```

---

## CDI Module (`cdi` module)

**Status:** ✅ Implemented in Feature 003 (Miller Columns)

The CDI module provides comprehensive support for parsing, navigating, and analyzing Configuration Description Information (CDI) XML documents according to the OpenLCB S-9.7.4.1 standard.

### Module Organization

```
lcc_rs::cdi
├── mod.rs          - Type definitions (Cdi, Segment, DataElement, Group, etc.)
├── parser.rs       - XML parsing with roxmltree
└── hierarchy.rs    - Navigation and hierarchy helpers
```

###Type Definitions

#### `Cdi`

Root CDI structure representing the complete configuration description.

```rust
pub struct Cdi {
    pub identification: Option<Identification>,
    pub acdi: Option<Acdi>,
    pub segments: Vec<Segment>,
}
```

#### `Segment`

Configuration segment with an address space and data elements.

```rust
pub struct Segment {
    pub name: Option<String>,
    pub description: Option<String>,
    pub space: u8,              // Address space (0-255)
    pub origin: i32,            // Starting address
    pub elements: Vec<DataElement>,
}
```

#### `DataElement`

Enumeration of all possible CDI elements.

```rust
pub enum DataElement {
    Group(Group),
    Int(IntElement),
    String(StringElement),
    EventId(EventIdElement),
    Float(FloatElement),
    Action(ActionElement),
    Blob(BlobElement),
}
```

#### `Group`

Grouping element that can contain nested elements and support replication.

```rust
pub struct Group {
    pub name: Option<String>,
    pub description: Option<String>,
    pub offset: i32,
    pub replication: usize,     // Number of instances (1 = non-replicated)
    pub repname: Vec<String>,   // Replication name templates
    pub elements: Vec<DataElement>,
    pub hints: Option<Hints>,
}
```

**Key Methods:**
- `should_render() -> bool` - Returns false if group is empty (per S-9.7.4.1 Footnote 4)
- `expand_replications() -> Vec<ExpandedGroup>` - Generates N instances with computed names

#### `IntElement`, `StringElement`, `EventIdElement`, etc.

Primitive configuration elements with their specific attributes (size, constraints, defaults).

### Parsing Functions

#### `parse_cdi`

Parse CDI XML into structured data model.

```rust
pub fn parse_cdi(xml: &str) -> Result<Cdi, String>
```

**Parameters:**
- `xml`: CDI XML content as string

**Returns:**
- `Ok(Cdi)`: Parsed CDI structure
- `Err(String)`: Parse error with context

**Example:**
```rust
use lcc_rs::cdi::parse_cdi;

let cdi_xml = r#"<?xml version="1.0"?>
<cdi>
  <segment space="253">
    <name>Configuration</name>
    <group replication="16">
      <name>Input</name>
      <eventid><name>Producer</name></eventid>
    </group>
  </segment>
</cdi>"#;

let cdi = parse_cdi(cdi_xml)?;
println!("Parsed {} segments", cdi.segments.len());
```

**Features:**
- Recursive element parsing (handles unlimited nesting depth)
- Error recovery (parses valid portions, reports issues)
- Schema validation (checks required attributes, data types)
- Replication support (parse replication count and repname templates)

### Navigation Functions

#### `navigate_to_path`

Navigate CDI hierarchy using index-based pathId system.

```rust
pub fn navigate_to_path<'a>(
    cdi: &'a Cdi,
    path: &[String]
) -> Result<NavigationResult<'a>, String>
```

**Parameters:**
- `cdi`: CDI structure to navigate
- `path`: Array of pathIds (e.g., `["seg:0", "elem:0#12", "elem:2"]`)

**Returns:**
- `Ok(NavigationResult::Segment(&Segment))` - Found segment
- `Ok(NavigationResult::Element(&DataElement))` - Found element
- `Err(String)` - Path not found or invalid

**pathId Format:**
- Segment: `seg:N` (N = 0-based segment index)
- Element: `elem:N` (N = 0-based element index)
- Replicated instance: `elem:N#I` (N = element index, I = 1-based instance number)

**Example:**
```rust
use lcc_rs::cdi::{parse_cdi, navigate_to_path, NavigationResult};

let cdi = parse_cdi(cdi_xml)?;

// Navigate to segment 0
let result = navigate_to_path(&cdi, &["seg:0".to_string()])?;
match result {
    NavigationResult::Segment(seg) => {
        println!("Found segment: {}", seg.name.as_ref().unwrap());
    }
    _ => {}
}

// Navigate to replicated group instance #12
let path = vec![
    "seg:0".to_string(),
    "elem:0#12".to_string(),  // First element, instance #12
];
let result = navigate_to_path(&cdi, &path)?;
```

**Why Index-Based:**
- Eliminates ambiguity with element names containing '#' (e.g., "Variable #1")
- O(1) lookup via array indexing
- Stable references independent of name changes

#### `calculate_max_depth`

Calculate maximum nesting depth in CDI structure.

```rust
pub fn calculate_max_depth(cdi: &Cdi) -> usize
```

**Returns:** Maximum depth (1 = segments only, 2+ = nested groups/elements)

**Example:**
```rust
use lcc_rs::cdi::{parse_cdi, calculate_max_depth};

let cdi = parse_cdi(cdi_xml)?;
let depth = calculate_max_depth(&cdi);
println!("CDI has {} levels of nesting", depth);
```

### Helper Types

#### `NavigationResult<'a>`

Result of navigation operation.

```rust
pub enum NavigationResult<'a> {
    Segment(&'a Segment),
    Element(&'a DataElement),
}
```

#### `ExpandedGroup`

Expanded instance from replicated group.

```rust
pub struct ExpandedGroup {
    pub index: usize,       // 0-based instance index
    pub name: String,       // Computed instance name (e.g., "Logic 12")
}
```

### Testing

The CDI module includes comprehensive test coverage:

- **Unit tests:** Parser validation, element extraction, replication expansion
- **Integration tests:** Real CDI XML samples from Tower-LCC, I/O nodes
- **Property-based tests:** Path round-trip validation (using proptest)
- **Edge case tests:** Malformed XML, empty groups, names with special characters

**Test Example:**
```rust
#[test]
fn test_navigate_to_replicated_instance() {
    let cdi = parse_cdi(test_cdi_xml).unwrap();
    
    // Navigate to Logic group instance #12
    let path = vec!["seg:0".to_string(), "elem:0#12".to_string()];
    let result = navigate_to_path(&cdi, &path);
    
    assert!(result.is_ok());
}
```

---

## Testing Support

### Mock Transport

For testing, implement `LccTransport` trait:

```rust
use lcc_rs::transport::LccTransport;
use lcc_rs::protocol::GridConnectFrame;
use lcc_rs::Result;
use async_trait::async_trait;

struct MockTransport {
    responses: Vec<GridConnectFrame>,
    sent_frames: Vec<GridConnectFrame>,
    response_index: usize,
}

impl MockTransport {
    fn new(responses: Vec<GridConnectFrame>) -> Self {
        Self {
            responses,
            sent_frames: Vec::new(),
            response_index: 0,
        }
    }
}

#[async_trait]
impl LccTransport for MockTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        self.sent_frames.push(frame.clone());
        Ok(())
    }
    
    async fn receive(&mut self, _timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
        if self.response_index < self.responses.len() {
            let frame = self.responses[self.response_index].clone();
            self.response_index += 1;
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }
    
    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
```

---

## Performance Considerations

### Concurrency Control

When querying SNIP data for multiple nodes, use a semaphore to limit concurrent requests:

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

let semaphore = Arc::new(Semaphore::new(5)); // Max 5 concurrent SNIP requests
```

**Why?** Prevents overwhelming the network and reduces packet loss.

### Timeouts

- **Node discovery**: 250ms typical (25ms silence detection)
- **Node verification**: 500ms typical
- **SNIP requests**: 5 seconds (includes multi-frame reassembly)

### TCP Settings

The TCP transport automatically sets `TCP_NODELAY` for lower latency.

---

## Protocol References

### OpenLCB Standards

- **S-9.7.0.3**: Unique Identifiers
- **S-9.7.0.4**: Event Identifiers
- **S-9.7.1.1**: CAN Physical Layer
- **S-9.7.2.1**: CAN Frame Transfer
- **S-9.7.3**: Message Network
- **S-9.7.3.1**: Event Transport
- **S-9.7.3.2**: Datagram Transport
- **S-9.7.4.1**: Configuration Description Information (SNIP)

### GridConnect Format

GridConnect is the ASCII representation of CAN frames:

```
:X[header]N[data];
```

- `:X` - Start marker
- `[header]` - 8 hex digits (29-bit CAN header)
- `N` - Separator
- `[data]` - 0-16 hex digits (0-8 bytes)
- `;` - End marker

---

## Dependencies

- **tokio**: Async runtime
- **async-trait**: Async trait support
- **serde**: Serialization/deserialization
- **chrono**: DateTime handling
- **thiserror**: Error handling
- **roxmltree**: CDI XML parsing (zero-copy, fast)
- **lazy_static**: CDI parsing cache
- **uuid**: Unique identifier generation for UI elements

---

## Future Enhancements

Potential future additions to the library:

- [ ] Event production and consumption
- [x] CDI XML parsing and structured navigation
- [ ] Configuration value reading from node memory
- [ ] Configuration value writing to node memory
- [ ] Firmware upgrade support
- [ ] CAN transport (in addition to TCP)
- [ ] USB transport
- [ ] Node alias allocation
- [ ] Full datagram protocol (not just SNIP)
- [ ] Stream protocol
- [ ] Clock synchronization

---

## License

See LICENSE file in the repository.

---

## Contributing

Contributions welcome! Please ensure all tests pass and maintain consistent code style.

---

## Example: Complete Application

```rust
use lcc_rs::LccConnection;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to LCC network...");
    let mut connection = LccConnection::connect("localhost", 12021).await?;
    
    println!("Discovering nodes...");
    let nodes = connection.discover_nodes(250).await?;
    println!("Discovered {} nodes\n", nodes.len());
    
    // Query SNIP data for each node
    let semaphore = Arc::new(Semaphore::new(5));
    
    for node in nodes {
        println!("Node: {} (alias: {})", node.node_id, node.alias);
        
        let (snip_data, status) = connection.query_snip(
            node.alias.value(),
            Some(semaphore.clone())
        ).await?;
        
        match (snip_data, status) {
            (Some(data), _) => {
                println!("  Manufacturer: {}", data.manufacturer);
                println!("  Model: {}", data.model);
                println!("  HW Version: {}", data.hardware_version);
                println!("  SW Version: {}", data.software_version);
                if !data.user_name.is_empty() {
                    println!("  User Name: {}", data.user_name);
                }
                if !data.user_description.is_empty() {
                    println!("  Description: {}", data.user_description);
                }
            }
            (None, status) => {
                println!("  SNIP Status: {:?}", status);
            }
        }
        println!();
    }
    
    println!("Closing connection...");
    connection.close().await?;
    println!("Done!");
    
    Ok(())
}
```

---

**End of Documentation**
