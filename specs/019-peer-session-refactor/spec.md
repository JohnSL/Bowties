# Feature Specification: Peer Session Actor — Per-Node Protocol Ownership

**Feature Branch**: `sprog-1.4` (reusing the SPROG-regression bugfix branch)
**Created**: 2026-07-05
**Status**: Draft
**Input**: Consolidate all OpenLCB/LCC protocol interactions with a remote node inside a single per-node actor (`PeerSession`) that owns the pending exchange, the ACK obligations in both directions, the retry state, and the peer-cleanup contract on failure. Retire the pattern of scattered, stateless helpers that each grab a shared `TransportHandle`, run an exchange, and return — this pattern has repeatedly produced desynchronised state machines between Bowties and its peers, duplicate ACKs, mid-flight restarts, and transport-writer deadlocks under back-pressure. Make the transport layer purely wire-level: reader dispatches inbound frames by source alias to the owning peer session; writer serialises outbound frames FIFO with a bounded per-send timeout. Reshape the Tauri command layer to express intent (`download_cdi`, `read_config`, `query_snip`) as commands to the destination peer's session, not as free calls into `lcc-rs`. Ship the refactor as vertical slices that migrate one protocol operation at a time, starting with CDI download (the current pain point), with each slice replacing the corresponding stateless helper by a compatibility shim that forwards to the peer session, then eventually retires the shim.

## Clarifications

### Session 2026-07-05

- Q: `LiveNodeProxy` fate — one actor or two, and which module owns what? → A: Split-layer. `lcc-rs::peer_session::PeerSession` owns wire-protocol state (datagram exchanges, ACKs, retries, peer cleanup, SNIP/PIP wire logic). `bowties-core::node_proxy::LiveNodeProxy` is retained as a thin app-layer per-node aggregator that holds a `PeerSessionHandle` and delegates every protocol call to it. `LiveNodeProxy` keeps app-layer concerns (`last_seen` / `last_verified` timestamps, snapshot for the frontend, config-tree hookup point) and the `NodeProxyHandle::Live \| Synthesized` polymorphism for placeholder nodes. Cached SNIP/PIP data and query coalescing move out of `LiveNodeProxy` into the session; the proxy reads them via the session handle.
- Q: Who is responsible for persisting a completed CDI download into `LayoutState`? → A: The `download_cdi` Tauri command handler. `PeerCommand::DownloadCDI` returns assembled bytes on its result oneshot; the command awaits that result and calls `layout_state.record_captured(node_id, bytes).await` — the same seam that already calls `record_captured` today (`app/src-tauri/src/commands/cdi.rs`). `LiveNodeProxy` stays a bus↔app bridge with no `LayoutState` coupling (matches its current shape and ADR-0015 single-owner boundary). `lcc-rs::peer_session` has zero knowledge of `LayoutState`.
- Q: Slice 6 — do we retire the transport inbound broadcast, or keep it and enforce single ownership at consumers? → A: **Keep the broadcast.** In Rust, filter-at-consumer is essentially free, and the bugs the refactor targets are all consumer-side ownership problems (duplicate ACKs, redundant SNIP/PIP bursts, mystery mid-flight reads, missing peer cleanup), not delivery-shape problems. The transport reader continues to parse frames and publish them to the transport broadcast; `PeerSession`, `NetworkSession`, `EventRouter`, diagnostic recorders, and tests each subscribe and filter by their own criteria. Bug closure is enforced by single-owner rules at consumers: `PeerSession` is the sole ACK owner and sole outbound sender for its peer; `EventRouter` is the sole fan-out point for bus-level event-report traffic; `NetworkSession` is the sole owner of bus-membership frames. Slice 6 narrows to *outbound* consolidation only: retire `send_direct`, retire `direct_write_count`, confirm every outbound frame is session-owned. Keeping the broadcast preserves the "add an observer for free" property (diagnostic recorders, trace loggers, protocol conformance tests, future analytical consumers).
- Q: `PeerSessionRegistry` — is there a race on first-time session creation for a peer? → A: **No, by construction.** `PeerSessionRegistry` is the sole spawner of sessions. It spawns exactly on inbound frames that carry a full NodeID: `Verified Node ID Number`, `Initialization Complete`, and `Alias Map Definition` (AMD). Frames that carry only an alias (PCER, EventReportWithPayload, addressed exchange frames) never trigger session creation — they are handled by `EventRouter` (for event traffic) or by the existing session for that peer (for addressed exchanges). All other callers use `registry.get(node_id) -> Option<PeerSessionHandle>`; nobody else calls `spawn`. Repeat observations of the same NodeID (re-scan, alias renegotiation) update the existing session's alias in place, idempotently. This matches the current shape (`bowties-core::node_registry::NodeRegistry` is already the sole caller of `LiveNodeProxy::spawn`). The concurrency-primitive choice for the internal map (`tokio::sync::Mutex`, `RwLock`, `DashMap`, etc.) is a plan-phase implementation detail because there is only one writer.
- Q: Slice 7 (frontend intent single-owner) — include in this feature, or defer? → A: **Defer to a follow-up feature.** Slices 1–6 close the identified regression by construction: sessions serialize per-peer, so redundant frontend commands queue safely rather than corrupting state. Slice 7's benefits (single "download in progress" indicator, cancel-then-restart primitive, reduced redundant traffic) are UX polish, not correctness. Feature scope for this spec is Slices 1–6. Frontend intent single-owner is captured as a `kind/idea` GitHub issue (tagged `area/frontend` + `area/orchestration`) for follow-up after this refactor lands and gets real-world validation.

## Context

