# Current Architecture

*This document describes the current implementation of Bowties, including what's built, what's in progress, and what remains to be implemented. For the aspirational vision, see [docs/design/vision.md](../design/vision.md).*

**Last Updated:** 2026-03-20

## Implementation Status

### ✅ Completed (Phase 1)

**Connection & Discovery MVP:**
- TCP connection to GridConnect hubs (JMRI, standalone bridges)
- Direct USB-to-CAN adapter support (GridConnect Serial and SLCAN Serial protocols)
- Connection state persistence
- Node discovery using Verify Node ID protocol
- SNIP data retrieval (manufacturer, model, version, user name)
- Compact UX for connection and discovery

**lcc-rs Library:**
- GridConnect frame parsing/encoding
- MTI encoder/decoder
- TCP transport with tokio async I/O
- Serial/USB transport (GridConnect Serial and SLCAN protocols)
- Node discovery protocol
- SNIP datagram protocol
- Memory Configuration Protocol (CDI retrieval)
- Datagram assembly/disassembly
- Async I/O with proper error handling

**Tauri Backend:**
- Connection management commands
- Discovery and SNIP query commands
- CDI retrieval and caching commands
- Type-safe IPC layer
- State management for connections and nodes
- Platform-specific cache directory management

**Frontend (SvelteKit):**
- Connection form with host/port config
- Compact status bar when connected
- Node discovery interface with single discover/refresh button
- Node list table with SNIP data display
- CDI XML Viewer modal with syntax highlighting
- Context menu for node actions (View CDI, Force Re-download)
- XML formatting utility with proper indentation
- Advanced timeout control (collapsible)
- Responsive layout (800px → 1200px)

### ✅ Completed (Phase 2)

**CDI XML Viewer (Debugging Tool):**
- Memory Configuration Protocol implementation
- CDI retrieval from nodes via datagram protocol
- Platform-specific disk caching (app data directory)
- XML formatting with proper indentation
- Syntax highlighting using Prism.js
- Modal viewer component with copy-to-clipboard
- Context menu integration (right-click on nodes)
- Force re-download capability
- Error handling for missing/invalid CDI
- Large file warning (>500KB)

### ✅ Completed (Phase 3)

**Miller Columns Configuration Navigator (Feature 003) — Superseded:**
- This phase implemented a Miller Columns navigation approach for CDI browsing, which was subsequently replaced by a simpler flat configuration view.
- The lcc-rs CDI module (parser, type system, path navigation) developed here remains in use by the current implementation.

**lcc-rs CDI Module (retained):**
- Complete CDI type system (Cdi, Segment, DataElement, Group, etc.)
- Recursive XML parser with error recovery
- Group replication expansion logic
- Hierarchy navigation helpers (navigate_to_path, calculate_max_depth)
- Index-based path resolution (eliminates name ambiguity)
- Comprehensive test coverage (unit, integration, property-based)

### ✅ Completed (Phase 4)

**Persistent Message Monitoring & Event-Driven Architecture:**
- Message dispatcher with background listener task (tokio)
- Broadcast channels for multi-subscriber message distribution
- MTI-based message filtering and routing
- Event router for backend-to-frontend notifications
- Tauri event emissions (`lcc-node-discovered`, `lcc-message-received`)
- Arc<Mutex<>> shared connection pattern for concurrent access
- Channel-based discovery (replaces polling pattern)
- Automatic node discovery event notifications
- Foundation for real-time event monitoring

**lcc-rs Message Dispatcher:**
- `MessageDispatcher` struct with background receive loop
- `ReceivedMessage` with frame and timestamp
- `subscribe_all()` for monitoring all messages
- `subscribe_mti(mti)` for filtered message subscriptions
- Graceful shutdown handling
- Connection resilience support

**Tauri Event System:**
- `EventRouter` for message-to-event translation
- `NodeDiscoveredEvent` payload type
- `MessageReceivedEvent` payload type
- Automatic event emission on node discovery
- Background routing task lifecycle management

**Protocol Monitor (Traffic Monitor View):**
- Real-time event log displaying all LCC messages flowing on the network
- PCER (`0x095B4`) event capture and display
- MTI-based filtering and live traffic inspection

### ✅ Completed (Phase 5)

