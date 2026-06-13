# Decompose `+page.svelte` God Component

**Date**: 2026-06-07
**Prior art**: `specs/ideas/refactors/page-svelte-god-orchestrator.md`
**Architecture health entry**: `aiwiki/architecture-health.md` → "+page.svelte god component"

## Progress

| Slice | Status |
|-------|--------|
| S1 — config-acquisition workflow owner | ✅ Done (2026-06-07) |
| S2 — CDI-inspection owner | ✅ Done (2026-06-07) |
| S3 — connection logic into `syncSessionOrchestrator` | ✅ Done (2026-06-07) |
| S4 — menu wiring (enable policy + listener registrar) | ✅ Done (2026-06-07) |
| S5 — dead-code pass + derived consolidation | ✅ Done (2026-06-07) |
| S6 — `cdiCacheStore` for `nodesWithCdi` | ✅ Done (2026-06-07) |

**~20 `$state` vars removed from the route so far** (12 in S1, 8 in S2). Full suite green after each slice.

## Problem

`app/src/routes/+page.svelte` is ~1,942 lines with 37 `$state` variables managing 11 distinct concerns. It inlines multi-step async workflows that belong in orchestrators, violating `product/architecture/code-placement-and-ownership.md`. Every workflow change is fragile because unrelated state is co-located with no isolation.

Cross-cutting bugs (placeholder sidebar disappearing, stale catalog on save, roster bleeding across layout switches) repeatedly trace back to route-local state not being updated by a different section of the same file.

## Guiding goal (drives every decision)

**Make the frontend a set of deep modules with clear single ownership, governed by DRY and SOLID.**

A *deep module* is a **simple interface hiding a complex implementation**. The win is not "fewer lines in the route" — it is that each workflow has exactly one owner exposing a narrow interface, with all of its sequencing, state, and edge cases hidden behind that interface. Line reduction in `+page.svelte` is a *symptom* of success, not the target.

Use this goal to resolve any surprise we hit mid-implementation. When a choice appears, prefer the option that:

1. **Cuts module boundaries along real workflow seams**, not along incidental code groupings. If two "separate" modules must call back and forth to complete one user action, the boundary is in the wrong place — that is a single workflow and belongs in one deep module.
2. **Keeps interfaces narrow.** A module that exposes many setters/getters its callers must orchestrate is shallow. Prefer one method that performs the whole workflow over several methods the route must sequence.
3. **Avoids cross-module cycles.** A bidirectional import between two orchestrators is a signal the seam is wrong — merge or re-cut rather than wire a cycle.
4. **Removes duplication (DRY) and gives each piece of state one owner (SOLID/SRP).** Shared mutable state read/written by multiple workflows belongs in a store with one owner, not a route-local field passed around.
5. **Stays the smallest change that achieves the above (YAGNI).** Don't invent generic frameworks; deepen or re-cut the specific modules in play.

If we discover a slice boundary is wrong (as happened with the original S1/S2 split — see below), **re-cut the slice to match the real seam** rather than preserving an artificial boundary. Updating this plan is expected, not exceptional.

## Strategy

- Slice-by-slice extraction (one **workflow** per slice), not a big-bang refactor
- Each slice is independently testable and deployable
- **Deepen existing modules** — orchestrators already exist for most workflows; make them stateful owners instead of creating new shallow modules
- **Cut along workflow seams.** A slice owns a complete user action end-to-end. If extracting a "concern" leaves sequencing glue in the route or forces a cross-orchestrator call, the concern was mis-identified — fold it into the workflow that owns it.
- Connection state stays in `layout.svelte.ts` where it already lives; dialog visibility stays in the route where it belongs as page-composition state
- Only create new modules when no existing module owns the concern (menu wiring)

## Re-slice decision (2026-06-07)

