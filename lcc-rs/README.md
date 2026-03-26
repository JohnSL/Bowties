# lcc-rs

A Rust implementation of the LCC (Layout Command Control) / OpenLCB protocol.

## Overview

This crate provides a client library for the OpenLCB/LCC protocol, commonly used in model railroading for distributed control systems. It is the protocol layer for the [Bowties](../README.md) desktop application.

### Quick Example

```rust,no_run
use lcc_rs::{LccConnection, NodeID};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let node_id = NodeID::new([0x05, 0x01, 0x01, 0x01, 0xA2, 0xFF]);
    let mut connection = LccConnection::connect("localhost", 12021, node_id).await?;
    let nodes = connection.discover_nodes(250).await?;

    for node in nodes {
        println!("Found node: {}", node.node_id);
    }

    Ok(())
}
```

## Implemented Features

### Message Network (S-9.7.3)
- All standard MTI encoding/decoding (34 message types)
- GridConnect frame parsing and encoding (`:X[header]N[data];`)
- Addressed and global message handling
- Optional Interaction Rejected / Terminate Due to Error

### CAN Physical Layer (S-9.7.1.1) & Frame Transfer (S-9.7.2.1)
- CAN alias allocation (CID/RID/AMD/AMR sequences)
- Alias conflict detection and automatic re-acquisition
- Alias map tracking (AMD/AMR/AliasMapEnquiry)

### Node Discovery
- `VerifyNodeGlobal` / `VerifyNodeAddressed` → `VerifiedNode` flow
- Concurrent multi-node discovery with configurable timeouts
- Automatic protocol query responder (replies to Verify, PIP, SNIP requests)

### Simple Node Information Protocol — SNIP (S-9.7.4.3)
- Multi-frame SNIP response assembly
- Payload parsing and encoding (`SNIPData`)
- SNIP request/response serving

### Protocol Identification Protocol — PIP (S-9.7.3)
- `ProtocolSupportInquiry` / `ProtocolSupportReply`
- `ProtocolFlags` bitmap (Simple/Datagram/Stream/MemoryConfig/CDI/SNIP/etc.)

### Configuration Description Information — CDI (S-9.7.4.1)
- Full CDI XML parsing into typed Rust structs (`Cdi`, `Segment`, `Group`, data elements)
- Hierarchical CDI navigation (`walk_event_slots`)
- Event role classification (Producer/Consumer/Ambiguous) with two-tier heuristic

### Memory Configuration Protocol (S-9.7.4.2)
- Read memory (single and multi-datagram, with adaptive timing)
- Write memory (with reply-pending support and retry logic)
- CDI read (chunked, with 0x1082 end-of-CDI detection)
- Address space info queries
- Factory reset and reboot commands
- Update Complete signaling

### Datagram Protocol (S-9.7.2)
- Single-frame (`DatagramOnly`) and multi-frame (`First`/`Middle`/`Final`) assembly
- `DatagramReceivedOk` / `DatagramRejected` handling with retry
- Configurable retry with resend-OK and reply-pending flags

### Event Transport (S-9.7.3.1)
- `IdentifyConsumers` / `IdentifyProducers` query and response parsing
- `IdentifyEventsGlobal` / `IdentifyEventsAddressed`
- `ConsumerIdentified` / `ProducerIdentified` (Valid/Invalid/Unknown)
- Range-identified messages

### Transport Layer
- **TCP** — GridConnect-over-TCP (`TcpTransport`)
- **Serial** — GridConnect-over-serial (`GridConnectSerialTransport`)
- **SLCAN** — SLCAN serial (`SlcanSerialTransport`)
- **Mock** — Queue-based mock for testing (`MockTransport`)

### Message Dispatcher
- Filter-based async message routing (`MessageDispatcher`)
- One-shot and persistent filters with timeout support
- Background receive loop with shared `Arc<Mutex<>>` access

## Not Yet Implemented

