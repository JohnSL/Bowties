# Current Architecture

*This document describes the current implementation of Bowties, including what's built, what's in progress, and what remains to be implemented. For the aspirational vision, see [docs/design/vision.md](../design/vision.md).*

**Last Updated:** 2026-02-16

## Implementation Status

### ✅ Completed (Phase 1)

**Connection & Discovery MVP:**
- TCP connection to GridConnect hub
- Connection state persistence
- Node discovery using Verify Node ID protocol
- SNIP data retrieval (manufacturer, model, version, user name)
- Compact UX for connection and discovery

**lcc-rs Library:**
- GridConnect frame parsing/encoding
- MTI encoder/decoder
- Basic TCP transport with tokio
- Node discovery protocol
- SNIP datagram protocol
- Async I/O with proper error handling

**Tauri Backend:**
- Connection management commands
- Discovery and SNIP query commands
- Type-safe IPC layer
- State management for connections and nodes

**Frontend (SvelteKit):**
- Connection form with host/port config
- Compact status bar when connected
- Node discovery interface with single discover/refresh button
- Node list table with SNIP data display
- Advanced timeout control (collapsible)
- Responsive layout (800px → 1200px)

### 🚧 In Progress

**Protocol Implementation:**
- Event discovery (Identify Events protocol)
- CDI retrieval and caching
- Configuration memory read/write

**UI Enhancements:**
- Dark mode support (partially implemented in components)
- Error handling refinement
- Loading state improvements

### ⏳ Not Yet Implemented

**Three Main Views:**
- Configuration View (Miller Columns)
- Event Bowties View (canvas with diagrams)
- Event Monitor View (real-time event log)

**Core Features:**
- CDI parsing and display
- Event link visualization
- Drag-and-drop event linking
- Configuration value editing
- Real-time event monitoring

See [docs/project/roadmap.md](../project/roadmap.md) for detailed feature timeline.

## Technology Stack

### Frontend

**Framework:** SvelteKit 2.x
- **Why:** Modern reactive framework with excellent TypeScript support
- **Mode:** SPA (SSR disabled for Tauri compatibility)
- **Reactivity:** Svelte 5 runes (`$state`, `$derived`, `$props`)
- **Component Library:** None (custom components)
- **Styling:** Scoped CSS in `.svelte` files (no CSS framework, no Tailwind)

**State Management:**
- Svelte stores (in `app/src/lib/stores/`)
- Currently local component state in `+page.svelte`
- Prepared node store not yet integrated

**Type Safety:**
- TypeScript strict mode enabled
- Type definitions for all Tauri commands
- Interfaces for all data structures

### Backend

**Framework:** Tauri 2.x
- **Why:** Lightweight native desktop framework with Rust backend
- **IPC:** Tauri commands (type-safe frontend ↔ backend communication)
- **Events:** Tauri event system for backend → frontend notifications

**Protocol Library:** lcc-rs
- **Location:** `lcc-rs/` workspace crate
- **Purpose:** Reusable LCC/OpenLCB protocol implementation
- **Key Features:**
  - GridConnect frame parser/formatter
  - MTI encoding/decoding (30+ message types)
  - TCP transport with tokio async I/O
  - Node discovery and SNIP protocols
  - Datagram assembly/disassembly

**Dependencies:**
- `tokio` (v1.x): Async runtime
- `serde` (v1.x): Serialization
- `thiserror` (v1.x): Error handling
- `async-trait` (v0.1.x): Trait support for async methods

See [docs/technical/lcc-rs-api.md](lcc-rs-api.md) for complete API documentation.

##Current Implementation Details

### Connection & Discovery Setup View

**Purpose:** MVP interface for connecting to LCC network and discovering nodes before implementing the three main views.

**UX Characteristics:**
- **Compact status bar** when connected (vs. full card)
- **Single discover/refresh button** (consolidates two actions)
- **Hidden advanced controls** (timeout collapsible)
- **Responsive table** (expands from 800px to 1200px)
- **Reduced visual density** (tighter spacing, smaller fonts)