**Event Discovery & Bowties Tab (Feature 006):**
- `IdentifyEventsAddressed` exchange per discovered node (125 ms between sends, 500 ms collection window)
- `ProducerIdentified` / `ConsumerIdentified` reply collection from all nodes
- `EventRole` enum (`Producer` / `Consumer` / `Ambiguous`) in `lcc-rs/src/cdi/role.rs`
- Two-tier heuristic classifier: Tier 1 parent group name keywords, Tier 2 CDI `<description>` phrase patterns
- Ancestor group name propagation during CDI event-slot traversal (`hierarchy.rs`)
- `BowtieCatalog` / `BowtieCard` / `EventSlotEntry` types in `app/src-tauri/src/commands/bowties.rs`
- `build_bowtie_catalog` function: cross-node protocol-definitive classification; same-node fallback heuristic; one card per unique event ID with ≥1 confirmed producer AND ≥1 consumer
- `query_event_roles` function: sends `IdentifyEventsAddressed`; collects role replies
- `get_bowties` Tauri command returning `Option<BowtieCatalog>` from `AppState`
- `cdi-read-complete` Tauri event emitted with full `BowtieCatalog` when CDI reads finish
- `bowtieCatalogStore` + `usedInMap` derived store (Svelte; O(1) event → bowtie lookup)
- `BowtieCard.svelte` three-column layout (producers | arrow | consumers)
- `ElementEntry.svelte` (node name + element label per slot)
- `ConnectorArrow.svelte` (rightward arrow + event ID label)
- `EmptyState.svelte` (illustration + guidance text + disabled CTA)
- Bowties tab as in-page panel inside `+page.svelte` (no SvelteKit route navigation)
- "Used in" cross-reference on `EventSlotRow.svelte` linking to matching bowtie card
- Highlight-on-navigate support via `?highlight=` query parameter
- Tab disabled until `cdi-read-complete` fires; enables automatically after first build
- Full Vitest test coverage for all new Svelte components; cargo unit tests for `build_bowtie_catalog` and `classify_event_slot`

### ✅ Completed (Phase 6)

**Configuration Viewing & Editing:**
- Configuration memory read/write (value retrieval and editing)
- Configuration value display and editing UI

### ✅ Completed (Phase 7)

**Editable Bowties — Feature 009 (Phases 3–8 of spec):**

*Persistence (US7):*
- YAML layout file I/O: atomic write (temp → flush → rename), schema validation, corrupted-file degraded mode
- `load_layout` / `save_layout` Tauri commands with `layout-loaded` / `layout-save-error` events
- `get_recent_layout` / `set_recent_layout` Tauri commands using `app_data_dir/recent-layout.json`
- `build_bowtie_catalog` extended to accept optional `LayoutFile` parameter; merges role classifications, names, tags
- Layout store (`layout.svelte.ts`) with native OS file dialogs, dirty state, open/save/save-as methods
- Automatic recent-layout reopen prompt on startup

*Bidirectional Sync & Unsaved Tracking (US2):*
- `BowtieMetadataStore` (`bowtieMetadata.svelte.ts`) with `$state` runes for names, tags, role classifications; mutations: `createBowtie`, `deleteBowtie`, `renameBowtie`, `addTag`, `removeTag`, `classifyRole`
- `pendingEditsStore` extended with `source: 'config' | 'bowtie'` discriminator
- `EditableBowtiePreviewStore` — `$derived` merging live catalog + pending edits + metadata
- `enrichEntryLabel()` — computes `element_label` from the live node tree (frontend-only; no longer sent by Rust)
- `isEntryStillActive()` — filters catalog entries whose event ID has been reassigned
- Unified save flow: node writes first, then YAML save with retry/Save As prompt
- Unified discard flow: clears both stores atomically
- Unsaved-change indicators (dirty dot/badge) on `BowtieCard` and toolbar

*Visual Connection Creation — Core MVP (US1):*
- `ElementPicker.svelte` — browsable node → segment → group → event slot tree, filtered by role, search, free-slot gating
- `NewConnectionDialog.svelte` — dual-panel (producer + consumer) with optional name, event ID resolution rules (FR-002)
- `BowtieCatalogPanel.svelte` — **+ New Connection** button, dialog orchestration
- Multi-node sequential write with rollback on failure; `WriteOperation` / `WriteStep` tracking
- `BowtieCard.svelte` write feedback: spinner, success confirmation, error with retry (FR-030)

*Add/Remove Elements (US5):*
- **+ Add producer** / **+ Add consumer** buttons on `BowtieCard` open role-filtered `AddElementDialog`
- `AddElementDialog.svelte` — modal wrapping `ElementPicker` for adding to an existing bowtie
- Remove button per element with `generateFreshEventIdForNode()` to restore slot uniquely
- Remove confirmation dialog; delete confirmation when removing the last element(s) (FR-011)
- Incomplete-state visual indicator: amber border when one side is empty (FR-010)
- `generateFreshEventIdForNode()` in `app/src/lib/utils/eventIds.ts` — generates a unique node-scoped event ID avoiding all existing slot values

