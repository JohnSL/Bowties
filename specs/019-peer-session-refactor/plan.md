# Implementation Plan: Peer Session Actor — Per-Node Protocol Ownership

**Branch**: `019-peer-session-refactor` | **Date**: 2026-07-05 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/019-peer-session-refactor/spec.md`

## Summary

Consolidate all OpenLCB/LCC protocol interactions with a remote node inside a single per-node actor (`PeerSession`) in `lcc-rs`, retiring the pattern of scattered stateless helpers that each grabbed the shared transport. The transport becomes purely wire-level: reader publishes to a broadcast, writer serialises outbound frames FIFO with a bounded per-send timeout. Tauri commands become thin intent translators dispatching to the session registry.

Technical approach: ship as 6 vertical slices (transport health → SNIP → PIP → CDI → config r/w → outbound consolidation), each ending with the codebase compiling, all tests passing, and the app running end-to-end. Old `lcc-rs` free functions (`query_snip`, `query_pip`, `read_cdi`, `read_memory_timed`) stay as compatibility shims that forward to the session until Slice 6 retires them. `LiveNodeProxy` is retained as a thin app-layer aggregator that delegates every protocol call to a `PeerSessionHandle` (per Clarification 1). CDI persistence continues to run in the Tauri command layer, calling `layout_state.record_captured` (per Clarification 2, matching ADR-0015). Transport inbound broadcast is retained; single-ownership is enforced consumer-side (per Clarification 3). Slice 7 (frontend intent single-owner) is explicitly deferred (per Clarification 5).

## Revised scope (2026-07-18)

**The motivating premise was disproven.** This feature was opened on the assertion that the SPROG CDI regression was architectural and that consolidating protocol ownership would fix it. On 2026-07-18 the regression was root-caused to a **serial `\r\n` framing bug** — Bowties appended CR/LF after the `;`-terminated GridConnect frame; JMRI (the reference implementation) sends none, and SPROG USB-LCC v1.4's changed FTDI buffer handling cannot tolerate the extra bytes/frame under CDI load. Removing the `\r\n` fixed it (verified `cdi-probe` 10/10 at `--post-ack-delay-ms 0`, no power cycle). S3 did **not** close the regression. Full analysis: [../../temp/SESSION-HANDOFF-2026-07-18.md](../../temp/SESSION-HANDOFF-2026-07-18.md).

**The refactor is kept on independent merit.** S1–S3 close real bugs that exist on `main` and are unrelated to framing: the writer `Arc<Mutex<Writer>>` deadlock under back-pressure, duplicate `DatagramReceivedOK` ACKs, dropped `OptionalInteractionRejected`, and the missing peer-cleanup contract. The per-peer actor also reduces wire traffic, which helps marginal adapters like the SPROG. The refactored CDI path is now hardware-validated via `cdi-probe`.

**Remaining work is a merge-readiness gate**, tracked in [slices.md](./slices.md): **S7** (close the refactor-introduced broadcast-lag defect + revisit the subscribe-all delivery model) → **S4** (finish config r/w for pattern consistency) → **S8** (retire SPROG-debug scaffolding; default `post_ack_delay_ms` to 0; keep the write-timeout patch + hardware flow control) → **S9** (decide blocking-thread transport vs async + S1 timeout) → **S10** (correct the root-cause claims in spec.md + ADR-0018). **S5 and S6 are deferred** to `kind/idea` follow-ups — pure cleanup with no correctness payoff, not to be done on momentum from the original premise.

## Technical Context

**Language/Version**: Rust 1.75+ (2021 edition, tokio async runtime); TypeScript 5.x (SvelteKit 2.x) frontend.
**Primary Dependencies**: `tokio` (mpsc / broadcast / RwLock / timeout), `thiserror`, `async-trait`, `serde` (backend); Tauri 2 (IPC boundary); SvelteKit 2 + Vitest (frontend).
**Storage**: Layout persistence via existing `LayoutState` (JSON on disk, ADR-0015). No new storage introduced. Cached CDI XML continues to live in `LayoutState`; session hands off assembled bytes.
**Testing**: `cargo test -p lcc-rs`, `cargo test -p bowties-core`, `cargo test --manifest-path app/src-tauri/Cargo.toml`, `npm --prefix app test` (Vitest). TDD via `tdd-cycle` subagent (per copilot-instructions).
**Target Platform**: Windows / macOS / Linux desktop via Tauri 2. TCP + serial (SPROG USB-LCC) transports (Constitution VI restricts new work to TCP; serial is existing scope this refactor stabilises).
**Project Type**: Multi-crate Rust workspace + SvelteKit frontend (existing shape — no structure change).
**Performance Goals**: Serial-transport writer send bounded at `SERIAL_SEND_TIMEOUT` (500ms serial, 2000ms TCP) so a stuck adapter surfaces as `TransportHealth::Wedged` promptly rather than deadlocking. Per-peer sessions run in parallel; per-peer exchanges serialise.
**Constraints**: Every slice boundary compiles and passes all existing tests (FR-020). Tauri command shapes preserved (FR-018). Compatibility shims retained until Slice 6 (FR-019). No `unwrap()` in production paths (Constitution I). No panics in async workers.
**Scale/Scope**: ~10s of peer nodes per active layout. One `PeerSession` actor per discovered peer. Single active exchange per peer at any time (structural invariant of OpenLCB). Refactor touches `lcc-rs/src/{transport_actor,snip,pip,discovery,datagram_reader}.rs`, `bowties-core/src/{node_proxy,node_registry,cdi_inflight}.rs`, `app/src-tauri/src/{state.rs,commands/cdi.rs,events/router.rs}`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Rust 2021+ development | PASS | New modules use tokio async, `thiserror` error enums, no `unwrap()` in production paths. |
| II. Cargo-based development | PASS | No new toolchain, no new top-level crates. `peer_session` and `peer_session_registry` are new modules inside existing `lcc-rs` crate. |
| III. Test-Driven Development | PASS | TDD via `tdd-cycle` subagent for every slice (per copilot-instructions). Each slice defines its Red tests before Green implementation. Property tests preserved on GridConnect / datagram paths. |
| IV. LCC Protocol Correctness | PASS | Refactor consolidates but does not alter wire behaviour except to *add* missing conformance (emit `TerminateDueToError` on our timeout per TN-9.7.2.1; treat `OptionalInteractionRejected` as terminal per TN-9.7.3.2 §3.4). Cite standards in new tests. |
| V. UX-First Design | PASS | Tauri command shapes unchanged (FR-018). New progress events (`CdiProgress`) and transport-health surface improve UX. No regressions to existing flows. |
| VI. TCP-Only Focus | PASS | Serial transport is pre-existing scope (SPROG). No new transports introduced. Bounded writer timeout differentiates serial (500ms) vs TCP (2000ms). |
| VII. Event Management Excellence | PASS | `EventRouter` formalised as bus-scoped sole owner of event-report fan-out (PCER, EventReportWithPayload, Identify/Learn) in Slice 6. Improves current diffuse-ownership state. |

**Post-design re-check (after Phase 1)**: All gates still PASS. The Phase 1 artifacts (`data-model.md`, `contracts/`, `quickstart.md`) reinforce the same conclusions — no protocol change beyond conformance additions, no new dependencies, TDD-first, no violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/019-peer-session-refactor/
├── plan.md              # This file (/speckit.plan output)
├── spec.md              # Feature specification (already exists)
├── research.md          # Phase 0 output — technical decisions log
├── data-model.md        # Phase 1 output — new types & module boundaries
├── quickstart.md        # Phase 1 output — how to run/verify per slice
├── contracts/           # Phase 1 output — PeerCommand / PeerError / TransportHealth
│   ├── peer-session-api.md
│   ├── peer-error-taxonomy.md
│   └── transport-health.md
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created here)
```

