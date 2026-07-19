# ADR-0017: Transport Health broadcast + bounded FIFO writer

Status: accepted
Date: 2026-07-07
Related: ADR-0016 (Per-peer session actor ownership — S2 consumer of the Transport Health seam this ADR introduces; ADR-0016 authored 2026-07-07)

## Context

Prior to spec 019, `lcc-rs::transport_actor` had two outbound paths that both
serialized on the same `Arc<Mutex<Box<dyn TransportWriter>>>`:

- `writer_loop` — drained a `mpsc::Receiver<GridConnectFrame>` (`OUTBOUND_CAPACITY = 64`);
- `TransportHandle::send_direct` — locked the writer inline (`Arc<Mutex<…>>` across `.await`)
  to eliminate the ~13 ms Windows tokio writer-task-wakeup latency on the CDI-download
  hot loop.

Neither path had any bound on how long `writer.send(&frame).await` could take.
On a healthy connection this never mattered; on a wedged one (unplugged SPROG,
serial-port stall, kernel-level buffer exhaustion, adapter that stopped ACKing)
the awaited `send` never returned. Every subsequent caller — CDI download,
SNIP/PIP query, config read, discovery — either enqueued into a mpsc that would
never drain (fills to 64, then `send()` awaits capacity forever), or grabbed
the writer mutex behind the stalled `send_direct` future. Result: the entire
protocol layer went silently unresponsive with no visible symptom other than
"commands hang forever." The Tauri command layer had no way to distinguish
"the peer is quiet" from "the wire is stuck," so it could not fail gracefully,
retry, or surface a connection-status change to the user.

This is the load-bearing regression class the peer-session refactor (spec 019)
must resolve *before* any per-peer actor plumbing lands: every downstream slice
assumes that the transport writer will eventually make progress — a per-peer
session that never gets its enqueued frame drained is functionally equivalent
to no session at all, and the session-level `Cancel` / `TerminateDueToError`
contracts (S3+, TN-9.7.2.1) become dead code if the underlying wire is silently
stuck.

## Decision

Introduce a **Transport Health seam** owned by the transport writer task, and
bind every writer-holding send path to a per-transport-kind timeout so that a
stuck writer surfaces as a bounded, observable state change instead of an
unbounded hang.

### Shape

1. **`TransportHealth` enum** in `lcc-rs::transport_actor`:

   ```rust
   pub enum TransportHealth {
       Healthy,
       Degraded { reason: String },   // reserved; not emitted in S1
       Wedged   { reason: String },
   }
   ```

2. **Per-transport-kind timeout constants** (module-level, in `transport_actor`):

   ```rust
   pub const SERIAL_SEND_TIMEOUT: Duration = Duration::from_millis(500);
   pub const TCP_SEND_TIMEOUT:    Duration = Duration::from_millis(2000);
   ```

   Exposed via a new trait method:

   ```rust
   trait TransportWriter {
       fn send_timeout(&self) -> Duration { SERIAL_SEND_TIMEOUT }  // default
   }
   impl TransportWriter for TcpTransportWriter {
       fn send_timeout(&self) -> Duration { TCP_SEND_TIMEOUT }
   }
   ```

   Rationale for the two values: 500ms serial reflects real-world SPROG USB-LCC
   latency envelopes with margin; 2000ms TCP reflects kernel-level buffering
   and socket retry. TCP being 4× more forgiving is deliberate — a serial wedge
   is almost always a physical-cable event, a TCP wedge can be transient network
   congestion.

3. **`tokio::sync::watch<TransportHealth>` as the seam channel** (initial value
   `Healthy`). The writer task publishes via `send_if_modified` so:

   - Repeat `Healthy` after `Healthy` is a no-op — subscribers don't wake on
     non-transitions.
   - Late subscribers get the current state instantly via `rx.borrow()` — no
     `resubscribe` dance, no parallel `AtomicUsize` snapshot to keep in sync.

   Exposed via `TransportHandle::subscribe_health() -> Option<watch::Receiver<TransportHealth>>`.
   The `Option` is `None` only on the legacy `TransportHandle::from_parts` bridge
   path (which does not publish health); every handle produced by
   `TransportActor::new` returns `Some(receiver)`.

