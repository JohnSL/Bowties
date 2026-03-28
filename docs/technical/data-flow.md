# Data Flow Architecture

*High-level overview of how LCC/OpenLCB frames move through Bowties — from the
wire to the UI and back.*

**Last Updated:** 2026-03-27

---

## 1. Transport Layer

The physical connection (TCP or USB-serial) is owned by a single
**TransportActor** (`lcc-rs/src/transport_actor.rs`).  On creation the
underlying transport is split into two independent halves via `into_halves()`:

```
┌──────────────────────────────────────────────────────────────────┐
│  TransportActor                                                  │
│                                                                  │
│  reader_loop ──► broadcast::Sender<ReceivedMessage>  (capacity   │
│       │          2048, all inbound frames)                       │
│       │                                                          │
│       └──► per-MTI broadcast::Sender  (VerifiedNode,             │
│            InitComplete, SNIPRequest, etc.)                      │
│                                                                  │
│  mpsc::Receiver ◄── writer_loop ──► transport writer             │
│       │              (also echoes sent frames to broadcast)      │
└──────────────────────────────────────────────────────────────────┘
```

- **Read path**: `reader_loop` calls `reader.receive()` in a loop.  Each frame
  is wrapped in a `ReceivedMessage` (frame + timestamp) and sent to:
  1. The **all-frames** broadcast channel (every subscriber sees every frame).
  2. Any **per-MTI** broadcast channel that has been created for that MTI.
  3. The **alias map** (AMD/AMR frames maintain alias ↔ NodeID mappings).

- **Write path**: Callers enqueue frames via an `mpsc` channel.  `writer_loop`
  drains the queue and writes to the wire.  It also **echoes** each sent frame
  to the all-frames broadcast so traffic monitors see both directions.

No mutex is needed — the reader and writer own disjoint halves of the
underlying I/O.

### TransportHandle

A cheap-to-clone handle (`TransportHandle`) is the public API:

| Method              | Returns                        | Purpose                                |
|---------------------|--------------------------------|----------------------------------------|
| `send(&frame)`      | `Result<()>`                   | Enqueue a frame for transmission       |
| `subscribe_all()`   | `broadcast::Receiver`          | See every frame (inbound + echoed out) |
| `subscribe_mti(mti)`| `broadcast::Receiver`          | See only frames matching one MTI       |

Every component in the system (EventRouter, NodeProxy, protocol commands) holds
a cloned `TransportHandle`.  Subscriptions are created by calling `subscribe_*()`
on the handle, which returns a new `broadcast::Receiver`.  The receiver only
sees messages sent **after** the subscription is created — earlier buffered
messages are not replayed.

---

## 2. Subscription Patterns

There are two distinct subscription patterns in use:

### 2a. Persistent Subscriptions (connection lifetime)

Created once when the connection is established; held until disconnect.

| Owner                | Channel                             | Purpose                                    |
|----------------------|-------------------------------------|--------------------------------------------|
| **EventRouter**      | `subscribe_all()`                   | Forward all traffic to frontend monitor    |
| **EventRouter**      | `subscribe_mti(VerifiedNode)`       | Detect newly discovered nodes              |
| **EventRouter**      | `subscribe_mti(InitializationComplete)` | Detect nodes restarting                |
| **LccConnection**    | `subscribe_mti(VerifyNodeGlobal)`   | Respond to node-identity queries           |
| **LccConnection**    | `subscribe_mti(VerifyNodeAddressed)`| Respond to targeted identity queries       |
| **LccConnection**    | `subscribe_mti(AliasMapEnquiry)`    | Respond to alias-map queries               |
| **LccConnection**    | `subscribe_mti(ProtocolSupportInquiry)` | Respond to PIP queries                |
| **LccConnection**    | `subscribe_mti(AliasMapDefinition)` | Detect alias conflicts                     |
| **LccConnection**    | `subscribe_mti(SNIPRequest)`        | Respond to SNIP queries from other nodes   |

These are the protocol-mandated responder tasks — they make Bowties a
well-behaved node on the LCC bus.  The EventRouter subscriptions bridge the
network into the Tauri frontend for the traffic monitor and node list.

