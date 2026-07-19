# ADR-0018 — CDI download exchange ownership + peer-cleanup contract

**Status**: Accepted
**Date**: 2026-07-08
**Related**: [ADR-0015](0015-backend-layout-state-single-owner.md), [ADR-0016](0016-per-peer-session-actor.md), [ADR-0017](0017-transport-health-bounded-writer.md)
**Slice**: [specs/019-peer-session-refactor/slices.md — S3](../../../specs/019-peer-session-refactor/slices.md)

## Context

The CDI download path was the last place in the codebase where a
per-exchange state machine (`datagram_read_exchange` free function + a CDI
chunk loop in `discovery::LccConnection::read_cdi_with_handle`) ran on the
shared transport without the peer-session actor's guarantees. Three shapes
converged there:

1. **Runtime-guarded serialisation.** `bowties-core::cdi_inflight::CdiInflightRegistry`
   held a `Mutex<HashSet<NodeID>>` at the Tauri command boundary to reject
   a second `download_cdi` call for the same node with `CdiDownloadInProgress`.
   The invariant was defended by a runtime check every call, not by the
   type system or the ownership of the exchange.

2. **Global cancel atomic.** `AppState.cdi_download_cancel: Arc<AtomicBool>` was
   flipped by the `cancel_cdi_download` Tauri command; the chunk loop
   polled it between chunks. The atomic was global (no `NodeID`), so
   cancellation could only mean "cancel whatever CDI is running".

3. **No peer-cleanup on our failure.** On our timeout, the exchange
   returned to the app layer with `Error::Timeout`, but the peer never saw
   a `TerminateDueToError`. SPROG-brand nodes interpreted our silence as
   "still waiting" and continued to emit `DatagramRejected 0x2020`
   ("resend OK") for the abandoned request, storming the transport until
   the peer's own timeout expired. `OptionalInteractionRejected`
   (TN-9.7.3.2 §3.4) from the peer was silently coalesced with our timeout
   into a generic `Error::Timeout` — the terminal semantic was lost.

The immediate SPROG regression could have been patched by adding a
`TerminateDueToError` emission in the free function's timeout arm, but
that would have left every other cross-cutting protocol event (mid-exchange
`PeerReinitialised`, `AliasChanged`, `TransportHealth::Wedged`,
`PeerCommand::Cancel`) split between the free function and the actor.
S3's boundary decision was to **finish** the actor: migrate the CDI
state machine into `ActiveExchange::CdiDownload` so a single seam owns
every failure and cleanup class for every protocol on every peer.

## Decision

### D1 — CDI state-machine location: ported into the actor

The CDI datagram-exchange state machine lives inside
`PeerSession::ActiveExchange::CdiDownload`, extending the pattern S2
established for SNIP + PIP. This adds these responsibilities to the
per-peer actor:

- **Chunk request / reply loop.** `send_next_chunk_request` builds a
  `MemoryConfigCmd::build_read` datagram for address space 0xFF, sends
  every constituent GridConnect frame through `TransportHandle::send`,
  refreshes the per-chunk deadline, and resets the reassembler.
- **Reply reassembly.** `on_cdi_frame` routes inbound datagram frames
  through `DatagramAssembler`; on a complete reply datagram it ACKs (sole
  ACK owner per ADR-0016), parses the memory-config reply, advances the
  address cursor, and either issues the next chunk or completes the
  exchange.
- **DR-with-resend-OK retry.** `MTI::DatagramRejected` with bit 12 set
  (`0x2000`) increments the per-chunk retry counter and re-sends the
  current chunk. Cap = `config.max_retries` (default 3). On cap exhaustion,
  the session emits `TerminateDueToError` and completes with
  `PeerError::Rejected`.
- **OIR-terminal classification.** `MTI::OptionalInteractionRejected`
  ends the exchange with `PeerError::Rejected { mti: <wrapped>, code }`
  per TN-9.7.3.2 §3.4. `mti` is the wrapped MTI decoded from the OIR
  payload (first 2 bytes after the destination alias); `code` is the
  error code (next 2 bytes).