The original S1/S2 split (config-read session vs. CDI dialog) cut **through the middle of one workflow**. "Read a node's configuration" *is* "ensure the node's CDI is present — downloading it via the missing-CDI dialog if not — then read its config values and report progress." The CDI **download** dialog is a sub-step of config acquisition, not a peer concern:

- Config-read → download: the preflight opens the download dialog and stashes `pendingConfigNodes` when CDI is missing.
- Download → config-read: `handleCdiDownload` finishes downloading, then begins a config-read session and runs the reads.

Splitting these into two stateful orchestrators forces a **bidirectional dependency** between them — exactly the cycle the guiding goal forbids. So S1 and S2 are re-cut along the true seam:

- **S1 owns the whole config-acquisition workflow** (preflight + missing-CDI download + reads + progress + cancel) in one deep module.
- **S2 owns CDI *inspection*** (the read-only XML viewer + the menu-driven re-download), which genuinely does **not** participate in reading config.

This keeps both modules deep, acyclic, and independently testable while still removing the same ~16 `$state` variables from the route.

## Depth review (2026-06-07)

Audited all 14 orchestrators and 20+ stores before finalising slices. Key findings:
- `configReadSessionOrchestrator` is currently pure/stateless (returns patches). Deepen it into the config-acquisition workflow owner — see S1.
- `cdiDialogOrchestrator` is currently pure/stateless (returns state builders). Its **download** helpers fold into S1 (they're part of acquisition); its **viewer + redownload** helpers become the S2 inspection owner.
- `syncSessionOrchestrator` already owns connect/disconnect lifecycle (`connectLiveSession`, `disconnectWithOfflineFallback`). Deepen it instead of creating a new connection orchestrator.
- `layout.svelte.ts` already owns `_connected`, `setConnected()`, `isConnected`. Don't duplicate.
- Dialog visibility (`unsavedDialog`, `errorDialog`, etc.) is genuinely page-scoped composition state — routes are the right owner. No store needed.
- `nodesWithCdi` (the "which nodes have cached CDI" Set) is written by discovery, refresh, **and** acquisition, and read by the menu effect. It is shared mutable state with multiple owners — a DRY/SRP smell. Its honest home is a small `cdiCacheStore`. Deferred to a follow-up slice (S6) to keep S1/S2 focused; noted here so it isn't forgotten.

## Slices

### S1: Deepen `configReadSessionOrchestrator` into the config-acquisition workflow owner ✅ DONE (2026-06-07)

**Impact**: HIGH — eliminates ~12 `$state` variables (7 config-read + 5 CDI-download)
**Effort**: Medium
**New modules**: 0 — deepens existing `configReadSessionOrchestrator` (rename to `configAcquisitionOrchestrator` to reflect its real job)

The orchestrator is currently pure/stateless — it returns `ConfigReadSessionPatch` objects for the caller to apply. Convert it to a stateful class that owns the **entire config-acquisition workflow**: CDI preflight, the missing-CDI download dialog, the downloads, the config reads, progress reporting, and cancellation. The route stops sequencing these steps; it calls `readRemaining()` / `readSingleNode(id)` / `downloadMissingCdi()` and subscribes to reactive getters.

**Interface goal**: the route should express intent (`readRemaining`, `readSingleNode`, `cancel`, `downloadMissingCdi`, `cancelDownload`) and read state via getters. All preflight → download → read sequencing is hidden inside the module. No cross-orchestrator calls.

State to absorb into the orchestrator:
- [x] `readProgress` — progress state for config reads
- [x] `nodeReadStates` — per-node progress state array
- [x] `discoveryPhase` — progress phase: 'reading' | 'building-catalog' | 'complete' | 'cancelled'
- [x] `discoveryModalVisible` — progress modal visibility
- [x] `isCancelling` — config read cancellation in progress
- [x] `readingRemaining` — batch "read remaining" in progress
- [x] `unreadCount` — derived: count of unread nodes
- [x] `cdiDownloadDialogVisible` — missing-CDI download modal visibility
- [x] `cdiMissingNodes` — nodes missing CDI (download queue + per-node status)
- [x] `cdiDownloading` — CDI download in progress
- [x] `cdiDownloadedCount` — count of downloaded CDI files
- [x] `pendingConfigNodes` — unread nodes pending config read after the CDI download completes

Functions to move (full workflow, not thin wrappers):
- [x] `readRemainingNodes()` → orchestrator `readRemaining()`
- [x] `readSingleNodeConfig()` → orchestrator `readSingleNode()`
- [x] `handleCancelConfigReading()` → orchestrator `cancel()`
- [x] `handleCdiDownload()` → orchestrator `downloadMissingCdi()`
- [x] `handleCdiDownloadCancel()` → orchestrator `cancelDownload()`
- [x] the `config-read-progress` event application (`applyConfigReadProgressUpdate`) → orchestrator-owned listener/handler

Boundary notes:
- `nodesWithCdi` stays route-owned for this slice (shared with discovery/refresh/menu); the orchestrator receives/returns it via parameters until S6 extracts `cdiCacheStore`. Do **not** duplicate it inside the orchestrator.
- Pure helpers in `configReadOrchestrator` (preflight, candidate execution, waiting-state builders) stay where they are; the deepened orchestrator composes them — DRY, no re-implementation.

Work:
- [x] Convert the orchestrator from pure functions to a stateful class with `$state` and reactive getters
- [x] Move the full acquire-config workflow (preflight → missing-CDI download → reads → progress → cancel) into the orchestrator
- [x] Update `DiscoveryProgressModal` to read from orchestrator state instead of route props
- [x] Update `CdiDownloadDialog` to read from orchestrator state / emit intent to it
- [x] Update `SegmentView` progress indicators to read from orchestrator state
- [x] Update route to subscribe to orchestrator getters and delegate calls; remove the per-key `applyConfigReadSessionPatch` shim
- [x] Migrate existing orchestrator tests to the stateful API; add tests for: read-remaining happy path, single-node read, missing-CDI → download → read handoff, download-cancel, read-cancel, progress→complete
- [x] Verify existing vitest tests pass (cargo: no Rust delta; known src-tauri test-DLL issue)

### S2: Deepen `cdiDialogOrchestrator` into the CDI-inspection owner ✅ DONE (2026-06-07)

**Impact**: MEDIUM — eliminates 8 `$state` variables (5 viewer + 3 redownload)
**Effort**: Low–Medium
**New modules**: 0 — deepens existing `cdiDialogOrchestrator` (rename to `cdiInspectionOrchestrator`; download helpers move to S1)

CDI **inspection** is a genuinely separate concern from reading config: the XML viewer and the menu-driven re-download let the user look at / re-fetch a node's CDI without touching the config-acquisition workflow. Convert the viewer + redownload halves of the current pure orchestrator into a stateful class.

**Interface goal**: `openViewer(id)` / `closeViewer()` / `openRedownload(id)` / `closeRedownload()` plus reactive getters. Load sequencing (cache hit → download fallback → error) stays hidden inside `openViewer`.

State to absorb into the orchestrator:
- [x] `viewerVisible` — CDI XML viewer modal visibility
- [x] `viewerNodeId` — node whose CDI is shown
- [x] `viewerXmlContent` — CDI XML content for display
- [x] `viewerStatus` — viewer load status
- [x] `viewerErrorMessage` — viewer error message
- [x] `cdiRedownloadVisible` / `cdiRedownloadNodeId` / `cdiRedownloadNodeName` — redownload dialog state

Functions to move:
- [x] `openCdiViewer()` / `closeCdiViewer()` → orchestrator `openViewer()` / `closeViewer()`
- [x] `openCdiRedownload()` / `closeCdiRedownload()` → orchestrator `openRedownload()` / `closeRedownload()`

Boundary notes:
- The CDI **download** helpers (`createWaitingCdiDownloadNodes`, `updateCdiDownloadNodeStatus`, `resolvePostDownloadReadNodes`, `createCancelledCdiDownloadState`) move to S1's workflow — they belong to acquisition, not inspection.

Work:
- [x] Convert the viewer + redownload halves to a stateful class with `$state` and reactive getters
- [x] Update `CdiXmlViewer` to read from orchestrator state
- [x] Update `CdiRedownloadDialog` wiring to read orchestrator state / emit intent
- [x] Update route to delegate viewer/redownload intent to the orchestrator
- [x] Migrate existing viewer/redownload tests; add tests for viewer open → cache hit, open → download fallback, open → error, redownload open/close with name fallback
- [x] Verify existing vitest tests pass (cargo: no Rust delta; known src-tauri test-DLL issue)

### S3: Deepen `syncSessionOrchestrator` to absorb route connection logic ✅ DONE (2026-06-07)

**Impact**: MEDIUM — eliminates the `connected` shadow + `connectionLabel` route state
**Effort**: Medium
**New modules**: 0 — deepens existing `syncSessionOrchestrator` (becomes `syncSessionOrchestrator.svelte.ts`); connection state stays in `layout.svelte.ts`

`syncSessionOrchestrator` already owns `connectLiveSession()`, `disconnectWithOfflineFallback()`, and `bootstrapStartupLifecycle()`. The route's `handleConnected()`, `disconnect()`, and `disconnectBeforeLayoutSwitch()` are thin wrappers around these that also set route-local `connected`, `connectionLabel`, and `errorMessage`. Move the wrapper logic into the orchestrator; `connected` flag stays in `layout.svelte.ts` (its existing owner). `connectionLabel` becomes orchestrator getter state.

#### Re-slice decision (2026-06-07, Option 2)

The plan originally said to absorb `errorMessage` into the orchestrator as `$state`. That cuts through the wrong seam: `errorMessage` is the **page-wide error banner**, written by four unrelated workflows (connection disconnect, config-acquisition via its `setErrorMessage` dep, layout-switch, and `handleRefresh`) and read by the template banner. Making `syncSessionOrchestrator` its owner would force `configAcquisition` and `handleRefresh` to write *through* the sync orchestrator — a cross-orchestrator coupling the guiding goal forbids.

Resolution (Option 2):
- `connected` → drop the route shadow; read `layoutStore.isConnected` directly (single owner already exists).
- `connectionLabel` → **moves into the orchestrator** as `$state` + getter (genuinely connection-workflow-only).
- `errorMessage` → **stays route-owned** page-banner state (same category as dialog visibility under "Kept in route"). The orchestrator reports failures via an injected `setErrorMessage` dep — the identical narrow-setter pattern `configAcquisition` already uses. No cross-orchestrator cycle.

State to relocate:
- [x] `connected` — already in `layout.svelte.ts` (`_connected`); route reads `layoutStore.isConnected` directly
- [x] `connectionLabel` — moves into the orchestrator as `$state` + getter
- [x] `errorMessage` — stays route-owned; orchestrator reports via injected `setErrorMessage`

Functions to move:
- [x] `handleConnected()` → `syncSessionOrchestrator`
- [x] `disconnect()` → `syncSessionOrchestrator`
- [x] `disconnectBeforeLayoutSwitch()` → `syncSessionOrchestrator`

Work:
- [x] Rename `syncSessionOrchestrator.ts` → `syncSessionOrchestrator.svelte.ts` (needs `$state`)
- [x] Convert `SyncSessionOrchestrator` to own `connectionLabel` `$state` + getter, injected deps via constructor
- [x] Move `handleConnected`/`disconnect`/`disconnectBeforeLayoutSwitch` logic into the orchestrator as `connect()`/`disconnect()`/`disconnectBeforeLayoutSwitch()`
- [x] Route reads `layoutStore.isConnected` directly instead of local `connected` shadow
- [x] Route keeps `errorMessage`; passes `setErrorMessage` into the orchestrator
- [x] Update route to subscribe to orchestrator `connectionLabel` getter
- [x] Add orchestrator tests for connect, disconnect, disconnect-before-switch, error handling
- [x] Verify existing vitest tests pass (cargo: no Rust delta; known src-tauri test-DLL issue)

### S4: Extract menu wiring (enable policy + listener registrar) ✅ DONE (2026-06-07)

**Impact**: LOW — cleans up ~150 lines of menu-event listener setup + dedups enable rules
**Effort**: Low
**New modules**: 2 — `menuEnableState.ts` (pure util) + `menuListeners.ts` (orchestration)

#### Re-slice decision (2026-06-07, Option 3)

The plan originally proposed one stateful `menuOrchestrator` owning enable bits **and** listener wiring. Auditing the seam showed it is two concerns of very different depth:

- **Enable policy** is substantive pure derivation (the rules were duplicated between the shortcut guards and the `update_menu_state` `$effect` — a real DRY smell). It belongs in a tested pure util.
- **Listener wiring** is ~14 thin relays to route handlers with no state. A stateful `.svelte.ts` class wrapping these would be a *shallow pass-through* (Depth failure) and would force many injected reactive getters to re-track the enable `$effect` across a closure boundary (fragile).

Resolution (Option 3): split along the real seam, mirroring the S3 Option-2 precedent (reactive page state stays where reactivity lives; extractable logic moves to narrow tested interfaces):
- `menuEnableState.ts` — pure `computeMenuEnableState(inputs) → MenuEnableState`. One owner for the enable rules.
- `menuListeners.ts` — `registerMenuListeners(actions, listenFn?)` owns the `menu-*` listen/teardown lifecycle behind one call (same proven shape as `installMenuShortcuts`).
- The reactive `$effect` and `update_menu_state` IPC stay route-owned (reactivity belongs there); the effect now calls the pure helper.

Work:
- [x] Create `menuEnableState.ts` pure enable policy + tests (13 tests)
- [x] Create `menuListeners.ts` listener registrar (injectable `listenFn`) + tests (4 tests)
- [x] Route `$effect` builds a `MenuEnableInputs` snapshot and delegates to `computeMenuEnableState`
- [x] Route `onMount` replaces the inline `listen('menu-*')` block with one `registerMenuListeners({...})` call
- [x] Add both test files to the `test:refactor-gate` list
- [x] Verify existing vitest tests pass (cargo: no Rust delta; known src-tauri test-DLL issue)

### S5: Dead code pass + derived consolidation ✅ DONE (2026-06-07)

**Impact**: LOW — reduces noise, catches stale branches
**Effort**: Low
**New modules**: 0

Work:
- [x] Remove unreachable `{#if}` arms from specs 010–013 refactors — removed the redundant inner `{#if layoutStore.isConnected || layoutStore.isOfflineMode}` guard wrapping `SaveControls`; it duplicated the outer toolbar guard (same expression) and was always true in that scope.
- [x] Consolidate derived state that duplicates logic already in stores/utils — `discoveredOnlyNodeIds` (the `computeDiscoveredOnlyNodeIds` derived) was fully dead: defined but never read in the template or passed to any component. Removed it outright rather than consolidating.
- [x] Remove unused callback props on child components — audited the route's child-component prop usage (e.g. `BowtieCatalogPanel`); all passed callback props are consumed. No removals needed.
- [x] Remove stale imports — dropped `computeDiscoveredOnlyNodeIds` from the `$lib/utils/nodeRoster` import (kept `canonicalizeNodeId`); removed the dead `formatAlias` helper.
- [x] Delete `specs/ideas/refactors/page-svelte-god-orchestrator.md` (superseded by this plan)
- [x] Verify existing vitest + cargo tests pass (vitest 1070 green; cargo: no Rust delta; known src-tauri test-DLL issue)

### S6: Extract `cdiCacheStore` for `nodesWithCdi` (DRY/SRP follow-up) ✅ DONE (2026-06-07)

**Impact**: LOW — single owner for shared "cached CDI" state
**Effort**: Low
**New modules**: 1 — `cdiCacheStore` (durable frontend state with multiple readers)

`nodesWithCdi` is a Set written by discovery, refresh, and config-acquisition (S1) and read by the menu effect. As a route-local field it has no single owner — a DRY/SRP violation surfaced during the S1/S2 re-slice. Give it one home so the acquisition orchestrator and the menu effect read the same source instead of passing a Set around.

Work:
- [x] Create `cdiCacheStore` owning the cached-CDI node Set with `has()`, `add()`, `replace()`, `reset()` (plus a reactive `nodes` getter for the refresh reconciler / post-download filter)
- [x] Route discovery/refresh writes and menu reads through the store — `resetFreshLiveSessionState` → `cdiCacheStore.reset()`; refresh passes `cdiCacheStore.nodes` into `reconcileRefreshState` then `cdiCacheStore.replace(...)`; menu effect reads `cdiCacheStore.has(selectedNodeId)`
- [x] S1 orchestrator reads/writes the store instead of receiving `nodesWithCdi` by parameter — dropped the `getNodesWithCdi`/`setNodesWithCdi` deps; `#mergeNodesWithCdi` now calls `cdiCacheStore.add`, post-download filter reads `cdiCacheStore.nodes` (`resolvePostDownloadReadNodes` param widened to `ReadonlySet`)
- [x] Tests for store transitions (`cdiCache.svelte.test.ts`, 6 tests) + migrated orchestrator tests to reset/assert the store; added `cdiCache.svelte.test.ts` to `test:refactor-gate`
- [x] Verify existing vitest + cargo tests pass (vitest 1076 green; cargo: no Rust delta; known src-tauri test-DLL issue)

### Kept in route (not extracted)

Dialog visibility flags (`unsavedDialog`, `errorDialog`, `showAddBoardDialog`, `pendingDeletePlaceholderKey`, `syncPanelVisible`, `showConnectionDialog`) are genuinely page-scoped composition state. "Which modal is showing" is a rendering concern that routes are designed to own. No store or orchestrator needed.

## Expected outcome

- `+page.svelte` drops from ~1,942 lines to ~800–1,000 lines (screen composition + subscriptions + template)
- `$state` count drops from 37 to ~15 (tab, refs, dialog visibility, and truly route-scoped UI state)
- Modules cut along real workflow seams: config-acquisition (S1) and CDI-inspection (S2) are deep, acyclic owners — no cross-orchestrator cycle, no route-resident sequencing glue
- 3 existing orchestrators deepened (config-acquisition, CDI-inspection, sync session); 1 new orchestrator (menu); 1 new store (cdiCacheStore, S6)
- Each workflow has a single owner exposing a narrow interface, making future feature work safer
- Orchestrator tests cover workflows previously only testable through the 1,942-line route

## Session log

| Date | Slice | Work done | Tests |
|------|-------|-----------|-------|
| 2026-06-07 | S1 | Created `configAcquisitionOrchestrator.svelte.ts` (stateful class, constructor DI). Absorbed 12 route `$state` vars (config-read progress + CDI download). Moved `readRemaining`/`readSingleNode`/`cancel`/`downloadMissingCdi`/`cancelDownload` + progress-event handling into it. Deleted old pure `configReadSessionOrchestrator.ts`. Widened `DiscoveryProgressModal` phase union for `building-catalog`. Route now delegates + subscribes; `nodesWithCdi` stays route-owned (S6). | vitest 1045 pass; svelte-check 110→106 (no net-new) |
| 2026-06-07 | S2 | Created `cdiInspectionOrchestrator.svelte.ts` (stateful viewer + redownload owner, constructor DI). Absorbed 8 route `$state` vars (5 viewer + 3 redownload). Moved `openViewer`/`closeViewer`/`openRedownload`/`closeRedownload`; kept `loadCdiViewerState` as exported pure helper. Deleted old `cdiDialogOrchestrator.ts` (download helpers had moved to S1, viewer/redownload to here). Updated aiwiki/owners.md. | vitest 1046 pass; svelte-check 106 (no net-new) |
| 2026-06-07 | S3 | Renamed `syncSessionOrchestrator.ts` → `.svelte.ts`. `SyncSessionOrchestrator` now owns `connectionLabel` `$state` + getter and the connect/disconnect workflow (`connect`/`disconnect`/`disconnectBeforeLayoutSwitch`) via injected `SyncSessionConnectionDeps`; composes the existing pure `connectLiveSession`/`disconnectWithOfflineFallback` helpers. Dropped the route `connected` shadow + `connectionLabel` state — route reads `layoutStore.isConnected` (authoritative). Re-slice (Option 2): `errorMessage` stays route-owned page banner, reported via narrow `setErrorMessage` dep, avoiding a cross-orchestrator cycle. Removed redundant `setConnected` from `connectLiveSession`/`bootstrapStartupLifecycle`. Fixed stale `test:refactor-gate` paths (S1/S2 + S3 renames). Updated aiwiki/owners.md + flows.md. | vitest 1053 pass (+7 new); svelte-check 106 (no net-new) |
| 2026-06-07 | S4 | Re-slice (Option 3): split menu wiring into a pure `menuEnableState.ts` (`computeMenuEnableState`, 13 tests) + an orchestration `menuListeners.ts` (`registerMenuListeners` with injectable `listenFn`, 4 tests), instead of one stateful `menuOrchestrator` (would be a shallow pass-through with fragile injected reactivity). Killed the DRY dup between the shortcut guards and the `update_menu_state` `$effect`; the effect now builds a `MenuEnableInputs` snapshot and delegates. Route `onMount` replaced the ~14 inline `listen('menu-*')` calls with one `registerMenuListeners({...})`. Added both test files to `test:refactor-gate`. Updated aiwiki/owners.md (counts + entries) + flows.md (new Native Menu Wiring flow). | vitest 1070 pass (+17 new); svelte-check 106 (no net-new) |
| 2026-06-07 | S5 | Dead-code pass. Removed the fully-dead `discoveredOnlyNodeIds` derived + its `computeDiscoveredOnlyNodeIds` import (defined, never read). Removed the dead `formatAlias` helper. Removed the redundant inner `{#if isConnected || isOfflineMode}` guard around `SaveControls` (duplicated the outer toolbar guard — always true in scope). Audited child-component callback props (`BowtieCatalogPanel` et al.) — all consumed, no removals. Deleted the superseded `specs/ideas/refactors/page-svelte-god-orchestrator.md`. No test changes needed (pure removal). | vitest 1070 pass; svelte-check 106 (no net-new) |
| 2026-06-07 | S6 | Created `cdiCache.svelte.ts` (`cdiCacheStore`) owning the cached-CDI node-ID set — `nodes`/`has`/`add`/`replace`/`reset`. Removed the route's `nodesWithCdi` `$state` and the orchestrator's `getNodesWithCdi`/`setNodesWithCdi` deps; `configAcquisitionOrchestrator` now merges via `cdiCacheStore.add` and reads `cdiCacheStore.nodes` (post-download filter param widened to `ReadonlySet`). Route reset/refresh/menu go through the store. Added 6 store tests + migrated orchestrator tests to reset/assert the singleton; added `cdiCache.svelte.test.ts` to `test:refactor-gate`. Updated aiwiki/owners.md (orchestrator note + new store entry). | vitest 1076 pass (+6 new); svelte-check 106 (no net-new) |