### Source Code (repository root)

Existing workspace; no structure change. Files touched or added by this refactor:

```text
lcc-rs/src/
├── transport_actor.rs           # MODIFIED — bounded writer send, TransportHealth broadcast (Slice 1); send_direct retired (Slice 6)
├── peer_session.rs              # NEW — PeerSession actor, PeerCommand, PeerError, ActiveExchange, CdiProgress (Slice 2)
├── peer_session_registry.rs     # NEW — PeerSessionRegistry, PeerSessionHandle; sole spawner (Slice 2)
├── event_router.rs              # NEW — bus-scoped event-report fan-out (Slice 6). Classification core lifted from app/src-tauri.
├── snip.rs                      # MODIFIED — query_snip becomes shim forwarding to session (Slice 2), retired in Slice 6
├── pip.rs                       # MODIFIED — query_pip becomes shim (Slice 3), retired in Slice 6
├── discovery.rs                 # MODIFIED — read_cdi_cancellable_with_stats becomes shim (Slice 4); BatchReader internalised in session (Slice 5)
├── datagram_reader.rs           # MODIFIED — datagram_read_exchange integrated into session; OIR handled as terminal
└── lib.rs                       # MODIFIED — export peer_session, peer_session_registry, event_router

bowties-core/src/
├── node_proxy.rs                # MODIFIED — LiveNodeProxy delegates to PeerSessionHandle; drops snip_waiters/pip_waiters
├── node_registry.rs             # MODIFIED — no longer sole session spawner (registry does that); still spawns LiveNodeProxy shell
└── cdi_inflight.rs              # RETIRED (Slice 4) — module deleted

app/src-tauri/src/
├── state.rs                     # MODIFIED — add AppState.sessions: PeerSessionRegistry; remove cdi_inflight & cdi_download_cancel (Slice 4)
├── commands/cdi.rs              # MODIFIED — download_cdi/read_config/write_config rewritten to dispatch via session
└── events/router.rs             # MODIFIED (Slice 6) — thinned to Tauri-emit adapter around lcc-rs::event_router

app/src/lib/                     # UNCHANGED — Tauri command shapes preserved (FR-018)
```

