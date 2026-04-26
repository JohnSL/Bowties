# Node Proxy Implementation Plan

> **Status: Historical — implemented.** The Node Proxy actor architecture is in place. See `proxy-migration-finish-plan.md` for finish notes. Retain as implementation history.

## Problem

Per-node state is scattered across 5+ independent keyed collections in the Tauri backend:

| Collection | Location | Key | Content |
|---|---|---|---|
| `AppState.nodes` | state.rs | Vec by node_id | SNIP, PIP, CDI ref, status, timestamps |
| `AppState.config_value_cache` | state.rs | node_id_hex → path → bytes | Event ID bytes per CDI slot |
| `AppState.node_trees` | state.rs | node_id_hex | Full `NodeConfigTree` |
| `CDI_PARSE_CACHE` | commands/cdi.rs | node_id string | Parsed `Cdi` struct |
| `AppState.diag_stats` sub-maps | diagnostics.rs | node_id_hex | CDI download + config read stats |

This causes three classes of problems:

1. **Duplicate queries** — Three independent events (VerifiedNode reply, two InitializationComplete frames) each trigger SNIP+PIP queries to the same node. No deduplication or caching exists at the query boundary.

2. **Self-interleaving risk** — If the same node is queried concurrently from different code paths, replies to our own duplicate queries could interleave (same src+dst alias pair). The dest-alias filter protects against cross-node interleaving but not self-interleaving.

3. **Scattered state** — Every Tauri command that needs per-node data must acquire multiple independent locks (`nodes`, `config_value_cache`, `node_trees`, `CDI_PARSE_CACHE`), often redundantly looking up the same node across different maps.

## Solution

Replace the scattered per-node maps with a **NodeProxy actor per discovered node**, managed by a **NodeRegistry** in the Tauri layer. Each proxy owns all state for its node and serialises access through a mailbox channel, eliminating lock contention and enabling built-in query deduplication.

lcc-rs stays a focused protocol/transport library — unchanged.

## Architecture After

```
Frontend (Svelte)
    │
    ▼  Tauri commands
┌─────────────────────────────────────────────────┐
│  AppState                                       │
│    ├── connection: LccConnection (unchanged)    │
│    ├── transport_handle: TransportHandle         │
│    ├── node_registry: NodeRegistry ◄── NEW      │
│    ├── bowties_catalog (cross-node, unchanged)  │
│    ├── profiles (per-model, unchanged)          │
│    └── diag_log (global, unchanged)             │
│                                                  │
│  NodeRegistry: HashMap<NodeId, NodeProxyHandle> │
│    ├── Node 05.01.01.01.A2.FF ──► [actor task]  │
│    ├── Node 09.00.99.01.DD.94 ──► [actor task]  │
│    └── Node 02.01.12.4F.45.CC ──► [actor task] │
└─────────────────────────────────────────────────┘
    │
    ▼  TransportHandle (lcc-rs, unchanged)
┌──────────────────────┐
│  TransportActor      │
│    reader ──► write  │
│    alias_map         │
└──────────────────────┘
```

## NodeProxy Actor

Each `NodeProxy` runs as a `tokio::spawn` task with an mpsc mailbox:

```rust
struct NodeProxy {
    // Identity
    node_id: NodeId,
    alias: u16,

    // Mailbox
    mailbox_rx: mpsc::Receiver<ProxyMessage>,
    mailbox_tx: mpsc::Sender<ProxyMessage>,  // for spawned tasks to send results back

    // Transport (clone of shared handle)
    transport_handle: TransportHandle,
    our_alias: u16,

    // Cached state (currently in AppState.nodes / DiscoveredNode)
    snip: Option<SNIPData>,
    snip_status: QueryStatus,
    pip_flags: Option<ProtocolFlags>,
    pip_status: QueryStatus,
    connection_status: ConnectionStatus,
    last_seen: Instant,
    last_verified: Option<Instant>,

    // Cached state (currently in CDI_PARSE_CACHE)
    cdi_data: Option<CdiData>,       // raw XML bytes
    cdi_parsed: Option<Cdi>,         // parsed CDI struct

    // Cached state (currently in AppState.config_value_cache)
    config_values: HashMap<String, [u8; 8]>,  // element_path → event ID bytes

    // Cached state (currently in AppState.node_trees)
    config_tree: Option<NodeConfigTree>,

    // Diagnostics (currently in AppState.diag_stats sub-maps)
    diag: NodeDiagnostics,

    // In-flight guards
    snip_waiters: Option<Vec<oneshot::Sender<Result<SNIPData>>>>,
    pip_waiters: Option<Vec<oneshot::Sender<Result<ProtocolFlags>>>>,
    cdi_waiters: Option<Vec<oneshot::Sender<Result<CdiData>>>>,
    config_cancel: CancellationToken,
}
```