- **Per-chunk timeout.** The `run()` `select!` sleeps until the
  `ActiveExchange::CdiDownload.deadline`. On expiry the session emits
  `TerminateDueToError` and completes with `PeerError::Timeout`.
- **Short-read + null-terminator termination.** A reply chunk containing
  a `0x00` byte or with `len == 0` terminates the exchange successfully;
  a peer `error_code == 0x1082` ("address out of bounds") is also
  treated as clean termination.

Consequence: mid-exchange cross-cutting events (`PeerReinitialised`,
`AliasChanged`, `Wedged`, `Cancel`) already dispatched by the actor's
existing arms now automatically apply to CDI without any new plumbing.
The "concurrent CDI downloads on one peer" bug shape becomes
structurally unrepresentable (the actor holds `Option<ActiveExchange>`
— only one at a time). "TerminateDueToError emitted twice on a single
failure" also becomes structurally unrepresentable (a single
`emit_terminate_due_to_error` call in the abort/deadline path per
exchange termination).

### D2 — CDI progress surface: return-on-completion

`PeerSessionHandle::download_cdi` takes no `broadcast::Sender<CdiProgress>`
argument and there is no matching Tauri `cdi-progress` event. Instead, it
returns:

```rust
pub struct CdiCompletion {
    pub bytes: Vec<u8>,
    pub stats: CdiStats,
}
pub struct CdiStats {
    pub total_bytes: usize,
    pub chunks: usize,
    pub chunk_durations_ms: Vec<u32>,
    pub total_retries: usize,
    pub total_duration_ms: u64,
}
```

Rationale: no consumer of a `CdiProgress` event exists today. Adding a
public API surface without a consumer would constrain the future
frontend-streaming feature's design (Clarification 2026-07-05 deferred
frontend streaming to a follow-up). `GetCdiXmlResponse` at the Tauri
boundary is preserved (FR-018); `diag_stats.cdi_downloads` recording
uses fields that mirror the new `CdiStats` shape 1:1.

### `TerminateDueToError` emission policy

The session emits `TerminateDueToError` addressed to the peer **exactly
once** per exchange when the exchange terminates due to our fault on a
live wire:

| Failure class | Emit cleanup? |
|---|---|
| `PeerError::Timeout` (per-chunk deadline)             | Yes |
| `PeerError::Cancelled` (`PeerCommand::Cancel`)         | Yes |
| `PeerError::Rejected` (DR cap exhaustion or OIR)       | Yes |
| `PeerError::TransportUnhealthy` (mid-exchange Wedged)  | **No** — wire is dead (ADR-0017 D1) |
| `PeerError::PeerReinitialised`                         | No — peer already released exchange state |
| `PeerError::AliasChanged`                              | No — same rationale |
| `PeerError::Protocol` (broadcast Lagged, parse error)  | Case-by-case; today: not emitted (D3 rationale) |
| Reassembly error (invalid datagram sequence)          | Yes — the peer thinks we're still receiving |

The error code payload for `TerminateDueToError` uses `0x0200`
("temporary error, no additional information") for our-side timeouts
and cancels, and forwards the peer's own error code for
peer-initiated rejections.

### `CdiInflightRegistry` retirement

`bowties-core::cdi_inflight::CdiInflightRegistry` and `CdiInflightGuard`
are deleted. The invariant "one CDI download in flight per peer" is now
structural — the actor holds `Option<ActiveExchange>`, and a second
`PeerCommand::DownloadCdi` while the current exchange is active is
queued FIFO in `cdi_pending: VecDeque<(MemoryReadConfig, oneshot::Sender<CdiResult>)>`
until the current exchange completes. `AppState::cdi_inflight` and
`AppState::cdi_download_cancel` fields are removed.

`cancel_cdi_download` is rewired to snapshot every `PeerSessionHandle`
from the registry and dispatch `PeerCommand::Cancel` to each,
preserving the pre-refactor global-cancel semantic without an
`AtomicBool` at the app layer (S3 T6 sub-decision (ii)).

## Consequences

**Positive**:

