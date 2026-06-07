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

### Per-node persistability seam consolidated into `effectiveNodeStore`; lifecycle reset has a single owner
- **Area**: `app/src/lib/layout/effectiveNodeStore.svelte.ts`, `app/src/lib/orchestration/layoutLifecycleOrchestrator.ts`, `app/src/routes/+page.svelte`, all per-node Save/dirty/badge consumers
- **Risk** (resolved): The Save predicate, the orange in-memory-changes dot, the unsaved-count, and the unsaved-new sidebar badge each derived persistability independently. Variants drifted (R5: tree-without-configRead was being treated as persistable; R6: an unread real node was being treated as dirty). The `layoutStore.setUnsavedInMemoryNodeIds` round-trip from `+page.svelte` made the route the de-facto owner of a projection that belonged in the layout layer. The reset path was split between the route and `offlineLayoutOrchestrator` with no single point that enumerated every store the projection read (R7: placeholder roster bled across layout switches).
- **Resolution** (ADR-0011, Spec 014 Steps 10–12): Persistability is now a single predicate `isPersistableInLayout(key) = isFullyCaptured(key) ∧ (isConfigRead(key) ∨ key.kind === 'placeholder')` owned by `effectiveNodeStore`, sibling to `effectiveLayoutStore`. The aggregate `isDirty` getter on the facade replaces four scattered derivations. `layoutLifecycleOrchestrator` is the single owner of `resetForNewLayout` (R7 fix: calls `nodeRoster.clearLayoutScope()`, not `replaceLiveRoster([])`) and `resetForFreshLiveSession`, and it imports every store the facade reads directly so the next added input cannot bypass the reset path.
- **Test pattern**: `layoutLifecycleOrchestrator.test.ts` asserts that after `resetForNewLayout` every facade-read store is empty AND that `effectiveNodeStore.isDirty === false`. Adding a new input to the facade without adding the corresponding reset call will fail the orchestrator test, not surface as a stale-state bug at user-action time.
- **Outstanding**: The standalone `resetLayoutStateForNoLayout` / `resetFreshLiveSessionState` exports in `offlineLayoutOrchestrator.ts` are now legacy wrappers used only by their own tests. They can be deleted once those tests are migrated to the lifecycle orchestrator's contract tests.

### Stringly-typed `NodeKey` resolved; `NodeKeyInput` shim remains as type-hygiene follow-up
- **Area**: `app/src/lib/utils/nodeKey.ts`, all NodeKey consumers
- **Risk** (resolved): `type NodeKey = string` permitted any string to flow into key-shaped slots, including unnormalized dotted-hex Node IDs, raw user input, and arbitrary substrings. Spec 014 Step 7 surfaced two regressions traceable to this: a Save no-op when the keying assumption diverged across layers, and silent miss-routing of edits keyed by un-canonicalized strings.
- **Resolution**: Step 7 Wave 1 introduced a branded discriminated union `NodeKey = LiveNodeKey | PlaceholderNodeKey` with a constructor (`nodeKey()`), equality (`nodeKeyEquals`), and canonical-form helpers (`nodeKeyToString`, `toCanonicalNodeKey`). The `BrandedNodeKey` working name was renamed to `NodeKey` (ADR-0010). Behaviour at boundaries is now safe because `toCanonicalNodeKey` is the single normalization seam.
- **Outstanding**: A transitional shim `NodeKeyInput = string | NodeKey` is still accepted by boundary functions. Full removal (every consumer takes branded `NodeKey` only) was attempted in Wave 2 and reverted because it produced 463 svelte-check errors across the frontend with no behavioural improvement — `toCanonicalNodeKey` already normalises strings safely. This is pure type tightness, not a correctness gap.
- **Suggested action**: Propose a kind/idea issue ("Fully remove `NodeKeyInput` shim; require branded `NodeKey` at all boundaries") so the work is tracked but not blocking. Capture and migrate incrementally as touched.

### Discovery pipeline crashes when `nodes` includes placeholders (resolved)
- **Area**: `+page.svelte` discovery handler, `discoveryOrchestrator.ts`, `nodeRoster.svelte.ts`
- **Risk** (resolved): S8.7 changed `nodes` from live-only `$state` to `$derived(nodeRoster.allEntries)` (live + placeholder) so the "No nodes found" gate worked with placeholders. However, three discovery-related call sites (`handleDiscoveredNode`, `refreshReinitializedNode`, `reconcileRefreshState`) still passed `nodes` as `currentNodes`. Placeholder entries have `node_id: []`, causing `keyOf()` → `nodeKey("")` → throw. Discovery of live nodes silently failed whenever a placeholder was in the roster.
- **Fix**: Changed all three call sites to pass `liveNodes` (the live-only derived view). Added a belt-and-braces guard in `replaceLiveRoster` to skip entries with empty `node_id`. Regression tests: `discoveryOrchestrator.test.ts` ("callers must filter placeholders"), `nodeRoster.svelte.test.ts` ("skips entries with empty node_id").

### Disconnect loses nodes after save because snapshot cache is stale (resolved)
- **Area**: `+page.svelte` save/disconnect flow, `saveLayoutOrchestrator.ts`, `syncSessionOrchestrator.ts`
- **Risk** (resolved): `currentLayoutSnapshots` was set only during layout open, not after save. When the user saved (writing snapshots to disk), then disconnected, the disconnect transition matrix saw `hasSnapshots: false` → `preserved_layout` → cleared all nodes → "No nodes found." Reopening the layout fixed it because open reads snapshots from disk.
- **Fix**: Added `nodeSnapshots: Vec<NodeSnapshot>` to the backend `SaveLayoutResult` and `SaveWithBusWriteResult`. The save orchestrator passes them through to the caller. `+page.svelte` caches `saveResult.nodeSnapshots` in `currentLayoutSnapshots` after a successful save, so disconnect takes the `rehydrated_offline` path.

