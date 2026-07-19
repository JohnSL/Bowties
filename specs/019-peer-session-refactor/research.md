# Phase 0 — Research & Technical Decisions

**Feature**: 019-peer-session-refactor
**Date**: 2026-07-05

The spec is fully clarified (5 clarification questions answered on 2026-07-05); the classic "NEEDS CLARIFICATION" set is empty. Remaining work for Phase 0 is to lock in **implementation-level decisions** flagged as plan-phase in the spec's clarifications, and to record research done to ground each decision.

---

## D1. `PeerSessionRegistry` concurrency primitive

**Decision**: `tokio::sync::RwLock<HashMap<NodeID, PeerSessionHandle>>`.

**Rationale**:
- Sole writer is the registry's own broadcast-subscriber task (per FR-017 / Clarification 4). One writer, many readers = the exact shape `RwLock` is optimised for.
- `PeerSessionHandle` is `Clone` (wraps `mpsc::Sender` + shared metadata), so reads take a short lock, clone the handle, drop the lock. No await while holding the guard.
- `DashMap` was considered but adds a sync-primitive dependency and does not offer async-aware fairness; it also complicates iteration for `clear()` on disconnect.
- Plain `Mutex` was considered but blocks reads unnecessarily during rare writes (initial discovery burst is ~10 peers within ~100ms).

**Alternatives considered**:
- `DashMap` — rejected: sync primitive, no async fairness, extra dep.
- `tokio::sync::Mutex` — rejected: unnecessary contention on reads.
- Lock-free (crossbeam `SkipMap`) — rejected: YAGNI, no measured hotspot.

**Risk mitigation**: Per user memory `tokio-rwlock-self-deadlock.md`, we MUST NOT hold `sessions.write()` while awaiting anything that might call `sessions.read()`. Registry mutation pattern: acquire write → mutate → drop → then dispatch further work. Session spawn also follows this: build the session's `mpsc::Sender` first (no lock), then take write lock only for the map insert.

---

## D2. `EventRouter` placement — `lcc-rs` vs `app/src-tauri`

**Decision**: **Move the classification/fan-out core into `lcc-rs::event_router`**; keep a thin Tauri-emit adapter in `app/src-tauri/src/events/router.rs` that subscribes to the core router and forwards to `AppHandle::emit`.

**Rationale**:
- Event-report classification (matching PCER, EventReportWithPayload, Identify Events, Learn Event by MTI and event ID) is protocol behaviour — the same rules would apply to any LCC/OpenLCB consumer, matching the code-placement rule "if it would matter to other LCC consumers, prefer `lcc-rs`."
- Tauri-event emission is app glue and stays in `app/src-tauri` (ADR-0015 boundary discipline: no Tauri types leak into `lcc-rs`).
- The spec's data-model text ("New in `lcc-rs::event_router`") anticipates this split; codifying it here removes ambiguity for Slice 6.

**Alternatives considered**:
- Leave everything in `app/src-tauri` and rename spec references — rejected: keeps protocol classification tied to app glue, which is what the refactor is trying to fix. Would leave a shallow module.
- Move everything including Tauri emit into `lcc-rs` — rejected: leaks `AppHandle` / Tauri types into the protocol library.

**Migration**:
- In Slice 6, extract the classification + subscription API from `app/src-tauri/src/events/router.rs` L54–L341 into `lcc-rs::event_router::EventRouter`.
- Leave `app/src-tauri/src/events/router.rs` as a thin wrapper that instantiates `EventRouter`, registers a Tauri-emit subscriber, and holds any Tauri-specific config.

---

## D3. `PeerCommand::WriteConfig` scope (Slice 5)

**Decision**: Include `WriteConfig` in Slice 5 alongside `ReadConfig`. Scope: single-region datagram write to a configuration-memory address, matching the existing capability exposed via `write_memory_timed`.

**Rationale**:
- Datagram write logic already exists in `lcc-rs` (used indirectly by config-write Tauri commands via `write_memory_timed` / `LccConnection::write_memory`). No net-new protocol work.
- Splitting reads-only into Slice 5 and writes-later would leave the Tauri command layer in a half-migrated state (`read_config` via session, `write_config` still via free function). That violates the "each slice ends in a state where the app runs end-to-end" invariant.
- Peer cleanup on write timeout is identical to read timeout — same `TerminateDueToError` obligation.

**Alternatives considered**:
- Defer writes to a follow-up feature — rejected: leaves the shim retirement (Slice 6) unable to remove `write_memory_timed`, forcing an incomplete Slice 6.