> **2026-07-18 root-cause correction (spec 019 S10).** This spec was written on the premise that the SPROG USB-LCC CDI regression was *architectural* and would be "closed by construction" by the peer-session refactor. **That premise is false.** The SPROG download failure was a serial **`\r\n` framing bug**: Bowties appended CR/LF after every `;`-terminated GridConnect frame on serial; JMRI (the reference implementation these adapters target) sends none, and SPROG USB-LCC v1.4's changed FTDI buffer handling cannot tolerate the extra bytes/frame under CDI load. Removing the trailing bytes fixed it completely (`cdi-probe` 10/10 at `--post-ack-delay-ms 0`, no power cycle), independent of the refactor. The refactor (S1–S4) remains justified on **independent architectural merit** — the bugs listed below (no peer cleanup, duplicate ACKs, writer-mutex deadlock, silent OIR) are all real and independent of the framing bug, and several of them *compounded* the SPROG failure by putting extra frames on the wire — but the refactor did **not** close the SPROG regression. Wherever the text below says the bug is "closed by construction" or the class is "architectural, not incidental," read it against this correction. See [../../temp/SESSION-HANDOFF-2026-07-18.md](../../temp/SESSION-HANDOFF-2026-07-18.md) and the framing fix in [gridconnect_serial.rs](../../lcc-rs/src/transport/gridconnect_serial.rs) (`;`-terminated, no trailing CR/LF on serial).

### The bugs we keep finding are architectural

The `sprog-1.4` branch shipped Option B from change-analyze (`send_direct` for the CDI read loop plus a per-node `CdiInflightRegistry`) and it materially helped: CDI downloads that used to die at chunk 4 (256 bytes) now progress through 30+ chunks. But the underlying pattern that produced the original regression keeps producing new failure modes:

- **The peer's state machine holds an exchange open after we time out.** `datagram_read_exchange` returns `Err` without emitting `TerminateDueToError` back to the peer. modulino_io then rejects every subsequent Bowties datagram to that node with `DatagramRejected 0x2020` or `OptionalInteractionRejected` for many seconds. No entity in Bowties is responsible for peer cleanup.
- **Duplicate `DatagramReceivedOK` frames are emitted.** The trace shows Bowties sending 76 outbound ACKs for 43 replies. Multiple broadcast subscribers each react to the same reply. No single entity owns the "ACK for a reply from node X" obligation.
- **`OptionalInteractionRejected` frames are silently dropped.** `datagram_read_exchange` only recognises `DatagramRejected` and `DatagramReceivedOK`. Peer-error frames indicating "your ACK is for an exchange I don't know about" are ignored, so the caller times out instead of failing fast.
- **Mysterious mid-flight `Read addr=0x00` in the trace.** A fresh Read at address 0 fires while a CDI download is in progress at address `0xC0`. `CdiInflightRegistry` blocks a second `download_cdi` Tauri invocation, and the read loop only increments `address`, so the second Read at 0 has no single explanation in code. Something outside the CDI download path — a reactive Svelte effect, an unlogged retry inside the exchange loop, or a transport-level retransmission — is emitting that frame. Nobody owns the invariant "no CDI-shaped frames to node X while its CDI download is running."
- **Writer-loop mutex deadlock on serial back-pressure.** `writer_loop` in `transport_actor.rs` acquires the shared `Arc<Mutex<Box<dyn TransportWriter>>>` and calls `w.send(&frame).await` while holding it. On a SPROG USB-LCC with RTS/CTS flow control, a full serial buffer stalls that `.await` indefinitely, and every subsequent `send` and `send_direct` from every caller waits forever on the same mutex. This is the "no more LCC messages after failure" symptom the user reports. Once wedged, only tearing down and rebuilding the transport recovers.
- **A second SNIP+PIP burst during CDI.** The trace shows Bowties emitting a second SNIP+PIP wave to modulino and JMRI mid-CDI-download, from an unlogged trigger — either a frontend `$effect` re-firing on a status change, an EventRouter re-emission of `lcc-node-discovered`, or a discovery-orchestrator retry we could not locate. This should be impossible on a peer whose CDI exchange is active.

Each of these has a local fix. Adding `TerminateDueToError` on timeout. Handling `OptionalInteractionRejected` in the exchange loop. Wrapping `w.send().await` in a timeout. Introducing a per-peer transport lock. But every one of those local fixes is patching the same underlying gap: **no single entity in Bowties owns the state of a peer's protocol interaction.** Ownership is scattered across `LccConnection`, `datagram_read_exchange`, `TransportHandle`, `NodeProxy`, various Tauri command handlers, the frontend discovery orchestrator, and the config-acquisition orchestrator. Every operation grabs the shared bus, runs an exchange, and returns; nobody watches the whole picture for a peer. This shape works when nothing goes wrong; when anything goes wrong, no one is responsible for cleanup, and the pieces desynchronise.

### The OpenLCB protocol has a natural home for this ownership

OpenLCB's semantics are per-peer and serial-per-exchange. A datagram exchange between Bowties and node X is a small state machine that owns:

- The current request (address, count, address-space, command)
- The retry state (retry count, "resend OK" flag on the last rejection)
- The reply buffer (partial reassembly across First/Middle/Final frames)
- The ACK obligations in both directions ("I owe them an ACK for their reply" / "they owe me an ACK for my request")
- The timeout state
- The peer-cleanup obligation on our failure (send `TerminateDueToError` before releasing)

Multiple exchanges can proceed to *different* peers in parallel. At most one datagram exchange should exist between Bowties and any given peer at any time. That is a structural invariant of the protocol, and the code should encode it structurally rather than defending it with scattered runtime guards.

Node discovery (SNIP query, PIP query) is similarly per-peer: the state machine advances `Unknown → Querying → Complete | Timeout`. Live event delivery is per-peer (event roles produced/consumed by that node). Configuration read/write, CDI download, and firmware upgrade are all per-peer exchanges.

**The natural owner is the peer.** One actor per remote node, owning that node's exchanges. This is not a novel invention — `bowties-core::node_proxy::LiveNodeProxy` is already a per-peer actor with an mpsc command queue holding `SNIPStatus`, `PIPStatus`, and cached SNIP data. The gap is that the datagram exchange loops (CDI, config read/write) still live outside the proxy in `lcc-rs` free functions, and the transport is still broadcast-shaped instead of dispatch-shaped. This feature closes that gap.

### Why now, and why on this branch

