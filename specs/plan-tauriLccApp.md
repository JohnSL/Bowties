# Plan: Tauri LCC Configuration Tool

Set up a Tauri app with Svelte frontend in [bowtie/app](bowtie/app), backed by a reusable Rust `lcc-rs` crate for OpenLCB protocol operations. Initial focus: TCP-only transport with node discovery functionality, replicating proven Python POC patterns while establishing architecture for future expansion (event discovery, SNIP, CDI). **Emphasis on comprehensive unit testing for protocol correctness.**

## Steps

1. **Initialize Tauri app structure**
   - Run `npm create tauri-app@latest` targeting [bowtie/app](bowtie/app)
   - Select Svelte + TypeScript, package manager (npm/pnpm)
   - Generates `src-tauri/` (Rust backend), `src/` (Svelte frontend), config files
   
2. **Create reusable LCC crate with test infrastructure**
   - Create `bowtie/lcc-rs/` as workspace member with `cargo new --lib lcc-rs`
   - Add to [src-tauri/Cargo.toml](bowtie/app/src-tauri/Cargo.toml) workspace: `members = [".", "../lcc-rs"]`
   - Add dependencies: `tokio` (async runtime), `serde` (serialization), `thiserror` (error handling)
   - Add dev-dependencies: `tokio-test` (async testing), `proptest` (property-based testing for parsers), `hex` (test fixtures)
   - Set up `tests/` directory for integration tests, use `#[cfg(test)] mod tests` for unit tests

3. **Define LCC crate API (in [lcc-rs/src/lib.rs](bowtie/lcc-rs/src/lib.rs))**
   - Module structure: `transport`, `protocol`, `discovery`, `types`
   - Core types: `NodeID`, `GridConnectFrame`, `MTI` enum, `SNIPData` struct
   - High-level API: `LccConnection::connect(host, port)`, `discover_nodes(timeout)` → `Vec<NodeID>`
   - Mirror Python's [node_discovery.py](bowtie/webapp/app/node_discovery.py) logic: global Verify Node (MTI 0x1949), collect Verified Node responses (MTI 0x1917)

4. **Implement GridConnect frame handling (TDD approach)**
   - In [lcc-rs/src/protocol/frame.rs](bowtie/lcc-rs/src/protocol/frame.rs): Parser for `:X[header]N[data];` format
   - Reference [canolcbutils.py](OpenLCB_Python/canolcbutils.py) `makeframestring()` and `bodyArray()`
   - **Write tests first**: Parse valid frames, reject malformed input, round-trip encoding
   - Test fixtures from Python POC: `:X19170123N0102030405060708;` (Verified Node), `:X19490AAANFFFFFFFF;` (Verify Node Global)
   - Property tests: Any valid GridConnect string should parse and re-encode identically
   - Edge cases: Empty data, max data (8 bytes), invalid hex, missing semicolon
   - Encode/decode 29-bit CAN headers with MTI + alias fields - verify bit manipulation with known examples

5. **Test MTI encoding/decoding**
   - In [lcc-rs/src/protocol/mti.rs](bowtie/lcc-rs/src/protocol/mti.rs): MTI constants and header encoding logic
   - Unit tests for each MTI type: `MTI::VerifyNodeGlobal` → 0x1949 in header, reverse parse
   - Test alias extraction from 29-bit header
   - Validate against Python's bit manipulation in [canolcbutils.py](OpenLCB_Python/canolcbutils.py)

6. **Build TCP transport layer with mocking**
   - In [lcc-rs/src/transport/tcp.rs](bowtie/lcc-rs/src/transport/tcp.rs): async TCP client using `tokio::net::TcpStream`
   - Send/receive GridConnect frames (newline-delimited ASCII)
   - Configurable timeouts mirroring [node_discovery.py](bowtie/webapp/app/node_discovery.py#L50-L80): 10ms socket timeout, 250ms max discovery
   - Echo Python's [tcpolcblink.py](OpenLCB_Python/tcpolcblink.py) connection pattern (port 12021 default)
   - **Unit tests**: Mock transport trait for testing without network I/O
   - **Integration tests** (in `tests/`): Real TCP localhost connection, send/receive known frames

7. **Implement node discovery with comprehensive tests**
   - In [lcc-rs/src/discovery.rs](bowtie/lcc-rs/src/discovery.rs): `discover_all_nodes()` function
   - Send global Verify Node ID frame (MTI 0x1949), collect responses for timeout period
   - Parse Verified Node responses (MTI 0x1917) to extract 48-bit `NodeID`s
   - Return `Vec<DiscoveredNode>` with ID + alias mapping
   - **Unit tests**: Mock transport returns canned responses, verify NodeID extraction
   - **Test scenarios**: No nodes, single node, multiple nodes, interleaved unrelated messages, partial/corrupted responses
   - Reference behavior: Compare against [node_discovery.py](bowtie/webapp/app/node_discovery.py) `discoverAllNodes()` logic

8. **Create Tauri commands**
   - In [src-tauri/src/main.rs](bowtie/app/src-tauri/src/main.rs): Define `#[tauri::command]` functions
   - `connect_lcc(host: String, port: u16)` → stores connection in Tauri state
   - `discover_nodes()` → calls `lcc_rs::discovery::discover_all_nodes()`, returns JSON
   - Handle async with `async fn` + `tokio::spawn` for non-blocking UI
   - **Tests**: Use Tauri's test utilities to verify command serialization/error handling

9. **Build Svelte UI**
   - In [src/App.svelte](bowtie/app/src/App.svelte): Connection form (host/port inputs) + node list display
   - Import `invoke` from `@tauri-apps/api/tauri`
   - Display discovered nodes in table: NodeID (hex), alias
   - Reference [templates/index.html](bowtie/webapp/app/templates/index.html#L100-L300) for UI patterns

10. **Configure development environment**
    - In [src-tauri/tauri.conf.json](bowtie/app/src-tauri/tauri.conf.json): Set app name, window size (1200x800), security CSP
    - Add `.gitignore` for `node_modules/`, `dist/`, `target/`
    - Set up CI test runner in documentation: `cargo test --workspace`, `cargo test --doc`
    - Document in [app/README.md](bowtie/app/README.md): prerequisites (Rust, Node.js), development commands, **testing requirements**

## Verification

1. **Unit test coverage**: `cargo test --workspace` passes with >80% coverage for `lcc-rs` protocol modules
2. **Integration test**: Compare output against Python POC
   - Run Python: `cd bowtie/webapp && python run.py`
   - Run Tauri: `cd bowtie/app && npm run tauri dev`
   - Both connect to same LCC network (localhost:12021 or real hardware)
   - NodeIDs and aliases match exactly
3. **Wire protocol validation**: Optional Wireshark capture to verify GridConnect frames match Python byte-for-byte
4. **Property tests pass**: `cargo test proptest` validates parser invariants

## Decisions

- **Svelte** for frontend: Minimal boilerplate, fast reactivity, good Tauri integration
- **TCP-only initially**: Simplifies v1, matches Python default, covers majority use case
- **High-level API first**: Focus on usable operations (`discover_nodes()`) before low-level frame manipulation
- **Async Rust** with tokio: Non-blocking network I/O, better UX than Python's synchronous approach
- **Workspace structure**: Keeps `lcc-rs` separate from `src-tauri` for reusability in future projects
- **TDD for protocol code**: Write tests first for parsers/encoders, use Python implementation as test oracle
- **Property-based testing**: Use `proptest` to validate parser invariants (parse · encode = identity)
- **Mock transport trait**: Enable unit testing of discovery logic without network dependencies