## Message Protocol

```rust
enum ProxyMessage {
    // === Quick queries (handled inline, <100ms) ===
    QuerySnip { reply: oneshot::Sender<Result<SNIPData>> },
    QueryPip { reply: oneshot::Sender<Result<ProtocolFlags>> },
    GetSnapshot { reply: oneshot::Sender<DiscoveredNode> },

    // === Long operations (actor spawns a task) ===
    DownloadCdi { reply: oneshot::Sender<Result<CdiData>> },
    ReadAllConfig {
        total_node_count: usize,
        node_index: usize,
        reply: oneshot::Sender<Result<()>>,
    },
    ReadSingleConfig {
        element_path: Vec<String>,
        reply: oneshot::Sender<Result<ConfigValueWithMetadata>>,
    },
    WriteSingleConfig {
        element_path: Vec<String>,
        value: Vec<u8>,
        reply: oneshot::Sender<Result<()>>,
    },

    // === Results from spawned tasks (internal) ===
    SnipComplete { result: Result<SNIPData> },
    PipComplete { result: Result<ProtocolFlags> },
    CdiDownloadComplete { result: Result<CdiData> },
    ConfigReadProgress { values: HashMap<String, [u8; 8]> },
    ConfigReadComplete { result: Result<()> },

    // === Status updates ===
    VerifyStatus { timeout: Duration, reply: oneshot::Sender<Result<ConnectionStatus>> },
    UpdateAlias { alias: u16 },

    // === Lifecycle ===
    NodeReinitialised,
    Shutdown,
}
```

## Spawn-and-Report for Long Operations

CDI download and config reads are multi-round-trip operations spanning seconds. The actor stays responsive by spawning them as separate tasks:

```
  Tauri command                NodeProxy actor              Spawned task
       │                           │                            │
       ├── DownloadCdi{reply} ───► │                            │
       │                           ├── check cache (miss)       │
       │                           ├── store reply in waiters   │
       │                           ├── tokio::spawn ──────────► │
       │                           │   (owns TransportHandle    │
       │                           │    clone only, no state)   │
       │                           │                            ├── datagram reads...
       │                           │ ◄── CdiDownloadComplete ──┤
       │                           ├── cache result             │
       │                           ├── wake all waiters ──────► (dropped)
       │ ◄──────────── reply ──────┤
```

A second `DownloadCdi` arriving while the first is in flight just parks its `reply` in `cdi_waiters` — no duplicate network request.

## Cancellation

Each proxy holds a `CancellationToken` (from `tokio_util`). On reinitialisation or disconnect:

```rust
ProxyMessage::NodeReinitialised => {
    self.config_cancel.cancel();
    self.config_cancel = CancellationToken::new();
    // SNIP, PIP, config values are volatile — clear them
    self.snip = None;
    self.pip_flags = None;
    self.config_values.clear();
    self.config_tree = None;
    // CDI XML is stable across reinit — keep it
}

ProxyMessage::Shutdown => {
    self.config_cancel.cancel();
    break; // exit actor loop
}
```

Spawned tasks pass `cancel_token.clone()` and check `token.is_cancelled()` between batches.

## NodeProxyHandle

The caller-facing handle is a thin wrapper over the mailbox sender:

```rust
#[derive(Clone)]
struct NodeProxyHandle {
    node_id: NodeId,
    alias: u16,
    tx: mpsc::Sender<ProxyMessage>,
}

impl NodeProxyHandle {
    async fn query_snip(&self) -> Result<SNIPData> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(ProxyMessage::QuerySnip { reply: reply_tx }).await?;
        reply_rx.await?
    }

    async fn query_pip(&self) -> Result<ProtocolFlags> { ... }
    async fn download_cdi(&self) -> Result<CdiData> { ... }
    async fn get_snapshot(&self) -> DiscoveredNode { ... }
    // etc.
}
```

## NodeRegistry

```rust
struct NodeRegistry {
    nodes: RwLock<HashMap<NodeId, NodeProxyHandle>>,
    transport_handle: TransportHandle,
    our_alias: u16,
}

impl NodeRegistry {
    /// Get or create a proxy for a node. Idempotent.
    async fn get_or_create(&self, node_id: NodeId, alias: u16) -> NodeProxyHandle { ... }

    /// Get an existing proxy. Returns None if node not yet discovered.
    async fn get(&self, node_id: &NodeId) -> Option<NodeProxyHandle> { ... }

    /// Get proxy by alias (linear scan — rare path).
    async fn get_by_alias(&self, alias: u16) -> Option<NodeProxyHandle> { ... }

    /// Snapshot all nodes for frontend display.
    async fn all_snapshots(&self) -> Vec<DiscoveredNode> { ... }

    /// Shutdown all proxies and clear the registry.
    async fn shutdown_all(&self) { ... }
}
```