The `sprog-1.4` branch was opened to fix the SPROG CDI regression. Option B (`send_direct` + `CdiInflightRegistry`) shipped in that branch and stabilised the failure surface enough to reveal a genuine class of architectural ownership bugs (see the 2026-07-18 correction above: the SPROG download failure itself was a serial `\r\n` framing bug, not the architecture — but the ownership bugs below are real and independent). Continuing to add local fixes (writer timeout, OIR handling, peer cleanup on timeout) inside the current architecture is a legitimate stage-1 step, but each such fix accumulates further complexity in code the architecture wants to retire. This spec proposes doing the refactor now, on the same branch, before the local fixes calcify into permanence.

The refactor is large but not risky *if it is sliced correctly*. Each slice migrates one protocol operation from the old scattered pattern into the peer session, with a compatibility shim in the old location that forwards to the new owner. Tests are the safety net; every slice ships with its own protocol-level tests, and the old code path stays functional until the shim is retired. The frontend contract with Tauri commands does not change during the refactor — only the backend's internal shape does.

## Target Architecture

### Structural overview

Three layers, top-down:

**Layer 1 — Intent (Tauri command boundary).** Tauri commands express user intent for a specific peer or for the network. They translate the frontend RPC into a peer command (or a network command) and await the result. Commands hold no protocol state.

**Layer 2 — Peer Sessions (per-node actors).** One `PeerSession` actor per remote node. Owns every interaction with that peer: SNIP, PIP, CDI download, config read, config write, event subscriptions, cached state. Runs a single command at a time (natural per-peer serialization). Emits progress events to interested subscribers. Holds all peer-state cleanup obligations.

**Layer 3 — Transport (wire-level broadcast + FIFO writer).** Reader parses frames off the wire and publishes them to the transport broadcast; `PeerSession`, `NetworkSession`, `EventRouter`, and any diagnostic/test observers subscribe and filter by their own criteria (see Clarifications 2026-07-05). Writer accepts outbound frames from any session and drains them FIFO with a bounded per-send timeout so a stuck adapter surfaces as a health error, not a global deadlock.

```
┌───────────────────────────────────────────────────────────────────────┐
│                     Frontend (Tauri IPC boundary)                      │
├───────────────────────────────────────────────────────────────────────┤
│  Tauri commands (thin intent translation):                             │
│    download_cdi(node)   →  session.command(DownloadCDI)                │
│    read_config(node,…)  →  session.command(ReadConfig)                 │
│    query_snip(node)     →  session.command(QuerySNIP)  (cached)        │
├───────────────────────────────────────────────────────────────────────┤
│                     Peer Sessions (per remote node)                    │
│                                                                        │
│   PeerSession A   PeerSession B   PeerSession C   …   NetworkSession   │
│   ┌───────────┐   ┌───────────┐   ┌───────────┐       ┌────────────┐   │
│   │ mpsc cmd  │   │ mpsc cmd  │   │ mpsc cmd  │       │ alias neg. │   │
│   │ queue     │   │ queue     │   │ queue     │       │ VNIG probe │   │
│   │ inbound   │   │ inbound   │   │ inbound   │       │ InitComplete│  │
│   │ router-fed│   │ router-fed│   │ router-fed│       │ handler     │  │
│   └───────────┘   └───────────┘   └───────────┘       └────────────┘   │
├───────────────────────────────────────────────────────────────────────┤
│                       Transport Layer                                  │
│   Reader task:  raw frame → parse → publish to transport broadcast   │
│                 (PeerSession/NetworkSession/EventRouter subscribe)   │
│   Writer task:  mpsc outbound → wire (single-writer, bounded timeout) │
│   TransportHealth: healthy | degraded | wedged (surfaces to UI)       │
└───────────────────────────────────────────────────────────────────────┘
```

The transport writer becomes single-threaded and holds *no* async mutex across `.await`. It drains its own mpsc, writes to the wire with a bounded timeout, and reports transport-health state changes on a broadcast channel. No caller ever contends for a shared writer mutex; there is one writer, and callers post frames to it via mpsc.

### The `PeerSession` actor

**Ownership:** Every `PeerSession` owns the following for its peer:

- The `NodeID` and current `NodeAlias` for that peer (updated on alias renegotiation)
- An mpsc receiver of `PeerCommand` values
- An mpsc receiver of inbound `ReceivedMessage` frames pre-filtered by source alias
- An outbound `mpsc::Sender<GridConnectFrame>` to the transport writer
- Cached protocol state: `SNIPData`, `PIPStatus`, `ProtocolFlags`
- The current in-flight exchange (if any) with its full state (address, retry count, partial reply buffer, ACK obligations, timeouts)
- The subscribed event listeners for progress emission

**Command surface:**

```
enum PeerCommand {
    QuerySNIP    { result: oneshot<Result<SNIPData, PeerError>> },
    QueryPIP     { result: oneshot<Result<ProtocolFlags, PeerError>> },
    DownloadCDI  { progress: mpsc<CDIProgress>, result: oneshot<Result<CDIBytes, PeerError>> },
    ReadConfig   { addr: u32, count: u8, result: oneshot<Result<Vec<u8>, PeerError>> },
    WriteConfig  { addr: u32, data: Vec<u8>, result: oneshot<Result<(), PeerError>> },
    Cancel,
    Shutdown,
}
```

**Processing model:** The actor's main loop is `select!` over the command queue and the inbound frame queue. Commands are processed *sequentially*: a `DownloadCDI` command runs to completion (or cancellation) before the next command is even dequeued. Inbound frames are consumed by the currently active exchange handler; frames that arrive when no exchange is active are dispatched to a small set of always-on handlers (event-report delivery, `Verified Node ID Number` updates).

**Peer cleanup contract:** Any exchange that terminates in error — timeout, `OptionalInteractionRejected`, `DatagramRejected` with a permanent error code — is responsible for emitting the appropriate peer-cleanup frame (`TerminateDueToError` for datagram exchanges) before returning to the top of the loop. The peer's state machine is guaranteed to be released, or else it was already released and the extra frame is harmless.

**Idempotence and coalescing:** `QuerySNIP` and `QueryPIP` return cached values on repeat calls unless the cache has been explicitly invalidated (e.g., on `InitializationComplete` for that node). If a query is already in flight, subsequent callers are parked on the same result oneshot (fan-in via a shared `Vec<oneshot::Sender<...>>`); no second wire query is issued. This eliminates the "duplicate SNIP+PIP burst" symptom by construction.

