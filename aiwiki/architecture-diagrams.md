# Architecture Diagrams

Visual reference for Bowties architecture: layers, seams, deep modules, and shallow-module risks. Use these diagrams as a shared vocabulary in conversations.

---

## System Layers

Six layers from user-facing UI down to the LCC bus. Each layer has a single responsibility and communicates only with its immediate neighbors (except Tauri event emissions, which bridge backend → frontend).

```mermaid
graph TB
    subgraph Frontend ["Frontend (SvelteKit SPA)"]
        direction TB
        Routes["Routes<br/><small>Screen composition, tab state,<br/>LayoutPicker gate</small>"]
        Components["Components<br/><small>Render state, emit intent,<br/>no async workflows</small>"]
        Facade["Layout Facade<br/><small>effectiveLayoutStore,<br/>effectiveNodeStore</small>"]
        Stores["Stores<br/><small>Durable state,<br/>deterministic transitions</small>"]
        Orchestrators["Orchestrators<br/><small>Multi-step async workflows,<br/>lifecycle transitions</small>"]
        Utils["Utils<br/><small>Pure helpers: nodeKey,<br/>formatters, serialization</small>"]
    end

    subgraph IPC ["IPC Boundary"]
        API["API Layer<br/><small>8 modules: thin invoke() wrappers</small>"]
    end

    subgraph Backend ["Backend (Tauri + Rust)"]
        Commands["Command Modules<br/><small>9 groups: discovery, cdi,<br/>bowties, layout_capture, …</small>"]
        AppState["AppState<br/><small>Connection, registry,<br/>router, caches</small>"]
    end

    subgraph Core ["bowties-core (Pure Rust)"]
        NodeTree["NodeConfigTree<br/><small>CDI + addresses + values + roles</small>"]
        Layout["Layout Engine<br/><small>Journaled persistence,<br/>companion-dir structure</small>"]
        Catalog["Bowtie Catalog<br/><small>Slot walking, role extraction,<br/>metadata merge</small>"]
        Registry["NodeRegistry<br/><small>NodeKey → NodeProxyHandle</small>"]
        Profile["Profile<br/><small>Structure profiles,<br/>resolution, annotation</small>"]
    end

    subgraph Protocol ["lcc-rs (Protocol Library)"]
        Discovery["Discovery<br/><small>LccConnection, alias allocation</small>"]
        PeerSession["PeerSession / Registry<br/><small>Per-peer actor: SNIP/PIP/CDI,<br/>memory r/w, single ACK owner</small>"]
        ProtocolLayer["Protocol<br/><small>Frame, MTI, Datagram,<br/>MemoryConfig</small>"]
        CDI["CDI<br/><small>Parser, hierarchy walker,<br/>role classification</small>"]
        Transport["Transport<br/><small>TCP, GridConnect Serial,<br/>SLCAN Serial</small>"]
    end

    Bus["LCC / OpenLCB Bus"]

    Routes --> Components
    Components --> Facade
    Facade --> Stores
    Routes --> Orchestrators
    Orchestrators --> Stores
    Orchestrators --> API
    Components -.->|"emit intent"| Routes
    Utils -.->|"pure imports"| Stores
    Utils -.->|"pure imports"| Orchestrators
    Utils -.->|"pure imports"| Facade

    API -->|"invoke()"| Commands
    Commands --> AppState
    Commands --> Core
    AppState --> Registry

    Core --> Protocol

    Discovery --> ProtocolLayer
    ProtocolLayer --> Transport
    Transport --> Bus

    Commands -.->|"emit() events"| Routes
```

---

## Deep Modules

Deep modules hide significant complexity behind a narrow public API. These are the architectural anchors — the places where important decisions are encapsulated. Touching them should be deliberate.

### `layout/*` — bowties-core

- Journaled writes (ADR-0006)
- Companion-dir structure
- Snapshot read/write
- Offline change staging
- Manifest reconstruction
- Known-layouts registry
- **API:** `save_capture`, `read_capture`, `update_offline_changes`, `execute` (journal)

### `bowtie/catalog` — bowties-core

- CDI slot walking
- Event role extraction
- Producer/consumer pairing
- Layout metadata merge
- Well-known EventID filtering
- **API:** `build_catalog()`, `CdiReadCompletePayload`

### `effectiveLayoutStore` — Layout Facade

