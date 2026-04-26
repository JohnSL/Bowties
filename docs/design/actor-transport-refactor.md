# Design: Actor-Based Transport Refactor

> **Status: Historical — implemented.** The actor-based transport and Node Proxy architecture is in place. Current durable architecture is documented in `product/architecture/`. This file is retained as implementation history.

## Current Architecture (as of v0.1.7)

```
LccConnection
├── dispatcher: Option<Arc<Mutex<MessageDispatcher>>>   ← used for TCP
└── transport:  Option<Box<dyn LccTransport>>           ← used by tests only
```

`MessageDispatcher` wraps the transport in a `TapTransport` and runs a
`listener_loop` that polls `transport.receive(1ms)` continuously, broadcasting
every frame to a `broadcast::Sender<ReceivedMessage>`.  Call-sites that want to
receive a reply must:

1. Subscribe to the broadcast channel (`disp.subscribe_all()` or
   `disp.subscribe_mti(MTI::Foo)`).
2. Lock the mutex briefly to call `disp.send(frame)`.
3. `rx.recv().await` on the broadcast channel — no lock held while waiting.

The transport mutex is needed because `listener_loop` holds it while calling
`receive()`, and senders need it too.  Steps 1–3 are repeated in every
protocol operation (`read_memory_with_dispatcher`, `read_cdi_with_dispatcher`,
`write_memory_with_dispatcher`, `verify_node_with_dispatcher`, …).

### Problems

* **Mutex contention** — `listener_loop` holds the transport lock for the
  entire 1 ms poll window.  Any `send()` caller blocks for up to 1 ms even on a
  fast local connection.  Under high traffic this can compound.
* **Dual paths** — every public method (`read_memory`, `read_cdi`, etc.) has both
  a dispatcher branch and a `_direct` / `_impl` fallback.  They diverge over
  time (the CDI mutex-starvation bug was caused by exactly this).
* **`transport()` leak** — `MessageDispatcher::transport()` exposes the
  `Arc<Mutex<Box<dyn LccTransport>>>` to callers (`query_snip`, `query_pip`)
  that haven't been ported to the broadcast model yet.  They hold the mutex for
  their entire receive loop, starving the `listener_loop` for seconds at a time.
* **snip.rs / pip.rs poll the raw transport** — they use `transport.receive()`
  in a loop, which conflicts with the dispatcher's continuous polling on the
  same mutex.

---

## Proposed Architecture: Transport Actor

Replace the shared-mutex transport with a dedicated actor task that is the
**sole owner** of the transport.  All communication with the actor goes through
channels.

```
┌─────────────────────────────────────────────────────────┐
│  TransportActor (single tokio task, owns transport)     │
│                                                         │
│   recv loop ──► broadcast::Sender<GridConnectFrame>     │
│                                                         │
│   mpsc::Receiver<GridConnectFrame> ──► transport.send() │
└─────────────────────────────────────────────────────────┘
          ▲                        │
          │ subscribe()            │ tx.send(frame)
          │                        ▼
   broadcast::Receiver      mpsc::Sender<GridConnectFrame>
          │                        │
          └──────── callers ───────┘
               (read_memory, read_cdi,
                write_memory, snip, pip, …)
```

### Public API

```rust
/// Cheap clone — both fields are Arc-backed.
#[derive(Clone)]
pub struct TransportHandle {
    /// Send a frame to the network.
    tx: mpsc::Sender<GridConnectFrame>,
    /// Subscribe to all inbound frames.
    all_rx: broadcast::Sender<GridConnectFrame>,   // keep Sender for .subscribe()
}

impl TransportHandle {
    pub async fn send(&self, frame: &GridConnectFrame) -> Result<()>;
    pub fn subscribe(&self) -> broadcast::Receiver<GridConnectFrame>;
}
```

### Actor Task

```rust
async fn transport_actor(
    mut transport: Box<dyn LccTransport>,
    mut cmd_rx: mpsc::Receiver<GridConnectFrame>,
    all_tx: broadcast::Sender<GridConnectFrame>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            // Outbound: drain the send queue first (priority).
            frame = cmd_rx.recv() => {
                match frame {
                    Some(f) => { let _ = transport.send(&f).await; }
                    None    => break,   // sender dropped → shutdown
                }
            }
            // Inbound: poll with a short timeout so select! stays responsive.
            _ = transport.receive(1) => {
                if let Ok(Some(frame)) = transport.receive(0).await {
                    let _ = all_tx.send(frame);
                }
            }
            // Graceful shutdown signal.
            _ = &mut shutdown_rx => break,
        }
    }
    let _ = transport.close().await;
}
```