4. **Shared publication helper** `send_frame_with_timeout(writer, frame, health_tx)`
   at module scope. Both `writer_loop` (mpsc drain) and `TransportHandle::send_direct`
   route their `writer.send(&frame).await` calls through this helper. It:

   - Wraps the send in `tokio::time::timeout(writer.send_timeout(), …)`.
   - On success, publishes `Healthy` (dedup by `send_if_modified`).
   - On timeout, publishes `Wedged { reason: "send timeout after Nms: <frame>" }`
     and returns `Error::TransportUnhealthy(reason)` so the caller knows to
     abandon the frame.
   - On any other `Err`, returns that error unchanged (health is unchanged).

   `writer_loop`'s error handling: `Ok` → echo to inbound broadcast; `Err(TransportUnhealthy)`
   → drop the frame and continue draining (the underlying transport may recover);
   any other `Err` → break the writer loop (the transport itself has failed).

5. **Fail-fast at `TransportHandle::send()`**. When the observed health is
   `Wedged`, `send()` returns `Err(Error::TransportUnhealthy(reason))` **without
   enqueuing**. This is unconditional so every current and future outbound
   caller inherits the guarantee — no `if self.is_healthy() { … }` opt-in dance,
   no separate `try_send`. Callers that want to keep going despite a wedge can
   catch `TransportUnhealthy` and decide; callers that were only ever going to
   hang on a full mpsc get the fast failure they wanted.

6. **`send_direct` participates in the seam**. In the S1→S5 window `send_direct`
   still exists (retires in S5 per the spec 019 roadmap). Because it calls the
   same `send_frame_with_timeout` helper as the writer_loop, a stall on the
   direct path also publishes `Wedged` and returns `TransportUnhealthy`. The
   helper survives `send_direct`'s retirement in S5 — `writer_loop` remains one
   caller and any future writer-holding path is required (see the Per-slice
   plumbing rule in `aiwiki/seams.md`) to route through it.

### Fairness (explicitly not a transport-layer concern)

The transport writer is deliberately a **single-shared-FIFO**. The wire is
physically single-lane (SPROG USB-LCC, a TCP hub, an SLCAN adapter — all
serialise frames on the underlying byte stream), so any "fairness" between
peers is a scheduling question about *which* frame gets enqueued next, not
about how the writer drains. Per-frame `SEND_TIMEOUT` bounds the wire time any
single frame can consume, so a slow peer's frame cannot starve every other
peer's frame for more than the timeout window.