- catalog × tree × metadata × layout merge
- Pending-deletion filter
- `effectiveRole()` waterfall
- `effectiveValue()` waterfall
- `slotsByRole()`, `isSlotFree()`
- **API:** `preview`, `effectiveBowties`, `effectiveRole`, `effectiveValue`

### `effectiveNodeStore` — Layout Facade

- Per-node origin tracking
- Capture/read status projection
- Persistability predicate
- Aggregate isDirty signal
- **API:** `nodeOrigin()`, `isPersistableInLayout()`, `isDirty`, `unsavedInMemoryNodeIds`

### `nodeRoster` — Store

- Unified live + placeholder facade
- Reactive `allEntries`/`liveEntries`/`placeholderEntries`
- Layout-scope clearing
- Profile stem tracking
- **API:** `allEntries`, `upsertLive`, `addPlaceholder`, `clearLayoutScope`

### `layoutLifecycleOrchestrator` — Orchestrator

- Single owner of lifecycle resets (ADR-0011)
- `resetForNewLayout` (full teardown)
- `resetForFreshLiveSession`
- `closeLayout` sequencing
- **API:** 3 entry points

### `LccConnection` — lcc-rs

- TCP/Serial connect + alias allocation
- Node discovery protocol
- Transport actor lifecycle
- **API:** `connect()`, `discover_nodes()`
- SNIP/PIP queries, CDI download, and memory config read/write are owned per-peer by `PeerSession` (see ADR-0016 / ADR-0018), not by a shared `BatchReader`

### `DatagramAssembler` — lcc-rs

- Multi-frame reassembly
- Pending/complete datagram tracking
- Error datagram handling
- **API:** `process_frame()` → `Option<Datagram>`

---

## Key Seams

Seams are the boundaries where ownership changes. Getting a fix or feature into the wrong seam causes architectural decay. This diagram shows the major seams and what crosses each one.

```mermaid
graph TB
    subgraph S1 ["Seam 1: Component ↔ Route"]
        direction LR
        S1L["Components<br/><small>render + emit intent</small>"]
        S1R["Routes<br/><small>compose screens,<br/>wire orchestrators</small>"]
        S1L -->|"events, callbacks"| S1R
        S1R -->|"props, stores"| S1L
    end

    subgraph S2 ["Seam 2: Store ↔ Orchestrator"]
        direction LR
        S2L["Stores<br/><small>deterministic state</small>"]
        S2R["Orchestrators<br/><small>multi-step async</small>"]
        S2R -->|"mutate via public setters"| S2L
        S2L -->|"reactive reads ($derived)"| S2R
    end

    subgraph S3 ["Seam 3: Facade ↔ Backing Stores"]
        direction LR
        S3L["effectiveLayoutStore<br/>effectiveNodeStore"]
        S3R["bowtieCatalogStore<br/>nodeTreeStore<br/>configChangesStore<br/>bowtieMetadataStore<br/>layoutStore"]
        S3R -->|"composed into<br/>single read model"| S3L
    end

    subgraph S4 ["Seam 4: Frontend ↔ Backend (IPC)"]
        direction LR
        S4L["API modules<br/><small>invoke() wrappers</small>"]
        S4R["Command modules<br/><small>Result&lt;T, String&gt;</small>"]
        S4L -->|"Tauri invoke"| S4R
        S4R -.->|"Tauri emit"| S4E["Routes<br/><small>event listeners</small>"]
    end

    subgraph S5 ["Seam 5: src-tauri ↔ bowties-core"]
        direction LR
        S5L["Commands + AppState<br/><small>Tauri-dependent</small>"]
        S5R["bowties-core<br/><small>Pure Rust, zero Tauri deps</small>"]
        S5L -->|"call pure functions"| S5R
    end

    subgraph S6 ["Seam 6: bowties-core ↔ lcc-rs"]
        direction LR
        S6L["bowties-core<br/><small>App domain types</small>"]
        S6R["lcc-rs<br/><small>Protocol types + transport</small>"]
        S6L -->|"use NodeID, Cdi,<br/>EventRole, MemoryConfigCmd"| S6R
    end

    S1 ~~~ S2
    S2 ~~~ S3
    S3 ~~~ S4
    S4 ~~~ S5
    S5 ~~~ S6
```

---

## Shallow Module Risks

