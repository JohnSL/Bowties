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

- [ ] R007 Define a single source of truth for lifecycle transitions (layout open, reload, replay, discard, apply, disconnect). Partial: layout-open and layout-close transitions now use the offline-layout orchestrator, startup/connect/disconnect rules now share an explicit transition matrix, and sync-session auto-trigger state is centralized, but layout open/apply are not yet unified under the same transition owner.
- [x] R008 Make pending-value apply timing deterministic and documented at each lifecycle transition.
- [x] R009 Add guardrails for ordering-sensitive paths (reload, tree rebuild, post-apply restamp).

### A4. Store-First Test Gate

- [x] R010 Add focused store transition tests for offline replay lifecycle.
- [x] R011 Add focused store transition tests for sync-session lifecycle.
- [x] R012 Require these store tests to pass before further UI behavior changes in this area.

### A5. Integration With Current Phase 6c/6d Tasks

- [x] R013 Sequence Phase 6c/6d implementation on top of A1-A4 seams.
- [x] R014 Map each task `T047m`-`T047x` to the seam it depends on.
- [x] R015 Record any newly discovered orchestration/lifecycle defects as refactor follow-ups here.

### Current Progress (2026-04-25)

- Completed the first offline replay orchestration slice by extracting layout open/startup restore hydration and pending-value replay into `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`.
- Completed the first sync-session orchestration slice by extracting discovery settling, auto-trigger/manual re-open, and disconnect fallback handling into `app/src/lib/orchestration/syncSessionOrchestrator.ts`.
- Completed the disconnect transition refinement by routing disconnect behavior through explicit `rehydrated_offline` / `preserved_layout` / `cleared_to_connection` outcomes in `app/src/lib/orchestration/syncSessionOrchestrator.ts` and `app/src/routes/+page.svelte`.
- Completed the first sync apply orchestration slice by extracting post-apply tree rebuild and pending-value restamp into `app/src/lib/orchestration/syncApplyOrchestrator.ts`.
- Completed the discovery/reinitialization orchestration slice by extracting node upgrade/register/SNIP/PIP enrichment and reinitialized-node refresh into `app/src/lib/orchestration/discoveryOrchestrator.ts`.
- Added a route-level discovery regression harness in `app/src/routes/page.route.test.ts` covering fresh live discovery, stale config-read carryover, and stale sidebar-selection carryover on no-layout sessions.
- Fixed the concurrent live-discovery merge race by rebasing async SNIP/PIP completion onto the latest published node list instead of the stale event-local snapshot.
- Fixed the sync-session already-applied lifecycle gap so `build_sync_session` now prunes already-applied rows from the backend offline-change cache and the frontend reloads offline changes after a session build reports cleared rows.
- Fixed the post-apply snapshot refresh gap so sync apply now updates both snapshot baseline values and the node snapshot `captured_at` timestamp for successfully applied rows.
- Fixed the CDI preflight classification gap so only `CdiNotRetrieved` opens the download prompt while `CdiUnavailable`, `RetrievalFailed`, and similar failures surface through the existing error banner.
- Added explicit lifecycle helper functions and transition guardrails in `app/src/lib/stores/layoutOpenLifecycle.ts`.
- Added focused tests covering these seams in `app/src/lib/stores/layoutOpenLifecycle.test.ts`, `app/src/lib/stores/syncPanel.store.test.ts`, `app/src/lib/components/Sync/SyncPanel.lifecycle.test.ts`, `app/src/lib/orchestration/syncSessionOrchestrator.test.ts`, `app/src/lib/orchestration/discoveryOrchestrator.test.ts`, `app/src/lib/orchestration/configReadOrchestrator.test.ts`, `app/src/lib/orchestration/offlineLayoutOrchestrator.test.ts`, `app/src/lib/orchestration/syncApplyOrchestrator.test.ts`, `app/src/lib/orchestration/unsavedChangesGuard.test.ts`, `app/src/lib/components/ElementCardDeck/SaveControls.test.ts`, `app/src/lib/components/ConfigSidebar/ConfigSidebar.test.ts`, `app/src/lib/components/ElementCardDeck/TreeLeafRow.offline.test.ts`, `app/src/lib/stores/offlineChanges.store.test.ts`, `app/src/routes/page.route.test.ts`, and `app/src-tauri/src/layout/node_snapshot.rs` unit tests.
- Extracted the route-owned close/discard transition into `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`, so open/close lifecycle decisions are no longer split between `+page.svelte` and the orchestration layer.
- Added an explicit frontend regression gate via `app/package.json`, `.github/workflows/frontend-regression-gate.yml`, and `.github/pull_request_template.md` so offline/sync/discovery UI changes have a documented and automated test requirement.
- Documented the adopted frontend pattern in `docs/technical/architecture.md` and `docs/project/development.md`.
- Extracted SyncPanel mode-choice and apply/dismiss workflow branching into `app/src/lib/orchestration/syncPanelViewOrchestrator.ts`, leaving `SyncPanel.svelte` focused on rendering and intent wiring.
- Moved offline snapshot hydration, no-layout reset, fresh-live reset, and startup restore handling further into `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`, so the route now supplies state hooks instead of owning those lifecycle branches directly.
- Extracted refresh stale-node reconciliation into `app/src/lib/orchestration/discoveryOrchestrator.ts`, so node removal, cached CDI pruning, and stale sidebar-reset decisions are covered at the owner level instead of being inlined in `+page.svelte`.
- Extracted startup connection/layout bootstrap sequencing into `app/src/lib/orchestration/syncSessionOrchestrator.ts`, so connection-status resolution, recent-layout restore ordering, fresh-live reset, and initial probe timing are no longer split across the route `onMount` flow.
- Added an explicit lifecycle transition matrix in `app/src/lib/orchestration/lifecycleTransitionMatrix.ts` and wired startup/connect/disconnect helpers through it, with focused matrix coverage added to the frontend regression gate.
- Extracted config-read CDI preflight resolution and waiting-state setup into `app/src/lib/orchestration/configReadOrchestrator.ts`, so batch reads, single-node reads, and post-download reads now share the same owner-level branch logic instead of duplicating it in `app/src/routes/+page.svelte`.
- Extracted the post-download config-read execution pass into `app/src/lib/orchestration/configReadOrchestrator.ts`, so per-node status transitions, tree reloads, and read-success bookkeeping are no longer inlined inside the route's CDI-download handler.
- Routed the batch-read and single-node config-read execution paths through the same `app/src/lib/orchestration/configReadOrchestrator.ts` runner used by the post-download flow, so the route no longer owns three separate variants of per-node read execution.
- Extracted config-read session lifecycle patches into `app/src/lib/orchestration/configReadSessionOrchestrator.ts`, so modal open/close state, progress-event reactions, cancellation state, failure cleanup, and missing-CDI diversion are no longer repeated across the route handlers.
- Extracted CDI viewer open/close state, re-download dialog state, and post-download read planning into `app/src/lib/orchestration/cdiDialogOrchestrator.ts`, so the route no longer owns that modal-state branching inline.
- Extracted `ConfigSidebar.svelte` node naming, tooltip, unread-badge, and pending-state derivation into `app/src/lib/components/ConfigSidebar/configSidebarPresenter.ts`, keeping the component closer to render-and-intent wiring.
- Extracted `SaveControls.svelte` dirty-count, pending-label, and discard-count derivation into `app/src/lib/components/ElementCardDeck/saveControlsPresenter.ts` and added focused presenter coverage to the frontend regression gate.
- Remaining lifecycle gap: discard/apply/disconnect/open transitions are improved, but not yet unified under one transition matrix or owner.