**Structure Decision**: Multi-crate Rust workspace + SvelteKit frontend, matching the existing repo. All new backend code lives inside `lcc-rs` (protocol behaviour — per code-placement-and-ownership rule: "if a rule would matter to other LCC/OpenLCB consumers, prefer `lcc-rs`"). `bowties-core` is trimmed (per-node protocol state moves out). `app/src-tauri` is trimmed (CDI inflight registry deleted; event router core lifted to `lcc-rs`). Frontend untouched (Slice 7 deferred).

## Phase 0 — Outline & Research

Complete. See [research.md](./research.md) for the seven implementation-level decisions (D1–D7) that resolve plan-phase ambiguity called out by the spec's clarifications:

- **D1**: `PeerSessionRegistry` internal map = `tokio::sync::RwLock<HashMap<NodeID, PeerSessionHandle>>` (sole-writer, many-reader; guarded against the tokio-rwlock self-deadlock pattern per user memory).
- **D2**: `EventRouter` classification core → `lcc-rs::event_router`; Tauri-emit adapter stays in `app/src-tauri/src/events/router.rs`.
- **D3**: `PeerCommand::WriteConfig` included in Slice 5 (Config r/w migrate together so Slice 6 can retire `write_memory_timed`).
- **D4**: `SERIAL_SEND_TIMEOUT` = 500ms serial / 2000ms TCP, const per transport-writer construction.
- **D5**: Transport inbound broadcast retained (Clarification 3); Slice 6 narrows to outbound consolidation only.
- **D6**: All Red+Green loops delegated to `tdd-cycle` subagent (per copilot-instructions).
- **D7**: Reference implementations to consult: OpenLCB_Java (peer state), JMRI (datagram cleanup), TN-9.7.2.1 (`TerminateDueToError`), TN-9.7.3.2 (`OptionalInteractionRejected`, `DatagramRejected` codes).

No `NEEDS CLARIFICATION` remain. Phase 1 proceeded.

## Phase 1 — Design & Contracts

Complete. Artifacts:

- **[data-model.md](./data-model.md)** — `PeerSession`, `PeerCommand`, `PeerError`, `ActiveExchange`, `CdiProgress`, `RetryState`, `PeerSessionRegistry`, `PeerSessionHandle`, `EventRouter`, `TransportHealth`. Modified: `LiveNodeProxy` (delegate), `AppState` (add sessions field, remove cdi_inflight/cancel). Retired: `bowties-core::cdi_inflight`. Includes state-transition diagram for `ActiveExchange` and relationship diagram.
- **[contracts/peer-session-api.md](./contracts/peer-session-api.md)** — Registry lookup, typed convenience methods on `PeerSessionHandle`, behavioural guarantees (per-peer serialization, cross-peer parallelism, coalescing, cache invalidation, peer cleanup), test hooks.
- **[contracts/peer-error-taxonomy.md](./contracts/peer-error-taxonomy.md)** — All `PeerError` variants with serde tags, emission conditions, peer-cleanup obligations, frontend handling. Retry classification rules per FR-011. Standards references (TN-9.7.2.1, TN-9.7.3.2).
- **[contracts/transport-health.md](./contracts/transport-health.md)** — `TransportHealth` enum, emission rules, `subscribe_health` API, `PeerSession` short-circuit contract, tests.
- **[quickstart.md](./quickstart.md)** — Per-slice checklist (baseline, new tests, exit verification, manual validation). Success-criteria checklist at the end.

