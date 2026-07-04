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

### Event ID canonical-form mismatch between PCER handler and config tree (resolved)
- **Area**: `app/src-tauri/src/events/router.rs` (`handle_pcer`), `bowties-core/src/node_tree.rs` (`bytes_to_dotted_hex`), `app/src/lib/utils/channelState.ts` (`deriveChannelState`)
- **Risk** (resolved): PCER event handler produced contiguous hex (`0201570002D90100`), config tree resolution produced dotted hex (`02.01.57.00.02.D9.01.00`). `deriveChannelState()` used string equality to match them — occupancy indicators never left "unknown" state despite events arriving on the bus.
- **Evidence**: Spec 016 S1: traffic monitor showed PCER events arriving, but channel indicators stayed ○ (unknown). Same bug class as the Node ID canonical-form mismatch in ADR-0010's 2026-06-25 extension.
- **Resolution** (ADR-0010, 2026-06-26 extension): Adopted contiguous uppercase hex as the canonical form for all event IDs, matching the existing Node ID convention. `bytes_to_dotted_hex()` → `bytes_to_canonical_hex()` (private, contiguous). New public API: `parse_event_id_hex()` (both formats → bytes), `normalize_event_id_hex()` (→ canonical string), `bytes_to_display_hex()` (→ dotted display). All parsers accept both formats for backward compatibility with existing layout files.
- **DRY follow-up** (resolved, ADR-0010 2026-06-26 extension): Generic byte ↔ hex helpers now live in `lcc-rs/src/types.rs` (`format_canonical_hex<const N>`, `format_dotted_hex<const N>`, `parse_hex_id<const N>`) and `app/src/lib/utils/hexId.ts` (`formatCanonicalHex`, `formatDottedHex`, `parseHexId`). `NodeID` and `EventID` methods delegate; `bowties-core::node_tree` helpers delegate to `lcc_rs::EventID`; the previous inline `format!("{:02X}", b)` sites in `field_meta.rs` and `placeholder.rs`, and the inline TypeScript parsers in `editKey.ts` / `treeConfigValuePersistence.ts` / `offlineLayoutOrchestrator.ts` / `bowties.svelte.ts` / `eventIds.ts`, all route through the shared helpers. The duplicate `formatters.ts::formatEventId` was removed.

### Svelte 5 reactivity: assigned-but-unused dependencies are silently stripped (resolved)
- **Area**: `app/src/routes/+page.svelte` (Spec 016 channel-event resolution effect)
- **Risk** (resolved): A pattern like `const _trees = nodeTreeStore.trees; /* reactive dependency */` does NOT reliably create a reactive dependency in Svelte 5 effects. Build-time dead-code elimination (or the reactivity tracker itself) may treat the unused binding as dead. Result: the effect fires once on mount with the initial empty value and never re-runs when the underlying store updates.
- **Evidence**: Spec 016 S1: channel occupancy indicators stayed ○ (unknown) even after the event-ID canonical-form bug was fixed. The resolution effect ran once when `nodeTreeStore.trees` was empty (CDI not yet loaded), returned empty event IDs, and never re-ran when CDI reads populated trees. Adding a `console.log(..., _trees.size)` accidentally fixed it by turning the dead assignment into a real property read.
- **Resolution**: Read the property as part of a real expression — either the condition guard (`if (... && nodeTreeStore.trees.size > 0)`), a returned value, or a logged value. Never rely on a `const x = store.value;` assignment alone to register a dependency.
- **Rule**: Inside `$effect` and `$derived.by`, every store property that should trigger re-evaluation must be **read in an expression whose value is used**. Comments like `// reactive dependency` are not enforced by the compiler.

