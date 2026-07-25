# ADR-0016: Per-peer session actor ownership

Status: accepted
Date: 2026-07-07
Related: ADR-0017 (Transport Health broadcast + bounded FIFO writer — this ADR consumes the seam ADR-0017 established). Reserved before ADR-0017 was authored; the two-ADR pair jointly closes the wedge-under-back-pressure + scattered-exchange-ownership regression class.

## Context

Prior to feature 019 (peer-session-refactor), every LCC/OpenLCB protocol
interaction with a remote node was executed by a stateless free function
in `lcc-rs`:

- `query_snip` / `query_pip` — each grabbed the shared transport writer,
  subscribed to the inbound broadcast, timed out on its own deadline.
- `read_cdi_cancellable_with_stats` / `read_memory_timed` / `write_memory_timed` —
  same pattern via `datagram_read_exchange`.
- `LiveNodeProxy` layered a second concern on top: `snip_waiters` /
  `pip_waiters` for coalescing, plus cached SNIP/PIP wire state.
- `CdiInflightRegistry` in `bowties-core` layered a third: a runtime-defended
  "one CDI download per node at a time" invariant, needed because nothing
  structurally prevented two callers from starting two exchanges concurrently.

The consequences were load-bearing:

- The SPROG CDI regression that motivated feature 019 was not a single bug;
  it was the shape. When our timeout fired, no owner was responsible for
  emitting `TerminateDueToError` to the peer (TN-9.7.2.1), so the peer's
  exchange state persisted and a subsequent datagram triggered
  `DatagramRejected 0x2020` storms. Fixing the immediate symptom would have
  required adding cleanup emission at ~five call sites — each with a
  different piece of exchange state to encode.
- `OptionalInteractionRejected` (TN-9.7.3.2 §3.4) was silently coalesced
  with peer timeouts because no owner classified it as terminal.
- Ownership of "we are currently the ACK-receiver for peer P" was diffuse
  across whatever coroutine happened to be running; interleaved exchanges
  to the same peer could and did clobber each other's ACK state.
- Adding new protocol behaviour (event routing consolidation in S6, a
  future guided-configuration flow, connection health surfaces for the UI)
  required threading through five module boundaries that each held one
  fragment of the peer state machine.

The refactor consolidates every protocol interaction with a single remote
NodeID inside one owner — a per-peer actor task in `lcc-rs`. Every
downstream protocol slice (SNIP+PIP in S2, CDI in S3, config r/w in S4,
event routing in S6) becomes a variant on the same actor pattern rather
than a new file grabbing the transport.

## Decision

Introduce a **per-peer session actor pattern** owned by `lcc-rs`.

### Shape

1. **`PeerSession` — one actor task per remote NodeID.** Runs on a
   dedicated tokio task spawned by `PeerSessionRegistry`. Owns every
   protocol interaction with the peer: exchange state, ACK obligations
   (in both directions), retry accounting, cached SNIP/PIP, coalescing,
   and — from S3 onward — the peer-cleanup contract. Terminates when its
   `commands` mpsc closes or the transport disconnects.

2. **`PeerCommand` — the actor's typed inbox.** Each command variant
   carries a response `oneshot::Sender` so callers can await results.
   Variants added in vertical slices:

   - S2: `QuerySNIP`, `QueryPIP`, `PeerReinitialised`, `AliasChanged`, `Cancel`.
   - S3: `DownloadCDI` (with a `broadcast::Sender<CdiProgress>` for streaming).
   - S4: `ReadConfig`, `WriteConfig`.

3. **`ActiveExchange` — at most one in flight per peer.** An internal enum
   representing the current in-flight exchange. Callers that arrive while
   `active.is_some()` queue behind the current exchange in the `commands`
   mpsc; per-peer serialization is a structural property of the actor,
   not a runtime check. Cross-peer parallelism is preserved (different
   peers run on different tasks).

4. **`PeerSessionHandle` — cheaply cloneable dispatch surface.** Wraps
   `mpsc::Sender<PeerCommand>` + `NodeID` (for logging). `Clone + Send + Sync`.
   Typed convenience methods (`query_snip`, `query_pip`, `download_cdi`,
   `read_config`, `write_config`, `cancel`) construct the appropriate
   `PeerCommand` variant + oneshot and await the reply.

