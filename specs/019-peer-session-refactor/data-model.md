# Phase 1 — Data Model

**Feature**: 019-peer-session-refactor
**Date**: 2026-07-05

This document captures the types, fields, relationships, and state transitions introduced or modified by the refactor. Level of detail is architectural intent, not final Rust syntax — the code produces the syntax.

---

## New module: `lcc-rs::peer_session`

### `PeerSession` (actor struct)

The per-peer actor. Owns every protocol interaction with one remote NodeID.

**Fields**:
- `node_id: NodeID` — the remote peer this session represents. Immutable for session lifetime.
- `alias: Alias` — current CAN alias for the peer. Mutable; updated in place on Alias Map Definition or renegotiation (per FR-017).
- `outbound: TransportOutboundSender` — mpsc sender to the transport writer task.
- `inbound: broadcast::Receiver<Frame>` — subscription to the transport broadcast; the session filters to frames whose source alias matches `self.alias`.
- `commands: mpsc::Receiver<PeerCommand>` — command queue from callers.
- `snip_cache: Option<SnipData>` — cached SNIP result. `None` = never queried. Cleared on `PeerReinitialised`.
- `pip_cache: Option<PipData>` — cached PIP result. Same lifecycle as `snip_cache`.
- `active: Option<ActiveExchange>` — the current in-flight exchange (`None` if idle). At most one at a time by construction (per-peer serialization, FR-015).
- `snip_waiters: Vec<oneshot::Sender<Result<SnipData, PeerError>>>` — coalesced callers awaiting SNIP result (moved from `LiveNodeProxy.snip_waiters` per Clarification 1).
- `pip_waiters: Vec<oneshot::Sender<Result<PipData, PeerError>>>` — same for PIP.
- `event_subscribers: Vec<broadcast::Sender<PeerEvent>>` — session-local fan-out for progress events (e.g., `CdiProgress`).

**Ownership**: Spawned exclusively by `PeerSessionRegistry`. Runs on a dedicated tokio task per peer. Terminates when `commands` channel closes (session removed) or transport disconnects.

### `PeerCommand` (enum)

Commands accepted by a `PeerSession`. Each variant carries a response `oneshot::Sender` so callers can await the result.

Variants (per FR-003 through FR-007):
- `QuerySNIP { reply: oneshot::Sender<Result<SnipData, PeerError>> }`
- `QueryPIP { reply: oneshot::Sender<Result<PipData, PeerError>> }`
- `DownloadCDI { progress: broadcast::Sender<CdiProgress>, reply: oneshot::Sender<Result<Vec<u8>, PeerError>> }` — assembled CDI bytes returned on `reply`; the Tauri command persists them via `layout_state.record_captured` (per Clarification 2).
- `ReadConfig { address_space: u8, address: u32, count: u16, reply: oneshot::Sender<Result<Vec<u8>, PeerError>> }`
- `WriteConfig { address_space: u8, address: u32, data: Vec<u8>, reply: oneshot::Sender<Result<(), PeerError>> }`
- `Cancel { reason: String }` — cancels the active exchange (if any), emits peer cleanup, returns idle. No reply.
- `PeerReinitialised` — internal; forwarded from transport when the peer emits `Verified Node ID Number` or `Initialization Complete` after already having a session. Clears caches, aborts active exchange with `PeerError::PeerReinitialised`.
- `AliasChanged { new_alias: Alias }` — internal; updates `self.alias` in place. Aborts active exchange with `PeerError::AliasChanged` if the alias changed mid-exchange.

### `PeerError` (enum, `thiserror::Error`)

Public error type surfaced across the API boundary. Serde-serialised with stable string prefixes for the frontend (FR-018).

Variants:
- `Timeout { operation: &'static str, elapsed: Duration }`
- `PeerReinitialised` — peer emitted VNI/InitComplete during an active exchange.
- `AliasChanged { old: Alias, new: Alias }`
- `TransportUnhealthy { health: TransportHealth }`
- `Rejected { mti: u16, code: u16 }` — `DatagramRejected` (permanent) or `OptionalInteractionRejected`.
- `Cancelled { reason: String }`
- `NotSupported { operation: &'static str }` — peer lacks required PIP capability for the requested operation.
- `Protocol(String)` — malformed frame from peer, exchange-loop invariant violation.

### `ActiveExchange` (internal enum)

Internal state of the current in-flight exchange. Only one variant active at a time; drives the receive-loop's frame dispatch.

Variants:
- `SnipQuery { started_at: Instant, reply: oneshot::Sender<Result<SnipData, PeerError>>, assembly: SnipAssembly }`
- `PipQuery { started_at: Instant, reply: oneshot::Sender<Result<PipData, PeerError>>, assembly: PipAssembly }`
- `CdiDownload { started_at: Instant, progress: broadcast::Sender<CdiProgress>, reply: oneshot::Sender<Result<Vec<u8>, PeerError>>, cursor: u32, buffer: Vec<u8>, retry_state: RetryState }`
- `ConfigRead { started_at: Instant, reply: oneshot::Sender<Result<Vec<u8>, PeerError>>, address: u32, count: u16, buffer: Vec<u8> }`
- `ConfigWrite { started_at: Instant, reply: oneshot::Sender<Result<(), PeerError>>, address: u32, remaining: Vec<u8> }`