### State proxy equality comparison broken in reactive loops (T037, resolved)
- **Area**: `app/src/lib/components/Bowtie/BowtieCard.svelte`
- **Risk** (resolved): Svelte 5's `$state(...)` wraps values in reactive proxies. Comparing a `$state` proxy with `===` against an object from a reactive loop variable (`{#each}`) always returns `false` because the proxy identity never matches the loop variable identity across re-renders. Result: `{#if reclassifyingEntry === entry}` conditionals always evaluate false, blocking UI state transitions (role classification prompt never renders).
- **Evidence**: Console warning `state_proxy_equality_mismatch`. User clicking "?" on ambiguous event slots had no effect — the `RoleClassifyPrompt` component never appeared.
- **Fix**: Store the composite slot key string (`"${node_key}:${element_path.join('/')}"`) instead of the full object. Comparison becomes `{#if reclassifyingKey === entryKey}` — a string identity check that survives re-renders. Added shared convention to `aiwiki/owners.md` Slot Key Generation section documenting the pattern and the proxy-safety rule.
- **Test coverage**: Two regression tests in `BowtieCard.test.ts` verify: (1) clicking the ? button shows the prompt, (2) selecting a role fires the callback.
- **Precedent**: ADR-0010 documented a similar identity fragility when `NodeKey` was a plain string — the solution was to use a typed discriminated union. This fix extends that lesson to UI state management: prefer identifiers (strings, numbers) over object identity in reactive contexts.

### Reset callback consistency across layout orchestrator functions
- **Area**: `offlineLayoutOrchestrator.ts`, `+page.svelte`
- **Risk**: When a new reset function is added or an existing one is modified, it's easy to forget a callback (e.g. `resetSidebar` was missing from two of three reset paths). The set of stores that need clearing on layout transitions is implicit — there's no checklist or compile-time enforcement.
- **Evidence**: `resetLayoutStateForNoLayout` and `openOfflineLayoutWithReplay` both forgot to clear the config sidebar while `resetFreshLiveSessionState` included it. Fixed May 2026.
- **Suggested action**: When adding new store state that must be cleared on layout transitions, check all three reset functions and their tests. Consider adding a comment in the orchestrator listing the full set of reset paths for cross-reference.

### Online save ordering causes stale catalog / blank bowties (resolved)
- **Area**: `SaveControls.svelte`, `configChangesStore`, `bowties.svelte.ts` (preview getter), `+page.svelte` (`node-tree-updated` listener)
- **Risk** (resolved): Bus writes before layout file save triggered draft pruning (`pruneResolvedDraftsForNode` via `node-tree-updated`), which switched the bowtie preview to the fast path with a stale catalog. Bowties showed "No producers" / "No consumers." Cancel after writes left blank bowties with no recovery path.
- **Evidence**: Reproduced May 2026 — reset boards, create connections, click Save → blank bowties during dialog and after cancel.
- **Resolution** (Spec 013, S1–S8, 13 slices): Three-phase save reorder saves layout **before** bus writes (ADR-0001). Backend-authoritative save (ADR-0002) accepts edit deltas and returns the persisted layout. Unified display resolution (ADR-0003). Layout facade + effective view store (ADR-0004) clears drafts on save so catalog never goes stale. Deep layout module (ADR-0005) keeps file knowledge private. Journaled in-place save (ADR-0006) eliminates cloud-sync contention. Known-layout registry (S5) + layout picker gate (S6) + layout-scoped connections (S7) + durable node roster (S8) complete the feature. 929 vitest + 328 cargo tests pass.

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
- **Fix**: `node_registry` holds `saved_trees` (populated during `open_layout_directory`, cleared on `clear_layout_scope`). `get_or_create` seeds new proxies with saved trees on bus rediscovery. Bulk emit scoped to only annotated nodes. Save path guards against partial-snapshot downgrades. Content-diff guard skips unchanged node files.
- **Slice-3a follow-up (2026-06-28)**: `saved_trees` is retained as a one-shot proxy-seeding cache. The save path no longer reads from it — `LayoutState.saved[].tree` is the canonical source. Both maps are populated by the same loop in `open_layout_directory`, so they cannot drift; `saved_trees` could be replaced with a tree-lookup callback on `get_or_create`, but the structural duplication is benign (load-once, read-once-on-spawn) and removing it forces a wider API change.