**Cancellation:** A `Cancel` command is honoured at the next exchange checkpoint (between chunks for CDI, before the next retry for a rejected exchange). The active exchange emits its peer cleanup, returns an error to its pending oneshot, and the loop advances. Cancellation is naturally bounded to one exchange because only one is running.

### The transport layer

**Reader task** (one per transport):
- Parses raw bytes from the wire into `GridConnectFrame` values
- Publishes every parsed frame to the transport broadcast channel
- Consumers (`PeerSession`, `NetworkSession`, `EventRouter`, diagnostic recorders, tests) subscribe and filter by their own criteria: each `PeerSession` filters by its peer's source alias; `NetworkSession` filters by bus-membership frame class (CID/RID/AMR/AMD, `InitializationComplete`, `Verify Node ID Global`, alias-negotiation); `EventRouter` filters by event-report MTI (PCER, EventReportWithPayload, Identify Events, Learn Event)
- Bug closure (single ACK per reply, single outbound source per peer, session-owned peer cleanup) is enforced at the consumer side, not by narrowing delivery (see Clarifications 2026-07-05)

**Writer task** (one per transport):
- Drains a single outbound mpsc of `GridConnectFrame`
- Writes each frame to the wire wrapped in `tokio::time::timeout(SERIAL_SEND_TIMEOUT, …)`
- On timeout, logs, breaks the loop, and marks the transport unhealthy — never blocks a caller
- Emits `TransportHealth` events on a broadcast channel (`Healthy | Degraded { reason } | Wedged { reason }`)
- No mutex, no shared writer, no `send_direct`. Callers `.await` on `mpsc::Sender::send(frame)` which only blocks on channel capacity, never on the wire

**`send_direct` is retired.** The reason it exists today (bypassing the mpsc scheduler hop for reply-ACK ordering) becomes moot: each `PeerSession` emits its ACKs in order relative to its own outbound frames, and the writer drains FIFO. Per-peer ordering falls out of the actor's single-threaded command loop.

**Broadcast is retained.** The reader continues to publish parsed frames to the transport broadcast (see Clarifications 2026-07-05). Bug closure is enforced at the consumer side — `PeerSession` is the single ACK owner and single outbound sender for its peer, `EventRouter` is the single owner of event-report fan-out to app-side subscribers, `NetworkSession` is the single owner of bus-membership frame handling — not by narrowing delivery. In Rust, filter-at-consumer is cheap, and keeping the broadcast preserves the "add an observer for free" property (diagnostic recorders, trace loggers, protocol conformance tests, future analytical consumers).

### The Tauri command boundary

Tauri commands become thin wrappers:

```rust
#[tauri::command]
pub async fn download_cdi(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<GetCdiXmlResponse, String> {
    let node = NodeID::from_hex_string(&node_id)?;
    let session = state.sessions.get(&node).ok_or("node not discovered")?;
    let (progress_tx, progress_rx) = mpsc::channel(64);
    let (result_tx, result_rx) = oneshot::channel();
    session.command(PeerCommand::DownloadCDI { progress: progress_tx, result: result_tx }).await?;
    // Stream progress → Tauri events; await result_rx
    ...
}
```

The `AppState.sessions` field owns a `HashMap<NodeID, PeerSession>` (or an `Arc<PeerSessionRegistry>`). Sessions are created on first node discovery and reaped on `NodeRemoved` (future work — outside the scope of this refactor).

**CDI bytes handoff to `LayoutState`.** `PeerCommand::DownloadCDI` returns the assembled bytes on its result oneshot. Persistence into `LayoutState` is the responsibility of the `download_cdi` Tauri command handler — the same seam that calls `layout_state.record_captured(...)` today in `app/src-tauri/src/commands/cdi.rs`. `LiveNodeProxy` and `lcc-rs::peer_session` both remain free of any `LayoutState` reference (see Clarifications 2026-07-05).

`CdiInflightRegistry` and `cdi_download_cancel` become unnecessary. The invariant "one CDI download in flight per node" is now inherent (the session processes commands sequentially). Cancellation is per-session via `PeerCommand::Cancel`.

### Frontend layer

The frontend contract with Tauri commands is unchanged during the refactor. Every Tauri command that existed before still exists, still takes the same arguments, still returns the same shape (subject to minor error-variant additions). This is the pillar that lets us ship the refactor as slices.

Longer-term the frontend can benefit from the actor model (Option C from earlier discussion — single frontend intent owner per operation), but that is a separate spec.

## Functional Requirements

**FR-001 — Peer session existence.** For every remote node discovered on the bus, exactly one `PeerSession` actor exists during the lifetime of the transport connection. The session is created lazily on first observation of the node (`Verified Node ID Number` reply) and destroyed when the transport is torn down.

**FR-002 — Sequential command processing.** A `PeerSession` processes commands strictly in the order they are received. No command begins until the previous command has produced a result on its response channel. Long-running commands (`DownloadCDI`) block subsequent commands to the same peer; this is intentional and matches the OpenLCB per-peer serialization rule.

**FR-003 — CDI download command.** A `PeerCommand::DownloadCDI` runs the full 64-byte-per-chunk CDI read loop against the peer, streams progress on the provided mpsc, and returns the assembled CDI bytes on the result oneshot. On any error, it emits peer cleanup (`TerminateDueToError`) before returning. The session's `NodeID` is the only identifier — no alias is passed by the caller (the session tracks it).

**FR-004 — Config read command.** `PeerCommand::ReadConfig { addr, count }` runs a single datagram read exchange in space `0xFD` for the specified address and count. Same peer-cleanup contract as CDI download.

**FR-005 — Config write command.** `PeerCommand::WriteConfig { addr, data }` runs a datagram write exchange. Handles the WriteReply / WriteReplyFail branch. Same peer-cleanup contract.

**FR-006 — SNIP and PIP query commands.** `PeerCommand::QuerySNIP` and `PeerCommand::QueryPIP` are idempotent and cache-aware. Repeat calls with a valid cache return immediately without wire traffic. Concurrent callers of the same query are coalesced into one wire exchange; all callers receive the same result on their respective oneshots.