### 2b. Ephemeral Subscriptions (per-operation)

Created immediately before sending a request; dropped when the reply arrives or
the operation times out.  Typical lifetime: 100 ms – 5 seconds.

| Operation          | Function                               | Subscription    | Lifetime              |
|--------------------|----------------------------------------|-----------------|-----------------------|
| **CDI download**   | `read_cdi_with_handle()`               | `subscribe_all` | Entire download (seconds–minutes) |
| **SNIP query**     | `query_snip_handle_internal()`         | `subscribe_all` | One query (~1–5 s)   |
| **PIP query**      | `query_pip()`                          | `subscribe_all` | One query (~1–5 s)   |
| **Memory read**    | `read_memory_with_handle_timed()`      | `subscribe_all` | One read (~100–500 ms)|
| **Memory write**   | `write_memory_with_handle()`           | `subscribe_all` | One write (~100–500 ms)|
| **Node discovery**  | `discover_nodes()`                    | `subscribe_mti` | One scan (~250 ms)   |

All ephemeral subscriptions use the **subscribe-before-send** pattern:
subscribe first, then transmit the request.  This ensures no reply frames are
missed between sending and listening.

The CDI download subscribes once for the entire multi-chunk transfer,
ensuring no frames are lost between chunks.  An address check inside the
receive loop provides additional defence against stale replies.

---

## 3. Actor Hierarchy

```
┌──────────────────────────────────────────────────────────┐
│  Tauri Process                                            │
│                                                           │
│  AppState                                                 │
│  ├── connection: Arc<Mutex<LccConnection>>                │
│  │   ├── TransportActor (owns reader_loop + writer_loop)  │
│  │   ├── TransportHandle (cloneable)                      │
│  │   └── responder tasks (VerifyNode, AME, PIP, SNIP,    │
│  │       alias conflict)                                  │
│  │                                                        │
│  ├── event_router: EventRouter                            │
│  │   └── router_loop task (3 persistent subscriptions)    │
│  │                                                        │
│  └── node_registry: NodeRegistry                          │
│      ├── Node A → NodeProxyHandle → NodeProxy task        │
│      ├── Node B → NodeProxyHandle → NodeProxy task        │
│      └── Node C → NodeProxyHandle → NodeProxy task        │
│                                                           │
│  Tauri Commands (frontend ↔ backend IPC)                  │
│  ├── download_cdi()                                       │
│  ├── read_config_value() / write_config_value()           │
│  ├── get_discovered_nodes()                               │
│  └── ...                                                  │
└──────────────────────────────────────────────────────────┘
```

### LccConnection

Singleton.  Owns the `TransportActor` and holds the negotiated alias.
Spawns the persistent protocol-responder tasks.  Exposes high-level
operations (`read_cdi`, `read_memory`, `write_memory`, `discover_nodes`)
that create ephemeral subscriptions internally.

### EventRouter

Singleton.  Subscribes persistently to three channels and runs a
`tokio::select!` loop:

- **`all_rx`** — emits `lcc-message-received` events to the frontend for
  the traffic monitor window.
- **`verified_node_rx`** — calls `registry.get_or_create(node_id, alias)` to
  auto-register a NodeProxy when a new node appears.
- **`init_complete_rx`** — tells existing proxies to clear volatile state
  (SNIP, PIP, config) when a node reinitialises.

### NodeProxy (per-node actor)

One per discovered node.  Runs as a `tokio::spawn` task with an `mpsc`
mailbox.  Callers interact via `NodeProxyHandle` (cloneable, send messages
and receive replies via oneshot channels).

**What it stores:**
- SNIP data + status (cached after first query)
- PIP flags + status
- CDI XML + parsed CDI tree
- Configuration values and config tree
- Connection status, timestamps

**What it does NOT store:**
- A persistent `broadcast::Receiver`.  It holds a `TransportHandle` so it
  *can* subscribe, but it does not maintain a long-lived subscription.

**Query deduplication:** If multiple callers request SNIP or PIP
simultaneously, only one network query runs.  Extra callers are parked in a
`Vec<oneshot::Sender>` and all receive the same result when it completes.

