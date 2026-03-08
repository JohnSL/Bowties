# lcc-rs

A Rust implementation of the LCC (Layout Command Control) / OpenLCB protocol.

## Overview

This library provides a comprehensive implementation of the OpenLCB/LCC protocol for model railroad control and other CAN-based control networks. It includes:

- **GridConnect frame parsing/encoding** - ASCII representation of CAN frames
- **MTI (Message Type Identifier) handling** - All standard OpenLCB message types
- **Node discovery** - Finding nodes on the network
- **Event handling** - Producer/Consumer event model
- **Datagram support** - Single and multi-frame datagrams
- **Addressed messaging** - Point-to-point communication
- **Alias allocation** - CAN alias management (CID/RID/AMR)

## Test Coverage

The library has comprehensive test coverage with **113 unit tests** and **11 integration tests**, organized as follows:

### Unit Tests (113 total)

#### Protocol Layer - MTI Tests (16 tests)
- All MTI value encoding/decoding against Python reference
- Datagram MTI with destination alias encoding
- Alias allocation MTIs (CID, RID, AMR)
- Range identified MTIs
- Round-trip encoding/decoding
- Header bit manipulation validation

**Python Reference:** `canolcbutils.py`, `defaults.py`

#### Protocol Layer - Frame Tests (61 tests)
Organized into sub-modules:

**Basic Frame Operations (18 tests):**
- GridConnect format parsing (`:X[header]N[data];`)
- Encoding to string with uppercase hex
- Round-trip parse/encode identity
- MTI and alias extraction
- Error handling (malformed frames, invalid lengths)

**Python Reference Fixtures (11 tests):**
- Frame encoding matching `makeframestring()` pattern
- Byte zero-padding (`[0x01]` → `"01"`)
- Case-insensitive parsing
- Common message patterns from Python tests
- Header exactly 8 hex digits

**Edge Cases (13 tests):**
- Standard vs extended frame format detection
- Header boundary values (29-bit max)
- Data length boundaries (0-8 bytes)
- Malformed delimiters and whitespace
- Invalid hex characters
- Mixed case handling

**Addressed Message Body Format (8 tests):**
- Destination alias in first 2 bytes (big-endian)
- Payload extraction
- 12-bit boundary validation
- Python reference pattern verification

**Alias Allocation Sequences (8 tests):**
- CID/RID/AMR frame generation
- InitializationComplete with NodeID
- Complete allocation sequence
- Conflict detection and recovery

**Property-Based Tests (8 tests using proptest):**
- Round-trip encode/decode for any valid frame
- Header bit preservation (29-bit)
- Alias extraction (12-bit)
- Hex case insensitivity
- MTI round-trip through headers
- Output always uppercase
- Addressed message dest round-trip

**Python Reference:** `canolcbutils.py` `makeframestring()`, `bodyArray()`, `testStartup.py`, `testAliasConflict.py`, `verifyNodeAddressed.py`

#### Type Tests (24 tests)
- NodeID encoding/decoding (6 bytes, big-endian)
- EventID encoding (8 bytes)
- NodeAlias validation (12-bit)
- Hex string parsing (various formats)
- Python reference constants (`thisNodeID`, `testNodeID`, `testEventID`)

**Python Reference:** `defaults.py`

#### Discovery Tests (5 tests)
- Mock transport patterns
- VerifyNodeGlobal → VerifiedNode flow
- Multiple node discovery
- Message filtering

**Python Reference:** `node_discovery.py` `discoverAllNodes()`

#### Transport Tests (1 test)
- Frame encoding for TCP transport

### Integration Tests (11 tests)

Located in `tests/protocol_integration.rs`:

1. **Complete discovery sequence** - VerifyNodeGlobal → VerifiedNode
2. **Event query flows** - IdentifyProducers/Consumers with responses
3. **Addressed messaging** - VerifyNodeAddressed with dest in body
4. **Alias allocation** - CID → RID → InitComplete → VerifiedNode
5. **Alias conflict recovery** - Conflict detection → AMR → re-acquire
6. **Single-frame datagrams** - DatagramOnly with ack
7. **Multi-frame datagrams** - First → Middle → Final sequence
8. **Global event queries** - IdentifyEventsGlobal
9. **Range messages** - ProducerRangeIdentified, ConsumerRangeIdentified
10. **Python log sequences** - Real frames from test execution
11. **End-to-end validation** - Multiple message types in sequence

**Python Reference:** `node_discovery.py`, `identifyProducers.py`, `identifyConsumers.py`, `testStartup.py`, `testAliasConflict.py`, `testDatagram.py`

## Python Reference Implementation

This Rust implementation is validated against the Python OpenLCB implementation. Key reference files:

### Core Protocol
- **canolcbutils.py** - Frame encoding/decoding patterns, bit manipulation
  - `makeframestring(header, body)` - Encoding pattern
  - `bodyArray(frame)` - Decoding pattern
  - Header/data hex formatting rules

### Test Data
- **defaults.py** - Standard test constants
  - `thisNodeID = [1,2,3,4,5,6]`
  - `testNodeID = [2,3,4,5,6,1]`
  - `testEventID = [0x05, 0x02, 0x01, 0x02, 0x02, 0x00, 0x00, 0x00]`
  - `thisNodeAlias = 0xAAA`
  - `testNodeAlias = 0xDDD`