Shallow modules expose complexity rather than hiding it. They force callers to understand implementation details, create coupling, and make changes fragile. These are candidates for deepening.

```mermaid
graph TB
    subgraph Shallow ["Shallow Modules — Candidates for Deepening"]
        direction TB

        PP["<b>+page.svelte</b><br/>(god component, ~1942 lines)<br/><br/>▸ ~40 $state variables<br/>▸ Inlines discovery, CDI, config read,<br/>  layout lifecycle, sync, dialog workflows<br/>▸ Owns save-progress wiring that belongs<br/>  in saveLayoutOrchestrator<br/>▸ Acts as implicit orchestrator<br/><br/><i>Risk: every workflow change is fragile<br/>because all state is local</i>"]

        BR["<b>bowties.rs</b><br/>(commands, ~1962 lines, 0 tests)<br/><br/>▸ Catalog build + layout YAML + protocol exchange<br/>  all in one module<br/>▸ Mixes IPC boundary with core algorithm<br/>▸ Zero test coverage on the most complex<br/>  backend algorithm<br/><br/><i>Risk: untestable, tightly coupled to AppState</i>"]

        CD["<b>configDraftOrchestrator</b><br/>(no tests)<br/><br/>▸ Mirrors config drafts to backend IPC<br/>  or offline persistence<br/>▸ Decision logic for online vs offline path<br/>  is untested<br/><br/><i>Risk: silent save data loss if<br/>draft routing logic is wrong</i>"]

        API["<b>API layer</b><br/>(8 modules, pure pass-through)<br/><br/>▸ Every function is invoke('cmd', args)<br/>▸ No type validation, no retry, no batching<br/>▸ Zero added value over raw invoke<br/><br/><i>Risk: not harmful but not deep — just<br/>a naming layer. Could add value with<br/>error normalization or retry logic.</i>"]

        CS["<b>configChanges.svelte.ts</b><br/>(3 layers: draft/offlinePending/baseline)<br/><br/>▸ Multiple overlapping change layers<br/>▸ Pruning logic spread across orchestrators<br/>  and the store itself<br/>▸ commitForSave() vs clearPersistedDrafts() —<br/>  two paths for clearing<br/><br/><i>Risk: subtle data-loss bugs when layers<br/>interact unexpectedly</i>"]
    end
```

---

## Data Ownership Map

Which module is the authoritative owner of each major piece of state. When state is owned in the wrong place, bugs appear as stale data, race conditions, or reset leaks.

```mermaid
graph LR
    subgraph "Frontend State Ownership"
        direction TB

        nodeRoster["<b>nodeRoster</b><br/>Unified node list<br/>(live + placeholder)"]
        nodeTree["<b>nodeTreeStore</b><br/>CDI trees per node"]
        configChanges["<b>configChangesStore</b><br/>Draft / offline / baseline layers"]
        layout["<b>layoutStore</b><br/>Layout file state, active context"]
        bowtieCatalog["<b>bowtieCatalogStore</b><br/>Backend-built catalog"]
        bowtieMetadata["<b>bowtieMetadataStore</b><br/>Pending name/tag/role edits"]
        offlineChanges["<b>offlineChangesStore</b><br/>Persisted offline diffs"]
        syncPanel["<b>syncPanelStore</b><br/>Conflict/clean row state"]
        connSel["<b>connectorSelectionsStore</b><br/>Slot selections per node"]
    end

    subgraph "Facade Layer (read-only merge)"
        effectiveLayout["<b>effectiveLayoutStore</b><br/>Merged bowtie view"]
        effectiveNode["<b>effectiveNodeStore</b><br/>Per-node status"]
    end

    bowtieCatalog --> effectiveLayout
    nodeTree --> effectiveLayout
    bowtieMetadata --> effectiveLayout
    configChanges --> effectiveLayout
    layout --> effectiveLayout

    nodeTree --> effectiveNode
    layout --> effectiveNode

    subgraph "Backend State Ownership"
        appState["<b>AppState</b><br/>Connection, registry, caches"]
        registry["<b>NodeRegistry</b><br/>NodeKey → proxy handle"]
        savedTrees["<b>saved_trees</b><br/>Snapshot-seeded config trees"]
        layoutCtx["<b>ActiveLayoutContext</b><br/>Companion dir, node IDs"]
    end

    appState --> registry
    appState --> layoutCtx
    registry --> savedTrees
```

