# Implementation Plan: Bowties Tab — Discover Existing Connections

**Branch**: `006-bowties-event-discovery` | **Date**: 2026-02-22 | **Spec**: [spec.md](spec.md)  
**Input**: Feature specification from `/specs/006-bowties-event-discovery/spec.md`

## Summary

Scan all discovered LCC nodes' event slots (already loaded via `read_all_config_values`) and perform a targeted `IdentifyEventsAddressed` exchange — one message per node, 125 ms apart (per JMRI's `EventTablePane` reference pattern in OpenLCB_Java) — to determine node-level producer/consumer roles. Display the results as bowtie cards in a new read-only Bowties tab. For cross-node cases the protocol reply is definitive. For same-node cases (a node both produces and consumes the same event ID), a two-tier CDI text heuristic is applied as a fallback; slots the heuristic cannot resolve are shown as *Ambiguous* in the bowtie card, pending user clarification in a future phase. The tab is disabled until both CDI reads and the Identify Events exchange complete; it rebuilds automatically after each full refresh.

---

## Technical Context

**Language/Version**: Rust 2021 (stable ≥1.70) + TypeScript strict / SvelteKit 2.x  
**Primary Dependencies**: `lcc-rs` (workspace crate), `tokio`, `serde`, `tauri 2.x`; SvelteKit 2.x + Tauri JS API  
**Storage**: In-memory only — `AppState.nodes` cache (already exists); no new persistence in this phase  
**Testing**: `cargo test` (lcc-rs unit + integration), Vitest (SvelteKit components)  
**Target Platform**: Desktop — Windows, macOS, Linux via Tauri 2.x  
**Project Type**: Multi (Rust Tauri backend + SvelteKit frontend, single repository workspace)  
**Performance Goals**: SC-001 — bowtie catalog built within 5 s of last CDI read completing; SC-004 — empty-state visible within 1 s  
**Constraints**: One new network exchange — `IdentifyEventsAddressed` per known node (125 ms between sends, ref: JMRI `EventTablePane.sendRequestEvents` in OpenLCB_Java) after CDI reads; read-only tab in this phase; tab disabled until both CDI reads AND Identify Events collection complete  
**Scale/Scope**: Typical deployment = dozens of nodes, hundreds of event slots total; O(n) algorithm in slot count

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked post Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Rust 2021+ Development | ✅ PASS | New `lcc-rs/src/cdi/role.rs` and `commands/bowties.rs` use Rust 2021 idioms, `Result`/`?`, no `unwrap` in production paths |
| II. Cargo-based Environment | ✅ PASS | All new Rust code inside existing Cargo workspace; no new toolchain required |
| III. Test-Driven Development | ✅ PASS | Role classifier needs unit tests for each keyword tier + property tests for boundary strings; bowtie grouping logic needs unit tests; BowtieCard + EmptyState need Vitest tests |
| IV. LCC Protocol Correctness | ✅ PASS | No new protocol operations — reuses existing CDI read data. Heuristic classifier documented in research.md with citation to S-9.7.4.1: no XML role attribute exists. |
| V. UX-First Design | ✅ PASS | Disabled tab with grey label; empty-state illustration + guidance text; card header = dotted event ID if no user name; "Used in" cross-reference with navigation; all addressed by spec |
| VI. TCP-Only Focus | ✅ PASS | No transport changes |
| VII. Event Management Excellence | ✅ PASS | Core event discovery feature; well-tested; human-readable event IDs throughout |

**No violations — all gates pass.**


---

## Project Structure

### Documentation (this feature)

```text
specs/006-bowties-event-discovery/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── tauri-commands.md
│   └── frontend-types.ts
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created here)
```

### Source Code Changes

```text
lcc-rs/src/
└── cdi/
    ├── mod.rs           [MODIFY] re-export EventRole; add ancestor context to EventId walk
    ├── role.rs          [NEW] EventRole enum + classify_event_slot() + unit tests
    └── hierarchy.rs     [MODIFY] expose ancestor group names during element traversal

app/src-tauri/src/
├── commands/
│   ├── bowties.rs       [NEW] get_bowties Tauri command + BowtieCatalog builder + Identify Events query
│   └── mod.rs           [MODIFY] register get_bowties command
└── state.rs             [MODIFY] add bowties_catalog + event_roles fields to AppState

app/src/
├── lib/
│   ├── api/
│   │   └── tauri.ts     [MODIFY] add getBowties() + BowtieCard / EventSlotEntry types
│   ├── stores/
│   │   └── bowties.ts   [NEW] bowtieCatalogStore, cdiReadCompleteStore, usedInMap derived store
│   └── components/
│       └── Bowtie/
│           ├── BowtieCard.svelte       [NEW] three-column layout (FR-004, FR-005, FR-014)
│           ├── ElementEntry.svelte     [NEW] producer/consumer slot card within a BowtieCard
│           ├── ConnectorArrow.svelte   [NEW] centre column with right-pointing arrow + event ID label
│           └── EmptyState.svelte       [NEW] illustration + guidance text (FR-006)
└── routes/
    └── bowties/
        └── +page.svelte [NEW] Bowties tab page (disabled state until cdi-read-complete)
```

Existing files modified for cross-reference (FR-008, FR-009):
- `app/src/lib/components/ElementCardDeck/EventSlotRow.svelte` — add optional `usedIn` prop with navigable link

**Structure Decision**: Multi-project Tauri desktop app. New Rust code follows `commands/<feature>.rs` convention. New SvelteKit code follows `lib/components/<Feature>/` + `lib/stores/<feature>.ts` conventions. No new dependencies needed.

---

## Complexity Tracking

No constitution violations. No new dependencies required beyond what the project already uses.

