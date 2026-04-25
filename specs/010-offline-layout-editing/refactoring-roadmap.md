# Refactoring Roadmap: Offline/Sync UX Orchestration

**Spec**: `/specs/010-offline-layout-editing/`  
**Purpose**: Track refactoring work separately from feature delivery tasks so we can execute the first thin-slice safely and keep a visible backlog of future refactors.

## How To Use This File

- Keep this file focused on architecture and code-structure refactors.
- Keep `tasks.md` focused on feature and bugfix delivery tasks.
- Link each refactor slice to concrete feature tasks when relevant.
- Prefer incremental slices over big-bang rewrites.

---

## Track A: First Thin-Slice Refactor (Do Before Next Bugfix Wave)

**Goal**: Reduce regressions from state orchestration and lifecycle timing before implementing additional Phase 6c/6d fixes.

### A1. Orchestration Boundaries

- [x] R001 Create dedicated orchestration boundaries for offline replay and sync-session lifecycle in frontend stores/services.
- [x] R002 Move side-effect sequencing out of components and into orchestration adapters.
- [ ] R003 Ensure components are declarative: render state, emit intent events, avoid business branching.

### A2. NodeID Normalization Boundary

- [x] R004 Introduce one normalization module for NodeID mapping (canonical compare rules shared by all call sites).
- [x] R005 Replace ad-hoc NodeID comparisons with the shared helper in offline, sync, and tree-apply paths.
- [x] R006 Add regression tests for mixed dotted/canonical NodeID formats.

### A3. Lifecycle State Model

- [ ] R007 Define a single source of truth for lifecycle transitions (layout open, reload, replay, discard, apply, disconnect). Partial: layout-open transitions now use explicit lifecycle helpers, sync-session auto-trigger state is centralized, and disconnect transitions are now routed through explicit preserve/rehydrate/clear outcomes, but one unified transition owner does not exist yet.
- [x] R008 Make pending-value apply timing deterministic and documented at each lifecycle transition.
- [x] R009 Add guardrails for ordering-sensitive paths (reload, tree rebuild, post-apply restamp).

### A4. Store-First Test Gate

- [x] R010 Add focused store transition tests for offline replay lifecycle.
- [x] R011 Add focused store transition tests for sync-session lifecycle.
- [ ] R012 Require these store tests to pass before further UI behavior changes in this area. Partial: focused tests now exist for the extracted seams, but there is no explicit policy or CI/PR gate recorded yet.

### A5. Integration With Current Phase 6c/6d Tasks

- [ ] R013 Sequence Phase 6c/6d implementation on top of A1-A4 seams.
- [ ] R014 Map each task `T047m`-`T047x` to the seam it depends on.
- [ ] R015 Record any newly discovered orchestration/lifecycle defects as refactor follow-ups here.

### Current Progress (2026-04-21)

- Completed the first offline replay orchestration slice by extracting layout open/startup restore hydration and pending-value replay into `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`.
- Completed the first sync-session orchestration slice by extracting discovery settling, auto-trigger/manual re-open, and disconnect fallback handling into `app/src/lib/orchestration/syncSessionOrchestrator.ts`.
- Completed the disconnect transition refinement by routing disconnect behavior through explicit `rehydrated_offline` / `preserved_layout` / `cleared_to_connection` outcomes in `app/src/lib/orchestration/syncSessionOrchestrator.ts` and `app/src/routes/+page.svelte`.
- Completed the first sync apply orchestration slice by extracting post-apply tree rebuild and pending-value restamp into `app/src/lib/orchestration/syncApplyOrchestrator.ts`.
- Completed the discovery/reinitialization orchestration slice by extracting node upgrade/register/SNIP/PIP enrichment and reinitialized-node refresh into `app/src/lib/orchestration/discoveryOrchestrator.ts`.
- Added explicit lifecycle helper functions and transition guardrails in `app/src/lib/stores/layoutOpenLifecycle.ts`.
- Added focused tests covering these seams in `app/src/lib/stores/layoutOpenLifecycle.test.ts`, `app/src/lib/components/Sync/SyncPanel.lifecycle.test.ts`, `app/src/lib/orchestration/syncSessionOrchestrator.test.ts`, `app/src/lib/orchestration/discoveryOrchestrator.test.ts`, `app/src/lib/components/ElementCardDeck/TreeLeafRow.offline.test.ts`, and `app/src/lib/stores/offlineChanges.store.test.ts`.
- Remaining lifecycle gap: discard/apply/disconnect/open transitions are improved, but not yet unified under one transition matrix or owner.

---

## Track B: Future Refactor Backlog (Beyond Current Revised Plan)

**Goal**: Keep a visible backlog of high-value refactors outside the immediate thin-slice scope.

### B1. Store Coupling And Derived-State Isolation

- [ ] F001 Reduce cross-store coupling where derived maps read broad mutable state.
- [ ] F002 Extract expensive or branch-heavy derived logic into pure domain modules.
- [ ] F003 Add explicit contracts for store-to-store dependencies.

### B2. Layout/Open/Disconnect Flow Consolidation

- [ ] F004 Consolidate layout open/hydration/reconnect/disconnect transition handling into one orchestrated flow.
- [ ] F005 Remove duplicate transition logic spread across page-level handlers and store listeners.
- [ ] F006 Add transition matrix tests covering offline/online mode changes.

### B3. Backend/Frontend Boundary Hardening

- [ ] F007 Standardize command response shapes and error mapping at API adapter boundaries.
- [ ] F008 Isolate YAML snapshot write/read mapping logic into dedicated backend domain services.
- [ ] F009 Add deterministic snapshot-update tests for post-apply and post-save consistency.

### B4. UI Layer Simplification

- [ ] F010 Continue migrating view components toward pure rendering and intent emission.
- [ ] F011 Introduce small presenter/view-model helpers where component files still contain branch-heavy decisions.
- [ ] F012 Keep component tests focused on rendering contracts and intent wiring.

### B5. Architecture Governance

- [ ] F013 Add a short architecture note documenting the adopted pattern: Container + Reactive Store + Pure Domain Logic.
- [ ] F014 Add PR checklist items that block new business logic in view components without justification.
- [ ] F015 Reassess this roadmap after each major phase checkpoint and re-prioritize.

---

## Status Snapshot

- Last updated: `2026-04-21`
- Thin-slice refactor started: `Yes`
- Thin-slice refactor completed: `No`
- Future backlog reviewed this cycle: `No`

## Notes

- This roadmap intentionally avoids Playwright-specific Tauri E2E as a planning dependency.
- Validation emphasis remains on unit tests, store-level transition tests, and targeted component contract tests.