---

## Major Workflow Seam Crossings

Each workflow crosses multiple seams. The number of seam crossings indicates the coordination cost of a change. High-crossing workflows need more careful testing.

| Workflow | Seams Crossed | Modules Involved | Key Risk |
|----------|:---:|---|---|
| **Discovery** | 5 | Route → Orchestrator → API → Commands → lcc-rs → Bus | Placeholder crash if unfiltered |
| **Config Read Session** | 5 | Route → Orchestrator → API → Commands → lcc-rs → Bus | Phase machine in orchestrator + backend |
| **Save Layout** | 4 | Route → Orchestrator → API → Commands → bowties-core | 3-phase ordering, stale catalog |
| **Sync Apply** | 5 | Route → Orchestrator → API → Commands → lcc-rs → Bus | Offline→online value reconciliation |
| **Layout Open** | 4 | Route → Orchestrator → API → Commands → bowties-core | Snapshot hydration, catalog rebuild |
| **Bowtie Catalog Build** | 3 | Orchestrator → API → Commands → bowties-core | Event role exchange timing |
| **Connector Selection** | 4 | Orchestrator → API → Commands → Profile → tree re-annotate | Tree refresh after mode change |
| **Placeholder Add** | 4 | Orchestrator → API → Commands → bowties-core → profile | UUID minting, CDI synthesis |

---

## Module Depth Assessment

Depth = complexity hidden behind the API ÷ API surface area. Deep modules encapsulate; shallow modules spread complexity to callers.

| Module | Depth | Rationale |
|--------|:---:|---|
| `layout/*` (bowties-core) | **Deep** | Journaled writes, manifest reconstruction, companion-dir structure — all behind `save_capture`/`read_capture` |
| `bowtie/catalog` (bowties-core) | **Deep** | Complex slot-walking + role extraction behind `build_catalog()` |
| `effectiveLayoutStore` | **Deep** | 5-store merge hidden behind `preview` / `effectiveBowties` |
| `effectiveNodeStore` | **Deep** | Multi-store projection behind `isPersistableInLayout()` / `isDirty` |
| `LccConnection` (lcc-rs) | **Deep** | Protocol orchestration, alias allocation, transport actor — behind `connect()`/`discover()` |
| `DatagramAssembler` (lcc-rs) | **Deep** | Multi-frame reassembly behind `process_frame()` |
| `layoutLifecycleOrchestrator` | **Deep** | All reset paths consolidated into 3 entry points (ADR-0011) |
| `nodeRoster` | **Deep** | Unifies 4 backing stores behind one reactive facade |
| `saveLayoutOrchestrator` | **Medium** | Coordinates 3-phase save but callers still manage some wiring |
| `configChanges.svelte.ts` | **Medium** | 3-layer stack is powerful but pruning is partially external |
| `+page.svelte` | **Shallow** | 1942 lines, ~40 state vars, inlines workflows that should be orchestrated |
| `bowties.rs` (commands) | **Shallow** | 1962 lines mixing IPC + algorithm + protocol, untested |
| `configDraftOrchestrator` | **Shallow** | Untested routing logic for online/offline draft persistence |
| API layer (8 modules) | **Shallow** | Pure pass-through; no error normalization, no retry |
| `configSidebarPresenter` | **Medium** | Derives sidebar state but some badge logic lives in callers |

---

## Deepening Opportunities

Concrete actions that would convert shallow modules into deep ones:

1. **`+page.svelte` → extract workflows**: Move discovery, CDI download, and config-read session state into their respective orchestrators. The route should wire callbacks and render, not sequence multi-step flows.

2. **`bowties.rs` → decompose**: Split into catalog-builder (extract to `bowties-core`), layout YAML commands, and protocol exchange. The catalog algorithm should be testable without `AppState`.

3. **`configDraftOrchestrator` → add tests**: The online/offline routing decision is a critical data-integrity seam. Test it.

4. **API layer → add error normalization**: A thin `invokeWithErrorHandling()` wrapper could normalize backend error strings into typed frontend errors, adding depth without complexity.

5. **`configChanges` → consolidate pruning**: Move all layer-interaction logic (commitForSave, clearPersistedDrafts, pruneResolvedDrafts) into the store's public API with clear preconditions, rather than spreading across orchestrators.