**Transition rule**: at most one `Some(ActiveExchange)` at a time. Callers that arrive while `active.is_some()` are queued behind the current exchange in the `commands` mpsc (natural FIFO serialization per FR-015).

**Cleanup obligation**: on `Timeout`, `Cancelled`, terminal `Rejected`, or `PeerReinitialised`, the session emits `TerminateDueToError` addressed to the peer (per FR-009) *before* completing the reply. The session tracks whether it has emitted the terminate to avoid duplicates.

### `CdiProgress` (payload struct)

Progress event for CDI download subscribers.

Fields:
- `bytes_read: u32`
- `retry_count: u32`

Consumed by the Tauri `download_cdi` command; forwarded to the frontend as a Tauri event on the existing progress channel (FR-018 — no shape change).

### `RetryState` (internal struct)

Datagram-exchange retry accounting for CDI/Config exchanges.

Fields:
- `attempts_at_current_address: u8`
- `max_attempts: u8` (const, default 3)
- `last_rejection_was_resend_ok: bool` — set from `DatagramRejected` error code bit 12 (per TN-9.7.3.2). Determines whether the next attempt is a retransmit vs an abort (per FR-011).

---

## New module: `lcc-rs::peer_session_registry`

### `PeerSessionRegistry`

Sole spawner and owner of `PeerSession` actors (per FR-017 / Clarification 4).

**Fields**:
- `sessions: tokio::sync::RwLock<HashMap<NodeID, PeerSessionHandle>>` (concurrency choice per research D1).
- `transport: TransportHandle` — for supplying to newly-spawned sessions.
- `spawn_watcher: JoinHandle<()>` — the task that subscribes to the transport broadcast and spawns sessions on qualifying inbound frames.

**Public methods**:
- `new(transport: TransportHandle) -> Self` — constructs and starts the spawn-watcher task.
- `get(&self, node_id: NodeID) -> Option<PeerSessionHandle>` — read-only lookup.
- `remove(&self, node_id: NodeID)` — used when a peer is explicitly forgotten. Rare.
- `clear(&self)` — called on transport disconnect. Aborts all sessions.

**Private method**:
- `spawn(&self, node_id: NodeID, alias: Alias) -> PeerSessionHandle` — called only by the spawn-watcher task on qualifying frames.

**Spawn qualification (FR-017)**: The spawn-watcher subscribes to the transport broadcast. It calls `spawn` exactly on:
- `Verified Node ID Number` frames (MTI `0x0170`)
- `Initialization Complete` frames (MTI `0x0100`)
- `Alias Map Definition` (AMD) frames (MTI `0x0701`)

All three carry the full NodeID payload. Frames that carry only an alias (PCER, EventReportWithPayload, addressed exchange frames) never trigger session creation.

**Idempotence**: repeat observations of the same NodeID update the existing session's alias in place (via `PeerCommand::AliasChanged`) and do NOT spawn a new session.

### `PeerSessionHandle`

Cloneable handle wrapping the session's command sender.

**Fields**:
- `commands: mpsc::Sender<PeerCommand>`
- `node_id: NodeID` (for logging)

**Public methods**:
- `command(&self, cmd: PeerCommand) -> Result<(), PeerError>` — raw dispatch.
- Typed convenience methods that construct the appropriate variant + oneshot channel and await the reply:
  - `query_snip(&self) -> Result<SnipData, PeerError>`
  - `query_pip(&self) -> Result<PipData, PeerError>`
  - `download_cdi(&self, progress: broadcast::Sender<CdiProgress>) -> Result<Vec<u8>, PeerError>`
  - `read_config(&self, address_space: u8, address: u32, count: u16) -> Result<Vec<u8>, PeerError>`
  - `write_config(&self, address_space: u8, address: u32, data: Vec<u8>) -> Result<(), PeerError>`
  - `cancel(&self, reason: impl Into<String>)`

---

## New module: `lcc-rs::event_router` (per research D2)

### `EventRouter`

Bus-scoped subscriber of the transport broadcast; sole owner of event-report fan-out.

**Fields**:
- `inbound: broadcast::Receiver<Frame>`
- `subscribers_by_event: HashMap<EventId, Vec<broadcast::Sender<EventReport>>>`
- `subscribers_by_role: HashMap<EventRole, Vec<broadcast::Sender<EventReport>>>`

**Public methods**:
- `subscribe_event(&mut self, event_id: EventId) -> broadcast::Receiver<EventReport>`
- `subscribe_role(&mut self, role: EventRole) -> broadcast::Receiver<EventReport>`
- `run(self)` — the task loop; classifies frames by MTI (PCER `0x5B4`, EventReportWithPayload `0x5B5`, Identify Events `0x968/0x970`, Learn Event `0x594`) and fans out.