### Phase 6c/6d Sequencing

- Start with pure/shared rules first: NodeID normalization and display-name fallbacks.
- Extract lifecycle owners next: offline open/replay, sync-session triggering, disconnect fallback, and layout close/discard.
- Add owner-level regression tests before changing route wiring.
- Land backend state corrections after the frontend seams exist: already-applied row pruning and snapshot refresh.
- Finish with route-level workflow checks only for behavior that crosses component and orchestrator boundaries.

### Phase 6c/6d Seam Map

- `T047m`, `T047v`, `T047w`: backend sync persistence seam in `app/src-tauri/src/commands/sync_panel.rs` and snapshot helpers in `app/src-tauri/src/layout/node_snapshot.rs`.
- `T047n`, `T047q`: component intent and unsaved-state seam in `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte`, `app/src/lib/components/ElementCardDeck/SaveControls.svelte`, and `app/src/lib/orchestration/unsavedChangesGuard.ts`.
- `T047o`, `T047t`: saved-vs-working offline state seam in `app/src/lib/stores/offlineChanges.svelte.ts`.
- `T047p`: disconnect transition seam in `app/src/lib/orchestration/syncSessionOrchestrator.ts` and route wiring in `app/src/routes/+page.svelte`.
- `T047r`: shared NodeID normalization seam in `app/src/lib/utils/nodeId.ts` and consumers such as `app/src/lib/stores/nodeTree.svelte.ts`.
- `T047s`: offline layout open/replay seam in `app/src/lib/orchestration/offlineLayoutOrchestrator.ts`, plus route ordering in `app/src/routes/+page.svelte`.
- `T047u`: post-apply tree reconciliation seam in `app/src/lib/orchestration/syncApplyOrchestrator.ts`.
- `T047x`: regression coverage spread across `app/src/lib/stores/nodeTree.store.test.ts`, `app/src/lib/stores/offlineChanges.store.test.ts`, `app/src/lib/orchestration/syncApplyOrchestrator.test.ts`, `app/src/lib/stores/syncPanel.store.test.ts`, and backend unit helpers in `app/src-tauri/src/layout/node_snapshot.rs` and `app/src-tauri/src/commands/sync_panel.rs`.

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
- [x] F006 Add transition matrix tests covering offline/online mode changes.