### NodeRegistry

Thread-safe `HashMap<NodeID, NodeProxyHandle>` behind `RwLock`.  Provides
`get_or_create()` (spawns a new proxy if one doesn't exist) and `get()`
(lookup only).  Holds a `TransportHandle` so new proxies can be spawned
at any time.

---

## 4. Typical Operation Flows

### 4a. Connect + Discover

```
Frontend                 Tauri Command            LccConnection / EventRouter
───────                  ─────────────            ────────────────────────────
 connect() ──────────►  connect_to_network()
                         │ create TcpTransport
                         │ LccConnection::connect_with_dispatcher()
                         │   ├── alias negotiation (CID/RID/AMD)
                         │   ├── TransportActor::new() → reader_loop + writer_loop
                         │   └── start_query_responders() → 6 persistent subscriptions
                         │ EventRouter::start() → 3 persistent subscriptions
                         │ registry.set_transport()
                         ◄──────────────────────
 discover() ─────────►  discover_nodes()
                         │ connection.discover_nodes()
                         │   ├── subscribe_mti(VerifiedNode)     [ephemeral]
                         │   ├── send VerifyNodeGlobal
                         │   └── collect replies for 250ms, return list
                         │
                         │ EventRouter also sees VerifiedNode replies
                         │   └── registry.get_or_create() for each → spawns NodeProxy
                         ◄──────────────────────
```

### 4b. CDI Download (per node)

```
Frontend                 Tauri Command            LccConnection            Transport
───────                  ─────────────            ─────────────            ─────────
 downloadCdi() ──────►  download_cdi(node_id)
                         │ proxy.get_snapshot() → get alias
                         │ connection.read_cdi(alias, 5000)
                         │   └── read_cdi_with_handle():
                         │       subscribe_all()  [held for entire download]
                         │       for each 64-byte chunk:
                         │         ┌─'retry loop (max 3 attempts)──────────────────┐
                         │         │ send MemoryRead request frames                │
                         │         │ inner recv loop:                               │
                         │         │   ├── assemble datagram frames                │
                         │         │   ├── validate size (>71 → retry)             │
                         │         │   ├── validate address (mismatch → discard,   │
                         │         │   │   reset assembler, keep listening)         │
                         │         │   ├── ACK + accept                            │
                         │         │   └── handle DatagramRejected / timeout       │
                         │         └───────────────────────────────────────────────┘
                         │         advance to next chunk
                         │
                         │ proxy.set_cdi_data()
                         │ write to disk cache
                         ◄──────────────────────
```

### 4c. SNIP Query (per node, deduplicated)

```
Frontend            NodeProxy                      lcc-rs / Transport
───────             ─────────                      ──────────────────
 querySNIP() ───►  ProxyMessage::QuerySnip
                    │ cache hit? → return immediately
                    │ in-flight? → park caller in snip_waiters
                    │ first request:
                    │   tokio::spawn query_snip()
                    │     ├── subscribe_all()       [ephemeral]
                    │     ├── send SNIPRequest
                    │     ├── collect SNIPResponse frames (5s timeout)
                    │     └── parse SNIP payload
                    │   ProxyMessage::SnipQueryDone
                    │   wake all parked callers
                    ◄──────────────────
```

---

## 5. Channel Semantics

Bowties uses `tokio::sync::broadcast` for the read path.  Key properties:

- **Fan-out**: Every subscriber gets a copy of every message.  Subscribers
  do not compete — they all see the full stream.
- **Bounded buffer**: Capacity 2048 messages.  If a slow subscriber falls
  behind, it receives a `Lagged` error and loses the skipped messages.
- **No replay**: A new `subscribe()` call only sees messages sent after that
  point.  Older messages still in the buffer are invisible to the new
  receiver.
- **Drop = unsubscribe**: When a `broadcast::Receiver` is dropped, it is
  automatically removed from the sender's subscriber list.

The write path uses `tokio::sync::mpsc` (capacity 64).  This is many-to-one:
multiple `TransportHandle` clones can enqueue frames concurrently; the single
`writer_loop` serialises them onto the wire.
