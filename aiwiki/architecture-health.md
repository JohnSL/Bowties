# Architecture Health

Coupling risks, depth assessments, and architecture debt discovered during feature work. This file grows incrementally — add entries as issues are found.

## Format

Each entry:
- **Area**: affected modules/layers
- **Risk**: what could go wrong
- **Evidence**: where the issue was observed
- **Suggested action**: fix or investigation needed

---

## Entries

### Reset callback consistency across layout orchestrator functions
- **Area**: `offlineLayoutOrchestrator.ts`, `+page.svelte`
- **Risk**: When a new reset function is added or an existing one is modified, it's easy to forget a callback (e.g. `resetSidebar` was missing from two of three reset paths). The set of stores that need clearing on layout transitions is implicit — there's no checklist or compile-time enforcement.
- **Evidence**: `resetLayoutStateForNoLayout` and `openOfflineLayoutWithReplay` both forgot to clear the config sidebar while `resetFreshLiveSessionState` included it. Fixed May 2026.
- **Suggested action**: When adding new store state that must be cleared on layout transitions, check all three reset functions and their tests. Consider adding a comment in the orchestrator listing the full set of reset paths for cross-reference.

### Online save ordering causes stale catalog / blank bowties
- **Area**: `SaveControls.svelte`, `configChangesStore`, `bowties.svelte.ts` (preview getter), `+page.svelte` (`node-tree-updated` listener)
- **Risk**: Bus writes before layout file save triggers draft pruning (`pruneResolvedDraftsForNode` via `node-tree-updated`), which switches the bowtie preview to the fast path with a stale catalog. Bowties show "No producers" / "No consumers." Cancel after writes leaves blank bowties with no recovery path.
- **Evidence**: Reproduced May 2026 — reset boards, create connections, click Save → blank bowties during dialog and after cancel.
- **Suggested action**: Implement three-phase save reorder (`specs/013-save-flow-reorder/plan.md`, slices S1+S2). ADR: `0001-save-layout-before-bus-writes.md`.
- **Status**: Fix in progress — spec 013 architecture assessment complete, slices defined.

### `+page.svelte` god component (1,942 lines)
- **Area**: `app/src/routes/+page.svelte`
- **Risk**: ~40 `$state` variables managing unrelated concerns (discovery, CDI download, config reading, layout lifecycle, sync, dialogs). Inlines multi-step async workflows that belong in orchestrators. Every workflow change is fragile because all state is local.
- **Evidence**: `saveCurrentCaptureToFile` bypasses `saveLayoutOrchestrator` with inlined save-and-rebuild logic. Spec 013 assessment identified this as the single biggest risk to feature delivery.
- **Suggested action**: Spec 013 S1 extracts the save flow. Longer-term, extract discovery, CDI download, and config read session workflows to their respective orchestrators with route-level state replaced by store subscriptions.

### `bowties.rs` untested core algorithm (1,962 lines, 0 tests)
- **Area**: `app/src-tauri/src/commands/bowties.rs`
- **Risk**: The catalog builder is the intellectual core of the app. Mixed with layout YAML commands and protocol exchange. Zero test coverage on the most complex algorithm in the backend.
- **Evidence**: Spec 013 assessment. See deferred idea: `specs/ideas/bowties-rs-decomposition.md`.
- **Suggested action**: Decompose into catalog builder + layout YAML commands + protocol exchange. Add test coverage for the catalog builder with synthetic CDI trees.
