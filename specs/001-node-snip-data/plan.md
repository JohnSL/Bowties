# Implementation Plan: Enhanced Node Discovery with SNIP Data

**Branch**: `001-node-snip-data` | **Date**: 2026-02-16 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-node-snip-data/spec.md`

**Note**: This template is filled in by the `/speckit.plan` command. See `.specify/templates/commands/plan.md` for the execution workflow.

## Summary

Enhance node discovery to retrieve and display Simple Node Identification Protocol (SNIP) data from discovered LCC nodes. SNIP provides manufacturer, model, software version, hardware version, user-assigned names, and descriptions via MTI 0x19DE8 (datagram protocol). The system will display nodes with friendly names instead of cryptic Node IDs, provide on-demand status verification, automatically detect newly joined nodes, and handle concurrent SNIP requests efficiently (max 5 concurrent, 5-second timeout). Target: 95% of nodes retrieve SNIP within 3 seconds on networks up to 20 nodes.

## Technical Context

**Language/Version**: Rust 2021 edition (backend), TypeScript 5.6 (frontend)  
**Primary Dependencies**: 
  - Backend: tokio (async runtime), serde (serialization), thiserror (errors), async-trait
  - Frontend: SvelteKit 2.9, Tauri 2 API, Vite 6  
**Storage**: In-memory for this feature (future: SQLite for CDI cache)  
**Testing**: cargo test (unit + integration), tokio-test, proptest (property-based)  
**Target Platform**: Desktop (Windows/macOS/Linux via Tauri 2)  
**Project Type**: Desktop application (Tauri: Rust backend + SvelteKit frontend)  
**Performance Goals**: 
  - SNIP retrieval for 95% of nodes within 3 seconds (networks up to 20 nodes)
  - Manual refresh completes within 5 seconds (20 nodes)
  - New node detection within 10 seconds
  - No UI degradation up to 50 nodes  
**Constraints**: 
  - Max 5 concurrent SNIP requests (prevent network flooding)
  - 5-second timeout per SNIP request
  - TCP-only transport (no serial/CAN support)
  - LCC datagram protocol for multi-frame SNIP responses  
**Scale/Scope**: 
  - Target: 20 nodes typical, 50 nodes maximum
  - SNIP data: 6 string fields (manufacturer, model, SW version, HW version, user name, user description)
  - Event listeners for Verified Node ID broadcasts (auto-discovery)

### Existing lcc-rs Infrastructure (Already Implemented)

This feature **builds upon** substantial existing functionality in lcc-rs:

✅ **Core Types** (`src/types.rs`):
  - `NodeID` - 48-bit identifier with hex formatting
  - `NodeAlias` - 12-bit alias validation
  - `EventID` - 64-bit event identifiers
  - `SNIPData` - Struct placeholder (fields: manufacturer, model, hardware_version, software_version, user_name, user_description)
  - `DiscoveredNode` - Contains `node_id`, `alias`, `snip_data: Option<SNIPData>`

✅ **Node Discovery** (`src/discovery.rs`):
  - `LccConnection::discover_nodes()` - Global Verify Node ID + collect responses
  - Silence detection (25ms timeout)
  - Already returns `Vec<DiscoveredNode>` with `snip_data: None`

✅ **MTI Definitions** (`src/protocol/mti.rs`):
  - `MTI::VerifyNodeGlobal` (0x19490)
  - `MTI::VerifiedNode` (0x19170)
  - `MTI::DatagramOnly`, `DatagramFirst`, `DatagramMiddle`, `DatagramFinal` (0x1A000-0x1D000)
  - `MTI::DatagramReceivedOk` (0x19A28)
  - `MTI::DatagramRejected` (0x19A48)

✅ **Transport Layer** (`src/transport/tcp.rs`):
  - `TcpTransport` - Async TCP connection to LCC hub
  - `LccTransport` trait for send/receive/close
  - GridConnect frame encoding/decoding

✅ **Protocol Framework** (`src/protocol/frame.rs`):
  - `GridConnectFrame` - Parse/encode `:X[header]N[data];` format
  - Header manipulation (MTI + alias encoding)

### What This Feature Adds (New Implementation)

🆕 **SNIP Protocol Support**:
  - Add `MTI::SNIPRequest` (0x19DE8) and `MTI::SNIPResponse` (0x19A08)
  - Implement SNIP request/response handlers
  - Parse SNIP datagram payload (2 sections, 6 null-terminated strings)

🆕 **Datagram Assembly** (`src/protocol/datagram.rs` - NEW):
  - State machine for multi-frame datagram reassembly
  - Handle DatagramFirst → DatagramMiddle* → DatagramFinal sequence
  - Extract payload bytes (skip addressing header)
  - Send DatagramReceivedOk acknowledgments

🆕 **SNIP Module** (`src/snip.rs` - NEW):
  - `query_snip(dest_alias)` - High-level SNIP query API
  - Concurrent request queueing (tokio::Semaphore with capacity 5)
  - Timeout handling (5 seconds per request)
  - String sanitization and validation

🆕 **Status Tracking**:
  - `SNIPStatus` enum (Unknown, InProgress, Complete, Partial, NotSupported, Timeout, Error)
  - `ConnectionStatus` enum (Unknown, Verifying, Connected, NotResponding)
  - Timestamp tracking for `last_verified` and `last_seen`

🆕 **Tauri Commands** (`app/src-tauri/src/commands/snip.rs` - NEW):
  - `discover_nodes()` - Wrapper around existing discovery
  - `query_snip(dest_alias)` - Query single node
  - `query_snip_batch(aliases)` - Query multiple nodes concurrently
  - `refresh_all_nodes()` - Re-discover and re-query SNIP
  - `verify_node_status(dest_alias)` - Check node reachability

🆕 **Frontend Components** (SvelteKit):
  - `NodeList.svelte` - Node list with SNIP display
  - `NodeStatus.svelte` - Status indicators
  - `RefreshButton.svelte` - Manual refresh UI
  - Svelte stores for reactive node state

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Initial Assessment (Phase 0)

**⚠️ CONSTITUTIONAL MISMATCH DETECTED**

The constitution at `.specify/memory/constitution.md` describes a **Python 3.12+ project using UV** ("Bowtie"), but this repository implements a **Rust/Tauri desktop application** ("Bowties"). This appears to be either:
1. An outdated constitution from a previous Python prototype
2. A constitution for a different project repository
3. A missing constitution amendment for the Rust migration

**Principle-by-Principle Assessment:**

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Python 3.12+ Compatibility** | ❌ VIOLATION | Project uses Rust 2021 + TypeScript, not Python |
| **II. UV-Based Development** | N/A | Not applicable to Rust/Cargo ecosystem |
| **III. UX-First Design** | ✅ ALIGNED | Feature emphasizes friendly names, visual indicators, clear status feedback per spec scenarios |
| **IV. TCP-Only Focus** | ✅ ALIGNED | lcc-rs implements TCP transport only, no serial/CAN support |
| **V. Event Management Excellence** | ⚠️ PARTIAL | This feature focuses on node discovery/SNIP, not event management; future features will address events |

**Decision for this feature**: Proceeding with implementation that aligns with Principles III (UX-First) and IV (TCP-Only), acknowledging technology stack divergence from constitution. This feature does NOT violate the *spirit* of the constitution (quality, UX focus, testing rigor) even though it uses different technologies.

---

### Post-Design Re-evaluation (Phase 1 Complete)

**Date**: 2026-02-16  
**Design artifacts reviewed**: research.md, data-model.md, contracts/tauri-commands.json, quickstart.md

#### Alignment with Constitutional Spirit

While the **language** difference remains (Rust vs Python), the feature design **fully embraces constitutional principles**:

**✅ UX-First Design (Principle III)** - EXCELLENT ALIGNMENT
- Friendly name formatting prioritizes user-assigned names over technical IDs
- Progressive disclosure (basic info → tooltip → detail view)
- Clear status indicators with color coding (green/red/gray)
- Automatic disambiguation of duplicate names
- "Last verified" timestamps in human-readable format
- Graceful degradation (SNIP not supported → shows Node ID)
- All UI decisions documented with user scenarios in quickstart.md

**✅ TCP-Only Focus (Principle IV)** - PERFECT ALIGNMENT
- SNIP implementation exclusively uses TCP transport
- No serial/CAN/GridConnect support added
- All network operations via `lcc-rs` TCP module
- Research confirms Python POC also TCP-only

**✅ Quality & Testing Rigor** (Spirit of Principles I & II)
- Comprehensive data model with validation rules and invariants
- Type-safe Rust entities (NodeID, SNIPData) with sanitization
- Property-based testing planned (proptest for datagram assembly)
- Integration tests against Python POC as reference oracle
- Clear error handling and timeout strategies
- Performance targets defined (95% nodes <3s SNIP retrieval)

**⚠️ Event Management (Principle V)** - NOT YET APPLICABLE
- This feature is prerequisite infrastructure for event management
- Node discovery must work before event discovery can function
- Future feature (002-event-discovery) will address this principle
- No violations introduced; foundation properly laid

#### New Technologies Align with Quality Goals

| Technology | Constitutional Equivalent | Justification |
|------------|--------------------------|---------------|
| Rust 2021 + Cargo | Python 3.12 + UV | Modern toolchain with reproducible builds |
| tokio async | Python asyncio | Non-blocking I/O for responsive UI |
| SvelteKit | Flask (mentioned in constitution) | Reactive UI framework for desktop |
| cargo test + proptest | pytest | Comprehensive test coverage with property tests |
| Tauri commands | Python function calls | Type-safe IPC between frontend/backend |

#### Design Decisions Supporting Constitutional Values

1. **In-memory storage** (this feature): Simplicity over premature optimization → aligns with POC/MVP mindset
2. **5 concurrent SNIP requests**: Prevents network flooding → responsible engineering
3. **5-second timeouts**: User-friendly (not too short/long) → UX-first
4. **Sanitized string handling**: Prevents UI crashes from bad data → robustness
5. **Friendly name priority**: User name > Manufacturer > Node ID → end-user focus
6. **TypeScript contracts**: Type safety for Tauri IPC → quality assurance

#### No New Violations Introduced

- ✅ No additional dependencies beyond essential (tokio, serde, thiserror)
- ✅ No complexity violations (no repositories, factories, or overengineering)
- ✅ No scope creep (strictly SNIP discovery, no CDI retrieval yet)
- ✅ No transport violations (TCP-only maintained)

### Final Gate Assessment

**GATE STATUS**: ✅ **PASS** with constitutional mismatch caveat

This feature:
- **Honors the spirit** of the constitution (quality, UX, testing, scope)
- **Does not introduce** new complexity or anti-patterns
- **Maintains focus** on TCP transport and user experience
- **Violates only** the language/toolchain specification (Rust vs Python)

**Recommendation**: 
1. **Proceed with implementation** - design is sound and well-aligned
2. **Separately**: Update `.specify/memory/constitution.md` to reflect Rust/Tauri reality (MAJOR version bump) OR create `constitution-bowties.md` for this repository
3. **Track**: Constitution amendment as technical debt item (does not block this feature)

## Project Structure

### Documentation (this feature)

```text
specs/001-node-snip-data/
├── spec.md              # Feature specification (input)
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (/speckit.plan command)
├── data-model.md        # Phase 1 output (/speckit.plan command)
├── quickstart.md        # Phase 1 output (/speckit.plan command)
├── contracts/           # Phase 1 output (/speckit.plan command)
│   └── snip-api.json    # SNIP data structures and Tauri command contracts
└── tasks.md             # Phase 2 output (/speckit.tasks command - NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
lcc-rs/                  # Reusable LCC protocol library (Rust)
├── src/
│   ├── lib.rs           # ✅ EXISTING - Re-exports core types
│   ├── types.rs         # ✅ EXISTING - NodeID, Alias, SNIPData, DiscoveredNode
│   ├── discovery.rs     # ✅ EXISTING - discover_nodes() already implemented
│   ├── snip.rs          # 🆕 NEW: SNIP request/response handling
│   └── protocol/
│       ├── mod.rs       # ✅ EXISTING
│       ├── frame.rs     # ✅ EXISTING - GridConnect frame encoding/parsing
│       ├── mti.rs       # 📝 UPDATE: Add SNIP MTIs (0x19DE8, 0x19A08)
│       └── datagram.rs  # 🆕 NEW: Multi-frame datagram assembly
├── tests/
│   ├── protocol_integration.rs  # ✅ EXISTING integration tests
│   └── snip_integration.rs      # 🆕 NEW: SNIP protocol tests
└── Cargo.toml           # ✅ EXISTING - All dependencies already present