**FR-007 — Cancellation.** `PeerCommand::Cancel` interrupts the currently active exchange at the next checkpoint, emits peer cleanup, and returns a cancellation error on the active exchange's result channel. Subsequent queued commands proceed normally. The session is not terminated.

**FR-008 — Bidirectional ACK ownership.** For every reply datagram received from the peer, the session emits exactly one `DatagramReceivedOK` back to the peer before advancing to the next exchange step. For every request datagram we send, we track the peer's `DatagramReceivedOK` and honour timeout-extension flags. Duplicate ACKs are eliminated by construction (the actor is the sole ACK authority for its peer).

**FR-009 — Peer cleanup on error.** Any exchange termination that leaves the peer's state machine holding an open exchange (timeout waiting for reply, timeout waiting for our reply-ACK, `OptionalInteractionRejected` for our ACK, `DatagramRejected` with permanent-error code) results in the session emitting the appropriate peer-cleanup frame before the error propagates. For datagram exchanges this is `TerminateDueToError` with the error code from the rejection (or a generic code on timeout).

**FR-010 — OIR handling.** `OptionalInteractionRejected` from the destination node is a first-class terminal event for the active exchange. The session decodes the wrapped MTI and error code, emits peer cleanup if applicable, and returns an error with a diagnostic message identifying the rejected MTI and error code. The error surfaces to the Tauri command caller.

**FR-011 — DatagramRejected with resend-OK.** When the peer sends `DatagramRejected` with the "resend OK" flag (`0x2000` bit), the session retries the current exchange up to a configurable cap (`MemoryReadConfig::max_retries`, default 3). On cap exhaustion, the session emits peer cleanup and returns an error.

**FR-012 — Transport reader broadcast.** The transport reader parses each inbound frame and publishes it to the transport broadcast channel. `PeerSession`, `NetworkSession`, `EventRouter`, and any diagnostic/test observers subscribe and filter by their own criteria. Filter-at-consumer is preferred over dispatch-at-reader (see Clarifications 2026-07-05): filtering is essentially free in Rust, and keeping the broadcast preserves the "add an observer for free" property. Bug closure (single ACK per reply, single outbound source per peer, session-owned peer cleanup on failure) is enforced at the consumer side.

**FR-013 — Transport writer with bounded send.** The transport writer wraps each `TransportWriter::send(frame).await` in `tokio::time::timeout(SERIAL_SEND_TIMEOUT, …)`. `SERIAL_SEND_TIMEOUT` is configurable per transport (default 500 ms for serial, 2000 ms for TCP). On timeout, the writer emits `TransportHealth::Wedged { reason: "serial write timeout" }` on the health channel and terminates the writer task. All queued outbound frames are dropped with an error; all pending session commands receive `PeerError::TransportUnhealthy`.

**FR-014 — Transport health broadcast.** A dedicated broadcast channel emits `TransportHealth` events (`Healthy | Degraded { reason } | Wedged { reason }`). The Tauri command layer subscribes to this and surfaces the state to the frontend, which can display a connection-status indicator and/or prompt the user to reconnect. Health transitions to `Wedged` are non-recoverable within the current transport — a reconnect is required.

**FR-015 — Alias renegotiation handled by NetworkSession.** When Bowties re-runs its own alias allocation (CID 7 → CID 4 → RID → `Verify Node ID Global`), the `NetworkSession` re-issues node discovery and each existing `PeerSession` updates its cached alias to match the new mapping observed via `Verified Node ID Number`. Any exchange in flight when the renegotiation happens fails with `PeerError::AliasChanged` and its peer-cleanup contract fires (if applicable). Callers can retry.

**FR-016 — InitializationComplete handled by PeerSession.** When a peer emits `Initialization Complete`, its session clears its cached SNIP and PIP state (setting them back to `Unknown`) and drops any in-flight exchange with `PeerError::PeerReinitialised`. Callers can retry.

**FR-017 — Session registry.** `AppState` holds a `PeerSessionRegistry` (a keyed collection of `NodeID → PeerSession` handles). `PeerSessionRegistry` is the **sole spawner** of sessions: it subscribes to the transport broadcast and spawns a session exactly on inbound frames that carry a full NodeID (`Verified Node ID Number`, `Initialization Complete`, `Alias Map Definition`). No other entity calls spawn. All callers use `registry.get(node_id) -> Option<PeerSessionHandle>` and receive `None` if the peer has not yet been discovered. Repeat observations of the same NodeID update the existing session's alias in place, idempotently. Session lifetimes end when the transport terminates; the registry is cleared on disconnect. Retention across reconnects (session identity survives disconnect+reconnect) is out of scope for this refactor. (See Clarifications 2026-07-05.)

**FR-018 — Backward-compatible Tauri command shapes.** Every Tauri command that exists at the start of this refactor continues to accept the same arguments and return the same shape. New error variants may be added (e.g., `PeerError::TransportUnhealthy`), but existing variants and their string tags are preserved.

**FR-019 — Compatibility shims during migration.** During each migration slice, the corresponding `lcc-rs` free function (`LccConnection::read_cdi`, `read_memory_timed`, `query_snip`, `query_pip`) remains available but is reimplemented as a shim that forwards to the peer session. The shim maintains the free-function's public signature so nothing outside the refactor breaks. Shims are removed in the final migration slice.

**FR-020 — No regressions in existing behaviour.** Every currently-passing test in `lcc-rs`, `bowties-core`, `app/src-tauri`, and `app` (Vitest) must continue to pass at every slice boundary. Slices that would break existing tests must first update those tests to reflect the new architecture, with the updates justified in the slice's commit message.

## Data Model

The following are the additions and modifications to existing modules. Non-trivial types are named; their fields are enumerated at the level of intent, not final Rust syntax.

**New in `lcc-rs::peer_session` (new module):**