- SPROG CDI regression: the peer-cleanup contract stops the
  `DatagramRejected 0x2020` bursts *after* a timeout (our timeout emits
  `TerminateDueToError`, so the peer releases its exchange state). **This did
  NOT close the SPROG download failure — see the 2026-07-18 root-cause
  correction extension below; the true root cause was a serial `\r\n` framing
  bug, fixed independently. This bullet's mechanism reduces the blast radius of
  a timeout; it is not the SPROG fix.**
- OIR becomes a first-class terminal event with wrapped-MTI decode.
- Every cross-cutting event (`PeerReinitialised`, `AliasChanged`,
  `Wedged`, `Cancel`) that S2 added for SNIP+PIP now automatically
  applies to CDI — no new per-protocol plumbing.
- Per-peer FIFO serialisation and single-active-exchange are
  structural, not runtime-guarded.
- The Tauri command surface is preserved: `download_cdi` returns
  `GetCdiXmlResponse` unchanged; `cancel_cdi_download` still takes no
  arguments; `diag_stats.cdi_downloads` recording preserved.

**Negative**:

- `PeerError::Rejected.mti` widened from `u16` to `u32` to hold the
  full 17-bit MTI value from `MTI::value()`. Serialised wire shape
  changes for that variant, but no external consumer of the variant
  exists today (S2 shipped without any code populating it).
- `discovery::read_cdi_cancellable_with_stats` is deprecated (retained
  as a compatibility shim for external `LccConnection` consumers;
  retired in S5).
- The peer-cleanup emission uses `TransportHandle::send` (mpsc-buffered
  writer). On a slow serial gateway serving multiple peers concurrently,
  the emission can be delayed behind unrelated queued frames. This is
  the same trade-off S2 accepted for SNIP+PIP; if a future performance
  audit shows the delay costs enough peer-cleanup latency to matter,
  the emit path can be swapped to `send_direct` in a targeted change
  (documented here as a known lever).

## Alternatives considered

**Option A′ — Retain `datagram_read_exchange`, add `TerminateDueToError` at its call sites.**
Rejected: leaves the exchange lifecycle split between the free function
and the actor. Every cross-cutting event that S2 centralised in the
actor would need a new "poke the free function to abort" arm. Would
have to be redone in S4 (config r/w) and S6 (event_router).

**Option B′ — Emit `CdiProgress` via a `broadcast::Sender` argument.**
Rejected: no consumer today; the frontend already renders progress
from diagnostics. Adding the API would constrain the future streaming
feature's design without solving a current problem.

**Option C′ — Keep `CdiInflightRegistry` as a belt-and-braces guard.**
Rejected: the structural invariant supersedes the runtime check. Two
layers defending the same invariant is a documented anti-pattern
(ADR-0016 §Alternatives considered).

## Cross-references

- **ADR-0015** — `LayoutState.record_captured(node_id, CapturedNode { cdi_xml: Some(bytes), .. })`
  contract preserved: `commands/cdi.rs::download_cdi` still records
  the captured CDI after successful acquisition.
- **ADR-0016** — extends `§ Invariants` with "CDI exchange lives inside
  `ActiveExchange::CdiDownload`"; upholds single-ACK-owner + single-active-
  exchange invariants for CDI.
- **ADR-0017** — mid-exchange `Wedged` abort skips cleanup emission
  (same rationale as SNIP/PIP: the wire is dead).

## References

- OpenLCB Standard 9.7.2.1 — `TerminateDueToError` obligation on our
  failure.
- OpenLCB Technical Note 9.7.3.2 §3.4 — `OptionalInteractionRejected`
  terminal semantics with wrapped MTI.
- `lcc-rs/src/peer_session.rs` — implementation.
- `lcc-rs/tests/peer_session_cdi.rs` — behavioural pinning tests
  (a)–(g) per S3-T1.
- `app/src-tauri/src/commands/cdi.rs` — thin intent-translator rewrite.

## 2026-07-14 extension: Reply-identity invariant + assembler-error recovery