app/                     # Tauri desktop application
├── src/                 # SvelteKit frontend
│   ├── routes/
│   │   ├── +page.svelte          # Main node list view (UPDATE)
│   │   └── +layout.ts             # Existing layout
│   ├── lib/
│   │   ├── components/
│   │   │   ├── NodeList.svelte    # NEW: Node list component with SNIP display
│   │   │   ├── NodeStatus.svelte  # NEW: Status indicator component
│   │   │   └── RefreshButton.svelte # NEW: Manual refresh control
│   │   ├── stores/
│   │   │   └── nodes.ts           # NEW: Writable store for discovered nodes
│   │   └── api/
│   │       └── tauri.ts           # NEW: Typed Tauri command wrappers
│   └── app.html
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── main.rs                # UPDATE: Add SNIP-related Tauri commands
│   │   ├── lib.rs
│   │   ├── state.rs               # NEW: Application state (connection, nodes)
│   │   └── commands/
│   │       ├── discovery.rs       # Existing discovery commands
│   │       └── snip.rs            # NEW: SNIP command handlers
│   ├── Cargo.toml                 # UPDATE: Add lcc-rs dependency
│   └── tauri.conf.json
├── package.json
└── README.md            # UPDATE: Add SNIP feature documentation
```

**Structure Decision**: Tauri monorepo with shared `lcc-rs` library. The app folder contains both frontend (`src/`) and backend (`src-tauri/`), following standard Tauri project structure. SNIP functionality will be implemented in the `lcc-rs` library for protocol correctness and testability, then exposed via Tauri commands to the SvelteKit UI. This maintains separation between protocol logic (Rust library), application backend (Tauri commands), and presentation (Svelte components).

## Complexity Tracking

**No unjustified complexity violations.** The constitutional mismatch (Python vs Rust) is a documentation issue, not a design complexity concern. This feature follows UX-first principles and TCP-only constraints as outlined in the aligned constitutional principles.