## AppState Field Changes

### Removed fields (absorbed by NodeProxy)

| Field | Absorbed into |
|---|---|
| `nodes: Arc<RwLock<Vec<DiscoveredNode>>>` | `NodeProxy` cached fields + `NodeRegistry.all_snapshots()` |
| `config_value_cache: Arc<RwLock<HashMap<...>>>` | `NodeProxy.config_values` |
| `node_trees: Arc<RwLock<HashMap<...>>>` | `NodeProxy.config_tree` |
| `config_read_cancel: Arc<AtomicBool>` | `NodeProxy.config_cancel` (per-node, not global) |

### Removed statics

| Static | Absorbed into |
|---|---|
| `CDI_PARSE_CACHE` (lazy\_static in cdi.rs) | `NodeProxy.cdi_parsed` |

### New field

| Field | Type |
|---|---|
| `node_registry` | `Arc<NodeRegistry>` |

### Unchanged fields

| Field | Reason |
|---|---|
| `connection` | Needed for alias allocation (pre-actor, pre-proxy) |
| `transport_handle` | Shared with NodeRegistry and EventRouter |
| `event_router` | Stateless pass-through (no per-node state) |
| `bowties_catalog` | Cross-node aggregate — not per-node |
| `profiles` | Per-model, not per-node |
| `active_connection` | Connection config (UI concern) |
| `diag_log` | Global ring buffer |
| `diag_stats` | Per-node sub-maps could migrate later; low priority |

## EventRouter Changes

Currently `handle_node_reinitialized` and `handle_node_discovered` emit Tauri events that the frontend catches to trigger SNIP/PIP queries. With the proxy:

- `handle_node_discovered(node_id, alias)` → calls `registry.get_or_create(node_id, alias)` to ensure proxy exists, then emits `lcc-node-discovered` (frontend fetches snapshot from proxy)
- `handle_node_reinitialized(node_id, alias)` → calls `proxy.send(NodeReinitialised)` to invalidate volatile cache, emits `lcc-node-reinitialized` (frontend re-fetches)

This means the EventRouter needs a reference to the NodeRegistry. It already holds `app: AppHandle`, so it can access managed state, or it can hold an `Arc<NodeRegistry>` directly.

**Dedup effect**: Multiple InitializationComplete frames arriving 26ms apart both send `NodeReinitialised` to the proxy. The second one is a no-op (cache already cleared). The frontend's re-query hits the proxy which does a single network SNIP+PIP exchange, not three.

## Tauri Command Migration

### Before (query_snip_single)
```rust
let conn = state.connection.read().await.clone().unwrap();
let mut conn = conn.lock().await;
let result = conn.query_snip(alias, None).await;
state.update_node(node_id, |n| { n.snip_data = ...; n.snip_status = ...; }).await;
```

### After
```rust
let proxy = state.node_registry.get_by_alias(alias).await.unwrap();
let snip = proxy.query_snip().await?;
// State update happened inside the proxy actor — nothing else to do.
// Return the snapshot for the frontend:
Ok(proxy.get_snapshot().await)
```

### Before (download_cdi)
```rust
let conn = state.connection.read().await.clone().unwrap();
let mut conn = conn.lock().await;
let cdi_data = conn.read_cdi(alias, timeout).await?;
state.update_node(node_id, |n| n.cdi = Some(cdi_data.clone())).await;
CDI_PARSE_CACHE.write().await.insert(node_id_str, parsed_cdi);
```

### After
```rust
let proxy = state.node_registry.get(&node_id).await.unwrap();
let cdi_data = proxy.download_cdi().await?;
// CDI bytes + parsed struct both cached inside proxy.
```

### Before (read_all_config_values)
```rust
// Acquires connection lock, nodes lock, config_value_cache lock, node_trees lock,
// CDI_PARSE_CACHE lock, profiles lock, bowties_catalog lock across ~400 lines
```

### After
```rust
let proxy = state.node_registry.get(&node_id).await.unwrap();
proxy.read_all_config(total_node_count, node_index).await?;
// Config values, config tree all updated inside proxy.
// Cross-node bowtie assembly triggered after last node completes.
```

## Cross-Node Operations

Two operations span multiple nodes and remain outside the proxy:

1. **Bowtie catalog assembly** — After all nodes' config reads complete, event roles are correlated across nodes. This stays as a top-level operation that reads snapshots from each proxy:
   ```rust
   let snapshots: Vec<_> = registry.all_snapshots().await;
   let catalog = build_bowtie_catalog(&snapshots, &profiles);
   state.bowties_catalog.write().await.replace(catalog);
   ```

