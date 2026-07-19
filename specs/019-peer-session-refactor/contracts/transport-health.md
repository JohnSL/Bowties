# Contract: `TransportHealth` on `tokio::sync::watch`

**Module**: `lcc-rs::transport_actor`
**Governing ADR**: ADR-0017 (Transport Health broadcast + bounded FIFO writer)
**Consumers (S1)**: `TransportHandle::send()` (fail-fast).
**Consumers (S2+)**: `PeerSession` (translates into `PeerError::TransportUnhealthy`);
Tauri connection command (logs initially, connection-status UI surface once wired).

---

## Enum shape

```rust
pub enum TransportHealth {
    Healthy,
    Degraded { reason: String },   // reserved; not emitted in S1
    Wedged   { reason: String },
}
```

`Degraded` is reserved for a later slice that surfaces high-latency-but-not-wedged
conditions (e.g. sustained > 50 % of `SEND_TIMEOUT` used per send). The S1 writer
publishes only `Healthy` and `Wedged`.

---

## Channel primitive

`tokio::sync::watch<TransportHealth>` — one channel per `TransportActor`, initial
value `Healthy`. Chosen over `broadcast` because:

- **Deduplication for free**. `send_if_modified` treats `Healthy → Healthy` and
  `Wedged{r} → Wedged{r}` as no-ops. Subscribers wake only on real transitions.
- **Late subscribers see the current state instantly**. `rx.borrow()` returns
  the latest published value with no separate atomic snapshot to keep in sync
  with the channel.
- **Exactly one seam owner**. The writer task is the only publisher; the
  `send_if_modified` closure is the only place the state machine lives.

`Sender` is held by `TransportHandle::health_tx: Option<watch::Sender<TransportHealth>>`
(`None` only on the legacy `from_parts` bridge path). Every handle produced by
`TransportActor::new` carries `Some(sender)`.

---

## Emission rules (writer task)

Every writer-holding send path (`writer_loop` and `TransportHandle::send_direct`)
routes through the shared helper:

```rust
async fn send_frame_with_timeout(
    writer: &mut dyn TransportWriter,
    frame: &GridConnectFrame,
    health_tx: Option<&watch::Sender<TransportHealth>>,
) -> Result<()>
```

Per-transport-kind timeout via `TransportWriter::send_timeout()`:

- `SERIAL_SEND_TIMEOUT = Duration::from_millis(500)` (default for the trait —
  used by serial and mock transports).
- `TCP_SEND_TIMEOUT = Duration::from_millis(2000)` (override on
  `TcpTransportWriter`).

Transitions:

1. **Success within timeout** → publish `Healthy` via `send_if_modified` (dedup
   makes repeat `Healthy` a no-op).
2. **Timeout** → publish `Wedged { reason: format!("send timeout after {ms}ms: {frame}") }`
   via `send_if_modified`, drop the current frame, return
   `Err(Error::TransportUnhealthy(reason))`. The writer_loop absorbs
   `TransportUnhealthy` and continues draining — the underlying transport
   may recover on the next frame.
3. **Non-timeout error** (`Err` from the underlying `writer.send`) → return
   the error unchanged; health is not modified.

---

## Subscription API

```rust
impl TransportHandle {
    /// Returns `None` only on the legacy `from_parts` bridge path.
    pub fn subscribe_health(&self) -> Option<watch::Receiver<TransportHealth>>;
}
```

Consumer patterns:

- **Read current state**: `let health = rx.borrow().clone();` (or
  `rx.borrow_and_update()` if the caller wants to consume any pending
  transition first).
- **Await next transition**: `rx.changed().await?;` then
  `rx.borrow_and_update()`.
- **Cheap-to-clone**: `watch::Receiver` clones observe the same underlying
  state; every subscription starts from the current value.

---

## Consumer contracts

### `TransportHandle::send()` (S1 — fail-fast, unconditional)

Short-circuits with `Err(Error::TransportUnhealthy(reason))` when
`*health_tx.borrow()` is `Wedged`. No frame is enqueued onto the mpsc.
Fail-fast is unconditional so every current and future outbound caller
inherits the guarantee without opt-in.

Rationale: enqueuing more frames onto a wedged writer only fills the mpsc
capacity (64) and makes later `send()` calls wait for capacity that will
never be freed. Fast failure gives the caller a decision point.

### `TransportHandle::send_direct()` (D2 outcome from the S1 slice)

Routes its inline `writer.send(&frame).await` through the same
`send_frame_with_timeout` helper. A stall on the direct path publishes
`Wedged` and returns `TransportUnhealthy` — the seam sees both paths as one
publisher.

### `PeerSession` (planned S2)

Subscribes on construction. If the observed health is `Wedged` when the
session tries to send, the send is short-circuited to
`PeerError::TransportUnhealthy { health }`. The `Error::TransportUnhealthy`
from the transport layer maps 1:1 to `PeerError::TransportUnhealthy` — that
mapping is authored in ADR-0016.

### Tauri connection command (planned S2+)

Subscribes on connection establishment. Logs every transition. A follow-up
slice (S2–S6) surfaces `Wedged` as a connection-status indicator in the UI.

---

## Fairness (out of scope for this seam)

The single-shared-FIFO writer is deliberate — the wire is physically
single-lane and per-frame `SEND_TIMEOUT` bounds each frame's head-of-line
blocking cost. Per-peer output prioritization is an explicit non-goal at the
transport layer. If fairness ever becomes required, the correct seam is a
scheduling policy inside `PeerSessionRegistry` (S2+), not a redesign of the
transport writer. See ADR-0017 Fairness section.

---

## Tests

Located in `lcc-rs/src/transport_actor.rs` `#[cfg(test)] mod tests`:

1. **`writer_bounded_send_emits_wedged_on_stall`** — the parametric-stall
   `MockTransportWriter` never returns from `send()`. Assert `Wedged`
   published on the watch channel within `SERIAL_SEND_TIMEOUT + slack`
   (300ms). Assert concurrent `subscribe_all()` and a second
   `subscribe_health()` do not deadlock.
2. **`writer_returns_to_healthy_after_stall_clears`** — the mock stalls
   once, then the test clears the stall and drives another frame through
   `send_direct`. Assert `Wedged → Healthy` transition; assert
   `send_if_modified` dedups so `Healthy` is published exactly once.
3. **`send_short_circuits_with_transport_unhealthy_when_wedged`** —
   `TransportHandle::send()` returns `Err(Error::TransportUnhealthy)` in
   <50ms when health is `Wedged`, without enqueuing.
4. **`send_direct_wedges_and_returns_transport_unhealthy_on_stall`** — D2
   assertion: `send_direct` participates in the seam via the shared helper.

Shared fixture: `MockTransport::stall_handle() -> Arc<AtomicBool>` gives
tests parametric control over the writer's `send()` — toggling it to `true`
makes every subsequent `send()` await forever via `std::future::pending()`.