### Agent context update

Copilot is the active agent. Copilot's context comes from `.github/copilot-instructions.md`, `.github/instructions/*.instructions.md`, and `aiwiki/`. The relevant technologies for this feature (Rust, tokio async, thiserror, Tauri IPC, SvelteKit) are already present in those files. No mechanical update needed via `.specify/scripts/powershell/update-agent-context.ps1` — new module names (`peer_session`, `peer_session_registry`, `event_router`) will be added to `aiwiki/owners.md` at slice-completion time per the "Post-Work Enrichment" section of copilot-instructions.

Recorded plan-phase enrichment intent (to be executed by the `/build` skill as slices land):
- `aiwiki/owners.md` — add `lcc-rs::peer_session`, `lcc-rs::peer_session_registry`, `lcc-rs::event_router` as new owners; note retirement of `bowties-core::cdi_inflight`.
- `aiwiki/flows.md` — update CDI download flow, config read/write flow, SNIP/PIP query flow to show session-mediated dispatch.
- `aiwiki/seams.md` — new seam: `PeerSessionRegistry` (Owner) → `PeerSession` (Contributor); `TransportActor` inbound broadcast (Owner) → many Consumers (`PeerSession`, `EventRouter`, `NetworkSession`, diagnostics).
- `product/architecture/adr/0016-per-peer-session-actor.md` — author before Slice 2 lands (per session-memory plan).

## Complexity Tracking

*No Constitution violations to justify — table intentionally empty.*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |

## Architecture Assessment

*Added 2026-07-05 by `/design` skill run. Assessed against `product/architecture/code-placement-and-ownership.md`, ADRs, `aiwiki/owners.md`, `aiwiki/flows.md`, `aiwiki/seams.md`.*

### Affected Modules

| Module | Layer | Impact | Notes |
|--------|-------|--------|-------|
| `lcc-rs::transport_actor` | Protocol / transport | Modified | Bounded FIFO writer, `TransportHealth` broadcast (S1); `send_direct` retired (S5) |
| `lcc-rs::peer_session` | Protocol | New | Per-peer actor; sole owner of exchange state, ACK obligations, retries, peer cleanup (S2+) |
| `lcc-rs::peer_session_registry` | Protocol | New | Sole spawner on VNI / InitComplete / AMD; `tokio::sync::RwLock<HashMap<NodeID, PeerSessionHandle>>` (S2) |
| `lcc-rs::event_router` | Protocol | New | Classification + fan-out core; lifted from `app/src-tauri/events/router.rs` (S6) |
| `lcc-rs::snip`, `pip`, `discovery`, `datagram_reader` | Protocol | Modified | Become shims forwarding to session (S2–S4); retired in S5–S6. `datagram_reader` gains OIR-terminal + `TerminateDueToError` on our timeout (S3) |
| `bowties-core::node_proxy` (`LiveNodeProxy`) | Backend domain | Modified | Coalescing (`snip_waiters` / `pip_waiters`) + cached SNIP/PIP moved out; retained as app-layer aggregator (`last_seen`, `last_verified`, snapshot, `Live \| Synthesized`) delegating to `PeerSessionHandle`. Depth audit at end of S2. |
| `bowties-core::cdi_inflight` | Backend domain | Retired | Deleted in S3; invariant becomes structural via per-peer serialization |
| `app/src-tauri/state::AppState` | Backend | Modified | Adds `sessions: Arc<PeerSessionRegistry>`; removes `cdi_inflight` and `cdi_download_cancel` (S3) |
| `app/src-tauri/commands/cdi` | Backend | Modified | Rewritten as thin intent translator; retains `layout_state.record_captured` call per ADR-0015 |
| `app/src-tauri/events/router` | Backend | Modified | Thinned to Tauri-emit adapter over `lcc-rs::event_router` (S6) |

### Assessment Summary