*Role Classification (US8):*
- `RoleClassifyPrompt.svelte` — inline prompt (amber card with "?" icon, Producer/Consumer buttons)
- `ElementPicker` intercepts ambiguous/null-role slots; auto-classifies when picker has a definite role filter; shows `RoleClassifyPrompt` overlay otherwise
- Reclassify role inline on `BowtieCard` — click ambiguous entry → `RoleClassifyPrompt` replaces it; updates `BowtieMetadataStore`
- Ambiguous entries shown in dedicated section between producers and consumers until classified

*Config-First Entry (US3):*
- **→ New Connection** button on `TreeLeafRow` for unlinked event slots
- `connectionRequestStore` (`connectionRequest.svelte.ts`) — singleton store carrying pending selection + role to the Bowties tab
- `+page.svelte` watches `connectionRequestStore` and switches to Bowties tab
- `BowtieCatalogPanel` handles pre-fill: opens `NewConnectionDialog` with producer or consumer side pre-filled; ambiguous role triggers `RoleClassifyPrompt` first

*Cross-Cutting Refactors:*
- `element_label` removed from Rust `EventSlotEntry` / `state.rs`; now computed lazily by the frontend via `enrichEntryLabel()` so labels reflect live pending name edits and `getInstanceDisplayName()` (e.g. "GPIO13 (1)")
- `findLeafByPath()` added to `nodeTree.ts` for O(n) path-based leaf lookup
- `getInstanceDisplayName()` fixed to use `effectiveValue()` so pending string edits appear in group headers
- `pillSelections` store (`pillSelection.ts`) — persists selected pill (instance) index across view switches; `TreeGroupAccordion` reads/writes it
- `isWellKnownEvent()` in `formatters.ts` — gates "No producers / No consumers" hints on `BowtieCard` for LCC well-known event IDs
- `SegmentView.svelte` — fixed if/else ordering so a segment renders immediately when available even while a parallel load is in progress

### 🚧 In Progress

**UI Enhancements:**
- Dark mode / theme consistency across Bowtie components (inherits app theme; dark-mode CSS variables need alignment with global custom properties)

### ⏳ Not Yet Implemented

**Editable Bowties — Remaining Stories (Feature 009):**
- US6 (Phase 9): Inline bowtie rename, "Used in" Config tab cross-reference, filter bar — T042–T044
- US4 (Phase 10): Intent-first / planning-state bowtie creation (empty named bowties) — T045–T048
- Polish (Phase 11): Tag management UI, prompt-to-save guard, performance validation — T049–T054

**Other Core Features:**
- Event link visualization (canvas drag-and-drop)

See [docs/project/roadmap.md](../project/roadmap.md) for detailed feature timeline.

## Technology Stack

### Frontend

**Framework:** SvelteKit 2.x
- **Why:** Modern reactive framework with excellent TypeScript support
- **Mode:** SPA (SSR disabled for Tauri compatibility)
- **Reactivity:** Svelte 5 runes (`$state`, `$derived`, `$props`)
- **Component Library:** None (custom components)
- **Styling:** Scoped CSS in `.svelte` files (no CSS framework, no Tailwind)
- **Syntax Highlighting:** Prism.js for XML display

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
  - Memory Configuration Protocol (CDI, configuration memory)
  - Datagram assembly/disassembly
  - Message dispatcher with background listening
  - Broadcast channels for event distribution
  - Persistent connection management

**Dependencies:**
- `tokio` (v1.x): Async runtime
- `serde` (v1.x): Serialization
- `thiserror` (v1.x): Error handling
- `async-trait` (v0.1.x): Trait support for async methods
- `chrono` (v0.4.x): Timestamp handling for cache metadata
- `prismjs` (v1.x): XML syntax highlighting (frontend)
- `roxmltree` (v0.20): CDI XML parsing
- `lazy_static` (v1.4): CDI parsing cache
- `uuid` (v1.10): Unique identifier generation

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
5. User views CDI → Triggers cache-first retrieval and displays in modal