- `PeerSession` — the actor struct. Fields: node_id, current alias, outbound sender, inbound receiver, command receiver, cached SNIP+PIP, active exchange state (an enum), event subscribers.
- `PeerCommand` — the command enum (variants listed in FR-003 through FR-007).
- `PeerError` — the error enum. Variants: `Timeout`, `PeerReinitialised`, `AliasChanged`, `TransportUnhealthy`, `Rejected { mti, code }`, `Cancelled`, `NotSupported`. Serializable via `thiserror::Error` with stable string prefixes for the frontend.
- `ActiveExchange` — internal enum for the session's current-exchange state. Variants for each protocol operation (`CdiDownload`, `ConfigRead`, `ConfigWrite`, `SnipQuery`, `PipQuery`).
- `CdiProgress` — the progress payload for `DownloadCDI` subscribers. Fields: `bytes_read: u32`, `retry_count: u32`.

**New in `lcc-rs::peer_session_registry` (new module):**

- `PeerSessionRegistry` — thread-safe map of `NodeID → PeerSessionHandle`. Sole spawner of sessions (subscribes to the transport broadcast and spawns on frames that carry a full NodeID — see Clarifications 2026-07-05). Public methods: `get(node_id) -> Option<PeerSessionHandle>`, `remove(node_id)`, `clear()`. `spawn` is registry-internal.
- `PeerSessionHandle` — cloneable handle wrapping the command sender. Public methods: `command(cmd) -> Result<(), PeerError>` plus typed convenience methods `download_cdi`, `read_config`, `query_snip`, etc. that construct the appropriate `PeerCommand` variant and return the corresponding future.

**New in `lcc-rs::event_router` (new or revised module):**

- `EventRouter` — bus-scoped subscriber of the transport broadcast. Owns classification and fan-out of event-report frames (PCER, EventReportWithPayload, Identify Events, Learn Event) to app-side subscribers registered by event id or by role. Peer of `NetworkSession` and `PeerSession` at the same layer — **not** a component of them (see Clarifications 2026-07-05). Sole owner of event-report fan-out.

**New in `lcc-rs::transport_actor` (existing module, revised):**

- `TransportHealth` enum: `Healthy | Degraded { reason } | Wedged { reason }`.
- Writer task revised to accept a `SERIAL_SEND_TIMEOUT` parameter and emit health on a broadcast channel.
- Reader task **unchanged in shape** — it parses frames and publishes to the transport broadcast (see Clarifications 2026-07-05). Consumers subscribe and filter.
- `TransportHandle::send_direct` removed. `TransportHandle::send` remains but is called only by sessions (not by application code directly).
- `TransportHandle::subscribe_all` (or equivalent broadcast-subscribe API) is retained for consumers.
- `direct_write_count` observability hook (added in the Option B fix) is retired.

**Modified in `bowties-core::node_proxy`:**

- `LiveNodeProxy` is **retained** as a thin app-layer per-node aggregator (see Clarifications 2026-07-05). It stops owning SNIP/PIP wire state and query coalescing; those move into `lcc-rs::peer_session::PeerSession`.
- `LiveNodeProxy` gains a `PeerSessionHandle` field and delegates every protocol operation (SNIP/PIP query, CDI download, config r/w) to it.
- `LiveNodeProxy` retains app-layer concerns: `last_seen` / `last_verified` timestamps, `ConnectionStatus`, `DiscoveredNode` snapshot for the frontend, and the config-tree hookup point (`GetConfigTree` / `SetConfigTree` message pass-through to `LayoutState`, per ADR-0015).
- `NodeProxyHandle::Live \| Synthesized` polymorphism is preserved so placeholder / synthesized nodes continue to work uniformly.
- `ProxyMessage::QuerySnip` / `QueryPip` become pass-through: the proxy forwards to `PeerSessionHandle` and awaits the result. `ProxyMessage::SnipQueryDone` / `PipQueryDone` and the `snip_waiters` / `pip_waiters` fields are removed (coalescing now lives in the session).
- `ProxyMessage::NodeReinitialised` continues to originate from the transport / event router; the proxy forwards it to the session (which invalidates cached SNIP/PIP and cancels any in-flight exchange, per FR-016).

**Modified in `app/src-tauri/src/state.rs`:**

- `AppState.sessions: PeerSessionRegistry` — new field.
- `AppState.cdi_inflight: CdiInflightRegistry` — **retired** in the final slice. Slices before the CDI migration keep it as a defensive check.
- `AppState.cdi_download_cancel: Arc<AtomicBool>` — **retired** in the CDI slice. Cancellation becomes `PeerCommand::Cancel`.

**Modified in `app/src-tauri/src/commands/*.rs`:**

- Each command that currently calls `connection.read_cdi(...)`, `connection.read_memory(...)`, `query_snip(...)`, or `query_pip(...)` is rewritten to look up the session and issue the corresponding command. The command's public shape is unchanged.

## Migration Slices

The refactor ships as vertical slices. Each slice is independently testable, independently commit-able, and — crucially — each slice ends in a state where the app runs end-to-end. No slice leaves the codebase in a half-refactored state that requires the next slice to compile.

### Slice 1 — Transport writer bounded send + health broadcast

**Deliverable.** `transport_actor.rs` writer wraps each `w.send(frame).await` in `tokio::time::timeout`. A `TransportHealth` enum is defined and the writer emits transitions on a broadcast channel. The Tauri connection command subscribes to this and surfaces it (initially as a log line; UI wiring in a later slice).

**Why first.** This closes the "cannot send new LCC messages after failure" symptom immediately, before the larger refactor lands. It also serves as the foundation for later slices — every subsequent slice can assume a bounded-latency writer.

**Testing.** New unit test in `lcc-rs/src/transport_actor.rs` that injects a mock writer that stalls forever and asserts the writer task terminates within `SERIAL_SEND_TIMEOUT + slack` with a `Wedged` health event. Verify no callers block indefinitely.

**Retirement of `send_direct`.** Not in this slice. `send_direct` continues to exist and continues to be used by the CDI path (Option B). The bounded timeout applies equally to both paths since both go through the same writer.

**Exit criteria.** All existing tests pass. Manual verification: connecting to a SPROG, running CDI on a large node, and physically disconnecting the SPROG cable produces a `Wedged` event within `SERIAL_SEND_TIMEOUT`, and subsequent commands return `TransportUnhealthy` promptly rather than hanging.

### Slice 2 — PeerSession scaffolding + SNIP migration