If future work exposes a fairness need — a chatty peer starving a slow peer, a
priority tier for connection-establishment traffic vs. bulk CDI reads — the
correct seam is a **scheduling policy inside `PeerSessionRegistry`** (introduced
in S2 by ADR-0016) that decides which peer's session gets to enqueue next. The
transport writer stays FIFO. This is called out here so that a future reader
looking at "how do we make the writer fair" reaches the right answer
(reorder before you enqueue, don't teach the wire to prioritise) instead of
proposing a per-peer output queue at the transport layer.

## Consequences

### What downstream slices inherit

- **S2 (`PeerSession` + `PeerSessionRegistry`)**: sessions subscribe on
  construction via `TransportHandle::subscribe_health()`; session commands
  short-circuit to `PeerError::TransportUnhealthy` when the observed state is
  `Wedged`. `Error::TransportUnhealthy` from the transport layer maps 1:1
  to `PeerError::TransportUnhealthy` — that mapping is authored in ADR-0016.
- **S3+ (CDI, config r/w)**: every outbound frame now has a bounded latency
  budget, so per-exchange timeouts (`TerminateDueToError`, `OptionalInteractionRejected`)
  can be reasoned about — the writer will not block their expiry.
- **UI (planned S2–S6)**: a `Wedged` health event becomes the connection-status
  surface signal ("Connection is unresponsive — is the cable connected?"),
  replacing today's "commands hang" symptom.

### Breaking / near-breaking changes

- New public `Error::TransportUnhealthy(String)` variant. Callers that
  `match err { Error::Transport(_) => …, _ => panic!() }` must handle the new
  variant. Search sweep at merge time: no non-test call sites in the current
  codebase.
- `TransportWriter` trait gains a defaulted `send_timeout()` method. All
  existing impls (mock, serial, TCP) compile unchanged.
- `TransportHandle::send()` no longer unconditionally enqueues; a `Wedged`
  state causes a fast failure. This is the intended behaviour change (fail
  fast beats hang forever), but consumers that assumed "if `send()` returns
  `Ok(())` the frame is on the wire" got the same treatment before — `Ok(())`
  from `send()` has always meant "enqueued to mpsc," not "wire has ACKed."

### Rejected alternatives

- **Broadcast + `current_health() -> AtomicHealth`**: the earlier draft in
  `specs/019-peer-session-refactor/contracts/transport-health.md` (pre-rewrite).
  Rejected because it needs a parallel atomic to give late subscribers the
  current value — a second source of truth for the same fact — while `watch`
  gives that for free via `borrow()`.
- **Per-caller opt-in**: `TransportHandle::send()` continues to enqueue
  unconditionally; peer sessions check health themselves. Rejected because
  every current and future outbound caller has to remember to opt in, and
  the wedge-plus-full-mpsc "silent hang" bug class comes back as soon as
  one caller forgets.
- **Retire `send_direct` in S1**: keeps the surface smaller. Rejected because
  the CDI-download 13ms scheduler-hop cost that motivated `send_direct` in
  the first place is real, and no S1 acceptance criterion requires the retirement.
  Deferred to S5 per the spec 019 roadmap so CDI performance regressions and
  bypass-audit work are contained to their own slice.
- **Per-peer output queue at the transport layer**: see Fairness above.
  Rejected because the wire is physically single-lane and per-frame
  `SEND_TIMEOUT` already bounds head-of-line blocking. Future fairness work
  belongs in `PeerSessionRegistry`.

## Test coverage

Behavior contracts pinned in `lcc-rs/src/transport_actor.rs` `#[cfg(test)] mod tests`:

- `writer_bounded_send_emits_wedged_on_stall` — writer task publishes `Wedged`
  within `SERIAL_SEND_TIMEOUT + slack` when the writer stalls; concurrent
  `subscribe_all()` and `subscribe_health()` don't deadlock.
- `writer_returns_to_healthy_after_stall_clears` — clearing the stall and
  pushing another frame publishes `Healthy` exactly once (`send_if_modified` dedup).
- `send_short_circuits_with_transport_unhealthy_when_wedged` —
  `TransportHandle::send()` returns `Err(Error::TransportUnhealthy)` in <50ms
  when health is `Wedged`, without enqueuing.
- `send_direct_wedges_and_returns_transport_unhealthy_on_stall` — D2 assertion:
  `send_direct` participates in the seam via the shared helper.

The parametric stall variant on `MockTransportWriter` (`MockTransport::stall_handle() -> Arc<AtomicBool>`)
is the shared test fixture; regression tests for wedge-adjacent bugs reuse it.

## 2026-07-18 extension: transport-model ownership for the serial path (spec 019 S9)

Context: during the SPROG USB-LCC debugging, `gridconnect_serial` was rewritten
from an async transport to **blocking OS reader/writer threads** bridged to
tokio via mpsc. That left two mechanisms that both appear to guard against a
stuck serial writer — this ADR's per-send `SERIAL_SEND_TIMEOUT` at the actor
layer, and the blocking-thread rewrite at the wire layer — and their necessity
relative to each other was never recorded. S9 resolves the ownership.

### Decision (S9 D1 = A)

**Keep the blocking wire layer; the two layers are complementary owners, not
redundant.** `transport_actor` owns async coordination (single-shared-FIFO
ordering + the `TransportHealth` broadcast); `gridconnect_serial` owns wire-byte
I/O + its OS-level write timeout, running the serial syscall off the tokio
executor. Code is unchanged by this decision — S9 is documentation-only.

Rationale, principle at stake = **Depth / structural-safety-over-runtime-assumption**:
the blocking-thread model is retained for **executor isolation** (a stuck
`write_all`/`flush` or `read` can never starve a tokio worker thread, on every
platform), *not* for the earlier asserted-but-unproven "Windows async I/O is
unreliable" claim. The SPROG CDI failure that motivated the rewrite was a serial
`\r\n` framing bug (fixed independently; see ADR-0018 and the 2026-07-18 session
handoff), so the transport model was never the SPROG root cause. The blocking
model is now hardware-validated (cdi-probe 10/10 CDI at 0 ms pacing).

Rejected alternative (S9 D1 = B): revert to async `tokio-serial` and collapse to
`SERIAL_SEND_TIMEOUT` as the single per-write guard. Rejected because (1) it
trades the structural executor-isolation guarantee for a runtime assumption
about `tokio-serial`'s Windows readiness path; (2) it re-opens a merge-gate
hardware-revalidation requirement (the SPROG 10/10 would need re-proving on
async) that the current session cannot discharge; and (3) it does not even
consolidate to one serial transport — `slcan_serial` still uses
`tokio_serial::SerialStream`, so `tokio-serial` stays a dependency either way.

### The serial-path timeout split (the fact this ADR must record)

On the serial path this ADR's `SERIAL_SEND_TIMEOUT` (500 ms) bounds the
**enqueue** onto the writer mpsc (`WRITER_CHANNEL_CAPACITY`), *not* the wire
write — for `gridconnect_serial`, `TransportWriter::send()` is `wire_tx.send().await`.
It therefore fires only once the queue backs up behind a stuck writer thread
(never during lockstep CDI, where there are never `WRITER_CHANNEL_CAPACITY`
outstanding frames). The **per-write** bound is platform-specific:

| Platform | `write()` bound | `flush()` bound | Real per-write guard |
|---|---|---|---|
| Windows | `WriteFile` w/ `fOutxCtsFlow` blocks up to DCB `WriteTotalTimeoutConstant` — patched 10 ms → **5000 ms** (`fix_write_timeout`) | ~no-op | OS 5000 ms DCB timeout |
| macOS / Linux | `poll(POLLOUT, timeout=10 ms)` then raw `write()`; returns on kernel-buffer accept | `tcdrain()` — blocks until physical transmission, **no effective timeout** | **none** — relies on the actor enqueue-fill `Wedged` backstop |

The `Wedged` health signal is uniform across platforms: a stalled writer thread
lets the mpsc fill, the actor's `writer.send().await` blocks, and
`send_frame_with_timeout` publishes `Wedged`. So the *observable-wedge* and
*executor-isolation* guarantees are platform-independent; only the *per-write*
OS bound differs.

### Known cross-platform gap (tracked, not fixed in S9)

`writer_thread` breaks on any write/flush error, including a transient
`TimedOut`. On macOS/Linux a sustained CTS back-pressure makes `flush()`'s
`tcdrain()` block unbounded, and a >10 ms kernel-buffer-full stall on `write()`
would exit the writer thread (dropping the port) with no recovery — a narrow,
currently-hypothetical scenario (small ~18-byte frames rarely fill the kernel
buffer) that S1's enqueue-fill `Wedged` already backstops for observability.
Making the writer thread resilient to a transient `TimedOut` and/or giving the
Unix path a per-write timeout is deferred to a `kind/idea` follow-up, to be
implemented and validated on Mac/Linux hardware rather than written blind on
Windows against the hardware-validated path.