| Feature | Standard | Notes |
|---------|----------|-------|
| Producer/Consumer Event Report (PCER) | S-9.7.3.1 | Sending/receiving `ProducerConsumerEventReport` (MTI 0x05B4). See [PCER design doc](../docs/design/pcer-implementation.md). |
| Teach/Learn Protocol | S-9.7.4.1 | Teaching event IDs between nodes |
| Traction Control | S-9.7.4.5 | DCC locomotive control |
| Firmware Upgrade | S-9.7.4.4 | Node firmware upload |
| Stream Transport | S-9.7.2 | Stream datagrams (allocated but unused) |
| Display Protocol | — | Text display messaging |
| Remote Button | — | Physical button emulation |

## Crate Structure

```
lcc-rs/
├── src/
│   ├── lib.rs                  # Public API re-exports
│   ├── types.rs                # NodeID, EventID, NodeAlias, SNIPData, ProtocolFlags, etc.
│   ├── constants.rs            # Timing and retry constants
│   ├── discovery.rs            # LccConnection — connect, discover, read/write memory, CDI
│   ├── dispatcher.rs           # MessageDispatcher — async filter-based message routing
│   ├── alias_allocation.rs     # CAN alias allocation (CID/RID/AMD sequences)
│   ├── snip.rs                 # SNIP query, parse, encode
│   ├── pip.rs                  # PIP query and ProtocolFlags parsing
│   ├── cdi/
│   │   ├── parser.rs           # CDI XML → Cdi struct
│   │   ├── hierarchy.rs        # walk_event_slots traversal
│   │   └── role.rs             # EventRole classification heuristic
│   ├── protocol/
│   │   ├── frame.rs            # GridConnectFrame parse/encode
│   │   ├── mti.rs              # MTI enum and header encoding
│   │   ├── datagram.rs         # DatagramAssembler (multi-frame)
│   │   └── memory_config.rs    # MemoryConfigCmd build/parse
│   └── transport/
│       ├── tcp.rs              # TcpTransport
│       ├── gridconnect_serial.rs
│       ├── slcan_serial.rs
│       └── mock.rs             # MockTransport for tests
└── tests/
    ├── protocol_integration.rs # 11 protocol sequence tests
    └── cdi_parsing.rs          # 21 CDI XML parsing tests
```

## Tests

325 tests total: 288 unit, 32 integration, 5 doc-tests.

```bash
cargo test              # all tests
cargo test --lib        # unit tests only
cargo test --test protocol_integration
cargo test --test cdi_parsing
```

Unit tests by module:

| Module | Tests | Coverage |
|--------|------:|----------|
| `protocol` (frame, mti, datagram, memory_config) | 137 | Frame encoding, MTI values, datagram assembly, memory config build/parse |
| `cdi` | 62 | XML parsing, hierarchy walk, event role classification |
| `types` | 26 | NodeID, EventID, NodeAlias, ProtocolFlags encoding |
| `discovery` | 26 | Node discovery, CDI read, memory read/write, alias conflict, reply-pending |
| `snip` | 14 | Multi-frame assembly, boundary-spanning, payload parse/encode |
| `transport` | 10 | TCP frame encoding, serial transports |
| `pip` | 6 | PIP query, OIR fast-fail, ProtocolFlags parsing |
| `dispatcher` | 5 | Filter routing, alias map tracking |
| `alias_allocation` | 2 | CID/RID sequence generation |

Property-based tests (via `proptest`) cover frame round-trip encoding, header bit preservation, and alias extraction.

## Reference Implementation

This crate is cross-validated against the [OpenLCB Java](https://github.com/openlcb/OpenLCB_Java) reference implementation, particularly for memory configuration protocol behavior, datagram retry semantics, and SNIP/PIP response handling.

## License

Licensed under either of [MIT License](../LICENSE-MIT) or [Apache License, Version 2.0](../LICENSE-APACHE) at your option.