**Component Structure:**
```
app/src/routes/+page.svelte         # Main page (connection + discovery + Miller Columns + Bowties tab)
app/src/lib/components/
  NodeList.svelte                   # Table of discovered nodes with context menu
  CdiXmlViewer.svelte               # Modal XML viewer with syntax highlighting
  NodeStatus.svelte                 # Status indicator component
  RefreshButton.svelte              # (Unused, replaced by consolidated button)
  MillerColumns/
    MillerColumnsNav.svelte         # Main container with error boundary
    NodesColumn.svelte              # Left column - discovered nodes
    NavigationColumn.svelte         # Dynamic columns (segments/groups/elements)
    DetailsPanel.svelte             # Right panel - element metadata
    Breadcrumb.svelte               # Navigation breadcrumb
    README.md                       # Component documentation
  ElementCardDeck/
    EventSlotRow.svelte             # Event ID field row; shows "Used in" cross-ref when in a bowtie
  Bowtie/
    BowtieCard.svelte               # Three-column layout (producers | arrow | consumers)
    ElementEntry.svelte             # One EventSlotEntry row (node_name + element_label computed by frontend)
    ConnectorArrow.svelte           # Rightward arrow with event ID label
    EmptyState.svelte               # Shown when no bowties found
app/src/lib/stores/
  millerColumns.ts                  # Miller Columns state management
  bowties.svelte.ts                 # BowtieCatalogStore + usedInMap derived store
  pillSelection.ts                  # Persists pill (instance) selector positions across view switches
app/src/lib/utils/
  xmlFormatter.ts                   # XML indentation utility
app/src/lib/api/
  cdi.ts                            # CDI-specific Tauri commands
  tauri.ts                          # General Tauri command wrappers
app/src/lib/types/
  cdi.ts                            # CDI type definitions
```