**State Flow:**
1. App launches → Check connection status from backend
2. User connects → Connection state persisted in backend
3. User discovers → Scans network + queries SNIP data
4. User refreshes → Re-queries all nodes for updated status

**Component Structure:**
```
app/src/routes/+page.svelte         # Main page (connection + discovery)
app/src/lib/components/
  NodeList.svelte                   # Table of discovered nodes
  NodeStatus.svelte                 # Status indicator component
  RefreshButton.svelte              # (Unused, replaced by consolidated button)
```

**Styling Approach:**
- Scoped `<style>` blocks in each component
- No global CSS
- No CSS preprocessors (SCSS/LESS)
- Utility-like class names defined locally (not Tailwind)
- Purple gradient theme (#667eea to #764ba2)
- Consistent spacing with rem units

### Data Flow

**Connection:**
```
Frontend                    Tauri Backend
-------                     -------------
connect(host, port)    →    connect_lcc(host, port)
                       ←    Result<(), Error>
get_connection_status  →    get_connection_status()
                       ←    { connected, host, port }
```

**Discovery:**
```
Frontend                    Tauri Backend               lcc-rs Library
-------                     -------------               --------------
discoverNodes(timeout) →    discover_nodes(timeout) →   LccConnection::discover_nodes()
                       ←    Vec<DiscoveredNode>     ←   (internal protocol)

querySnipBatch(aliases) →   query_snip_batch()      →   query_snip_concurrent()
                        ←   Vec<SNIPResult>        ←   (datagram protocol)
```

**State Management:**
- Backend holds TCP connection and discovered nodes
- Frontend caches nodes in local/store state
- Tauri events notify frontend of connection changes (planned)

See [docs/technical/tauri-api.md](tauri-api.md) for complete command reference.

### File Organization

```
Bowties/
  app/
    src/
      routes/+page.svelte           # Main UI
      lib/
        api/tauri.ts                # Tauri command wrappers
        components/                 # UI components
        stores/                     # Svelte stores (prepared, not used yet)
    src-tauri/
      src/
        lib.rs                      # Tauri setup
        connection.rs               # Connection management
        discovery.rs                # Node discovery commands
        snip.rs                     # SNIP query commands
  lcc-rs/
    src/
      lib.rs                        # Public API exports
      types.rs                      # NodeID, EventID, etc.
      gridconnect.rs                # Frame parsing
      mti.rs                        # MTI enum
      transport.rs                  # LccTransport trait, TcpTransport
      connection.rs                 # LccConnection
      discovery.rs                  # Node discovery
      snip.rs                       # SNIP protocol
      datagram.rs                   # Datagram assembly
  docs/                             # Documentation (this file)
  specs/                            # SpecKit feature specs
```

### CDI and Configuration Strategy

**Planned (not yet implemented):**

**1. CDI XML (Schema/Structure):**
- Cache key: `{manufacturer}_{model}_{software_version}.cdi.xml`
- Location: Platform-specific app data directory
- On-demand retrieval when user expands node
- Background prefetch for nodes with event links

**2. Configuration Values (Runtime Data):**
- At startup/refresh: Event IDs + user descriptions
- On-demand: Other parameters when user clicks element
- Session-only caching
- Dirty flags for tracking modifications

**Current Reality:**
- CDI retrieval not yet implemented
- Configuration editing not available
- No caching layer

## UX Implementation Notes

###Compact Layout Rationale

**Problem Solved:** Original design had excessive vertical spacing that wasted screen real estate.

**Changes Made:**
1. **Status Bar:** Connection info condensed from full card (2rem padding) to slim bar (0.75rem)
2. **Consolidated Button:** "Discover Nodes" serves both initial and refresh operations
3. **Advanced Toggle:** Timeout control hidden by default (250ms works for most users)
4. **Responsive Width:** Increased max-width from 800px to 1200px for better table display
5. **Reduced Density:** Smaller fonts, tighter spacing throughout

**Impact:**
- More nodes visible without scrolling
- Cleaner visual hierarchy
- Faster access to common actions
- Advanced controls available but not distracting

### Design Deviations from Vision

**Current MVP intentionally omits:**
- Miller Columns navigation
- Event Bowties canvas
- Event Monitor view
- View switching tabs
- CDI configuration editing

**Rationale:** Building foundation (connection, discovery, protocol) before complex UI features. Current interface validates core protocol implementation and establishes patterns for future views.

## Performance Characteristics

**Current Measurements:**

| Operation | Target | Current Status |
|-----------|--------|----------------|
| Node discovery | <1s | ✅ ~250-500ms for 3 nodes |
| SNIP query (single) | <500ms | ✅ ~200-400ms per node |
| SNIP batch (3 nodes) | <1s | ✅ ~600-900ms (concurrent) |
| Connection establishment | <1s | ✅ ~100-300ms |
| UI responsiveness | <50ms | ✅ Svelte reactivity instant |

**Not yet measured:**
- CDI retrieval (not implemented)
- Configuration read/write (not implemented)
- Event monitoring latency (not implemented)
- Large network performance (100+ nodes)

## Technical Debt & Known Issues

**Frontend:**
- Node store prepared but not used (state management in component)
- RefreshButton component unused (replaced by consolidated button)
- Dark mode partially implemented (inconsistent across components)
- No error boundary for component failures

**Backend:**
- No connection pooling or retry logic
- No rate limiting for SNIP queries
- Discovery timeout not configurable from UI (hardcoded to user input)
- No persistent configuration storage

**Protocol:**
- Event discovery not implemented
- CDI retrieval not implemented
- Configuration memory operations not implemented
- Datagram retries not fully tested

**Testing:**
- No end-to-end tests
- Limited integration tests
- No protocol compliance validation suite
- No performance benchmarks

## Next Implementation Steps

**Immediate (Current Sprint):**
1. Remove unused RefreshButton component
2. Integrate node store for state management
3. Add error boundary to main page
4. Implement consistent dark mode

**Short-Term (Next 2-4 weeks):**
1. Implement CDI retrieval protocol
2. Add CDI caching layer
3. Begin Event discovery implementation
4. Plan Miller Columns component architecture

**Medium-Term (Next 1-3 months):**
1. Build Configuration View with Miller Columns
2. Implement CDI parsing and display
3. Add configuration value reading
4. Create event link visualization

See [docs/project/roadmap.md](../project/roadmap.md) for complete timeline.

## Resolved Technical Decisions

✅ **UI Framework:** Svelte 5 (reactive, TypeScript-friendly, lightweight)  
✅ **No CSS Framework:** Custom scoped styles (avoids bloat, full control)  
✅ **State in Component:** Start simple, migrate to stores as needed  
✅ **Svelte 5 Runes:** Modern reactive patterns (`$state`, `$derived`)  
✅ **Compact UX:** Single page before implementing three-view architecture  
✅ **Consolidated Discovery:** One button for discover/refresh (simpler mental model)  
✅ **Responsive Width:** 1200px max (allows table to breathe on wider screens)  

## Open Technical Questions

1. **State Management:** When to migrate from component state to Svelte stores?
2. **Connection Resilience:** How to handle network interruptions and reconnection?
3. **Concurrent Operations:** Allow multiple SNIP queries or enforce serial execution?
4. **CDI Parsing:** Use quick-xml or write custom parser for better error handling?
5. **Large Networks:** Pagination strategy for 100+ nodes? Lazy loading?
6. **Event Monitoring:** Separate TCP connection or share with command channel?

---

*For aspirational architecture, see [docs/design/vision.md](../design/vision.md)*  
*For API reference, see [docs/technical/lcc-rs-api.md](lcc-rs-api.md) and [docs/technical/tauri-api.md](tauri-api.md)*