> **Note on `tokio::select!` with receive:** the 1 ms `receive()` arm in
> `select!` needs care — `BufReader::read_line` is not cancellation-safe (see
> the existing comment in `tcp.rs`).  The cleanest solution is to split the TCP
> stream into a dedicated read-half task that feeds a `mpsc` channel, and have
> the actor only do writes.  See "Variant B" below.

### Variant A — Single Actor, Non-Cancellation-Safe Receive

Works correctly for transports where `receive()` is cancellation-safe (e.g.
serial with a local accumulation buffer).  For TCP, `receive()` must be called
with a 0 ms timeout after a successful `tokio::select!` poll to avoid
re-entering a cancelled future.

### Variant B — Split Read/Write Actor (Recommended for TCP)

```
TcpStream::into_split()
    ├── ReadHalf  → read_task (loops on read_line) → broadcast::Sender
    └── WriteHalf → write_task (loops on mpsc::Receiver) → WriteHalf.write_all
```

No `select!` cancellation issue.  Graceful shutdown: drop the `mpsc::Sender`
(write_task exits) then send a oneshot to read_task (or close the socket, which
causes `read_line` to return 0 bytes).

---

## Migration Plan

### Phase 1 — Remove the direct transport path (in progress)

Already done as of this session:
- `with_transport()` constructor kept for tests only (uses `MockTransport`
  which is cancellation-safe).
- All public methods made to require a dispatcher; return
  `Err("No dispatcher available")` instead of falling back to the transport.
- `_direct` / `_impl` static helpers removed (dead code).
- `transport()` accessor removed from `MessageDispatcher`.

Still needed after Phase 1:
- `query_snip` / `query_pip` in `discovery.rs` still extract `transport()`.
  These must be ported to the broadcast-channel pattern (subscribe → send →
  receive from channel) before `transport()` can be removed.

### Phase 2 — Port snip.rs and pip.rs to dispatcher channels

`query_snip` and `query_pip` currently take `&mut dyn LccTransport`.  Replace
with a `TransportHandle` (or `Arc<MessageDispatcher>`) parameter and rewrite
their receive loops to use `broadcast::Receiver`.

The silence-detection pattern (`SILENCE_TIMEOUT = 100ms`) translates cleanly:
```rust
loop {
    match tokio::time::timeout(silence_timeout, rx.recv()).await {
        Ok(Ok(msg)) => { /* process frame */ }
        _           => break,  // silence or channel closed
    }
}
```

### Phase 3 — Introduce TransportActor / TransportHandle

Replace `MessageDispatcher` with the actor design above (Variant B for TCP,
Variant A for serial).  `TransportHandle` is `Clone`, so it can be passed
directly to `query_snip`, `query_pip`, and every protocol operation without
`Arc<Mutex<…>>` wrapping.

Remove:
- `MessageDispatcher` struct
- `TapTransport` wrapper
- `Arc<Mutex<Box<dyn LccTransport>>>` everywhere
- `LccConnection::transport` field

Keep:
- `LccConnection::with_transport()` (test-only, wraps `MockTransport` in a
  minimal `TransportHandle`)

### Phase 4 — Alias map and responders

The alias map (AMD/AMR tracking) and the background responder tasks
(VerifyNodeGlobal, SNIP, PIP, alias-conflict) currently live inside
`MessageDispatcher`.  Move them to `LccConnection`, subscribing via
`TransportHandle::subscribe()`.

---

## Files Affected

| File | Change |
|------|--------|
| `lcc-rs/src/dispatcher.rs` | Replace with `TransportActor` + `TransportHandle` (Phase 3) |
| `lcc-rs/src/discovery.rs` | Remove direct path (Phase 1, done); port snip/pip (Phase 2); use `TransportHandle` (Phase 3) |
| `lcc-rs/src/snip.rs` | Port to channel-based receive (Phase 2) |
| `lcc-rs/src/pip.rs` | Port to channel-based receive (Phase 2) |
| `lcc-rs/src/transport/tcp.rs` | Split into read-half / write-half in actor (Phase 3) |

---

## Test Strategy

* `with_transport(MockTransport)` still compiles and runs all existing unit tests
  throughout all phases — no test rewrite needed until Phase 3.
* Phase 3 adds a `MockTransportHandle` that wraps a `MockTransport` behind
  channels for dispatcher-mode tests.
