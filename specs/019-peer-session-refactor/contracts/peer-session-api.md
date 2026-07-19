# Contract: `PeerSession` Public API

**Module**: `lcc-rs::peer_session` + `lcc-rs::peer_session_registry`
**Consumer**: `bowties-core::node_proxy::LiveNodeProxy`, `app/src-tauri/src/commands/*`

---

## Registry lookup

```
PeerSessionRegistry::get(&self, node_id: NodeID) -> Option<PeerSessionHandle>
```

- Returns `None` if the peer has not yet been observed emitting a full NodeID (via VNI, InitComplete, or AMD).
- Returns `Some(handle)` cheaply (single read-lock + clone).
- **Never** spawns. Callers that need a session for a not-yet-discovered peer must wait for discovery (or use a `Synthesized` proxy shell, unchanged from today).

## Session command dispatch

Callers should prefer the typed convenience methods on `PeerSessionHandle`:

```
handle.query_snip()                                        -> Result<Option<SnipData>, PeerError>
handle.query_pip()                                         -> Result<Option<PipData>, PeerError>
handle.download_cdi(config: MemoryReadConfig)              -> Result<CdiCompletion, PeerError>
handle.read_memory(space, addr, count, timeout_ms)         -> Result<(Vec<u8>, MemoryReadTiming), PeerError>
handle.write_memory(space, addr, data, timeout_ms)         -> Result<(), PeerError>
handle.cancel(reason)                                      -> ()
```

### `read_memory` / `write_memory` (S4 amendment 2026-07-18 — D1=A outcome)

Per S4 mid-slice escalation D1=A (2026-07-18), config read/write are single-datagram
exchange primitives on the actor (not a full `read_config`/`write_config` multi-descriptor
exchange in `lcc-rs`). `read_memory` performs one memory-config read round-trip against an
arbitrary address space (`AddressSpace::from_u8`), returning the raw reply bytes plus
`discovery::MemoryReadTiming`. `write_memory` follows the RequestWithNoReply pattern
(`DatagramReceivedOk` = success, resend-OK DR retry to `WRITE_MEMORY_MAX_RETRIES`); payloads
`>64` bytes are chunked by the handle method into sequential single-datagram writes. Both are
first-class `ActiveExchange` variants (`MemoryRead` / `MemoryWrite`) serialized behind the
per-peer FIFO alongside `CdiDownload`. CDI-element-driven batch planning stays app-side in
`commands/cdi.rs` (Locality — see ADR-0018 §2026-07-18 extension). Earlier drafts of this
contract named these `read_config` / `write_config` with a `Vec<u8>` return; that speculative
shape is superseded.

### `CdiCompletion` (S3 amendment 2026-07-08 — D2 outcome)

Per S3 HITL decision D2 (2026-07-08), `download_cdi` returns its completion
payload synchronously; there is **no** `broadcast::Sender<CdiProgress>`
argument and **no** matching Tauri `cdi-progress` event surface. The
frontend already renders progress from `CdiDownloadStats` at the diagnostics
boundary today; frontend-streaming is deferred to a follow-up feature per
Clarification 2026-07-05.

```rust
pub struct CdiCompletion {
    pub bytes: Vec<u8>,        // assembled CDI XML (up to and excluding first NUL)
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

### Behavioural guarantees

- **Per-peer serialization**: two concurrent commands to the same handle are serialised FIFO inside the session; only one `ActiveExchange` at a time.
- **Cross-peer parallelism**: commands to different handles run in parallel on separate tasks.
- **Coalescing**: two concurrent `query_snip` (or `query_pip`) callers share a single wire exchange; both receive the same result. A cached result returns immediately without wire traffic.
- **Cache invalidation**: caches clear on `PeerCommand::PeerReinitialised` (delivered by the transport when the peer emits VNI/InitComplete after having a session).
- **Peer cleanup**: on our timeout, our cancel, or terminal peer rejection, the session emits `TerminateDueToError` to the peer before completing the reply.

## `PeerError` — stable string prefixes for the frontend

Serde-serialised discriminants (matching FR-018 — no existing string tag broken):

- `"Timeout"` — with `operation` and `elapsed_ms`.
- `"PeerReinitialised"`.
- `"AliasChanged"` — with `old` and `new`.
- `"TransportUnhealthy"` — with nested `TransportHealth` payload.
- `"Rejected"` — with `mti: u32` (17-bit MTI value from the rejecting/wrapped message) and `code: u16` (error code from the DR/OIR payload). Widened from u16 → u32 in S3 (2026-07-08) so the full 17-bit MTI storage matches `lcc_rs::MTI::value()`.
- `"Cancelled"` — with `reason`.
- `"NotSupported"` — with `operation`.
- `"Protocol"` — with human-readable diagnostic string.

## Concurrency contract

- `PeerSessionHandle` is `Clone + Send + Sync`.
- All command methods are `async` and return futures that may be awaited from any tokio task.
- Dropping the last handle for a NodeID does NOT terminate the session; the session lives as long as the registry retains its entry (i.e., until transport disconnect or explicit `remove`).

## Test hooks

For deterministic testing (see `tests/` directory of `lcc-rs`):

- `PeerSession::new_for_test(node_id, alias, transport_stub) -> (Self, PeerSessionHandle)` — bypasses the registry spawn-watcher; used only by unit tests.
- `PeerSessionRegistry::new_empty_for_test() -> Self` — no transport subscription; tests inject sessions manually.