**Problem observed.** Re-downloading CDI from a real peer (`Blocks
Detection`, alias `0x3AE`) via a **SPROG USB-LCC serial adapter**
fails partway through with `CDI datagram reassembly failed: Protocol
error: DatagramMiddle from unknown source 3AE`. The same peer via an
**RR-CirKits Buffer LCC** succeeds. All 8 S3 integration tests pass —
the mock transport in tests injects frames one-at-a-time via a
controlled API and does not exercise stale/duplicate/interleaved
frames the way slow-serial hardware does.

**Root cause.** The S3 port moved the CDI exchange into
`PeerSession` but left the exchange without an explicit **reply-
identity contract** — nothing verified that a reply belonged to the
request currently in flight. The legacy `datagram_read_exchange` had
two defenses at [lcc-rs/src/datagram_reader.rs:249-268](lcc-rs/src/datagram_reader.rs):
(a) stale-reply detection via echoed-address comparison, and (b)
assembler-error recovery (ACK + re-arm rather than fatal). Both were
dropped without transferring ownership. Under slow-serial pressure a
residual/retransmitted/interleaved reply from a prior request
generation reaches `on_cdi_frame` when `send_next_chunk_request` has
just reset the assembler → `DatagramMiddle` arrives without its
preceding `First` in the current assembler → previous code fatally
errored the exchange and emitted `TerminateDueToError(0x0200)`.

**Contract added.** `PeerSession` becomes the single owner of exchange
reply-identity, not just cursor state. Two changes in
[lcc-rs/src/peer_session.rs::on_cdi_frame](lcc-rs/src/peer_session.rs):

1. **Reply-address check** (on `ReadReply::Success`). Destructure
   `address` from the parsed reply; compare against
   `active.address_cursor`. On mismatch: ACK already sent above,
   discard the reply content (do not extend `assembled`, do not
   advance cursor), do not `send_next_chunk_request`, do not
   `complete_cdi`. The exchange stays in-flight; the next legitimate
   reply reaches this point again.

2. **Recoverable assembler-error path**. On
   `assembler.handle_frame(...) → Err(_)`, call
   `assembler.clear_source(self.alias)` to reset the in-progress
   buffer for the peer, and return. Do NOT emit
   `TerminateDueToError` — the peer did nothing fatally wrong; a
   stray/residual frame reached us. The exchange stays in-flight; the
   next legitimate `DatagramFirst` re-establishes state.

Both changes preserve the peer-cleanup contract in §
"`TerminateDueToError` emission policy": timeout, cancel, DR-terminal,
OIR-terminal, and other our-fault classes still emit `TDE` exactly
once. Only the stale-reply and stale-Middle classes now recover
silently.

**Rejected alternatives** (per architecture-first-fix analysis):
- **Loosen `DatagramAssembler::handle_frame` `Err` to a warning +
  continue** — cannot honestly name a regression class prevented;
  suppresses the symptom without owning stale-reply detection
  anywhere. Stopgap.
- **Narrow `DatagramAssembler` to a per-exchange, single-source
  reassembler** — legitimate deepening of the assembler contract
  (Depth / SOLID/ISP / YAGNI), but does NOT prevent the observed bug
  because a stale reply from the SAME peer still fires the same class
  of error. Deferred as a follow-up.
- **Reinstate `CdiInflightRegistry` or app-layer mutex** — reverses
  the S3 structural decision.

**Test coverage** (added to `lcc-rs/tests/peer_session_cdi.rs`):
- `cdi_mismatched_reply_address_is_acked_and_discarded_no_fatal` —
  peer replies with `ReadReply.address` that does not match
  `address_cursor`; session ACKs, discards, does not advance, does
  not error, resumes on the next legitimate reply.
- `cdi_stale_datagram_middle_from_peer_is_recoverable` — extra
  `DatagramMiddle` after chunk N's Final and before chunk N+1's
  First; session recovers via `clear_source` and does not emit
  `TerminateDueToError`.

Suite: 10 CDI integration tests passing (8 pre-existing + 2 new).

**Follow-up** (deferred, captured for future work):
- Narrow `DatagramAssembler` API to single-source-per-instance since
  it lives inside a per-peer session — the multi-source HashMap is
  dead-code inheritance from the shared free-function era. Governed
  by `aiwiki/seams.md § Peer Session Ownership` if promoted to work.