**Deliverable.** The `lcc-rs::peer_session` module is created with the `PeerSession` struct, `PeerCommand::QuerySNIP` variant, and the `PeerSessionRegistry`. The `PeerSession` subscribes to the transport broadcast and filters for frames whose source alias matches its peer. `LccConnection::query_snip` is retained as a shim that looks up the session and forwards. `LiveNodeProxy::query_snip` is retained but internally delegates to the session.

**Why SNIP first.** SNIP is the simplest exchange: one addressed request, a multi-frame reply, one ACK back. No datagram protocol, no retries. It exercises the full session lifecycle (create, receive frames, process command, emit result) with minimal protocol surface. If the session architecture is right, SNIP fits it trivially; if the architecture is wrong, SNIP surfaces it cheaply.

**Testing.** Move existing SNIP tests into `peer_session::tests`, extend to cover: concurrent `QuerySNIP` callers coalesce; cached SNIP returns without wire traffic; peer reinit clears cache.

**Exit criteria.** Every SNIP query in the app flows through a `PeerSession`. The old `lcc-rs::snip::query_snip` function becomes a wrapper that goes through the session. All existing frontend behaviour that depends on SNIP data is unchanged.

### Slice 3 — PIP migration

**Deliverable.** `PeerCommand::QueryPIP` variant, PIP protocol handling in the session (using the same broadcast subscription established in Slice 2). `LccConnection::query_pip` becomes a shim.

**Why PIP next.** Same shape as SNIP (single request + single reply). By this point the session pattern is proven, and PIP is a small step. Also unblocks the "JMRI PIP misclassified as Timeout" investigation — the session-owned PIP handler can log more diagnostically.

**Testing.** Same shape as slice 2. Extend to cover the concurrent-caller coalescing behaviour.

**Exit criteria.** Every PIP query in the app flows through a `PeerSession`.

### Slice 4 — CDI download migration + peer cleanup + OIR handling

**Deliverable.** `PeerCommand::DownloadCDI` variant. The full CDI read loop moves into the session (`ActiveExchange::CdiDownload`). Peer cleanup (`TerminateDueToError`) is emitted on timeout, OIR, and terminal `DatagramRejected`. `OptionalInteractionRejected` is a first-class terminal event for the active exchange. `download_cdi` Tauri command is rewritten to dispatch to the session. `CdiInflightRegistry` and `cdi_download_cancel` are retired (their invariants are now inherent to the session).

**Why CDI here.** This is the current pain point and the biggest single migration. All the previous slices exist to make this slice cheap and confident. By this point the session, the registry, the transport dispatch, and the health broadcast are in place; CDI is "add a new command variant and its ActiveExchange handler."

**Testing.** Move existing CDI tests. Add: peer-cleanup-on-timeout emits `TerminateDueToError`; OIR terminates the exchange with a diagnostic error; a second `DownloadCDI` command from a different Tauri invocation queues behind the first (no `CdiInflightRegistry` needed); cancellation mid-CDI emits peer cleanup and returns promptly.

**Retirement.** `CdiInflightRegistry` (delete the module, remove from `AppState`), `CdiInflightGuard` (delete), `cdi_download_cancel` (delete from `AppState`), `LccConnection::read_cdi_cancellable_with_stats` (retain as shim), `send_direct` still retained (removed in slice 6).

**Exit criteria.** CDI download for modulino_io over SPROG completes end-to-end without user intervention. Peer cleanup on timeout, OIR-terminal handling, and per-peer serialisation are structural rather than runtime-guarded. *(2026-07-18 correction: the SPROG download failure was closed by the serial `\r\n` framing fix, not by this slice's architecture — see the correction callout at the top of Context. This slice's contribution is the single-ACK-owner + peer-cleanup + reduced-wire-traffic architecture, which compounded-symptom-reduces but did not root-cause-fix the SPROG regression.)*

### Slice 5 — Config read/write migration

**Deliverable.** `PeerCommand::ReadConfig` and `PeerCommand::WriteConfig` variants. Config-read and config-write Tauri commands are rewritten. Existing `read_memory_timed` and `write_memory_timed` become shims. `BatchReader` becomes a stateless helper *inside* the session, no longer a public API of `lcc-rs::discovery`.

**Testing.** Move existing config-read and config-write tests. Add: interleaved concurrent commands to the same peer serialize correctly; interleaved commands to different peers proceed in parallel.

**Exit criteria.** Every memory-config exchange in the app flows through a `PeerSession`.

### Slice 6 — Outbound ownership consolidation + `send_direct` retirement

**Deliverable.** Confirm every outbound frame flows through a session's outbound queue (the writer mpsc) rather than through `send_direct`. `send_direct` is retired. `direct_write_count` observability hook is removed. `EventRouter` is formalised as a bus-scoped subscriber of the transport broadcast, focused on classifying and fanning out event-report frames (PCER, EventReportWithPayload, Identify Events, Learn Event) to app-side subscribers. The transport **inbound broadcast is retained** — see Clarifications 2026-07-05. `TransportHandle::subscribe_all` (or equivalent) remains for observers.

**Why last.** This is the final consolidation of *outbound* ownership. It only becomes safe once every protocol operation is inside a session (slices 2–5), because at that point every outbound frame has a session owner. The transport inbound broadcast is not touched.

**Testing.** Assert no production code path calls `send_direct`; assert `send_direct` is not part of any public API. Add: `EventRouter` fan-out correctness (registered subscribers receive matching event-report frames, non-matching frames are not delivered). Existing tests that subscribe to the transport broadcast continue to work unchanged.

**Exit criteria.** No production code path calls `send_direct`. `direct_write_count` is removed. `EventRouter` owns all event-report fan-out. Transport inbound broadcast and `TransportHandle::subscribe_all` remain, unchanged.

### Slice 7 — Frontend intent single-owner (deferred to follow-up feature)

**Status.** Deferred (see Clarifications 2026-07-05). Feature scope for this spec is Slices 1–6.

**Rationale for deferral.** Slices 1–6 close the identified regression by construction — sessions serialize commands per-peer, so redundant frontend commands (a viewer opening while a redownload dialog is also open) queue safely rather than corrupting state. Slice 7's benefits (single "download in progress" indicator, cancel-then-restart primitive, reduced redundant traffic) are UX polish, not correctness.