**App-layer adapter** (`app/src-tauri/src/events/router.rs`): instantiates `EventRouter`, subscribes to selected events, forwards to `AppHandle::emit`. No protocol classification logic in this layer.

---

## Modified module: `lcc-rs::transport_actor`

### New: `TransportHealth` enum

`Healthy | Degraded { reason: String } | Wedged { reason: String }`.

Emitted by the writer task on a `broadcast::Sender<TransportHealth>` when a `w.send(&frame).await` exceeds `SERIAL_SEND_TIMEOUT` (500ms serial, 2000ms TCP — per research D4).

### New: `SERIAL_SEND_TIMEOUT` constants

Per-transport-kind timeouts, resolved at transport construction time.

### Modified: writer task

Each `w.send(&frame).await` is wrapped in `tokio::time::timeout(SEND_TIMEOUT, …)`. On timeout: emit `TransportHealth::Wedged`, drop the current frame, continue draining (the health broadcast is the signal, not a panic).

### Retired (Slice 6): `send_direct`, `direct_write_count`

Removed from public API and from all call sites. The `use_send_direct: bool` parameter on `datagram_read_exchange` is removed with the function's internalisation in Slice 4.

### Retained: inbound broadcast + `subscribe_all`

Per Clarification 3. Unchanged in shape.

---

## Modified module: `bowties-core::node_proxy`

### `LiveNodeProxy` — retained, delegates to session (per Clarification 1)

**Removed fields**:
- `snip_waiters` — coalescing moves into `PeerSession`.
- `pip_waiters` — same.
- Any cached SNIP/PIP wire state that duplicates session state.

**New field**:
- `session: PeerSessionHandle` — supplied by `NodeRegistry` when the proxy is spawned. `LiveNodeProxy` forwards every protocol call to it.

**Retained fields**:
- `last_seen: Instant`, `last_verified: Instant`
- `ConnectionStatus`
- `DiscoveredNode` snapshot for the frontend
- Config-tree hookup point (`GetConfigTree` / `SetConfigTree` pass-through to `LayoutState` per ADR-0015).

**Retained polymorphism**: `NodeProxyHandle::Live | Synthesized` — placeholder / synthesized nodes continue to work uniformly.

**Modified messages**: `ProxyMessage::QuerySnip` / `QueryPip` become pass-through; `SnipQueryDone` / `PipQueryDone` messages are removed (coalescing moved to session). `NodeReinitialised` continues to originate from transport / event router and forwards to the session.

---

## Modified module: `app/src-tauri/src/state.rs`

### `AppState` — field changes

**New**: `sessions: Arc<PeerSessionRegistry>`.

**Retired (Slice 4)**:
- `cdi_inflight: CdiInflightRegistry` — deleted; invariant now inherent to per-peer serialization.
- `cdi_download_cancel: Arc<AtomicBool>` — deleted; replaced by `PeerCommand::Cancel`.

---

## Retired module: `bowties-core::cdi_inflight`

Deleted in Slice 4 (module file + inline tests). Its inflight-tracking invariant (at most one CDI download per peer) is now structural, not runtime-defended.

---

## Relationships (summary)

```
                    ┌──────────────────────┐
                    │  TransportActor      │
                    │  (existing, revised) │
                    └───────┬──────────────┘
              inbound broadcast│  outbound mpsc
                    ┌──────────┴──────────┐
                    │                     │
        ┌───────────▼─────────┐   ┌───────▼──────────┐
        │ PeerSessionRegistry │   │  EventRouter     │
        │ (sole spawner)      │   │ (event fan-out)  │
        └───────────┬─────────┘   └───────┬──────────┘
                    │spawns                │subscribes
                    ▼                      ▼
              PeerSession A/B/C   (Tauri emit adapter)
                    ▲
                    │ delegates
                    │
              LiveNodeProxy A/B/C
                    ▲
                    │ commands
              Tauri command layer
```

---

## State transitions — `ActiveExchange`

```
                (idle: active = None)
                        │
        command arrives │
                        ▼
        ┌───────────────────────────┐
        │  active = Some(Exchange)  │
        │  send request frame(s)    │
        └───────────────────────────┘
                        │
        ┌───────────────┼───────────────┬───────────────┐
        ▼               ▼               ▼               ▼
   reply frames     timeout        Rejected/OIR    PeerReinit /
   received &       elapsed        received        AliasChanged
   assembled                                       / Cancel
        │               │               │               │
        │               │               │               │
        ▼               ▼               ▼               ▼
   emit ACK,       emit             emit            emit
   complete        Terminate        Terminate       Terminate
   reply(Ok)       DueToError,      DueToError,     DueToError,
                   complete         complete        complete
                   reply(Err)       reply(Err)      reply(Err)
        │               │               │               │
        └───────────────┴───────────────┴───────────────┘
                        │
                        ▼
                (idle: active = None)
```

**Invariant**: `active = None` after every terminal transition, and every terminal transition that resulted from *our* failure emits `TerminateDueToError` to the peer exactly once.
