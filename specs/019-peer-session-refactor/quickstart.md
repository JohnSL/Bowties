# Quickstart — Peer Session Refactor

**Feature**: 019-peer-session-refactor
**Audience**: developer implementing or reviewing a slice

This is a per-slice checklist. Run it at the *start* of a slice (baseline) and *end* of a slice (exit).

---

## Baseline (before starting any slice)

```powershell
# From repo root:
cargo test -p lcc-rs
cargo test -p bowties-core
cargo test --manifest-path app/src-tauri/Cargo.toml
npm --prefix app test
```

All four MUST pass. If any fail on the untouched branch, fix or document before starting.

---

## Slice 1 — Transport writer bounded send + health broadcast

**Files**: `lcc-rs/src/transport_actor.rs`

**New tests (Red first — via `tdd-cycle` subagent):**
- `writer_bounded_send_emits_wedged_on_stall`
- `writer_returns_to_healthy_after_stall`

**Exit verification**:
```powershell
cargo test -p lcc-rs transport_actor
# All four baseline suites still pass.
```

**Manual**: Connect Bowties to a SPROG USB-LCC, start a CDI download on a real node, physically unplug the SPROG cable. Expected: within ~500ms, a `Wedged` log line appears; subsequent commands return `TransportUnhealthy` promptly (not a hang).

---

## Slice 2 — PeerSession scaffolding + SNIP migration

**Files**: `lcc-rs/src/peer_session.rs` (new), `lcc-rs/src/peer_session_registry.rs` (new), `lcc-rs/src/snip.rs` (modified), `lcc-rs/src/lib.rs` (exports), `bowties-core/src/node_proxy.rs` (delegate SNIP), `app/src-tauri/src/state.rs` (add `sessions` field).

**New tests** (moved from `snip.rs` + extended):
- SNIP query returns cached result after first success.
- Concurrent `query_snip` callers on the same peer coalesce into one wire exchange.
- `PeerReinitialised` clears the SNIP cache.
- Registry spawns a session on VNI, InitComplete, and AMD; ignores PCER/EventReport.
- Registry updates alias in place on second AMD (no duplicate session).

**Exit verification**:
```powershell
cargo test -p lcc-rs
cargo test -p bowties-core
cargo test --manifest-path app/src-tauri/Cargo.toml
npm --prefix app test
```

All four MUST pass. `LccConnection::query_snip` still callable (as shim).

**Manual**: Discover a real node; assert SNIP populates in the UI unchanged.

---

## Slice 3 — PIP migration

**Files**: `lcc-rs/src/peer_session.rs` (add `QueryPIP`), `lcc-rs/src/pip.rs` (shim), `bowties-core/src/node_proxy.rs` (delegate PIP).

**New tests** (moved from `pip.rs` + extended):
- PIP query returns cached result after first success.
- Concurrent `query_pip` callers coalesce.
- `PeerReinitialised` clears PIP cache.

**Exit verification**: All four baseline suites pass.

---

## Slice 4 — CDI download + peer cleanup + OIR handling

**Files**: `lcc-rs/src/peer_session.rs` (add `DownloadCDI` + `ActiveExchange::CdiDownload`), `lcc-rs/src/datagram_reader.rs` (OIR terminal + peer cleanup on our timeout), `lcc-rs/src/discovery.rs` (`read_cdi_cancellable_with_stats` becomes shim), `bowties-core/src/cdi_inflight.rs` (**DELETE**), `app/src-tauri/src/state.rs` (drop `cdi_inflight` + `cdi_download_cancel`), `app/src-tauri/src/commands/cdi.rs` (rewrite `download_cdi` — call session, then `layout_state.record_captured` unchanged).

**New tests** (Red-first, then Green):
- `cdi_timeout_emits_terminate_due_to_error`
- `cdi_oir_response_is_terminal_error`
- `cdi_permanent_datagram_rejected_is_terminal`
- `cdi_transient_datagram_rejected_retries_up_to_max_attempts`
- `cdi_cancel_mid_download_emits_terminate_and_returns_cancelled`
- `two_concurrent_download_cdi_commands_serialise_via_session` (no more inflight registry)

**Exit verification**:
```powershell
cargo test -p lcc-rs peer_session
cargo test -p bowties-core           # cdi_inflight tests removed with module
cargo test --manifest-path app/src-tauri/Cargo.toml
npm --prefix app test
```

**Manual (main bug closure)**: modulino_io CDI download over SPROG completes end-to-end without user intervention. `CdiInflightRegistry` no longer exists. Peer never wedges after Bowties timeout.

---

## Slice 5 — Config read/write migration

**Files**: `lcc-rs/src/peer_session.rs` (add `ReadConfig` + `WriteConfig`), `lcc-rs/src/discovery.rs` (`BatchReader` moves into session as private helper), `app/src-tauri/src/commands/cdi.rs` (rewrite config-read/write commands).

**New tests**:
- Serial `ReadConfig` commands to the same peer serialise.
- Parallel `ReadConfig` commands to different peers run in parallel.
- `WriteConfig` timeout emits `TerminateDueToError`.

**Exit verification**: All four baseline suites pass. Config editor in the UI reads and writes unchanged.

---

## Slice 6 — Outbound consolidation + `send_direct` retirement

**Files**: `lcc-rs/src/transport_actor.rs` (remove `send_direct`, `direct_write_count`), `lcc-rs/src/datagram_reader.rs` (remove `use_send_direct` param), `lcc-rs/src/event_router.rs` (**NEW** — lift classification core from `app/src-tauri/src/events/router.rs`), `app/src-tauri/src/events/router.rs` (becomes thin Tauri-emit adapter).

**New tests**:
- `event_router_fans_out_pcer_to_registered_subscribers`
- `event_router_ignores_non_event_frames`
- `no_production_caller_of_send_direct` (compile-time — `send_direct` no longer public).

**Exit verification**: All four baseline suites pass. `grep -r send_direct lcc-rs bowties-core app/src-tauri` returns no hits in production code.

---

## Success criteria checklist (from spec)

Tick each after Slice 6 exit:

- [ ] CDI download completes end-to-end (modulino_io) under back-pressure / concurrency / error paths. *(The SPROG download failure was closed by the serial `\r\n` framing fix, not the refactor architecture — see spec.md Context correction 2026-07-18.)*
- [ ] `cdi_inflight`, `cdi_download_cancel`, `send_direct`, `direct_write_count` all removed.
- [ ] No new local fixes in retired architecture.
- [ ] All existing tests pass at every slice boundary.
- [ ] New tests cover per-peer serialization, peer cleanup on all error paths, coalescing, transport-health.
- [ ] Frontend behaves identically (progress events, transport-health log, faster failure surfacing are only improvements).
- [ ] ADR-0016 authored and merged.
- [ ] `aiwiki/owners.md`, `aiwiki/flows.md`, `aiwiki/seams.md` enriched.
- [ ] `specs/backlog.md` updated (remove items this refactor resolves; add follow-up idea for Slice 7 as `kind/idea` GitHub issue).