**Follow-up.** Frontend intent single-owner will be captured as a `kind/idea` GitHub issue (`area/frontend` + `area/orchestration`) after this refactor lands and gets real-world validation. The scope will be: one clear frontend owner for each of CDI download intent per node, config-read intent per node, and node-discovery refresh. This is Option C from the earlier change-analyze, complementary to the backend refactor rather than required by it.

## Testing Strategy

Every slice ships with tests before it ships with the corresponding behaviour change (TDD via the `tdd-cycle` subagent workflow, matching the existing engineering discipline on this repo). The specific test locations are named per-slice above.

**Fixtures.** A `MockTransport` in `lcc-rs::transport::mock` (already exists) is extended to support:
- Programmable inbound frame sequences with timing
- Assertion of outbound frame sequences with ordering constraints
- Simulation of writer stall (for slice 1 tests)
- Simulation of dropped/reordered frames

**Actor testing pattern.** Each `PeerSession` variant test spawns the actor with a mock transport, sends commands, feeds inbound frames, and asserts the resulting outbound frames and command results. No `#[tokio::test]` polluted with global state; each test constructs its own registry, transport, and session.

**Integration tests.** `app/src-tauri/tests/` gets a new integration test for each protocol operation, verifying end-to-end that a Tauri command produces the correct outbound frame sequence via a mock transport and correctly assembles the reply.

**No stateful global test state.** The current test suite already avoids this; the refactor preserves it. Each test constructs the transport actor and session registry it needs and tears them down at test exit.

## Success Criteria

1. **CDI download completes end-to-end without user intervention** — modulino_io's CDI (or any node's CDI, on SPROG or any transport) completes with the transport under back-pressure, with concurrent discovery traffic, and with intermediate errors. *(2026-07-18 correction: the SPROG download failure itself was closed by the serial `\r\n` framing fix, not by this refactor's architecture — see the correction callout at the top of Context. The refactor's contribution to this criterion is robustness under back-pressure / concurrency / error paths, not the SPROG root-cause fix.)*
2. **`cdi_inflight`, `cdi_download_cancel`, `send_direct`, and `direct_write_count` are retired** — replaced by the session model's inherent invariants. The transport inbound broadcast is retained (see Clarifications 2026-07-05); bug closure is enforced by consumer-side single-owner rules, not by narrowing delivery.
3. **No new local fixes have been added inside the retired architecture** during the refactor. Every fix flows from the new shape.
4. **All existing tests pass at every slice boundary.**
5. **New tests cover the new invariants**: per-peer serialization, peer cleanup on all error paths, coalescing of idempotent queries, transport-health surface.
6. **The frontend behaves identically** from the user's perspective, except for improvements (progress events, transport-health indicator, faster failure surfacing).

## ADR Implications

This refactor introduces a new architectural seam and warrants a new ADR:

- **ADR-0016 (proposed): Per-peer session actor owns protocol state.** Documents the decision to consolidate per-node protocol state in `PeerSession` and the rationale (OpenLCB's per-peer semantics; the class of bugs the previous architecture repeatedly produced). Names `PeerSessionRegistry` as the owner of session lifecycle. References the retirement of `send_direct`, `CdiInflightRegistry`, and `direct_write_count`. Documents the deliberate decision to *retain* the transport inbound broadcast, with bug closure enforced by consumer-side single-owner rules (see Clarifications 2026-07-05).

ADR-0015 (backend layout-state single owner) is unaffected — layout state remains in `LayoutState`. The peer session owns *protocol* state (SNIP, PIP, active exchanges), not *layout* state (saved / captured / drafts). Cached CDI XML still lives in `LayoutState`; the session hands off assembled CDI bytes to it via the existing `record_captured` path.

## Out of Scope

- **Frontend intent single-owner (Slice 7).** Deferred to a follow-up feature per Clarifications 2026-07-05. See the Slice 7 note above for rationale. Will be captured as a `kind/idea` GitHub issue after this refactor lands.
- **Session persistence across reconnects.** A `PeerSession` today lives only as long as its transport. Retaining cached SNIP/PIP across disconnect+reconnect is a valuable future feature but requires distinct design (durable storage of SNIP data, invalidation on `InitializationComplete` observed on the new connection). Not this refactor.
- **Peer-side flow control.** OpenLCB Datagram Transport defines back-pressure via `DatagramReceivedOK` with the "reject temporarily, resend after N seconds" flag. The current code partially handles this; the refactor preserves the current behaviour but does not extend it. A future feature can add adaptive pacing per peer.
- **Session-scoped event-role subscriptions.** Currently event-report delivery is global (any node's PCER broadcasts to any listener). The session model could optionally route event reports per peer, but this couples channels to sessions in ways that need their own design. Not this refactor.
- **Firmware upgrade command.** `PeerCommand::FirmwareUpgrade` is a natural fit but is not implemented today in Bowties. Adding it is a separate feature; the session architecture is designed to accept it as a new variant.
- **Cross-peer coordination (e.g., "download CDI for all nodes in parallel").** The registry naturally supports this via `join_all` on multiple sessions' commands. UX for it is a separate feature (probably tied to the configAcquisitionOrchestrator revamp).
- **Frontend `$effect` audit and repair.** The mysterious "second SNIP+PIP burst during CDI" trace observation is likely a frontend reactive-effect re-fire, but its precise source is unknown. The refactor eliminates its *impact* (a session ignores redundant SNIP/PIP commands by cache-hit), but the underlying frontend bug should be tracked as a separate `kind/idea` issue after the refactor lands.

## Future Considerations

- **Structured tracing.** A `tracing` span per session command would make protocol-level debugging dramatically easier. The refactor is a natural moment to introduce `tracing` throughout `lcc-rs` if not already present.
- **Protocol correctness fuzzing.** With the session as a state machine, `proptest`-based fuzzing of exchange sequences becomes tractable. Consider a follow-up feature.
- **Per-session metrics.** Emit per-peer counters (exchanges completed, retries, timeouts) on a metrics channel. Useful for diagnostic reports and user-facing "network health" panels.