### Protocol Sequences
- **testStartup.py** - Alias allocation sequence (CID/RID/Init)
- **testAliasConflict.py** - Conflict detection and AMR handling
- **testDatagram.py** - Multi-frame datagram patterns
- **verifyNodeAddressed.py** - Addressed message body format
- **node_discovery.py** - Discovery timing and flow

## Running Tests

### All Tests
```bash
cargo test --workspace
```

### Unit Tests Only
```bash
cargo test --lib
```

### Integration Tests Only
```bash
cargo test --test protocol_integration
```

### Specific Test Module
```bash
cargo test --lib protocol::mti
cargo test --lib protocol::frame
cargo test --lib types
```

### Property-Based Tests with More Cases
```bash
PROPTEST_CASES=10000 cargo test proptest
```

### With Coverage (requires tarpaulin)
```bash
cargo tarpaulin --out Html --lib
```

## Test Coverage Goals

- **Protocol modules (frame, mti):** >90% coverage ✓
- **Type conversions:** >85% coverage ✓
- **Discovery logic:** >80% coverage ✓
- **Property tests:** Validate encoding invariants ✓

## Key Test Fixtures

### MTI Values (from Python)
```rust
MTI::VerifyNodeGlobal       => 0x19490
MTI::VerifiedNode           => 0x19170
MTI::IdentifyConsumers      => 0x198F4
MTI::IdentifyProducers      => 0x19914
MTI::ConsumerIdentifiedValid => 0x194C4
MTI::ProducerIdentifiedValid => 0x19544
MTI::CheckID                => 0x17020
MTI::ReserveID              => 0x10700
MTI::AliasMapReset          => 0x10703
MTI::DatagramOnly           => 0x1A000
MTI::DatagramFirst          => 0x1B000
MTI::DatagramMiddle         => 0x1C000
MTI::DatagramFinal          => 0x1D000
```

### Frame Examples (Python-validated)
```
:X19490AAAN;                    # VerifyNodeGlobal from 0xAAA
:X19170DDDNFFEEDDCCBBAA;        # VerifiedNode from 0xDDD
:X19488AAAN0DDD010203040506;   # VerifyNodeAddressed (dest in body)
:X198F4AAAN0102030405060708;   # IdentifyConsumers with EventID
:X1ADDDAAAN01020304;            # DatagramOnly (src=0xAAA, dest=0xDDD)
```

### Encoding Rules (Python patterns)
- Headers: Always 8 uppercase hex digits
- Data: Zero-padded to 2 hex digits per byte
- Format: `:X[8-hex]N[0-16-hex];`
- Empty data: `:X19490AAAN;` (no bytes after N)
- Max data: 8 bytes (16 hex digits)

## Test Organization

```
lcc-rs/
├── src/
│   ├── protocol/
│   │   ├── frame.rs       # 61 tests (parsing, encoding, edge cases)
│   │   ├── mti.rs         # 16 tests (MTI values, headers, datagrams)
│   │   └── mod.rs
│   ├── types.rs           # 24 tests (NodeID, EventID, alias)
│   ├── discovery.rs       # 5 tests (mock transport)
│   └── transport/
│       └── tcp.rs         # 7 tests (TCP-specific, not counted in main suite)
└── tests/
    └── protocol_integration.rs  # 11 integration tests
```

## Cross-Validation with Python

To verify compatibility:

1. **Frame Encoding:**
   ```python
   # Python
   from canolcbutils import makeframestring
   frame = makeframestring(0x19490AAA, [0x01, 0x02])
   # ":X19490AAAN0102;"
   ```
   
   ```rust
   // Rust
   let frame = GridConnectFrame::new(0x19490AAA, vec![0x01, 0x02]).unwrap();
   assert_eq!(frame.to_string(), ":X19490AAAN0102;");
   ```

2. **MTI Headers:**
   ```python
   # Python: 0x19490 << 12 | 0xAAA = 0x19490AAA
   ```
   
   ```rust
   // Rust
   let header = MTI::VerifyNodeGlobal.to_header(0xAAA).unwrap();
   assert_eq!(header, 0x19490AAA);
   ```

3. **Addressed Messages:**
   ```python
   # Python: body = [(dest>>8)&0xFF, dest&0xFF] + payload
   body = [0x0D, 0xDD, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]
   ```
   
   ```rust
   // Rust
   let frame = GridConnectFrame::from_addressed_mti(
       MTI::VerifyNodeAddressed, 0xAAA, 0x0DDD, 
       vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06]
   ).unwrap();
   assert_eq!(frame.data[0..2], [0x0D, 0xDD]);
   ```

## Contributing

When adding new protocol features:

1. Add unit tests for encoding/decoding
2. Add integration tests for protocol sequences
3. Cross-reference with Python implementation
4. Add property tests for invariants
5. Document Python reference file locations
6. Update this README with test coverage

## License

Licensed under either of [MIT License](../LICENSE-MIT) or [Apache License, Version 2.0](../LICENSE-APACHE) at your option.