**Styling Approach:**
- Scoped `<style>` blocks in each component
- No global CSS
- No CSS preprocessors (SCSS/LESS)
- Utility-like class names defined locally (not Tailwind)
- Purple gradient theme (#667eea to #764ba2)
- Consistent spacing with rem units

### Data Flow

**Connection (Event-Driven Architecture):**

Bowties supports three connection types. All use the same `LccConnection` abstraction in lcc-rs; only the transport layer differs.

| Type | Protocol | Typical devices | Default baud |
|------|----------|-----------------|-------------|
| **TCP** | GridConnect over TCP | JMRI, standalone bridges | N/A (port 12021) |
| **GridConnect Serial** | GridConnect over USB/Serial | SPROG CANISB, SPROG USB-LCC, RR-Cirkits Buffer LCC, CAN2USBINO, CANRS | 57600 |
| **SLCAN Serial** | SLCAN over USB/Serial | Canable, Lawicel CANUSB, slcand-compatible adapters | 115200 |

```
Frontend                    Tauri Backend               lcc-rs Library
-------                     -------------               --------------
connect(config)        →    connect_lcc(config)    →   LccConnection::connect_with_dispatcher()
                                                        - TCP: TcpTransport (host:port)
                                                        - GridConnect Serial: SerialTransport
                                                        - SLCAN Serial: SlcanTransport
                       ←    Result<(), Error>       ←   - Creates MessageDispatcher
                                                        - Spawns background listener
                                                        - Starts EventRouter
                       ←──  lcc-node-discovered event (auto-emitted on network activity)
                       ←──  lcc-message-received event (all messages)

get_connection_status  →    get_connection_status()
                       ←    { connected, host, port }
```

```mermaid
sequenceDiagram
    participant F as Frontend
    participant T as Tauri Backend

    F->>T: connect(config)
    note over T: TCP → TcpTransport<br/>GridConnect Serial → SerialTransport<br/>SLCAN Serial → SlcanTransport
    T-->>F: Result<(), Error>
    
    F->>T: get_connection_status()
    T-->>F: { connected, host, port }
```

**Discovery (Channel-Based):**

```
Frontend                    Tauri Backend               lcc-rs Library
-------                     -------------               --------------
discoverNodes(timeout) →    discover_nodes(timeout) →   LccConnection::discover_nodes()
                       ←    Vec<DiscoveredNode>     ←   - Subscribe to VerifiedNode MTI
                                                        - Send VerifyNodeGlobal
                                                        - Collect from channel (not polling)
                       ←──  lcc-node-discovered event (emitted per node)

querySnipBatch(aliases) →   query_snip_batch()      →   query_snip_concurrent()
                        ←   Vec<SNIPResult>        ←   (datagram protocol)

[Background EventRouter continuously emits events for all network activity]
```

```mermaid
sequenceDiagram
    participant F as Frontend
    participant T as Tauri Backend
    participant L as lcc-rs Library

    F->>T: discoverNodes(timeout)
    T->>L: discover_nodes(timeout)
    L-->>T: Vec<DiscoveredNode>
    T-->>F: Vec<DiscoveredNode>
    
    F->>T: querySnipBatch(aliases)
    T->>L: query_snip_concurrent()
    L-->>T: Vec<SNIPResult>
    T-->>F: Vec<SNIPResult>
```

**CDI Retrieval & Viewing:**

```
Frontend                    Tauri Backend               Cache Strategy
-------                     -------------               --------------
getCdiXml(nodeId)      →    get_cdi_xml(nodeId)    →   Check memory cache
                       ←    { xmlContent, ... }    ←   → Check file cache
                                                        → Network if not cached

downloadCdi(nodeId)    →    download_cdi(nodeId)   →   Retrieve from network
                       ←    { xmlContent, ... }    ←   → Save to both caches

viewCdiXml()           →    (format XML locally)
  ├─ formatXml()            (indent with 2 spaces)
  ├─ Prism.highlight()      (syntax coloring)
  └─ display in modal       (CdiXmlViewer.svelte)
```

```mermaid
sequenceDiagram
    participant F as Frontend
    participant T as Tauri Backend
    participant M as Memory Cache
    participant D as Disk Cache

    Note over F,D: get_cdi_xml (cache-first)
    F->>T: getCdiXml(nodeId)
    T->>M: Check node.cdi
    alt In memory cache
        M-->>T: Return cached CDI
        T-->>F: { xmlContent, ... }
    else Not in memory
        T->>D: Check file cache
        alt In file cache
            D-->>T: Return file contents
            T->>M: Store in memory
            T-->>F: { xmlContent, ... }
        else Not cached
            T-->>F: Error: CdiNotRetrieved
        end
    end

    Note over F,D: download_cdi (network retrieval)
    F->>T: downloadCdi(nodeId)
    T->>T: Retrieve via lcc-rs
    T->>M: Update node.cdi
    T->>D: Write to file cache
    T-->>F: { xmlContent, sizeBytes, retrievedAt }
```

**State Management:**
- Backend holds LCC connection (TCP or serial/USB) and discovered nodes
- Frontend caches nodes in local/store state
- CDI data cached on disk in platform-specific app data directory
- Cache key format: `cdi_{manufacturer}_{model}_{software_version}.xml`
- Tauri events notify frontend: `lcc-node-discovered`, `cdi-read-complete`

**Bowties Event Discovery (Feature 006):**

```
Frontend                    Tauri Backend               lcc-rs Library
-------                     -------------               --------------
[CDI reads complete]    →   read_all_config_values()   →   (last node read)
                            ↓ triggers
                            query_event_roles()        →   IdentifyEventsAddressed × n nodes
                       ←──                             ←   ProducerIdentified / ConsumerIdentified
                            ↓
                            build_bowtie_catalog()
                            ↓
                            AppState.bowties_catalog = Some(catalog)
                       ←──  cdi-read-complete event { catalog, node_count }
                            ↓ bowtieCatalogStore.set(catalog)
                            ↓ Bowties tab enables
getBowties()           →    get_bowties()
                       ←    Option<BowtieCatalog>
```

See [docs/technical/tauri-api.md](tauri-api.md) for complete command reference.

**Miller Columns Navigation:**

```
Frontend                    Tauri Backend               lcc-rs CDI Module
-------                     -------------               -----------------
getDiscoveredNodes()   →    get_discovered_nodes()  →   (query node cache)
                       ←    Vec<DiscoveredNode>     ←   

getCdiStructure(nodeId) →   get_cdi_structure()     →   parse_cdi(xml)
                        ←   { segments, maxDepth }  ←   calculate_max_depth()

getColumnItems(path)    →   get_column_items()      →   navigate_to_path(path)
                        ←   Vec<ColumnItem>         ←   expand_replications()

getElementDetails(path) →   get_element_details()   →   navigate_to_path(path)
                        ←   ElementDetails          ←   (extract metadata)
```

**pathId Navigation System:**

The Miller Columns feature uses an index-based pathId system for stable navigation:

**Format:** `seg:N` for segments, `elem:N` for elements, `elem:N#I` for replicated instances

**Why Index-Based:**
- Eliminates ambiguity with CDI element names containing special characters (e.g., "Variable #1")
- Provides stable references independent of name changes
- Enables efficient path resolution via array indexing

**UI vs Navigation IDs:**
- **Display ID (UUID):** Unique identifier for React/Svelte keys (prevents collision in UI)
- **Navigation pathId:** Index-based identifier for backend navigation (seg:0, elem:2#5)
- **Separation of concerns:** UUIDs for UI rendering, pathIds for data traversal

**Example Path:**
```
User navigates: Tower-LCC Node → Conditionals → Logic #12 → Variable #1 → Trigger

Backend path:   ["seg:0", "elem:0#12", "elem:2", "elem:0"]
                 └─────┘  └─────────┘  └─────┘  └─────┘
                 segment  group inst.  group    element
                 index 0  elem 0, #12  elem 2   elem 0

Display IDs:    [UUID-1, UUID-2, UUID-3, UUID-4]  (UI keys only)
```

**Path Resolution:**
1. Parse pathId (e.g., "elem:2#5")
2. Extract index (2) and optional instance (5)
3. Navigate to `elements[2]`
4. If replicated, expand to instance #5

**Benefits:**
- Handles names like "Variable #1", "Group#2", "Item #3" without parsing ambiguity
- Consistent with array-based data structures in Rust
- Fast O(1) lookup via direct indexing

### File Organization

```
Bowties/
  app/
    src/
      routes/+page.svelte           # Main UI
      lib/
        api/
          tauri.ts                    # Tauri command wrappers
          cdi.ts                      # CDI-specific commands
        components/
          NodeList.svelte             # Node table with context menu
          CdiXmlViewer.svelte         # XML viewer modal
        utils/
          xmlFormatter.ts             # XML formatting utility
        types/
          cdi.ts                      # CDI type definitions
        stores/                       # Svelte stores (prepared, not used yet)
    src-tauri/
      src/
        lib.rs                        # Tauri setup
        state.rs                      # AppState (nodes, bowties_catalog, event_roles)
        commands/
          connection.rs               # Connection management
          discovery.rs                # Node discovery commands
          snip.rs                     # SNIP query commands
          cdi.rs                      # CDI retrieval, caching, triggers bowtie build
          bowties.rs                  # get_bowties, build_bowtie_catalog, query_event_roles
          mod.rs                      # Command module exports
  lcc-rs/
    src/
      lib.rs                        # Public API exports
      types.rs                      # NodeID, EventID, etc.
      protocol/
        frame.rs                    # GridConnect frame parsing
        mti.rs                      # MTI enum
        datagram.rs                 # Datagram assembly
        memory_config.rs            # Memory Configuration Protocol
        mod.rs                      # Protocol module exports
      cdi/
        mod.rs                      # CDI type definitions + EventRole re-export
        parser.rs                   # XML parsing (roxmltree-based)
        hierarchy.rs                # Navigation helpers (expand, navigate_to_path, ancestor group names)
        role.rs                     # EventRole enum + classify_event_slot() two-tier heuristic
      transport.rs                  # LccTransport trait, TcpTransport
      connection.rs                 # LccConnection
      discovery.rs                  # Node discovery
      snip.rs                       # SNIP protocol
  docs/                             # Documentation (this file)
  specs/                            # SpecKit feature specs
    001-cdi-xml-viewer/             # CDI XML Viewer feature spec
    003-miller-columns/             # Miller Columns navigator spec
```

### CDI and Configuration Strategy

**1. CDI XML Retrieval (Implemented):**
- **Cache strategy:** Memory cache → File cache → Network retrieval
- **Cache key format:** `cdi_{manufacturer}_{model}_{software_version}.xml`
- **Location:** Platform-specific app data directory
  - Windows: `%APPDATA%\com.bowtiesapp.bowties\cdi\`
  - macOS: `~/Library/Application Support/com.bowtiesapp.bowties/cdi/`
  - Linux: `~/.local/share/com.bowtiesapp.bowties/cdi/`
- **Retrieval:** Memory Configuration Protocol (address space 0xFF)
- **Automatic caching:** First retrieval saves to both memory and disk
- **Force re-download:** Context menu option bypasses cache
- **Metadata:** Retrieval timestamp, file size

**2. CDI XML Viewer (Implemented):**
- Modal viewer with syntax-highlighted XML display
- Prism.js for XML syntax coloring
- Custom XML formatter with proper indentation (2 spaces)
- Copy-to-clipboard functionality
- Large file warning (>500KB)
- Error handling for missing/invalid CDI
- Context menu integration (right-click on nodes)

**3. Miller Columns CDI Navigator (Implemented):**
- Dynamic column-based UI for hierarchical navigation
- CDI XML parsing to structured data model (Cdi, Segment, Group, DataElement)
- pathId-based navigation system (index-based, eliminates name ambiguity)
- UUID-based unique IDs for UI elements (React/Svelte keys)
- Replicated group expansion with instance numbering
- Element metadata display (name, description, type, constraints, default value)
- Breadcrumb navigation with instance indicators
- Keyboard navigation (arrow keys, Enter/Space)
- Lazy parsing cache (parsed CDI structs cached in lazy_static HashMap)
- Graceful error handling (parsing errors, missing data, malformed XML)
- WCAG 2.1 AA accessibility compliance

**4. Configuration Values (Partially Implemented):**
- CDI structure navigation complete
- Element metadata extraction complete
- Value retrieval pending (Memory Configuration Protocol read)
- Value editing UI designed but not connected to backend

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
- Miller Columns navigation (CDI parsing not yet implemented)
- Event Bowties canvas
- View switching tabs

**Debugging Tools Implemented:**
- CDI XML Viewer (displays raw CDI for verification/debugging)

**Rationale:** Building foundation (connection, discovery, protocol) before complex UI features. Current interface validates core protocol implementation and establishes patterns for future views. CDI XML viewer provides debugging capability while CDI parsing and Miller Columns navigation are being developed.

## Performance Characteristics

**Current Measurements:**

| Operation | Target | Current Status |
|-----------|--------|----------------|
| Node discovery | <1s | ✅ ~250-500ms for 3 nodes |
| SNIP query (single) | <500ms | ✅ ~200-400ms per node |
| SNIP batch (3 nodes) | <1s | ✅ ~600-900ms (concurrent) |
| Connection establishment | <1s | ✅ ~100-300ms |
| CDI retrieval (cached) | <500ms | ✅ ~50-100ms (disk read) |
| CDI retrieval (network) | <3s | ✅ ~1-2s for typical CDI (~20KB) |
| XML formatting | <200ms | ✅ ~10-50ms for typical CDI |
| XML syntax highlighting | <500ms | ✅ ~50-200ms (Prism.js) |
| UI responsiveness | <50ms | ✅ Svelte reactivity instant |
| CDI parsing (first time) | <1s | ✅ ~100-500ms for typical CDI |
| CDI parsing (cached) | <50ms | ✅ ~10-30ms (lazy_static cache) |
| Column navigation | <100ms | ✅ ~20-80ms (path resolution + expansion) |
| Replicated group expansion | <200ms | ✅ ~50-150ms for 32 instances |
| Bowtie catalog build (post CDI read) | <5s | ✅ Within SC-001 target on typical networks |
| Empty-state render | <1s | ✅ Within SC-004 target |

**Not yet measured:**
- Configuration value read from node memory (not implemented)
- Configuration value write to node memory (not implemented)
- Event monitoring latency (not implemented)
- Large network performance (100+ nodes)
- Very large CDI files (>1MB) - parsing performance
- Deep hierarchy navigation (10+ levels)

## Technical Debt & Known Issues

**Frontend:**
- Node store prepared but not used (state management in component)
- RefreshButton component unused (replaced by consolidated button)
- Dark mode partially implemented (inconsistent across components)
- No error boundary for non-Miller Columns components
- CDI viewer modal could benefit from virtual scrolling for large files
- Main page styling needs simplification (gradient background removed in Miller Columns)

**Backend:**
- No connection pooling or retry logic
- No rate limiting for SNIP/CDI queries
- Discovery timeout not configurable from UI (hardcoded to user input)
- No persistent configuration storage (except CDI cache)
- CDI cache has no expiration or size limits
- No cleanup of old/stale cache files
- CDI parsing cache (lazy_static HashMap) has no eviction policy

**Protocol:**
- Configuration memory read/write operations not implemented (values from nodes)
- Datagram retries not fully tested
- Memory Configuration read assumes data fits in expected size
- No support for configuration write operations
- `config_value_cache` not yet added to `AppState`: bowtie element identification currently uses CDI keyword heuristic rather than matching by actual configured event ID bytes (identified in session-handoff-2026-02-22.md as the next precision improvement)

**Testing:**
- No end-to-end tests for Miller Columns navigation
- Limited integration tests for CDI commands
- No protocol compliance validation suite
- No performance benchmarks for CDI parsing with very large files (>500KB)
- CDI retrieval tested manually only

## Next Implementation Steps

**Immediate (Current Sprint):**
1. Add `config_value_cache` to `AppState` and populate it in `read_all_config_values` so bowtie element identification matches by actual configured event ID bytes (not just CDI keyword heuristic)
2. Implement configuration value reading from node memory
3. Add configuration value editing and write operations
4. Align Bowtie component CSS custom properties with global app theme (remove dark-mode overrides)
5. Add cache management (size limits, expiration, cleanup)

**Short-Term (Next 2-4 weeks):**
1. Add end-to-end tests for Miller Columns + Bowties workflow
2. Integrate node store for state management (query backend on mount instead of component-local `$state`)
3. Implement CDI cache eviction policy
4. Add virtual scrolling for large CDI files in XML viewer
5. Improve error handling and user feedback for connection issues

**Medium-Term (Next 1-3 months):**
1. Implement event link visualization (canvas drag-and-drop for creating new producer ↔ consumer pairs)
2. Create comprehensive performance benchmarks
3. Implement configuration value caching strategy

See [docs/project/roadmap.md](../project/roadmap.md) for complete timeline.

## Resolved Technical Decisions

✅ **UI Framework:** Svelte 5 (reactive, TypeScript-friendly, lightweight)  
✅ **No CSS Framework:** Custom scoped styles (avoids bloat, full control)  
✅ **State in Component:** Start simple, migrate to stores as needed  
✅ **Svelte 5 Runes:** Modern reactive patterns (`$state`, `$derived`)  
✅ **Compact UX:** Single page before implementing three-view architecture  
✅ **Consolidated Discovery:** One button for discover/refresh (simpler mental model)  
✅ **Responsive Width:** 1200px max (allows table to breathe on wider screens)  
✅ **CDI Caching:** Platform-specific app data directory with filename-based cache keys  
✅ **XML Formatting:** Frontend JavaScript (DOMParser) not Rust (minimizes dependencies)  
✅ **Syntax Highlighting:** Prism.js (established library, good performance)  
✅ **Memory Config Protocol:** Address space 0xFF for CDI, datagram-based retrieval  
✅ **CDI Parsing:** roxmltree for XML parsing (Rust-native, zero-copy, error recovery)  
✅ **Miller Columns UI:** Dynamic columns (not fixed 5-column layout) to support variable CDI depth  
✅ **pathId System:** Index-based navigation (seg:N, elem:N#I) to eliminate name ambiguity  
✅ **UUID for UI:** Separate UUIDs for React/Svelte keys vs pathIds for navigation  
✅ **Lazy Parsing Cache:** lazy_static HashMap for parsed CDI structs (90% faster navigation)  
✅ **No Loading Animations:** Removed spinners and transitions for simpler, faster UI  
✅ **Keyboard Navigation:** Arrow keys + Enter/Space for accessibility compliance  
✅ **Error Boundaries:** Component-level error handling with graceful degradation  
✅ **WCAG 2.1 AA:** Screen reader support, proper ARIA labels, focus management  
✅ **Persistent Connections:** Background dispatcher with continuous message monitoring  
✅ **Event-Driven Updates:** Tauri events for automatic UI updates on network changes  
✅ **Broadcast Channels:** tokio::broadcast for multi-subscriber message distribution  
✅ **Arc<Mutex<>> Pattern:** Shared connection access for concurrent operations  
✅ **IdentifyEventsAddressed Protocol:** 125 ms inter-send delay (per JMRI reference); 500 ms collection window after last send  
✅ **EventRole Heuristic:** Two-tier: parent group name keywords first; `<description>` phrase patterns second; Ambiguous fallback  
✅ **snake_case Serde Alignment:** Bowtie structs use `#[serde(rename_all = "snake_case")]`; TypeScript types match field names exactly  
✅ **Bowties as In-Page Tab:** No SvelteKit route navigation; all tab panels live inside `+page.svelte` to preserve discovery state  
✅ **cdi-read-complete Event:** Backend emits full `BowtieCatalog` payload when CDI read cycle ends; frontend stores drive tab enable and card render  

## Open Technical Questions

1. **Configuration Value Caching:** Session-only or persistent? Dirty tracking strategy?
2. ~~**Connection Resilience:**~~ ✅ **RESOLVED:** Persistent dispatcher with background monitoring; reconnection logic to be implemented
3. **Concurrent Operations:** Allow multiple CDI/SNIP queries or enforce serial execution?
4. **CDI Cache Management:** Implement automatic cleanup? Set size limits? TTL for cache entries?
5. **Large CDI Files:** Implement chunked rendering or virtual scrolling for parsed structures?
6. **Large Networks:** Pagination strategy for 100+ nodes? Lazy loading?
7. ~~**Event Monitoring:**~~ ✅ **RESOLVED:** Shared connection with EventRouter broadcasting to frontend via Tauri events
8. **Configuration Writes:** Transaction-based or immediate? Rollback on error?
9. **CDI Parsing Cache Eviction:** LRU? Size-based? Time-based? Or unlimited?
10. **Reconnection Strategy:** Automatic reconnect with exponential backoff? Manual reconnect only?
11. **Message Queue Size:** Channel capacity limits? Backpressure handling for slow frontend?

---

*For aspirational architecture, see [docs/design/vision.md](../design/vision.md)*  
*For API reference, see [docs/technical/lcc-rs-api.md](lcc-rs-api.md) and [docs/technical/tauri-api.md](tauri-api.md)*