5. **`PeerSessionRegistry` — sole spawner.** Owns
   `tokio::sync::RwLock<HashMap<NodeID, PeerSessionHandle>>` (research D1;
   sole-writer / many-reader pattern, guarded against the
   tokio-rwlock-self-deadlock class per user memory). Runs a spawn-watcher
   task subscribed to the transport inbound broadcast that calls
   `spawn(node_id, alias)` exactly on the three NodeID-carrying MTIs:

   - `Verified Node ID Number` — MTI `0x0170`.
   - `Initialization Complete` — MTI `0x0100`.
   - `Alias Map Definition` (AMD) — MTI `0x0701`.

   Frames that carry only an alias (PCER, `EventReportWithPayload`,
   addressed exchange frames) never trigger session creation. Repeat
   observations of the same NodeID dispatch `PeerCommand::AliasChanged`
   to the existing session — **never a re-spawn**. The registry map's
   read side clones a `PeerSessionHandle` and drops the guard before any
   `.await` on the returned handle.

6. **`PeerError` — the actor's typed output.** Serde-serialised with
   stable string prefixes (FR-018 — no existing frontend tag broken):
   `Timeout`, `PeerReinitialised`, `AliasChanged`, `TransportUnhealthy`,
   `Rejected` (`DatagramRejected` permanent or `OptionalInteractionRejected`),
   `Cancelled`, `NotSupported`, `Protocol`. `TransportUnhealthy` maps 1:1
   from `lcc-rs::transport_actor::Error::TransportUnhealthy` per ADR-0017.

### D1 outcome — Transport Health wiring depth (mid-exchange)

`PeerSession` subscribes to `TransportHandle::subscribe_health()` on
construction and `tokio::select!`s on `health.changed()` inside the
active-exchange loop, alongside `commands.recv()`, filtered `inbound.recv()`,
and the exchange deadline. On a `Wedged` transition mid-exchange:

- The session aborts the active exchange with `PeerError::TransportUnhealthy { health }`.
- The session **skips** the outbound `TerminateDueToError` cleanup
  emission — peer cleanup is a live-wire obligation (our-fault termination
  on a wire the peer can still receive on); when the wire is confirmed
  dead, we cannot honour the obligation and must not enqueue a frame that
  will hit the fail-fast in `TransportHandle::send()`.
- Command-entry `health.borrow()` check is retained as the fast-path for
  dispatch onto an already-wedged wire (short-circuit before entering the
  `ActiveExchange` state machine).

Rejected: command-entry-only wiring (relies on `send()` fail-fast for
outbound and per-exchange timeout for inbound wait — leaves 2–8 s hangs
in production for SNIP (5 s), PIP (2 s), and future CDI (8 s) despite
the transport publishing `Wedged` in ≤500 ms on a subscribed channel).

### D2 outcome — `LiveNodeProxy` session dispatch (fetch-per-call)

`LiveNodeProxy` holds `sessions: Arc<PeerSessionRegistry>` — never a
cached `PeerSessionHandle`. Every protocol handler (`query_snip`,
`query_pip`, future S3+ handlers) calls `self.sessions.get(self.node_id).await`
and dispatches on `Some(handle)`, or returns `PeerError::NotConnected`
on `None`. The `RwLock::read()` guard is dropped before any `.await` on
the returned handle (research D1 discipline).

Rejected: hold `Option<PeerSessionHandle>` filled in by `NodeRegistry`
after registry spawn (introduces a construction-window race between the
two broadcast subscribers of the same VNI/InitComplete/AMD frame; the
`Option<None>` state during that window IS the "cannot reach protocol"
state the option purports to eliminate).

### D3 outcome — Broadcast `Lagged(n)` policy (abort exchange, preserve caches)

> **Superseded for CDI by the 2026-07-18 extension (S7).** D3 assumed the
> lag itself was acceptable-and-rare on a shared `subscribe_all` broadcast.
> Under concurrent 7-peer CDI it was neither: fan-out amplification made lag
> reachable mid-download, and the abort skipped peer cleanup. S7 moves
> inbound delivery to per-peer routing and replaces "abort on lag" with
> "bounded in-place recovery, then clean cleanup" for CDI. SNIP/PIP retain
> the abort-and-continue policy described below.

On `broadcast::error::RecvError::Lagged(n)` from the session's inbound
subscription, the session:

- Aborts any `ActiveExchange` with
  `PeerError::Protocol("inbound broadcast lagged: dropped {n} frames")`.
  Every caller parked on the exchange (including coalesced waiters)
  receives the error.
- Preserves SNIP/PIP caches (value snapshots with no frame-history
  dependency).