The refactor consolidates scattered protocol state — currently spread across free functions in `lcc-rs`, the `LiveNodeProxy` waiters, and the `CdiInflightRegistry` — into a single deep module (`PeerSession`) per remote peer. `PeerSession` passes the deletion test strongly (removing it re-scatters the exchange state, ACK tracking, retries, coalescing, and peer-cleanup obligations across five files). Placement is correct: per-peer protocol behaviour belongs in `lcc-rs` per the code-placement rules. ADR-0015's invariants (LayoutState single owner) are preserved by keeping CDI persistence in the Tauri command layer (Clarification 2). Transport writer moves from `Arc<Mutex<Writer>>`-with-`send_direct`-bypass to a single-threaded FIFO with bounded per-send timeout, retiring the wedge-under-back-pressure pathology by construction.

Six findings drive slice reshape and enrichment; no ADR conflicts.

### Findings

**F1: Explicit `[REFACTOR]` labels on invariant-preserving slices**
- Category: Demo-ability / Vertical-Slice Gate
- Affected: S1, S2, S4, S5, S6 (all slices except S3, which is the SPROG CDI regression closure)
- Concern: Per SLICING.md, a slice with no user-visible change must be labelled `[REFACTOR]` with acceptance criteria naming the preserved invariant. Plan previously labelled slices HITL/AFK only.
- Decision: **Include** — labels applied in the Vertical Slices section below.

**F2: Merge SNIP + PIP migration into one slice**
- Category: YAGNI / slice granularity
- Affected: former S2 (SNIP) + former S3 (PIP)
- Concern: SNIP and PIP migrations are structurally identical (same command-with-oneshot + cache + coalescing pattern, same shim retirement). Two slices was ceremony without leverage.
- Decision: **Include** — merged into single slice S2 (HITL — establishes the query-command pattern for both). Total slices reduced from 6 to 6 (see F3 offsetting split).

**F3: Split former S6 into `send_direct` retirement (S5) and `event_router` lift (S6)**
- Category: SOLID (single-purpose slice) / SLICING
- Affected: former S6
- Concern: `send_direct` retirement + outbound-owner audit and `event_router` classification-core lift are independent changes with different risk profiles.
- Decision: **Include** — split into S5 (retire `send_direct`; explicit exit-checklist now includes `write_memory_timed` per F8) and S6 (`event_router` lift).

**F4: `LiveNodeProxy` depth audit at end of S2**
- Category: Depth (deletion test)
- Affected: `bowties-core::node_proxy::LiveNodeProxy`
- Concern: With coalescing and cached SNIP/PIP moved to `PeerSession`, and persistent CDI cache already relocated to `LayoutState` (2026-06-28 ADR-0015 extension), `LiveNodeProxy` may degenerate to a shallow wrapper around `{ session, last_seen, last_verified, ConnectionStatus, snapshot, Live \| Synthesized }`.
- Decision: **Include** as an S2 exit gate — audit and either (a) confirm the proxy retains a bounded app-layer responsibility, or (b) capture a follow-up `kind/idea` issue to fold it into `NodeRegistry`. Do not perform the fold inside this feature.

**F5: Author ADR-0016 as first work of S2**
- Category: Locality / documented invariants
- Affected: `product/architecture/adr/`
- Concern: This refactor introduces a new architectural pattern (per-peer actor) and several invariants not currently documented (single ACK owner per peer, single outbound sender per peer, single active exchange per peer, `TerminateDueToError` obligation on our failure, sole spawner on NodeID-carrying frames). Downstream slices must reference a stable contract.
- Decision: **Include** — S2 begins with authoring **ADR-0016 "Per-peer session actor ownership"** with a `## Invariants` section enumerating the above. Code lands after the ADR.

**F6: Register new seams in `aiwiki/seams.md` as slices land**
- Category: Post-Work Enrichment
- Affected: `aiwiki/seams.md`
- Concern: Feature introduces two new seams not currently registered: **Transport Health** (Owner `TransportActor` writer; Consumers `PeerSession`, UI status surface) and **Peer Session Ownership** (Owner `PeerSessionRegistry`; Contributors `TransportActor` inbound broadcast, `NetworkSession` peer lifecycle; Consumers `LiveNodeProxy`, Tauri commands, `EventRouter`).
- Decision: **Include** — enrichment step per slice: register Transport Health seam with S1, Peer Session Ownership seam with S2, reference ADR-0016.

**F7: Capture Slice 7 deferral (frontend intent single-owner) as `kind/idea` issue**
- Category: Issue Capture Protocol
- Affected: (deferred scope)
- Concern: Spec Clarification 5 defers frontend intent single-owner; Explore confirmed no matching `kind/idea` issue exists.
- Decision: **Include** — GitHub issue proposed and created 2026-07-05 (see "Deferred Improvements" below).