2. **Event role annotation** — `query_event_roles()` uses Identify Events protocol across multiple nodes, then annotates each node's config tree. This can be a coordinator task that calls into individual proxies:
   ```rust
   for handle in registry.all_handles().await {
       handle.annotate_event_roles(roles_for_node).await;
   }
   ```

## Implementation Steps

### Step 1: Define NodeProxy, ProxyMessage, NodeProxyHandle

Create `app/src-tauri/src/node_proxy.rs`:
- `NodeProxy` struct with all per-node fields
- `ProxyMessage` enum
- `NodeProxyHandle` with async request methods
- `NodeProxy::run()` actor loop handling inline messages only (SNIP, PIP, GetSnapshot, Shutdown)
- Unit tests: create proxy, send QuerySnip, get cached result

### Step 2: Define NodeRegistry

Create `app/src-tauri/src/node_registry.rs`:
- `NodeRegistry` struct with `HashMap<NodeId, NodeProxyHandle>`
- `get_or_create()`, `get()`, `get_by_alias()`, `all_snapshots()`, `shutdown_all()`
- Unit tests: create registry, add nodes, retrieve by ID and alias

### Step 3: Wire NodeRegistry into AppState

In `state.rs`:
- Add `node_registry: Arc<NodeRegistry>` field
- Remove `nodes`, `config_value_cache`, `node_trees`, `config_read_cancel`
- Update `set_connection_with_dispatcher()` to initialise NodeRegistry with TransportHandle
- Update `disconnect()` to call `registry.shutdown_all()`

### Step 4: Migrate discovery commands

In `commands/discovery.rs`:
- `discover_nodes` → probes network, creates proxies via `registry.get_or_create()`
- `register_node` → `registry.get_or_create(node_id, alias)`
- `query_snip_single` → `proxy.query_snip().await`
- `query_snip_batch` → parallel `proxy.query_snip()` per node
- `query_pip_single` → `proxy.query_pip().await`
- `query_pip_batch` → parallel `proxy.query_pip()` per node
- `verify_node_status` → `proxy.verify_status(timeout).await`
- `refresh_all_nodes` → `registry.all_handles()`, verify each, remove stale
- Remove all `state.update_node()` calls — proxy owns the mutation
- Run tests.

### Step 5: Add spawn-and-report for CDI download

In `node_proxy.rs`:
- Handle `DownloadCdi` message: check cache → check in-flight → spawn task
- Handle `CdiDownloadComplete` message: cache result, wake waiters
- Spawned task uses `TransportHandle` clone to call `read_cdi()`

### Step 6: Migrate CDI commands

In `commands/cdi.rs`:
- Remove `CDI_PARSE_CACHE` static
- `download_cdi` → `proxy.download_cdi().await`
- `get_cdi_xml` → `proxy.get_cdi_xml().await` (returns cached bytes)
- `get_cdi_structure` → `proxy.get_cdi_parsed().await` (returns cached parsed CDI)
- CDI navigation commands (`get_column_items`, `get_element_details`) → operate on `proxy.get_cdi_parsed()`
- Run tests.

### Step 7: Add spawn-and-report for config reads

In `node_proxy.rs`:
- Handle `ReadAllConfig` message: spawn task that does batched memory reads
- Spawned task sends `ConfigReadProgress` messages back to mailbox as batches complete
- Handle `ConfigReadComplete`: update config_values, build/update config_tree
- Cancellation via `CancellationToken` checked between batches

### Step 8: Migrate config commands

In `commands/cdi.rs`:
- `read_config_value` → `proxy.read_single_config(path).await`
- `read_all_config_values` → `proxy.read_all_config(count, index).await`
- `cancel_config_reading` → `proxy.cancel_config().await` (per-node cancellation)
- `get_node_tree` → `proxy.get_config_tree().await`
- `write_config_value` → `proxy.write_single_config(path, value).await`
- Cross-node bowtie assembly remains a top-level operation after all proxies report complete
- Run tests.

### Step 9: Update EventRouter

In `events/router.rs`:
- Add `Arc<NodeRegistry>` field (or access via `app.state::<AppState>()`)
- `handle_node_discovered` → `registry.get_or_create(node_id, alias)`, then emit event
- `handle_node_reinitialized` → `proxy.send(NodeReinitialised)`, then emit event
- Dedup is automatic: proxy ignores redundant invalidation

### Step 10: Clean up AppState

- Remove `AppState::get_nodes()`, `set_nodes()`, `add_node()`, `update_node()`, `clear_nodes()`
- Replace with `node_registry.all_snapshots()` where needed
- Verify no remaining references to removed fields
- Run full test suite