### B3. Backend/Frontend Boundary Hardening

- [ ] F007 Standardize command response shapes and error mapping at API adapter boundaries.
- [ ] F008 Isolate YAML snapshot write/read mapping logic into dedicated backend domain services.
- [ ] F009 Add deterministic snapshot-update tests for post-apply and post-save consistency.

### B4. UI Layer Simplification

- [ ] F010 Continue migrating view components toward pure rendering and intent emission.
- [ ] F011 Introduce small presenter/view-model helpers where component files still contain branch-heavy decisions.
- [ ] F012 Keep component tests focused on rendering contracts and intent wiring.

### B5. Architecture Governance

- [x] F013 Add a short architecture note documenting the adopted pattern: Container + Reactive Store + Pure Domain Logic.
- [x] F014 Add PR checklist items that block new business logic in view components without justification.
- [x] F015 Reassess this roadmap after each major phase checkpoint and re-prioritize.

### Next Backlog Order

- `F004`-`F006`: unify the remaining lifecycle matrix and add explicit transition coverage.
- `F010`-`F012`: keep pushing branch-heavy component logic into helpers/orchestrators.
- `F007`-`F009`: harden backend/frontend API shapes and snapshot-domain boundaries.
- `F001`-`F003`: reduce broad cross-store reads once lifecycle ownership is more centralized.

---

## Status Snapshot

- Last updated: `2026-04-25`
- Thin-slice refactor started: `Yes`
- Thin-slice refactor completed: `No`
- Future backlog reviewed this cycle: `Yes`

## Notes

- This roadmap intentionally avoids Playwright-specific Tauri E2E as a planning dependency.
- Validation emphasis remains on unit tests, store-level transition tests, and targeted component contract tests.