---

## D4. `SERIAL_SEND_TIMEOUT` values

**Decision**:
- Serial transports: `500ms` per `w.send(&frame).await`.
- TCP transports: `2000ms` per `w.send(&frame).await`.

**Rationale**:
- Serial (SPROG USB-LCC with RTS/CTS flow control) exhibits the wedge-on-full-buffer behaviour; a fresh CAN frame at 500 kbps takes ~200µs on-wire, so 500ms is 2500× wire time — plenty of headroom for legitimate back-pressure but well under any user-perceptible hang.
- TCP has kernel-level flow control; a 2s timeout differentiates "adapter dead / far side unreachable" from normal back-pressure without false alarms on WAN-scale latency spikes.
- Values are `const` in `transport_actor.rs`; selected at transport-construction time based on which `TransportWriter` impl is instantiated.

**Alternatives considered**:
- Single unified 1s timeout — rejected: too tight for TCP over some links, too loose to catch serial wedge fast.
- Configurable at runtime — rejected: YAGNI. Add if a real user reports needing tuning.

---

## D5. Transport broadcast retention (Slice 6)

**Decision**: **Retain the transport inbound broadcast unchanged** (per Clarification 3). `TransportHandle::subscribe_all` (or equivalent) stays as a public API.

**Rationale** (from Clarification 3, recorded here for the ADR):
- In Rust, filter-at-consumer is essentially free (broadcast fan-out is a `tokio::broadcast::Sender::send` with per-receiver ring buffer).
- The bugs the refactor targets are consumer-side ownership problems (duplicate ACKs, redundant SNIP/PIP bursts, mid-flight unexpected reads, missing peer cleanup) — not delivery-shape problems.
- Enforcing single-owner rules at consumers (`PeerSession` sole ACK owner + sole outbound sender; `EventRouter` sole event-report fan-out; `NetworkSession` sole bus-membership owner) is sufficient for bug closure.
- Preserves the "add an observer for free" property essential for diagnostic recorders, trace loggers, protocol-conformance tests, and future analytical consumers.

**Slice 6 outbound consolidation instead**: retire `send_direct`, retire `direct_write_count`, confirm every outbound frame is session-owned.

---

## D6. TDD subagent usage

**Decision**: Delegate every Red+Green loop in this feature to the `tdd-cycle` subagent, batched 1–3 behaviors per call (per copilot-instructions "Delegate the Red+Green loop..." rule).

**Rationale**:
- Copilot instructions default to subagent-driven TDD for all production behaviour changes.
- Slices vary in size; batching by 1–3 behaviors keeps subagent context tight and main-window growth constant.
- Refactor phase runs via `tdd-refactor` subagent once tests are green (per `/build` skill).

**Alternative**: inline TDD — rejected except for single trivial one-line behaviours (per copilot-instructions exception).

---

## D7. Reference implementations to consult

Per Constitution IV and copilot-instructions:
- **`OpenLCB_Java/`** — read `PeerLocalState` and `MessageStore` for their peer-tracking model. Notes: OpenLCB_Java tracks peer state per node; our `PeerSession` is a stricter version (single-active-exchange per peer, explicit `TerminateDueToError` on our timeout).
- **`JMRI/`** — read `jmri.jmrix.openlcb.OlcbDatagramReceiveHandler` for datagram-exchange peer-cleanup patterns. JMRI does emit `TerminateDueToError` on receive-side timeout; we adopt the same for send-side timeout.
- **`markdown/standards/TN-9.7.3.2`** — datagram protocol, `TerminateDueToError` and `OptionalInteractionRejected` semantics.
- **`docs/technical/protocol-reference.md`** — Bowties working reference (check first, per Constitution IV reference hierarchy).

**Findings summary** (informs Slice 4 tests):
- `TerminateDueToError` MTI: `0x0A08` (Terminate Due To Error). Payload: 6-byte destination NodeID (per TN-9.7.2.1). Emit when the peer must be told the exchange is aborted.
- `OptionalInteractionRejected` MTI: `0x1068`. Payload: 2-byte error code + 2-byte rejected MTI. Currently ignored by `datagram_read_exchange`. Slice 4 treats it as terminal for the active exchange with a diagnostic error.
- `DatagramRejected` error codes: `0x1000` (permanent) vs `0x2000` (transient — resend OK bit determines retry).

---

## Consolidated NEEDS CLARIFICATION log

**Empty.** All 5 clarification questions in the spec are resolved. All plan-phase implementation details (D1–D7 above) are decided here. Phase 1 may proceed.