### Persistent layout data scattered across parallel caches (resolved)
- **Area**: `bowties-core/src/node_proxy.rs`, `bowties-core/src/node_registry.rs`, `app/src-tauri/src/state.rs`, `app/src-tauri/src/commands/{bowties,cdi,layout_capture}.rs`, `bowties-core/src/layout/state.rs`
- **Risk** (resolved): Persistent in-memory layout state lived in three places — `LiveNodeProxy.cdi_data` / `cdi_parsed`, `node_registry.saved_trees`, and `AppState.offline_bowtie_data` — none of which had the complete picture. The save flow walked per-node proxies for snapshots, so any node whose proxy didn't currently hold CDI (the normal state after every reconnect) was silently dropped from the layout file on save — physically deleting its `nodes/<key>.yaml` and `cdi/<key>.xml`. Two regressions confirmed at byte level on 2026-06-28 (R1: open → connect → edit → save dropped 4 of 5 nodes; R2: Tower-LCC dropped on every save regardless of session phase).
- **Fix**: `bowties_core::layout::state::LayoutState` introduced as the single in-memory owner of an open layout's three-layer projection (saved → captured → drafts). Slice 1 stood it up alongside the existing scatter; slice 2 routed `proxy_snapshot_data`'s `cdi_xml_len` derivation through `LayoutState::cdi_xml` with a `bwlog!` for the truly-no-data case; slice 3a (2026-06-28) deleted `LiveNodeProxy::cdi_data` / `cdi_parsed` + their `ProxyMessage` variants and deleted `AppState::offline_bowtie_data` entirely. R1/R2 are now structurally impossible: the data source for save snapshots is `LayoutState`, populated once at open time from disk and updated via `record_captured` as live reads complete. R1/R2 behavior pins live in `bowties-core::layout::state` tests.
- **Pattern**: Working buffers on a per-actor mailbox (e.g. `LiveNodeProxy::config_tree` for in-flight read accumulation) are legitimately scoped to the actor and do NOT need to move into `LayoutState`. The principle test that motivated the slice was "is this a duplicate cache of *persistent* data that the save flow could read out-of-sync?" — the removed fields were yes; the retained fields are working memory for in-progress bus operations and have no save-path consumer.
- **2026-07-04 follow-up**: `LiveNodeProxy::config_values` (a legacy HashMap<String, [u8;8]> caching EventId bytes by path) deleted. This field was a pre-tree artifact that duplicated data the config tree already held, but was NOT updated by `commit_leaf_value` during bus writes. After Phase 2 bus writes, the catalog builder (Phase 4) read the stale cache and produced phantom entries mixing old and new event IDs. Fix: catalog builder now reads from the authoritative tree via `collect_event_id_leaves()`. All sync sites (`merge_config_values`, `set_config_values`) that existed solely to keep this cache coherent with the tree were also deleted. The `SynthesizedNodeProxy.config_values` field was likewise removed (placeholders already have trees populated identically at factory time).

### Companion file data loss in full save path (resolved)
- **Area**: `commands/layout_capture.rs`, `connectorSelectionsStore`, `bowties-core/layout/types.rs`
- **Risk** (resolved): Two related bugs where layout data changes were lost on full save:
  1. `save_layout_directory` constructed `LayoutDirectoryWriteData` with `channels: ChannelsDocument::default()` instead of carrying forward `previous.channels`. Every full save wiped `channels.yaml`.
  2. `connectorSelectionsStore.saveDocument()` skipped IPC for slots set to "None installed" (`selectedDaughterboardId` undefined). No `ClearNodeModeSelection` delta existed, so old selections were never removed from disk. Full save read the stale selection and preserved it.
- **Fix**: (1) Use `previous.as_ref().map(|p| p.channels.clone()).unwrap_or_default()`. (2) Add `ClearNodeModeSelection` delta variant; `saveDocument` now calls `clearNodeModeSelection` IPC for cleared slots.
- **Pattern**: When adding a new companion file or delta-persisted field, verify it flows through **both** the partial-update path (single IPC) **and** the full save path (`save_layout_directory`). The two paths diverge at the `LayoutDirectoryWriteData` construction point.
- **Resolution (ADR-0012)**: The write-through pattern that caused both bugs was eliminated. All layout edits now flow through the in-memory draft layer. No user-interaction handler calls a backend mutation IPC directly. The save workflow is the single path that writes layout data to disk. `set_node_mode_selection` and `clear_node_mode_selection` IPC commands removed; their delta variants remain and are collected at save time by `connectorSelectionsStore.collectDeltas()`.