- If `lcc-rs` grows a diagnostic emitter, the recoverable assembler-
  error path should emit a "stale-datagram-frame-discarded" event so
  operators can see how often SPROG-class residuals fire in the wild.
  Silent recovery today.

## 2026-07-18 extension: Config read/write exchange ownership (S4)

**Context.** S4 closed a data-loss regression: config read/write reached a
peer's alias-keyed `DatagramAssembler` on the bare `TransportHandle`
(`LccConnection::batch_reader` / `read_memory` / `write_memory`), un-serialized
against a concurrent `download_cdi` running inside the actor. The two paths
interleaved into one buffer → duplicate `DatagramReceivedOk` ACKs (52 ACKs for
~27 reply datagrams observed on a RR-CirKits Tower-LCC), cursor collision, and a
partial CDI that clobbered a good cached file. Root cause: the datagram memory
exchange (ACK obligation, reassembly cursor) was owned by the app layer on an
un-serialized handle, violating ADR-0016 "single ACK owner / single active
exchange per peer".

**Decision — D1=A (single-datagram exchange primitives in the actor).** The
per-peer actor gains two first-class exchanges:

- `PeerCommand::ReadMemory { space, address, count, timeout_ms, reply }` →
  `ActiveExchange::MemoryRead` — one memory-config read datagram round-trip
  (arbitrary address space via `AddressSpace::from_u8`), reply-identity guarded,
  sole-ACK-owner, returns `(Vec<u8>, MemoryReadTiming)`.
- `PeerCommand::WriteMemory { space, address, data, timeout_ms, reply }` →
  `ActiveExchange::MemoryWrite` — RequestWithNoReply pattern (`DatagramReceivedOk`
  = success), resend-OK DR retry to `WRITE_MEMORY_MAX_RETRIES`, `>64`-byte
  payloads chunked by `PeerSessionHandle::write_memory` into sequential single
  datagrams.

Both are wired into every actor match arm (`run()` deadline, `handle_command`,
`handle_inbound_frame`, `handle_inbound_lag` bounded recovery, `handle_deadline`,
`abort_active` fault-nature cleanup gate, `drain_parked_after_completion`
unified `mem_pending` FIFO). This extends ADR-0016 §Invariants: **every memory
exchange to a peer — CDI, config read, config write — is a first-class
`ActiveExchange` serialized behind the single per-peer FIFO.** The
CDI-element-driven batch planning (`build_read_plan`, batching, `fill_short_reply`
continuation, per-element parsing, progress) stays app-side in
`commands/cdi.rs` — it is a CDI/app concern, not protocol.

**Rejected — B** (full multi-descriptor batch exchange in `lcc-rs`): drags
CDI-element-driven batch grouping into the protocol library, a Locality
violation ADR-0016 draws an explicit boundary against, and still would not cover
the single-read/write/sync_panel paths. **Rejected — C** (opaque `BatchReader`
sub-loop inside the actor): bypasses the `select!` loop for the whole read, so a
`Wedged`/lag would be unobservable until the per-batch timeout — reintroducing
the ADR-0017 "unobservable multi-second hang" class.

**Decision — D2=ii (full interleave-class closure).** All bare-handle memory
call sites now dispatch via `AppState::peer_session(node_id)` (the single DRY
resolver): `commands/cdi.rs` (`read_config_value`, `read_all_config_values`,
`read_memory_with_retry`, `write_config_value` + 2 more write sites) **and**
`commands/sync_panel.rs` (targeted read + write). The dead `BatchReader` /
`BatchReadResult` / `batch_reader` / `batch_reader_with_config` are deleted from
`lcc-rs::discovery`; `read_memory` / `read_memory_timed` / `write_memory` and the
plain `BatchReadDescriptor` data struct are retained. `send_update_complete`
stays on the bare connection — it is not a read/write primitive.