- Continues its `run()` loop. The session task does **not** terminate;
  the registry does not re-spawn.

Rejected:

- Log-and-continue — cannot fill "Regression class prevented" honestly;
  leaves silent partial exchange success expressible, which is the exact
  class this refactor exists to prevent.
- Whole-session termination — stricter than the invariant requires today;
  drops caches unnecessarily and commits ADR-0016 to a contract that no
  present protocol needs. Reserved as a future tightening if S3+ adds
  session-lifetime state that implicitly depends on complete inbound
  observation.

### Peer-cleanup contract (declared here, implemented in S3)

`TerminateDueToError` obligation per TN-9.7.2.1: when the session
terminates an exchange due to **our** failure (timeout, cancellation,
terminal `DatagramRejected`, `OptionalInteractionRejected`), the session
emits `TerminateDueToError` addressed to the peer **exactly once** before
completing the reply. The session tracks whether it has emitted the
terminate to avoid duplicates.

**Live-wire precondition (D1)**: the emission is skipped when the
transport health is `Wedged` at the moment of termination. Peer cleanup
requires a wire the peer can receive on; abandoning the emission is not
a violation of the contract but a recognition that the contract's
predicate has failed.

Declared in this ADR because it is the load-bearing structural cure for
the SPROG CDI regression that motivated feature 019. Wiring lands in
S3 (`ActiveExchange::CdiDownload` + `datagram_reader` OIR-terminal +
`send TerminateDueToError` on our timeout).

## Consequences

### What downstream slices inherit

- **S2 (SNIP + PIP migration)**: `PeerSession` becomes the sole owner of
  SNIP/PIP wire state; `snip_waiters` / `pip_waiters` coalescing moves
  out of `LiveNodeProxy`; existing free functions retained as
  compatibility shims that resolve a handle from the registry and delegate.
- **S3 (CDI migration + `cdi_inflight` retirement)**: "one CDI download
  per node at a time" becomes a structural property of the actor (per-peer
  serialization via the `commands` mpsc + single `ActiveExchange`), not
  a runtime `CdiInflightRegistry`. `TerminateDueToError` wiring lives on
  the actor's exchange terminator. `AppState::cdi_inflight` and
  `AppState::cdi_download_cancel` fields are removed.
- **S4 (config r/w migration)**: `ActiveExchange::ConfigRead` /
  `ActiveExchange::ConfigWrite` extend the same actor pattern; single
  active exchange holds for r/w.
- **S5 (`send_direct` retirement + outbound-owner audit)**: every
  outbound frame is now session-owned; the audit is a workspace-wide
  grep, not a design task.
- **S6 (`event_router` classification lift)**: `EventRouter` becomes a
  bus-scoped peer of `PeerSessionRegistry`, subscribed to the same
  transport broadcast, classifying event-report MTIs and fanning out
  independently. Sessions do not mediate event-report traffic.

### Breaking / near-breaking changes

- **New public modules**: `lcc-rs::peer_session`, `lcc-rs::peer_session_registry`.
- **New public error variants** on `PeerError` (introduced piecewise as
  slices land). Callers that pattern-match exhaustively on the error
  enum must handle the new variants; the frontend uses tagged serde
  strings so the wire format is stable.
- **`LiveNodeProxy` field-shape change** (S2): `snip_waiters` /
  `pip_waiters` removed; a `sessions: Arc<PeerSessionRegistry>` field
  added. No frontend impact — the Tauri command surface is preserved
  per FR-018.
- **`bowties-core::cdi_inflight` retired** (S3): `CdiInflightRegistry`,
  `CdiInflightGuard`, `AppState::cdi_inflight`, `AppState::cdi_download_cancel`
  all deleted. Workspace-wide grep must return zero non-test hits at S3
  exit.
- **`lcc-rs` free-function API retirement** (S5): `query_snip`,
  `query_pip`, `read_cdi_cancellable_with_stats`, `read_memory_timed`,
  `write_memory_timed`, `datagram_read_exchange` retired from the public
  API. The shims retained in S2–S4 are removed in S5.

### Rejected alternatives

- **Extend the free-function pattern with a `PeerContext` struct passed
  everywhere.** Rejected: leaves ownership diffuse (the "who owns ACK
  state for peer P?" question still has no single answer at any given
  time), and does not close the SPROG CDI regression by construction —
  `TerminateDueToError` emission still lives at whatever call site
  happens to detect the timeout.