**F8: Explicit shim retirement list must include `write_memory_timed`**
- Category: Locality / clarity
- Affected: Plan.md exit criteria
- Concern: Retirement list previously enumerated `snip.rs`, `pip.rs`, `discovery.rs`, `datagram_reader.rs` but did not explicitly name `write_memory_timed`.
- Decision: **Include** — S5 exit checklist explicitly enumerates all of `query_snip`, `query_pip`, `read_cdi_cancellable_with_stats`, `read_memory_timed`, `write_memory_timed`, `datagram_read_exchange` retired from public API.

### ADR Invariant Audit

Feature meaningfully touches only ADR-0015. Other cited ADRs (0002, 0004, 0011, 0012, 0013) belong to the save-flow / layout-editing surface which this refactor does not modify.

**ADR-0015 — LayoutState single owner**

| Invariant | Status | Evidence |
|---|---|---|
| R1/R2 structurally impossible (save reads authoritative source) | OK | Feature adds no parallel CDI cache; `PeerSession` returns assembled bytes on result oneshot with no persistence coupling. See Clarification 2 and `data-model.md` `LiveNodeProxy` retained fields. |
| Backend mirrors frontend's saved/captured/drafts layering | OK | `layout_state.record_captured(node_id, bytes)` call retained in `commands/cdi.rs`. |
| One read path per concern | OK | Persistent CDI cache already removed from `LiveNodeProxy` (2026-06-28 extension); refactor does not reintroduce one. |

Bump `Last-audited` on ADR-0015-linked seams in `aiwiki/seams.md` when S3 lands.

### Vertical Slices

Ordering is risk-first (integration first) and linear: each slice depends on the previous. S3 is the sole user-visible slice (SPROG CDI regression closure). All others are `[REFACTOR]` preserving invariants.

**S1: Bounded FIFO transport writer + `TransportHealth` broadcast** `[REFACTOR]`
- Type: HITL — establishes new capability (writer bounded timeout, health broadcast) that downstream slices depend on
- Layers: `lcc-rs::transport_actor`
- Blocked by: None
- Test: Existing SPROG session still discovers and queries; a `w.send()` that exceeds 500ms serial / 2000ms TCP broadcasts `TransportHealth::Wedged` and does not deadlock other callers
- Acceptance (invariant preserved): all outbound frames still reach the wire under nominal conditions; existing lcc-rs and integration tests remain green
- Enrichment: register **Transport Health** seam in `aiwiki/seams.md`

**S2: Migrate SNIP + PIP through `PeerSession` (introduces registry, actor, handle)** `[REFACTOR]`
- Type: HITL — establishes the per-peer actor pattern, sole-spawner registry, session handle, query-command coalescing, and cache lifecycle
- Layers: `lcc-rs::peer_session`, `peer_session_registry`, `snip` (shim), `pip` (shim); `bowties-core::node_proxy` (delegate); `app/src-tauri::state` (add `sessions`)
- Blocked by: S1
- Prerequisite artefact: **ADR-0016 "Per-peer session actor ownership"** authored and merged before code lands (F5)
- Test: cold discovery on a modulino peer populates SNIP and PIP data; two concurrent `query_snip` calls to the same peer produce exactly one wire query; `PeerReinitialised` clears caches
- Acceptance (invariant preserved): SNIP+PIP data appears in the frontend snapshot identically to today; no duplicate SNIP/PIP burst per peer
- Enrichment: register **Peer Session Ownership** seam in `aiwiki/seams.md`; add `peer_session`, `peer_session_registry` to `aiwiki/owners.md`; execute **F4 depth audit** on `LiveNodeProxy` at slice exit

**S3: Migrate CDI download through `PeerSession`; retire `cdi_inflight`; peer cleanup on error**
- Type: HITL — introduces the peer-cleanup contract (`TerminateDueToError` on our timeout) and OIR-terminal handling
- Layers: `lcc-rs::peer_session` (add `DownloadCDI` command), `discovery` (shim), `datagram_reader` (OIR terminal + `TerminateDueToError`); `bowties-core::cdi_inflight` (delete); `app/src-tauri::state` (remove `cdi_inflight`, `cdi_download_cancel`); `app/src-tauri::commands/cdi` (rewrite as intent translator)
- Blocked by: S2
- Test: SPROG CDI download completes end-to-end; simulated peer timeout emits `TerminateDueToError` exactly once with the correct destination NodeID; `OptionalInteractionRejected` from peer terminates the exchange with `PeerError::Rejected`
- Acceptance (user-visible): **SPROG CDI download regression closed**. No `DatagramRejected 0x2020` follow-up storms after our timeout. Frontend progress events unchanged.
- Enrichment: bump `Last-audited` on ADR-0015-linked seams; note regression closure in `aiwiki/architecture-health.md`