### `get_node_tree` fast path skips profile annotation for config-read trees (resolved)
- **Area**: `commands/cdi.rs` (`get_node_tree`), `node_tree.rs` (`NodeConfigTree`)
- **Risk** (resolved): `read_all_config_values` stores the tree with merged config values on the proxy but does NOT apply profile metadata (event roles, connector profile, relevance rules). When the frontend subsequently calls `get_node_tree`, the fast path returns this cached tree without profile annotation. Connector controls, event-role overlays, and relevance filtering are all missing. The offline path (`build_offline_node_tree`) always applies the profile, which is why saved layouts worked.
- **Fix**: Added `profile_applied: bool` flag (`#[serde(skip)]`) to `NodeConfigTree`. `apply_profile_metadata_to_tree` sets it to `true`. The `get_node_tree` fast path checks the flag and lazily applies profile annotation when missing, then re-caches the annotated tree. This is a one-time cost per node per session.

### Real-CDI vs profile-signature drift caught only at user-connect time
- **Area**: `app/src-tauri/src/profile/mod.rs`, `app/src-tauri/profiles/*.profile.yaml`
- **Risk**: Inline unit tests for `build_connector_profile_with_diagnostics` synthesize CDIs that match the profile's `firmware-revision` signature by construction, so a profile signature can drift from what real hardware emits without any test failing. The first symptom is "daughterboard controls don't appear" on a real user's node.
- **Mitigation**: Spec 014 Step 7 Wave 6 added `app/src-tauri/tests/fixtures/cdi/tower-lcc-legacy.xml` (a captured legacy Tower-LCC CDI) and a regression test (`captured_legacy_tower_lcc_cdi_builds_connector_profile`) that asserts the bundled profile and library build a connector profile from the real CDI with no warning. The warning channel itself is already surfaced to users via `SegmentView.svelte` (`connectorError` derived from `tree.connectorProfileWarning`) — silent failure becomes loud failure.
- **Outstanding**: Add a captured `tower-lcc-c7.xml` fixture once rev-C7 hardware is available, and apply the same pattern to other carriers (Signal-LCC-P/S/32H) as real CDIs become available. The pattern is: copy `<layout>.d/cdi/<key>.cdi.xml` into `tests/fixtures/cdi/` and add a one-shot test.

---

## Historical entries

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
- **Evidence**: Spec 013 assessment. See deferred idea: `specs/ideas/refactors/bowties-rs-decomposition.md`.
- **Suggested action**: Decompose into catalog builder + layout YAML commands + protocol exchange. Extract the pure catalog-building logic into `bowties-core` via trait injection. Add test coverage for the catalog builder with synthetic CDI trees.
- **Partial resolution (Phase 2)**: The pure catalog-building algorithm (`walk_cdi_slots`, `best_slot`, `slot_for_event_id`, `build_bowtie_catalog`) was extracted to `bowties_core::bowtie::catalog` with 25+ unit tests. The command handler in `bowties.rs` remains a thin coordinator (gathers CDI/config data from AppState, calls core, emits Tauri events). The layout YAML and protocol exchange halves are still mixed into `bowties.rs`.

### Backend testability gap resolved by `bowties-core` extraction (resolved)
- **Area**: `bowties-core/`, `app/src-tauri/src/`
- **Risk** (resolved): All backend unit tests (~237) were blocked from running by the Tauri WebView2Loader DLL issue (`STATUS_ENTRYPOINT_NOT_FOUND` on Windows). Tests existed but had never executed, and 6 contained incorrect expectations that were never caught.
- **Fix**: Extracted pure domain modules across four phases into a `bowties-core` crate with no `tauri` dependency:
  - Phase 1: `placeholder` factory helpers
  - Phase 2: `bowtie/catalog` builder (25+ tests)
  - Phase 3: `layout/capture` snapshot builder (8 tests)
  - Phase 4: `sync/` domain logic — changes, field_meta, classifier (22 tests)
- **Result**: bowties-core runs 310 tests via `cargo test`. ADR-0010 extended to document all four phases.
- **Outstanding**: `placeholder.rs` and `profile/loader.rs` remain in src-tauri (depend on `AppHandle`). Their pure helper functions could be trait-injected and moved in a follow-up. `commands/bowties.rs` layout YAML + protocol exchange halves are still mixed together (partial resolution noted above).

### Saved layout config not seeded into backend proxies (resolved)
- **Area**: `node_registry.rs`, `commands/layout_capture.rs`, `commands/cdi.rs`
- **Risk** (resolved): When a layout was opened and a previously-saved node was rediscovered on the bus, its `LiveNodeProxy` spawned with `config_tree: None`. The previously-captured config existed only in the frontend's `nodeTreeStore` (loaded from snapshot). Three compounding bugs:
  1. The bulk `node-tree-updated` emit after profile annotation fired for ALL nodes, triggering re-fetches that returned zero-filled trees from CDI for unread nodes, overwriting good snapshot data.
  2. The save path unconditionally preferred fresh (empty) snapshots over previous (complete) ones.
  3. ADR-0006's "files that did not change are not rewritten" was not implemented.
- **Fix**: `node_registry` now holds `saved_trees` (populated during `open_layout_directory`, cleared on `clear_layout_scope`). `get_or_create` seeds new proxies with saved trees on bus rediscovery. Bulk emit scoped to only annotated nodes. Save path guards against partial-snapshot downgrades. Content-diff guard skips unchanged node files.