- **Fold sessions into `LiveNodeProxy` (backend domain layer).**
  Rejected: per-peer protocol behaviour belongs in `lcc-rs` per the
  code-placement rule ("if a rule would matter to other LCC/OpenLCB
  consumers, prefer `lcc-rs`"). Folding into `bowties-core` couples the
  protocol semantics to Bowties app-workflow concerns and forecloses
  future non-Bowties consumers of `lcc-rs` (JMRI-companion, headless
  diagnostic tools).
- **Registry per (NodeID, alias) rather than per NodeID.** Rejected:
  alias renegotiation is a peer-lifetime event, not a peer-identity
  event. Two sessions for the same peer with different aliases would
  make the "single ACK owner" invariant impossible to state. In-place
  `AliasChanged` update preserves the identity/lifetime distinction.
- **Sole spawn on `AMD` only** (rather than the three-frame set).
  Rejected: initialisation-time peers announce via `Verified Node ID
  Number` before their first `AMD`; delaying spawn to first AMD would
  either drop those peers (if no AMD is emitted for a session lifetime)
  or open a spawn-window race between the three MTIs. Three-frame
  qualification with `AliasChanged` idempotence covers all observed
  lifecycles.

## Test coverage

Behaviour contracts pin these invariants; details land per vertical
slice:

- **S2** — `lcc-rs/tests/peer_session_snip_pip.rs`: coalescing
  (two concurrent `query_snip` → one outbound SNIP-request frame),
  `PeerReinitialised` cache-clear, `AliasChanged` in-place update
  without re-spawn, `Lagged(n)` aborts active exchange with
  `PeerError::Protocol`, mid-exchange `Wedged` aborts with
  `PeerError::TransportUnhealthy` and does not emit
  `TerminateDueToError`.
- **S3** — session-scoped outbound-frame assertion:
  `TerminateDueToError` emitted exactly once on our timeout with the
  correct destination NodeID; `OptionalInteractionRejected` from peer
  terminates the exchange with `PeerError::Rejected { mti, code }`;
  `DatagramRejected` with resend-OK flag (bit 12) retries up to
  `max_retries` and then emits peer cleanup.
- **S4** — two concurrent `write_config` to the same peer serialise
  (per-peer FIFO); different peers proceed in parallel.
- **S5** — workspace-wide grep for the retired free-function names
  and for `send_direct` / `direct_write_count` / `use_send_direct`
  returns zero non-test hits.

## Invariants

1. **Single owner per peer.** For every discovered NodeID at most one
   `PeerSession` task exists at a time. Audit hint: grep for
   `PeerSessionRegistry::spawn` — sole call site is the spawn-watcher
   task in `peer_session_registry.rs`.
2. **Sole spawner.** `PeerSession` construction happens exclusively
   inside `PeerSessionRegistry::spawn`. Audit hint: grep for
   `PeerSession::new` and `PeerSession::spawn` across the workspace —
   zero call sites outside `peer_session_registry.rs` and its test
   modules (`new_for_test` on `PeerSession` is `#[cfg(test)]` only).
3. **NodeID-carrying qualification.** Session spawn is triggered
   exclusively by MTI `0x0170` (VNI), `0x0100` (InitComplete), and
   `0x0701` (AMD). Audit hint: read the spawn-watcher's match arms;
   no other MTIs appear.
4. **Alias renegotiation without re-spawn.** Repeat observations of a
   known NodeID dispatch `PeerCommand::AliasChanged` — never a fresh
   spawn. Audit hint: session-scoped test in
   `lcc-rs/tests/peer_session_snip_pip.rs` asserts the registry map
   size after two VNI observations of the same NodeID with different
   aliases is 1.
5. **Single active exchange per peer.** `PeerSession.active` is
   `Option<ActiveExchange>`; a second `PeerCommand` variant that would
   start an exchange queues on the `commands` mpsc until the current
   exchange completes. Audit hint: the actor's `run()` loop never
   assigns `self.active = Some(...)` when it is already `Some`.
6. **Single outbound sender per peer.** Every outbound frame from
   `PeerSession` is sent via `self.outbound` (the transport mpsc).
   Audit hint: grep inside `peer_session.rs` for `TransportHandle::send`
   / `send_direct` — zero direct calls; every outbound path goes
   through `self.outbound`.
7. **`TerminateDueToError` exactly once (live wire).** On our-fault
   exchange termination on a live wire, the session emits
   `TerminateDueToError` addressed to the peer exactly once before
   completing the reply. Audit hint: `PeerSession` maintains an
   `emitted_terminate: bool` per `ActiveExchange`; the S3
   session-scoped test asserts frame-count = 1.
8. **`TerminateDueToError` skipped on wedge.** When
   `TransportHealth::Wedged` is observed at exchange termination time,
   the emission is skipped. Audit hint: same S3 test with a `Wedged`
   variant asserts zero terminate frames.
9. **1:1 `TransportUnhealthy` mapping.** `Error::TransportUnhealthy`
   from the transport layer surfaces as `PeerError::TransportUnhealthy`
   at the session boundary — no wrapping loss, no re-classification.
   Audit hint: grep for `Error::TransportUnhealthy` in session code —
   sole handler returns `PeerError::TransportUnhealthy { .. }`.
10. **`LiveNodeProxy` never caches a session handle.** `LiveNodeProxy`
    holds `Arc<PeerSessionRegistry>` and fetches per-call; a
    `PeerSessionHandle` field is not present. Audit hint: grep
    `LiveNodeProxy` for `PeerSessionHandle` — zero field declarations.
11. **Aggregate owns spawn-watcher lifecycle.** `PeerSessionRegistry`
    stores its spawn-watcher `JoinHandle` in a drop-safe slot and
    exposes `pub async fn shutdown(&self)` that clears the sessions
    map, `.take()`s the handle, calls `abort()`, and awaits the
    aborted result. Dropping the last `Arc<PeerSessionRegistry>` does
    NOT rely on channel-close cascades to terminate the watcher
    (which would deadlock, since the watcher's captured
    `Arc<RegistryInner>` transitively holds the same broadcast
    sender it is waiting on to close). Audit hint: grep the registry
    for `_spawn_watcher: JoinHandle` — must not exist as a detached
    field; the field is named `spawn_watcher` and lives inside a
    `Mutex<Option<JoinHandle<()>>>` (or equivalent drop-safe wrapper)
    that `shutdown()` drains. `AppState::disconnect` calls
    `sessions.shutdown().await`, matching the discipline established
    by `TransportActor::shutdown` and `LccConnection::shutdown_responders`.

## 2026-07-14 extension: Aggregate spawn-watcher shutdown contract

**Problem observed.** On Windows, disconnect-then-reconnect against a
serial LCC adapter (SPROG or RR-CirKits Buffer) fails with `COM7:
Access is denied`. The OS-level serial handle stays open past
`AppState::disconnect` because a background task retains an
`Arc<TransportHandle>` refcount indefinitely.

**Root cause.** `PeerSessionRegistry` originally stored its
spawn-watcher as `_spawn_watcher: JoinHandle<()>` (leading-underscore
detached field). Dropping the registry detaches the JoinHandle without
aborting the task. The watcher captures `Arc<RegistryInner>` which
holds `TransportHandle` which holds a clone of the broadcast sender
the watcher waits on. Its only exit condition is
`broadcast::error::RecvError::Closed`, which requires every sender to
drop — including the one the watcher transitively holds. **Self-
referential Arc closure.** The cascade: leaked `TransportHandle` also
holds `watch::Sender<TransportHealth>`, so health-forwarders never
see channel-close, so `PeerSession` command mpscs never close, so
session actors never terminate, so the writer task keeps the OS
serial handle open.

**Contract.** `PeerSessionRegistry` becomes the single owner of every
task it spawns. `shutdown()` is the explicit termination entrypoint:
clear the sessions map, take the spawn-watcher `JoinHandle`, abort +
await it. This matches the pattern already applied to
`TransportActor::shutdown` and `LccConnection::shutdown_responders`
and completes the discipline across the three aggregates in the
connection stack. `AppState::disconnect` calls
`sessions.shutdown().await` in place of `sessions.clear().await`.

**Rejected alternatives** (per architecture-first-fix analysis):
- **`CancellationToken` at the transport root** — broader prevention
  class ("any task-termination signal that doesn't fire in the
  assumed sequence") but introduces a second termination mechanism
  alongside channel-close; both must keep working during the
  transition. Legitimate future direction if the cascade problem
  recurs at other seams; would warrant a new "Connection Teardown"
  ADR then.
- **`Weak<Arc>` audit at every task-spawn site** — narrowest fix;
  prevention is a pattern-audit rule, not a structural guarantee.
  Future contributors adding a new task must remember to use `Weak`;
  the aggregate-owns-shutdown pattern is enforced by the aggregate's
  API surface instead.

**Test coverage.** `lcc-rs/tests/peer_session_registry.rs::registry_shutdown_aborts_spawn_watcher_and_releases_transport`
constructs the registry via `PeerSessionRegistry::new` (production
path — not `new_empty_for_test`), calls `shutdown().await`, drops
outside senders, and asserts a probe `broadcast::Receiver` derived
from the transport returns `RecvError::Closed` within 100ms. Fails
before the fix (detached watcher keeps the sender alive); passes
after.

**Follow-up.** If `NodeRegistry::shutdown_all` ever retains its
`TransportHandle` / `PeerSessionRegistry` fields past the shutdown
call (as of this extension it clears both), the ADR-0016 contract
becomes insufficient on its own and either the audit re-runs or the
Cancellation Token option gets promoted from rejected to accepted.
Governed by `aiwiki/seams.md § Peer Session Ownership`.

## 2026-07-18 extension: Per-peer inbound routing + bounded lag recovery (S7)

**Problem observed.** Under concurrent 7-peer CDI load, ~1/626 exchanges
hit `broadcast::error::RecvError::Lagged(n)` on one session's inbound
subscription and aborted its live CDI download. Two defects compounded:

1. **Fan-out amplification (root cause).** Every `PeerSession` subscribed
   to the shared `TransportHandle::subscribe_all()` broadcast and discarded
   ~6/7 of what it received (frames for other peers). Ring pressure and
   per-frame CPU scaled with peer count, so one session lagged and aborted
   its in-flight exchange. The D3 (2026-07-07) premise — "lag is rare and
   acceptable on the shared bus" — was falsified by evidence.
2. **Silent orphaned exchange.** The lag arm aborted with
   `PeerError::Protocol(..)`, but `abort_active`'s CDI cleanup gate was the
   variant allowlist `matches!(err, Cancelled | Rejected)`. `Protocol` was
   excluded, so a lag-aborted CDI emitted **zero** `TerminateDueToError` —
   the peer kept half-open exchange state and a subsequent request drew
   `DatagramRejected 0x2020`.

**Decision (HITL-approved 2026-07-18 — D1=A, D2=C).**

- **D1=A — Registry-owned per-peer inbound routing.** `PeerSessionRegistry`
  demultiplexes the transport bus into one bounded per-peer `mpsc` channel
  (`INBOUND_CAPACITY = 256`) per session, keyed by **source alias** (the
  destination alias is always `our_alias` and cannot separate peers). Each
  `PeerSession` consumes its own channel instead of `subscribe_all()`; the
  per-frame source-alias filter is dropped (the destination check is kept).
  The bus `subscribe_all()` broadcast is **retained** for the registry
  spawn-watcher and the future S6 `EventRouter`. This places per-peer
  concerns in the registry — the same layering the Transport-Health
  *Fairness note* already commits to for outbound — and makes cross-peer
  interference **structurally impossible**: no shared ring exists whose fill
  rate scales with peer count. **This supersedes D3 for CDI.**

  Because `mpsc` has no `broadcast::Lagged`, the demux surfaces overflow
  explicitly: on channel-full it drops the frame, increments a per-peer
  `pending_drops: Arc<AtomicU64>`, and delivers a coalesced
  `InboundEvent::Lagged(pending)` *before the next successful frame*. The
  session treats `InboundEvent::Lagged` exactly like the old broadcast
  `Lagged(n)`.

  **Construction-window contract: DROP.** A frame whose source alias has no
  registered route yet is discarded (not buffered). Safe because a session
  issues no requests before it is registered, so no reply can be missed.

## 2026-07-25 extension: `TerminateDueToError` narrowed to our-fault-live-wire; peer-initiated terminals emit no cleanup (JMRI alignment)

**Problem.** [ADR-0019](0019-jmri-as-primary-compatibility-target.md)
(accepted 2026-07-23) makes `OpenLCB_Java` the primary behavioural
compatibility target: where the LCC standard permits a behaviour but
JMRI does not use it, Bowties mirrors JMRI's subset. `grep -r
TerminateDueToError OpenLCB_Java/src/` returns one hit — the enum
declaration in `MessageTypeIdentifier.java` — with no emitter and no
handler anywhere in the codebase.
`MessageDecoder.handleOptionalIntRejected` is a no-op default;
`MimicNodeStore` overrides it only for SNII/PIP temp-flag retry (never
emits TDE); `MemoryConfigurationService` has no override and falls
through to a `MAX_TRIES = 3` timeout-retry loop that also emits nothing.
Invariant #7 as originally stated required TDE emission after every
"our-fault" termination, including OIR and DR-cap-exhaustion — both
**peer-initiated** terminals under S-9.7.2.1 §3.4 / S-9.7.3.2 §3.4
semantics. Under ADR-0019 this is a divergence from the reference
implementation that real peers (only ever tested against JMRI) may not
tolerate.

**Decision.** Invariant #7 is narrowed to distinguish **our-fault-live-wire**
from **peer-initiated terminal**. TDE emission is required only for the
former:

- **Our-fault-live-wire (TDE required, exactly once)**: our-side
  per-chunk `Timeout` (`handle_deadline`), explicit `Cancel`
  (`abort_active` via `PeerError::Cancelled`), lag-recovery exhaustion
  (`abort_active` via `PeerError::Protocol`), and local
  reply-parse / assembled-size-exceeds-limit failures where we cannot
  decode the peer's bytes. These are all "the peer thinks we're still
  receiving; we're not". The original SPROG-timeout regression that
  motivated Invariant #7 lives entirely in this class and is untouched.
- **Peer-initiated terminal (no TDE)**: `OptionalInteractionRejected`
  (OIR), non-resend `DatagramRejected` (bit 0x1000 clear), memory-config
  `ReadReply::Failed` (peer-reported read error in a valid reply), and
  write-path retry-cap-exhaustion after `WRITE_MEMORY_MAX_RETRIES`
  consecutive resend-OK DRs (mirrors JMRI's `MAX_TRIES = 3` cap that
  also emits nothing). In every case the peer has already released
  its exchange state — echoing an error back at the peer is the
  divergence, not the cleanup.

**Call sites changed** (`lcc-rs/src/peer_session.rs`; all eight sites
removed the inline `emit_terminate_due_to_error` call — the completion
payload `PeerError::Rejected { mti, code }` is preserved at every site):

| Handler                    | Terminal class                          | Sub-cases removed |
|----------------------------|-----------------------------------------|-------------------|
| `on_cdi_frame`             | OIR / non-resend DR / `ReadReply::Failed` | 3 |
| `on_memory_read_frame`     | OIR / non-resend DR / `ReadReply::Failed` | 3 |
| `on_memory_write_frame`    | OIR / (non-resend DR or retry-cap-exhaustion, single inline branch) | 2 |

The `PeerError::is_our_fault_live_wire` classifier is unchanged. Its
`Rejected` arm is dead against the current call graph (every OIR / DR /
`Failed` emission happened inline before `complete_*`, never through
`abort_active`), and Option B explicitly declined to prune it — a
future refactor that consolidates the inline emits into `abort_active`
is the correct place to remove the arm.

`emit_terminate_due_to_error` (the method) is retained; the remaining
nine callers are all in the narrowed our-fault-live-wire class listed
above.

**Rationale.** ADR-0019 compliance at the entire peer-initiated-terminal
seam, not just OIR. The invariant becomes semantic ("who caused the
termination") instead of an enumeration of MTIs, which is a stronger
statement to enforce on future code review. Prevents future TDE
emission after any peer-initiated terminal by narrowing the invariant
text in the same commit as the emission-site changes.

**References.**
- [ADR-0019](0019-jmri-as-primary-compatibility-target.md) — governing
  policy.
- [ADR-0018 §2026-07-25 extension](0018-cdi-download-exchange-ownership.md)
  — mirror narrowing of the peer-cleanup contract table.
- `OpenLCB_Java/src/org/openlcb/MessageTypeIdentifier.java` (line 41
  — sole TDE reference), `MessageDecoder.java`,
  `MimicNodeStore.java`, `MemoryConfigurationService.java` —
  behavioural absence verified.
- `aiwiki/architecture-health.md` — resolution entry
  "TerminateDueToError emission narrowed to our-fault-live-wire
  (resolved 2026-07-25)".

**Test pattern.** `lcc-rs/tests/peer_session_cdi.rs` +
`peer_session_config.rs` — eight per-terminal-class TDE-count
assertions renamed to `..._no_tde` and flipped from `== 1` to
`== 0` (Batches 1–3, tdd-cycle red→green). Timeout / cancel /
lag-recovery-exhaustion TDE-emission tests
(`cdi_timeout_emits_one_terminate_due_to_error`,
`read_memory_timeout_emits_one_terminate_due_to_error`, and
`is_our_fault_live_wire`'s classifier tests) remain green — verified
unchanged after each batch. Full `cargo test --manifest-path
lcc-rs/Cargo.toml` green: 359 unit + all integration suites +
doctests.
  Route and session entry are registered atomically under the sessions
  write guard; lock order is **sessions → routes** everywhere. `AliasChanged`
  re-keys the route and the entry alias under both locks — lossless because
  the session dropped its source filter, so the move is key-only on the same
  channel.

  **Deviation flagged.** The inbound arm still drains only while
  `self.active.is_some()` (not a drain-while-idle model). The per-peer
  channel already removes the amplification bug class; stale inter-exchange
  frames are tolerated by the existing reply-identity guard and the
  stray-`DatagramMiddle` recovery. Drain-while-idle (D1=B's mechanism) was
  therefore not adopted.

- **D2=C — Bounded in-place recovery, then fail-clean-with-cleanup.** On an
  inbound lag while a `CdiDownload` is active, the session resets the current
  chunk's `DatagramAssembler` and re-issues the same-`address_cursor` read
  via `send_next_chunk_request` (idempotent memory read; the reply-identity
  guard discards any stale reply). A dedicated **cumulative per-download**
  `lag_recovery_count` (bound `config.max_retries`, distinct from the
  DR-resend `chunk_retry_count`, and **not** reset per chunk — which
  guarantees termination) bounds recovery. On exhaustion the exchange aborts
  through the corrected `abort_active` gate, which emits exactly one
  `TerminateDueToError`. SNIP/PIP lag keeps the D3 abort-and-continue.

- **Peer-cleanup contract recast to fault-nature.** `abort_active`'s CDI
  cleanup gate changes from a `PeerError`-variant allowlist to
  `PeerError::is_our_fault_live_wire()` — the single source of truth. Our-
  fault-live-wire variants (`Cancelled`, `Rejected`, `Protocol`, `Timeout`)
  emit cleanup; wire-dead (`TransportUnhealthy`) and peer-initiated
  (`PeerReinitialised`, `AliasChanged`) plus `NotSupported`/`NotConnected`
  do not. Exactly-once is preserved: CDI `Timeout` emits via `handle_deadline`
  (a direct path that never routes through `abort_active`), so classifying
  `Timeout` as our-fault-live-wire does not double-emit.

**Rejected alternatives** (per architecture-first-fix analysis):
- **D1=B — retain subscribe-all, tune capacity / drain while idle.** Rejected:
  addresses only idle accumulation; the *during-active* n× amplification
  survives and re-emerges under load. Mitigates frequency, does not prevent
  the class.
- **D2=A — fail-clean only (no recovery), caller retries.** Rejected as the
  primary path: violates the slice's recovery direction (a rare infra hiccup
  should not surface as a hard download failure). Retained as the exhaustion
  fallthrough inside C.
- **D2=B — unbounded recover-in-place.** Rejected: re-issuing resets the
  per-chunk deadline, so a sustained lag storm loops forever and masks a
  dropped terminal (OIR / permanent-DR). C's bound closes that class.

**Test coverage.**
- `lcc-rs/src/peer_session.rs::classifier_tests` — `is_our_fault_live_wire`
  covers the live-wire faults and excludes wire-dead / peer-initiated.
- `lcc-rs/tests/peer_session_cdi.rs::cdi_mid_download_lag_recovers_in_place_and_completes`
  (AC1+AC3) — a mid-CDI lag resets the assembler, re-issues the same cursor,
  completes with correct bytes, emits zero `TerminateDueToError`.
- `..::cdi_sustained_lag_storm_exhausts_recovery_then_cleans_up` (AC2) — the
  4th lag with `max_retries=3` exhausts recovery, aborts, and emits exactly
  one `TerminateDueToError` to the correct dest alias.
- `lcc-rs/tests/peer_session_snip_pip.rs::inbound_lag_aborts_active_pip_and_preserves_snip_cache`
  — D3 abort-and-continue retained for non-CDI exchanges.
- `lcc-rs/tests/peer_session_registry.rs::construction_window_frame_before_route_is_dropped`
  — the DROP contract, proven via a registered sync peer establishing
  happens-before.

**Follow-up.** The demux `spawn` test-convenience forwarder mirrors the whole
broadcast without a source filter (no alias-update channel); production uses
the re-keying registry demux. If a future consumer needs drain-while-idle
semantics (e.g. an unsolicited-event surface on an idle session), revisit the
`active.is_some()` inbound gate. Governed by `aiwiki/seams.md § Peer Session
Ownership`.