**S4: Migrate config read + config write through `PeerSession`** `[REFACTOR]`
- Type: AFK — extends S3's exchange pattern to config-space reads and writes
- Layers: `lcc-rs::peer_session` (add `ReadConfig`, `WriteConfig`); `app/src-tauri::commands/*` (dispatch via session)
- Blocked by: S3
- Test: config read on modulino returns identical bytes to pre-refactor call; config write applies and verifies; two concurrent writes to the same peer serialise
- Acceptance (invariant preserved): config r/w behaviour unchanged from user perspective; single active exchange per peer holds for r/w

**S5: Retire `send_direct` + outbound-owner audit** `[REFACTOR]`
- Type: AFK — public API removal after all callers have migrated
- Layers: `lcc-rs::transport_actor` (remove `send_direct`, `direct_write_count`), `datagram_reader` (remove `use_send_direct` parameter)
- Blocked by: S4
- Test: workspace-wide grep for `send_direct`, `direct_write_count`, `use_send_direct` returns zero non-test hits; all outbound tests remain green
- Acceptance (invariant preserved): every outbound frame is session-owned; all of `query_snip`, `query_pip`, `read_cdi_cancellable_with_stats`, `read_memory_timed`, `write_memory_timed`, and `datagram_read_exchange` retired from public API (F8)

**S6: Lift `event_router` classification core into `lcc-rs`** `[REFACTOR]`
- Type: HITL — establishes the protocol/Tauri split for event routing; sets pattern for future protocol-vs-app-glue separations
- Layers: `lcc-rs::event_router` (new); `app/src-tauri::events/router` (thin Tauri-emit adapter)
- Blocked by: S5
- Test: event-subscribed UI still updates on PCER / EventReportWithPayload / Identify / Learn; classification unit tests move to `lcc-rs::event_router`; Tauri-emit tests remain in `app/src-tauri`
- Acceptance (invariant preserved): event routing behaviour unchanged; ownership boundary sharpened; no `AppHandle` / Tauri types in `lcc-rs`

### Deferred Improvements

- **Frontend intent single-owner per-peer** — [JohnSL/Bowties#18](https://github.com/JohnSL/Bowties/issues/18) (`kind/idea`, `area/orchestration`, created 2026-07-05). Follow-up to this refactor. Note: repo has no `area/frontend` label, so `area/orchestration` alone was used.
- **Optional `LiveNodeProxy` fold into `NodeRegistry`** — contingent on F4 depth-audit outcome at end of S2. If audit shows the proxy is shallow, capture as `kind/idea` issue at that time.

### Architecture Decisions

- **ADR-0016 "Per-peer session actor ownership"** — to be authored as first work of S2 (F5). Documents: per-peer actor pattern; single ACK owner per peer; single outbound sender per peer; single active exchange per peer; `TerminateDueToError` obligation on our failure; sole spawner (registry) gated on VNI / InitComplete / AMD frames; retained transport inbound broadcast with single-owner-at-consumer enforcement.

## Command Completion

Phase 0 (research) and Phase 1 (design & contracts) are complete. Phase 2 (task generation) is the responsibility of `/speckit.tasks` and is deliberately not produced here.

**Generated artifacts**:
- [specs/019-peer-session-refactor/plan.md](./plan.md) (this file)
- [specs/019-peer-session-refactor/research.md](./research.md)
- [specs/019-peer-session-refactor/data-model.md](./data-model.md)
- [specs/019-peer-session-refactor/contracts/peer-session-api.md](./contracts/peer-session-api.md)
- [specs/019-peer-session-refactor/contracts/peer-error-taxonomy.md](./contracts/peer-error-taxonomy.md)
- [specs/019-peer-session-refactor/contracts/transport-health.md](./contracts/transport-health.md)
- [specs/019-peer-session-refactor/quickstart.md](./quickstart.md)

**Branch**: `019-peer-session-refactor`
**Next command**: `/speckit.tasks` (to generate `tasks.md`).