**Fix 2 — CDI cache-write guard (app-layer).** `is_usable_cdi(&str)`
(`MIN_USABLE_CDI_BYTES = 64`, non-empty, contains `</cdi>`) gates the cache
write: `write_cdi_to_cache` refuses to overwrite a good cache with an invalid
payload, and `download_cdi` surfaces a short/partial read as `RetrievalFailed`
**before** `record_captured` or cache write. Even after serialization removes the
interleave root cause, a legitimately-failed read must never destroy a good
cached CDI. The guard is app-layer because "what counts as a usable cached CDI"
is a Bowties concern (ADR-0015 capture invariant preserved).

**Test coverage** (`lcc-rs/tests/peer_session_config.rs`, 7 tests):
`read_memory_returns_bytes_and_timing`, `write_memory_completes_on_datagram_received_ok`,
`two_concurrent_reads_serialise_no_interleave` (FIFO ordering),
`concurrent_cdi_and_read_no_interleave_one_ack_each` (the 2026-07-18 regression —
exactly one ACK per reply, no cross-contamination), `read_memory_timeout_emits_one_terminate_due_to_error`,
`read_memory_oir_returns_rejected`, `read_memory_wedged_returns_transport_unhealthy_without_cleanup`.
Fix 2: 7 unit tests in `commands/cdi.rs` `cdi_cache_guard_tests`.

**Hardware-deferred ACs**: "bytes identical to pre-refactor on a modulino" and
"values persist on the node" are covered structurally by the mock-transport
tests; confirmable only on hardware.

## 2026-07-18 extension: SPROG CDI root-cause correction (spec 019 S10)

**Correction.** The original Context and the "Positive" Consequences bullet of
this ADR attributed the SPROG USB-LCC CDI regression to the ownership gaps this
refactor closes ("closed by construction"). **That attribution is wrong.** The
SPROG download failure was a serial **`\r\n` framing bug**: Bowties appended
CR/LF after every `;`-terminated GridConnect frame on serial, while JMRI (the
reference implementation these adapters target) sends none. SPROG USB-LCC v1.4's
changed FTDI buffer handling cannot tolerate the extra bytes/frame under CDI
load (most likely a PIC UART RX overrun → `OERR` → reception halts until power
cycle). Removing the trailing bytes fixed it completely — `cdi-probe` 10/10 at
`--post-ack-delay-ms 0`, 0 retries, no power cycle — **independent of this
refactor** (S3 never touches serial framing). The fix lives in
[gridconnect_serial.rs](../../../lcc-rs/src/transport/gridconnect_serial.rs)
(`send()` on both the `LccTransport` and `TransportWriter` paths emits the
`;`-terminated frame with no trailing CR/LF on serial; TCP GridConnect hubs are
line-oriented and legitimately keep `\n`).

**What S3 actually contributes.** The peer-cleanup contract
(`TerminateDueToError` on our-fault termination), single ACK owner per peer,
OIR-terminal handling, and per-peer FIFO serialisation are all real, independent
correctness improvements. Several of the pre-refactor defects (duplicate ACKs,
the mystery mid-flight read, the second SNIP+PIP burst) put **extra frames on
the wire**, so they almost certainly *compounded* the SPROG framing failure —
the refactor reduces wire pressure and stops the `DatagramRejected 0x2020`
bursts that follow a timeout. But S3's value is architectural blast-radius
reduction, **not** the SPROG root-cause fix.

**Why this correction matters.** Left uncorrected, this ADR would teach the next
maintainer the same wrong causal model (concurrency/ownership = the SPROG bug)
that motivated the refactor — a model the 2026-07-18 investigation disproved
(JMRI itself uses concurrent tx/rx threads and RTS/CTS at 460800; our
concurrency and flow control match the reference).

**References.** [../../../temp/SESSION-HANDOFF-2026-07-18.md](../../../temp/SESSION-HANDOFF-2026-07-18.md)
(full analysis + scorecard); ADR-0017 §2026-07-18 extension (transport-model
ownership); the S3 Behavior Summary + AC1 corrections in
[../../../specs/019-peer-session-refactor/slices.md](../../../specs/019-peer-session-refactor/slices.md).

