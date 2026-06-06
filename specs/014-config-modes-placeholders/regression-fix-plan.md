# Regression Fix Plan: Config Read / Bowtie Display / Layout Save

Branch: `014-config-modes-placeholders`
Created: 2026-05-31
Last revised: 2026-05-31 (Steps 1–6d + high-leverage 6e landed; Phase 9
appended for newly-surfaced regressions R5–R7)
Status: **Backend + frontend type migration substantially complete.**
ADR-0010 written; backend `NodeKey` sum type with full test coverage
(`app/src-tauri/src/node_key.rs`) landed; `NodeRegistry` migrated to
`HashMap<NodeKey, NodeProxyHandle>` with all callers parsing to `NodeKey`
at the registry boundary; `events/router.rs` `NodeDiscoveredEvent`
serializes via `NodeKey` (4a). Behavior contracts pinned at every backend
seam: `cdi.rs` parse-parity at the IPC layer (4b), `bowties.rs` catalog
already excludes zero/well-known event IDs (4c), `layout_capture.rs`
populated-tree → empty `missing` vec + placeholder-key round-trip (4d).
Step 4e (`promote_placeholder`) deferred until a real promotion call site
exists. Frontend Steps 5, 6a–6c landed (NodeKey factory, tree store,
roster, discovery orchestrator). Step 6d complete: all IPC wrappers
(`tauri.ts`, `cdi.ts`, `config.ts`, `layout.ts`, `connectorProfiles.ts`)
accept `NodeKeyInput` and canonicalize before invoke. Step 6e high-leverage
migrations complete: `nodeInfo.ts` and `configReadStatus.ts` rewritten to
canonical-key storage; `bowties.svelte.ts`, `ElementPicker`, `SegmentView`,
and `+page.svelte` `selectedUnreadNodeId` canonicalize on lookup. 997/997
frontend tests pass. The core lookup-miss bug class is structurally
impossible at the registry seam.

Remaining type-tightening: three flagged `+page.svelte` callsites
(lines ~306, ~1096, ~1109) still pass dotted form into orchestrators that
canonicalize internally — no behavior bug, type cleanup only. Step 7
(delete legacy `normalizeNodeKey` / `isPlaceholderKey` / `PLACEHOLDER_PREFIX`
/ legacy `type NodeKey = string`; rename `BrandedNodeKey` → `NodeKey`;
remove `NodeKeyInput` shims; audit backend `_node_alias` / `to_hex_string()`).
Step 8 (glossary, owners.md, architecture-health.md).

**Phase 9 added** for three regressions surfaced by manual testing after
Step 6e (R5: Save promotes unread real nodes; R6: orange in-memory-changes
dot on unread node; R7: placeholder survives layout close into new layout).
Root cause is architectural — the layout facade owns value/role projection
but not per-node persistability — not a NodeKey-migration regression. See
Phase 9 below.

---

## Problem Summary

After the placeholder board feature landed (S8 slices), four regressions appeared
in the pre-existing real-node workflow:

| # | Symptom | Root area |
|---|---------|-----------|
| R1 | All Event ID fields show zeros after "Read all configuration" | Config read → tree merge pipeline |
| R2 | "missing value" errors in terminal on layout save | Layout capture walks tree with None leaf values |
| R3 | Bowtie catalog shows single-event "Unknown" bowties | Catalog build or displayability filter |
| R4 | New bowtie connection picker shows all-zero event IDs | Tree-scan for available event slots |

R2–R4 are downstream of R1: if tree leaves lack real values, everything that
reads the tree produces wrong results.

## Root Cause (confirmed by Phase 0)

The failure is candidate #2 ("tree rebuilt without values"), and the underlying
cause is structural: **the `NodeKey` invariant introduced by S4 / ADR-0008 is
documented but not enforced.** The `NodeRegistry` stores proxies under
canonical 12-hex keys (`02015700 02D9`), but the frontend is passing the
dotted `Display` form (`02.01.57.00.02.D9`) to `get_node_tree`. The two lookup
methods on the registry diverge:

- `registry.get(&NodeID)` re-parses and canonicalizes internally → finds the
  proxy → the merge block stores the populated tree correctly.
- `registry.get_by_node_key(&str)` does a raw `HashMap::get(node_key)` with no
  normalization → misses → `get_node_tree` falls through to the
  "build fresh from CDI" path and returns a tree with all `None` EventId
  values → the UI shows zeros.

Phase 0 diagnostics confirmed this end-to-end:

```
[phase0-diag] node 02.01.57.00.02.D9 — registry.get(&parsed_node_id) found_proxy=true
[phase0-diag] node 02.01.57.00.02.D9 — after merge: event_leaves total=466 populated=466 none=0
[phase0-diag] get_node_tree('02.01.57.00.02.D9') fast-path: found_proxy=false had_cached_tree=false
```

The read pipeline, the batch merge, and `merge_config_values_by_space` all
work. The data is correctly cached on the proxy. It just becomes unreachable
from the frontend's point of view because the lookup key form doesn't match
the storage key form.

R2–R4 are all downstream of this same lookup miss: every consumer that goes
through `get_node_tree` (layout capture, catalog builder, connection picker)
sees the rebuilt-from-CDI tree with `None` values.

### Why it slipped past the S4 refactor

S4 (ADR-0008) widened the contract to `NodeKey = LiveNodeId | PlaceholderKey`,
renamed the parameter, and added `get_by_node_key` / `normalizeNodeKey` /
`isPlaceholderKey` helpers. It did **not** make the seams self-defending:

- `NodeRegistry::get_by_node_key` and `insert` accept the `NodeKey` string
  verbatim — no normalization on entry.
- Frontend IPC wrappers (`api/cdi.ts`, `nodeTree.svelte.ts:loadTree`) forward
  whatever string they receive without calling `normalizeNodeKey`.
- `DiscoveredNode`'s NodeID serialization in `lcc-rs` uses the default
  `Display`-based path (dotted form) rather than `to_canonical()`, so live
  nodes arrive in the frontend already in non-canonical form.

The contract is honour-system at every boundary. The bug is what happens when
one producer forgets.

### Why tests didn't catch it

| Gap | Detail |
|-----|--------|
| **No test for the core data path** | `read_all_config_values` is ~400 lines with zero tests. The 12 tests in `cdi.rs` cover only classification/routing helpers added for placeholders. |
| **Backend tree merge untested with values** | `node_tree.rs` has 22 tests for *structure* (CDI → tree shape) but none that verify leaves have non-None values after merge. |
| **Frontend mocks the IPC boundary** | `nodeTree.store.test.ts` mocks `invoke('get_node_tree')` with pre-populated trees, so it never sees the real backend returning zeros. |
| **`isPlaceholderEventId` filter untested** | Added at three layers (backend `bowties.rs`, frontend `bowties.svelte.ts`, `effectiveLayoutStore.svelte.ts`) with no direct test coverage. |
| **`bowties.rs` catalog builder: 1,962 lines, 0 tests** on the core algorithm (already flagged in `aiwiki/architecture-health.md`). Tests added for S8 cover new routing, not the pre-existing value-collection pipeline. |
| **No integration test crosses the IPC boundary** | Every test mocks the next layer down. No test exercises "backend returns a tree → frontend displays values." |

---


## Decision

Replace the stringly-typed `NodeKey` convention with a real sum type, owned by
the backend application layer (not by `lcc-rs`, which stays a pure protocol
library). The single type subsumes both the dotted-vs-canonical confusion and
the `placeholder:` prefix convention:

```rust
// Backend (app/src-tauri)
pub enum NodeKey {
    Live(lcc_rs::NodeID),    // serialises as canonical 12-hex
    Placeholder(uuid::Uuid), // serialises as "placeholder:<uuid>"
}
```

```ts
// Frontend (app/src) — branded discriminated union, mirror of the backend type
export type NodeKey =
  | { readonly kind: 'live'; readonly id: string /* canonical 12-hex */ }
  | { readonly kind: 'placeholder'; readonly id: string /* uuid */ };
```

The decision rests on three facts:

1. **The bug class is a missing type, not a missing call to `normalize()`.**
   Phase 1A/1D demonstrated empirically that enforcing the invariant at one
   seam (the registry) and a couple of producers (discovery events) leaves
   every other seam free to forget. With strings, the compiler cannot help.
   With a real type, identity has one constructor surface, one serializer,
   and the type system surfaces every site that needs to change.
2. **`NodeKey` is a Bowties concept, not an LCC concept.** The protocol only
   knows about `NodeID`. Placeholders exist because the user can configure a
   board before it appears on the bus. Per the placement rule in
   `product/architecture/code-placement-and-ownership.md`, that concept
   belongs in the backend application layer; pushing it into `lcc-rs` would
   be a layering violation.
3. **One sum type is deeper than two narrower types.** An earlier draft of
   this decision split live identity from roster identity into two types
   (`NodeKey` and `RosterKey`). That forced every caller to know which it
   held and to convert between them — exactly the friction that drives
   people back to strings. A single sum type with one factory surface and
   one serialization rule is the deeper module.

### Architectural options considered

| Option | Decision |
|--------|----------|
| **A. Sprinkle normalization at every seam** | Rejected. The bug class is structural; per-site normalization is what we already tried (Phase 1A/1D) and what failed. |
| **B. Single `NodeKey` sum type, backend-owned, frontend mirrors** | **Chosen.** Eliminates the bug class, deepens one module per side, respects layering. |
| **B-prime. Same as B, but in a separate `bowties-domain` crate** | Deferred. No second consumer exists yet; extracting a crate now is speculative structure. Revisit if a CLI tool, alternate frontend, or integration harness wants to share the types. |
| **C. Push `NodeKey` into `lcc-rs`** | Rejected. `lcc-rs` is a protocol library and only knows `NodeID`; placeholders are application concepts and would leak app concerns into the protocol. |
| **D. Two-type split (`NodeKey` for live, `RosterKey` for live\|placeholder)** | Rejected. Forces conversions at every store boundary, duplicates the serialization story, and replicates the same stringly-typed friction one layer up. |

### What `lcc-rs` does and does not change

- **Unchanged:** `NodeID` (6 bytes), all protocol behavior, all wire formats,
  all public API. `lcc-rs` continues to know nothing about NodeKeys,
  placeholders, or Bowties.
- **Optional polish (not required for the migration):** if `NodeID::Display`
  currently produces dotted form and that's surfacing anywhere it shouldn't,
  the application can choose to format via `to_canonical()` at its own
  boundaries. No `lcc-rs` change needed for the migration to succeed.

### Wire contract

Unchanged. `NodeKey` serializes as `"<canonical 12-hex>"` for live nodes and
`"placeholder:<uuid>"` for placeholders — the same strings the IPC layer
already exchanges. The migration is type-tightening only; existing layout
files and existing IPC payloads remain valid.

---

## Implementation Plan

**Strategy:** TDD-driven, compiler-guided migration. Each step is a vertical
slice that lands green (compiles + all existing tests pass + new tests for
the slice pass). No temporary stabilization patch: the app will be unable to
run between the start of Step 3 and the completion of Step 6 (frontend
migration). That is acceptable for this work; manual verification happens
once at the end.

The TDD discipline applies at **behavioral seams**, not at every internal
call site. The compiler is the primary "red" signal when a type signature
changes; tests pin down behavior the compiler cannot see — serializer
round-trips, lookup parity across input shapes, discovery merge correctness,
placeholder-promotion semantics.

### Phase 0 — Diagnose ✅ complete

Outcome above. The Phase 0 diagnostic logging has already been removed.

### Phase 1 — Partial normalization ⚠ superseded

Phase 1A (registry normalization) and Phase 1D (canonical event emission +
frontend `loadTree` normalization) landed and revealed the structural
problem this plan now addresses. Their code is consistent with the migration
direction:

- `node_registry::normalize_node_key` becomes unreachable once the registry
  takes `NodeKey` and is deleted in Step 4.
- The canonical-form switch in `events/router.rs` and `cdi.rs::get_discovered_nodes`
  becomes a one-line `NodeKey::serialize` call once those structs use the
  type (Step 4) and the frontend understands it (Step 6).
- `nodeTreeStore`'s `normalizeNodeKey` calls become unreachable once the
  store takes the branded `NodeKey` (Step 6).

No revert needed.

---

### Step 1 — ADR

**File:** `product/architecture/adr/0010-nodekey-sum-type.md` (supersedes
ADR-0008; subsumes the unfiled "ADR-0010" idea referenced by the Phase 1A/1D
registry-normalization patch).

Capture the *Decision* section above: `NodeKey` is a backend-owned sum type,
canonical 12-hex is the wire form for live nodes, `placeholder:<uuid>` is
the wire form for placeholders, `lcc-rs` is unchanged, the wire contract is
preserved. Document the rejected alternatives.

Writing the ADR first forces the design to be coherent before any code
changes and gives subsequent steps a reference document. No code in this
step.

---

### Step 2 — Backend `NodeKey` type (TDD)

**File:** `app/src-tauri/src/node_key.rs` (new module).

#### Tests first
1. `NodeKey::from_node_id(id).to_string() == id.to_canonical()`.
2. `NodeKey::placeholder(uuid).to_string() == format!("placeholder:{uuid}")`.
3. `NodeKey::parse("02.01.57.00.02.D9")` returns `Live(...)` with the
   canonical id.
4. `NodeKey::parse("020157000002D9")` returns the same `Live(...)`.
5. `NodeKey::parse("placeholder:<uuid>")` returns `Placeholder(uuid)`.
6. `NodeKey::parse("garbage")` returns `Err(NodeKeyParseError)`.
7. Serde round-trip: `serde_json::to_string` → `from_str` returns an equal
   value for both variants.
8. `Hash` + `Eq` parity: two `Live` keys parsed from dotted and canonical
   forms compare equal and hash equal.

#### Implementation
Smallest code that makes the tests pass. Nothing else in the codebase
changes in this step. The type sits alongside the existing string-keyed
APIs.

---

### Step 3 — Backend registry migration (TDD)

**File:** `app/src-tauri/src/node_registry.rs`.

#### Tests first
Update the existing 11 registry tests to construct `NodeKey` values
instead of strings. Add (or keep) tests that pin behavior the type itself
doesn't enforce:

- Insert via `NodeKey::Live(id)` → lookup via a `NodeKey` parsed from the
  dotted form returns the same handle. (Behavior pin; the type makes
  internal normalization unnecessary, but the test documents the contract.)
- Insert a `Placeholder(uuid)` → lookup with the same `Placeholder(uuid)`
  returns the handle.
- Lookup with a different `NodeKey` returns `None`.

#### Implementation
Change `NodeRegistry`'s public methods to take `&NodeKey` instead of
`&str`. The internal `HashMap<String, …>` can become
`HashMap<NodeKey, …>` (the type already implements `Hash + Eq` from Step 2).
Delete `normalize_node_key` — it is no longer reachable.

Follow rustc's errors out from the registry. Every direct caller compiles
or gets a `NodeKey::parse(...)` / `NodeKey::from_node_id(...)` at the
boundary. The compiler is the migration tool here.

---

### Step 4 — Backend domain modules (TDD, one at a time)

Each sub-step migrates one backend module from `String` node keys to
`NodeKey`, with a behavioral test added or updated as the red signal.

| Sub-step | Module | Behavioral test pinned |
|----------|--------|------------------------|
| 4a | `events/router.rs` | `lcc-node-discovered` payload deserializes as `NodeKey::Live(canonical)`. |
| 4b | `commands/cdi.rs` (IPC layer: `get_node_tree`, `get_discovered_nodes`, `read_all_config_values`) | `get_node_tree(NodeKey::Live(id))` returns the populated tree the registry has cached for that id. This is the test that would have failed before Phase 1A. |
| 4c | `commands/bowties.rs` (slot map + catalog) | First test for the catalog builder: given a `config_value_cache` keyed by `NodeKey` containing both zero and non-zero event IDs, the catalog excludes the zero entries. Resolves the long-standing 0-test gap on this 1,962-line module. |
| 4d | `commands/layout_capture.rs` | Layout capture for a populated node produces no "missing value" errors; layout capture for a placeholder serializes the `placeholder:` form correctly. |
| 4e | `placeholder.rs` | Promotion of a placeholder to a live node rewrites the `NodeKey` across the registry and any other backend store that holds one; the operation has a name (`promote_placeholder`) and a test. |

Per sub-step: write the test, watch the compiler surface every call site
that needs to change, thread `NodeKey` through, green. Delete any
string-form helpers in that module that become unreachable.

After Step 4 the entire backend speaks `NodeKey`. The dotted form exists
only as a parse input and as a display derivation; it is not a storage or
comparison form anywhere.

---

### Step 5 — Frontend `NodeKey` mirror (TDD)

**File:** `app/src/lib/utils/nodeKey.ts` (replaces the existing helper).

#### Tests first
- `nodeKey('02.01.57.00.02.D9').kind === 'live'` and its `id` is canonical.
- `nodeKey('020157000002D9')` returns an equal value.
- `nodeKey('placeholder:<uuid>').kind === 'placeholder'` and its `id` is the
  uuid.
- `nodeKey('garbage')` throws (or returns `null` — choose at design time).
- `nodeKeyEquals(a, b)` is true for two `live` keys parsed from dotted and
  canonical, false across kinds, true for matching placeholders.
- `nodeKeyToString(nodeKey('02.01.57.00.02.D9'))` returns the canonical form
  (round-trip with backend wire format).

#### Implementation
Branded discriminated union, one factory (`nodeKey(input: string): NodeKey`),
one stringifier, one equality helper. The factory is the **only** public
constructor; raw string literals cannot become a `NodeKey`. Delete
`normalizeNodeKey` and the `PLACEHOLDER_PREFIX` constant — they are
serialization details of the new type now.

---

### Step 6 — Frontend stores, orchestrators, components (TDD, one at a time)

Migrate in dependency order so each migrated module only depends on already-
migrated modules. The discovery orchestrator (where the visible regression
lives) lands in this step; the SNIP-merge test below is what closes the
current bug.

| Sub-step | Module | Behavioral test pinned |
|----------|--------|------------------------|
| 6a | `lib/stores/nodeTree.svelte.ts` | `loadTree(nodeKey)` invokes the backend with the canonical wire form regardless of how the caller constructed the key. |
| 6b | `lib/stores/nodeRoster.svelte.ts` | Roster entries are keyed by `NodeKey` (not by `formatNodeId(bytes)` strings). Looking up a live entry via a `NodeKey` constructed from dotted or canonical bytes returns the same entry. |
| 6c | `lib/orchestration/discoveryOrchestrator.ts` | **Regression contract:** given an incoming `lcc-node-discovered` event for a known node, after `querySnip`/`queryPip` resolve, the roster entry for that node has `snip_status === 'Complete'` (i.e. SNIP merged onto the existing entry, was not appended as a duplicate). This is the test that pins today's reported bug closed. |
| 6d | `lib/api/tauri.ts` and any other IPC wrappers | Every IPC wrapper that takes a node identity takes `NodeKey` (or `NodeID` bytes where the backend command takes raw bytes). No `string` overloads remain. |
| 6e | Components and routes (`+page.svelte`, sidebar, bowtie panel, etc.) | Components take `NodeKey` from stores. The only place dotted form appears is `nodeKeyToDisplay(key)` at render sites. |

Per sub-step: tsc's errors guide the change. Delete any `formatNodeId`-as-key
or stringly-typed comparison patterns that become unreachable.

---

### Step 7 — Clean up and re-verify

Step 7 has two parts. Part A is the original NodeKey-migration cleanup
(delete what the type migration made unreachable). Part B is the broader
**deletion-pass discipline** added 2026-05-31 after two regressions
(daughter-board UI vanished after the v2 profile gate; Save button no-op
after Phase 9 introduced `effectiveNodeStore`) traced back to the same
root cause: a new authority was added, but the old primitives it was
supposed to replace stayed reachable, and call sites kept reading them
under their old assumptions. The discipline names what "migrate but
don't delete" looks like and gives a checklist that catches it before it
ships.

#### Part A — NodeKey migration cleanup (original scope)

- Delete any helper, constant, or convention rendered unreachable by the
  migration (`normalize_node_key`, frontend `normalizeNodeKey`,
  `PLACEHOLDER_PREFIX`, `formatNodeId`-keyed comparisons).
- Confirm the backend command modules no longer contain `to_hex_string()`
  calls used as keys (display-only call sites are fine but should be
  audited).
- Run the full test suite (`run-all-tests.ps1`).

#### Part B — Deletion-pass discipline (this branch's three authorities)

For every authoritative module introduced or sharpened on this branch,
list the primitive(s) it is supposed to replace, grep for direct reads
of those primitives outside the authority, and either migrate the
caller or fail the build. The point of the pass is not aesthetic — each
"old surface still reachable" finding here corresponds to a concrete
regression the branch has already shipped or is at risk of shipping.

**Authority 1 — `effectiveNodeStore` (Phase 9, Step 10).** Primitives
it replaces:

- `layoutStore.isDirty` read by anything other than `effectiveNodeStore`
  itself. ADR-0011 narrowed its meaning to "LayoutFile-struct edits
  only"; any caller still treating it as "are there any unsaved
  changes?" produces the Save no-op (Option H, below) or the symmetric
  Discard no-op.
- Inline `unsavedInMemoryNodeIds` / `fullyCapturedNodeIds` derivations.
  After Phase 9, the facade is the only place these are computed.
  `+page.svelte` lines ~306 / ~1096 / ~1109 are the known remaining
  call sites flagged in the Status section.
- Ad-hoc `configChangesStore.draftCount > 0 || bowtieMetadataStore.isDirty
  || …` recombinations of dirty primitives anywhere a `canSave` /
  `hasEdits` decision is being made. The presenter
  (`saveControlsPresenter.deriveSaveControlsViewState`) is the only
  place that recombination is allowed, and after Phase 9 it should read
  from `effectiveNodeStore.isDirty` rather than the primitives directly.

Action: grep for `layoutStore.isDirty`, `bowtieMetadataStore.isDirty`,
`configChangesStore.draftCount`, `offlineChangesStore.draftCount`,
`fullyCapturedNodeIds`, `unsavedInMemoryNodeIds` outside
`effectiveNodeStore`, `saveControlsPresenter`, `configSidebarPresenter`,
and `changeTracker`. Every other hit migrates to the facade or is
deleted.

**Authority 2 — `saveLayoutOrchestrated` (already exists; under-used).**
Primitives it replaces:

- Mode-specific post-save cleanup in `SaveControls.handleSave`. The
  orchestrator's `sharedOrchestratorArgs` already owns `markClean`,
  `hydrateLayout`, `clearMetadata`, `clearPersistedDrafts`,
  `clearPersistedPlaceholders`, `clearPersistedRemovals`,
  `updatePartialCaptureNodes`. `SaveControls`'s offline branch
  duplicates `markClean` + `clearAll` + `reloadFromBackend` +
  `commitForSave`; its online branch does only `commitForSave`. The
  asymmetry is the surface area that produces the Save no-op and any
  future drift between modes.
- Ad-hoc gate at the call site
  (`hasNodeEdits = viewState.hasConfigEdits` etc.). The presenter's
  `viewState.canSave` is the single authority. Reading individual
  primitives at the call site is the bug class.

Action: see "Option H" below — this is the only deletion-pass item that
requires non-mechanical code work. The orchestrator absorbs the
remaining mode-specific cleanup callbacks; the component shrinks to a
thin delegate.

**Authority 3 — `NodeKey` sum type (Steps 2–6).** Primitives it
replaces — already covered by Part A. Add to the grep list:
`BrandedNodeKey` (rename target → `NodeKey`); `NodeKeyInput` shims
(remove once all callers pass the branded type); backend
`_node_alias` / `to_hex_string()` audit (display-only is fine; key
usage fails the pass).

#### Part B work — Option H: SaveControls thin-delegate (TDD)

**File:** `app/src/lib/components/ElementCardDeck/SaveControls.svelte`
+ `app/src/routes/+page.svelte` (orchestrator-args extension) +
`app/src/lib/orchestration/saveLayoutOrchestrator.ts` (callback
absorption).

This is the concrete code work that closes the Save no-op and removes
the orchestrator-vs-component ownership split. It is part of Step 7,
not a separate phase, because it is finishing the migration of save
ownership that `saveLayoutOrchestrated` already started.

##### Tests first

1. **Regression contract for the Save no-op.** Component test with
   `layoutStore.isOfflineMode = false`, `configChangesStore.draftCount
   = 0`, `bowtieMetadataStore.isDirty = false`, `layoutStore.isDirty
   = false`, and `effectiveNodeStore.unsavedInMemoryNodeIds = [oneKey]`.
   The presenter reports `canSave: true`. Clicking the Save button
   invokes the `onOfflineSave` prop exactly once. (Today this test
   fails: the handler's parallel gate early-returns.)
2. **Cleanup parity across modes.** Two component tests, one with
   `isOfflineMode = true` and one with `isOfflineMode = false`, both
   with the same set of dirty signals. After a successful save, both
   modes leave `layoutStore.isDirty`, `bowtieMetadataStore.isDirty`,
   `configChangesStore.draftCount`, and
   `effectiveNodeStore.unsavedInMemoryNodeIds` in the same shape.
3. **No duplicate cleanup in the component.** Property test:
   `SaveControls.handleSave` calls `onOfflineSave` and updates
   `saveProgress`; it does not call `layoutStore.markClean`,
   `bowtieMetadataStore.clearAll`, `offlineChangesStore.reloadFromBackend`,
   or `configChangesStore.commitForSave` directly. (These move into the
   orchestrator's callback set.)

##### Implementation

- `SaveControls.handleSave()` becomes a small adapter:
  1. If `!viewState.canSave` return. (No parallel gate.)
  2. Set `saveProgress = 'saving'` with `viewState.pendingEditCount` as
     the total.
  3. `try { const saved = await onOfflineSave(); }` etc. — same
     try/catch shape as today, no per-mode branch.
  4. On success: set `saveProgress = 'completed'`. Nothing else.
- Delete `stageDraftsForOfflineSave`, `commitForSave`, `clearAll`,
  `reloadFromBackend`, and `markClean` calls from `SaveControls`. Move
  any of these that the orchestrator does not already own into
  `sharedOrchestratorArgs` as named callbacks; have
  `saveLayoutOrchestrated` invoke them after the save succeeds, mode
  permitting (e.g. `reloadOfflineChanges` only runs in offline mode).
- The `onOfflineSave` prop is no longer offline-specific. Rename to
  `onSave` (and `onSaveAs`) in the same change. Update the four
  callsites in `+page.svelte` (toolbar + sidebar variants).
- If `stageDraftsForOfflineSave` is doing real work (draft → offline
  change promotion before the orchestrator runs), move that call into
  `saveCurrentCaptureToFile` in `+page.svelte` before the
  `saveLayoutOrchestrated` invocation, gated on
  `layoutStore.isOfflineMode`. The component should not own staging.

##### Acceptance criteria

- [ ] Empty layout → connect → "Read all configuration" → click Save:
  the orchestrator runs, the layout file is written, Save/Discard
  buttons clear.
- [ ] Same flow ending with Close Layout instead of Save: the
  "unsaved changes" prompt fires once and exactly matches the
  presenter's `canSave` signal.
- [ ] Offline mode regression suite still green: open offline layout,
  edit field, Save, reopen — edits persist; close without saving —
  edits discarded.
- [ ] `SaveControls.handleSave` is under ~30 lines and contains no
  references to `layoutStore`, `bowtieMetadataStore`,
  `offlineChangesStore`, or `configChangesStore`.

#### Part B work — Real-data fixture for the profile signature gate

The daughter-board regression (Tower-LCC controls vanished after Spec
014 S5/S6 introduced the top-level `Selector::CdiSignature` gate) is
the same shape as the Save no-op: a new authority was added (the
signature gate) and the old fallback behavior (per-variant filtering
that never returned an empty connector profile) became unreachable
silently. Step 7's contribution is the test discipline that catches
this class:

- Add a real captured Tower-LCC CDI XML as a fixture under
  `app/src-tauri/tests/fixtures/cdi/` (Bowties already saves CDIs to
  the layout companion dir per commit `b95a18a`, so capture is a copy
  step).
- Add a test that calls `build_connector_profile_with_diagnostics`
  against the fixture for both `RR-CirKits_Tower-LCC.profile.yaml`
  firmware variants and asserts `outcome.profile.is_some()`.
- Surface `tree.connector_profile_warning` in the frontend (today it
  is populated but never displayed). A small banner under the
  daughter-board panel is sufficient; the goal is "silent failure"
  becomes "loud failure."

The fix to the actual profile/CDI mismatch is a separate diagnose-first
step (`tree.connector_profile_warning` will name the exact path or
enum-count that disagrees). Step 7's job is to make that class of
regression *impossible to ship invisibly* by adding the real-CDI
fixture pattern and the warning surface. Synthetic test CDIs built by
the same test module that consumes them are circular and stay in the
file as unit tests for the algorithm shape, but they cannot stand
alone as the only signature-gate coverage.

#### Part B work — Contract-test pattern (carry forward)

The component tests added under Option H establish the pattern that
should be applied to any future "presenter computes a decision, handler
acts on it" pair. The pattern is:

- Mount the real component with the real presenter and lightweight
  in-memory stores. Do **not** mock the presenter.
- For every state where the presenter returns "yes" on a decision,
  exercising the action path invokes the corresponding orchestrator /
  emitted intent.
- For every state where the presenter returns "no," the action path
  is a no-op (button disabled or click ignored).

This catches the entire class of "the gate that decides whether to show
the button disagrees with the gate that decides whether to act on the
click." Add it as the standing test pattern for any presenter +
handler pair in `aiwiki/owners.md` once Option H lands.

#### Step 7 — Validation

- Run the full test suite (`run-all-tests.ps1`); all new contract tests
  green.
- Manual smoke (NodeKey path): connect → CDI scan → SNIP visible →
  expand node → segments visible → "Read all configuration" → event
  IDs are real values → save layout → no "missing value" errors →
  bowtie catalog shows correct multi-event entries → new-connection
  picker shows real event IDs.
- Manual smoke (Save/Phase 9 path): empty layout → connect → Read all
  configuration → click Save → file written → Save/Discard buttons
  clear → Close Layout → no unsaved-changes prompt.
- Manual smoke (daughter board): connect to Tower-LCC → daughter-board
  controls appear OR a visible warning explains why; never the silent
  empty state.

---

### Step 8 — `aiwiki/` and glossary update

- `product/glossary.md` — `NodeKey` becomes a first-class term: backend sum
  type with two variants, canonical wire form, frontend branded mirror.
  `NodeID` continues to mean the protocol's 6-byte identifier and only
  appears in protocol contexts.
- `aiwiki/owners.md` — point both the backend identity section and the
  frontend identity section at the new module. Remove references to
  stringly-typed NodeKey conventions.
- `aiwiki/architecture-health.md` — record that the dotted/canonical
  stringly-typed identity smell is resolved, and that `bowties.rs` now has
  its first behavioral test on the catalog builder (4c).

---

## Key Files Reference

| Area | File | Role in the migration |
|------|------|-----------------------|
| **Backend `NodeKey`** | `app/src-tauri/src/node_key.rs` *(new, Step 2)* | Sum type, parse/serialize, `Hash + Eq`. |
| Backend registry | `app/src-tauri/src/node_registry.rs` *(Step 3)* | First consumer of `NodeKey`. Loses `normalize_node_key`. |
| Backend events | `app/src-tauri/src/events/router.rs` *(Step 4a)* | Discovery / reinit events take `NodeKey` payloads. |
| Backend CDI commands | `app/src-tauri/src/commands/cdi.rs` *(Step 4b)* | `get_node_tree`, `get_discovered_nodes`, `read_all_config_values` take/return `NodeKey`. |
| Backend bowties | `app/src-tauri/src/commands/bowties.rs` *(Step 4c)* | Slot map + catalog keyed by `NodeKey`. First test on catalog builder. |
| Backend layout capture | `app/src-tauri/src/commands/layout_capture.rs` *(Step 4d)* | Snapshot/save speaks `NodeKey`. |
| Backend placeholder | `app/src-tauri/src/placeholder.rs` *(Step 4e)* | `promote_placeholder(NodeKey::Placeholder → NodeKey::Live)`. |
| **Frontend `NodeKey`** | `app/src/lib/utils/nodeKey.ts` *(replaces existing, Step 5)* | Branded sum type, factory, equality. |
| Frontend tree store | `app/src/lib/stores/nodeTree.svelte.ts` *(Step 6a)* | First frontend consumer of `NodeKey`. Loses `normalizeNodeKey` calls. |
| Frontend roster | `app/src/lib/stores/nodeRoster.svelte.ts` *(Step 6b)* | Keyed by `NodeKey`, not `formatNodeId(bytes)`. |
| Frontend discovery | `app/src/lib/orchestration/discoveryOrchestrator.ts` *(Step 6c)* | Today's regression closes here. |
| Frontend IPC wrappers | `app/src/lib/api/tauri.ts`, `app/src/lib/api/cdi.ts` *(Step 6d)* | Signatures take `NodeKey`. |
| Components / routes | `app/src/routes/+page.svelte`, sidebar, bowtie panel *(Step 6e)* | Receive `NodeKey` from stores; display via `nodeKeyToDisplay`. |
| ADR | `product/architecture/adr/0010-nodekey-sum-type.md` *(new, Step 1)* | Supersedes ADR-0008. |
| Glossary | `product/glossary.md` *(Step 8)* | `NodeKey` and `NodeID` terminology. |

---

## Out of Scope

- Any change to `lcc-rs`. The protocol library stays pure.
- Extracting `NodeKey` into a separate `bowties-domain` crate (Option
  B-prime). Defer until a second consumer needs it.
- Full decomposition of `bowties.rs` (tracked in `aiwiki/architecture-health.md`).
- Full rewrite of `read_all_config_values`. The migration touches its
  signatures; a deeper refactor is separate work.
- E2E test infrastructure with a real LCC bus.
- Refactoring `+page.svelte` god component (tracked separately).
- Backfilling tests for every untested code path uncovered along the way.
  Add tests for behavior the migration directly changes; queue the rest as
  backlog items.

---

## Phase 9 — Effective node facade + lifecycle owner (regressions R5–R7)

Added 2026-05-31 after manual testing of the Step 6e build surfaced three
new regressions in the layered layout/node state. Phase 9 is independent of
Steps 7–8 and can land in either order; it does not depend on the legacy
cleanup. It also does not depend on Step 4e.

### Regressions

| # | Symptom | Root |
|---|---------|------|
| R5 | Empty layout + connect → top bar shows "N unsaved changes"; Save promotes nodes whose config was never read. Offline reopen warns the values were not captured. | Route-level `fullyCapturedNodeIds` treats "has CDI tree" as persistable. It does not consult `configReadNodesStore`. |
| R6 | Clicking an unread real node shows the orange in-memory-changes dot. | `layoutStore.isDirty` is populated from the same `unsavedInMemoryNodeIds` derivation as R5. The node looks dirty before any config read happens. |
| R7 | Close a layout containing a placeholder, then create a new layout: the old placeholder reappears in the new layout. | `resetLayoutStateForNoLayout()` calls `nodeRoster.replaceLiveRoster([])`, which deliberately preserves placeholders. The full-clear path (`nodeRoster.clearLayoutScope()`) exists but is not wired into layout-close. Wrong reset chosen. |

### Why tests didn't catch these

- **No single persistability predicate exists.** Save (`canSaveLayoutAction`),
  the unsaved-changes count, the orange dot, and the unsaved-new badge each
  compute their own slice in different files (`+page.svelte`,
  `configSidebarPresenter.ts`, `saveControlsPresenter.ts`,
  `changeTracker.svelte.ts`). There is no shared function to test against,
  so the four slices have drifted.
- **No integration test asserts "empty layout + connect + Save → roster
  contains only nodes whose config was actually read."** The "fully
  captured" threshold from ADR-0007 is tested for the *tree-completeness*
  half but not for the *config-read* half — because the config-read half
  isn't part of the threshold today.
- **No test enumerates the stores a layout-close must reset.** When
  Spec 014 moved placeholders into `nodeInfoStore`, the reset path
  silently drifted: the existing path clears `nodeRoster` but the
  placeholder-bearing store (`nodeInfoStore`) is only partially cleared.

### Root cause

ADR-0004 set up `effectiveLayoutStore` as the facade that projects the
three layers (layout file, in-memory edits, transient drafts) into the
values and roles the UI renders. ADR-0007 named the same three layers for
the *node-promotion* question but located the threshold logic inline in
`+page.svelte` and **did not extend the facade** to own per-node
projection. Today the facade exposes `effectiveValue`, `effectiveRole`,
`slotsByRole`, `isSlotFree` — value-level projection only. It does not
expose `nodeOrigin`, `isFullyCaptured`, `isConfigRead`, or
`isPersistableInLayout`, and `configReadNodesStore` is not even an input
to the facade. The depth ADR-0007 implied was never built.

R7 is a related but distinct symptom of the same architectural shape:
because no single owner enumerates the stores that constitute "a layout's
worth of state," every store added (placeholders into `nodeInfoStore`)
risks drifting from every reset path.

### Decision

Extend the layout facade to own per-node projection across the same three
layers — the depth ADR-0007 named but never enforced — and centralise
lifecycle reset behind a single owner. This is an extension of ADR-0004,
not a new architectural direction.

---

### Step 9 — ADR amendment

**File:** `product/architecture/adr/0011-effective-node-facade.md` (extends
ADR-0004 and ADR-0007).

Capture: `effectiveLayoutStore` projects values; a new sibling
`effectiveNodeStore` (or extension to the same module) projects per-node
state across the same three layers — taking `configReadNodesStore` as an
input. Persistability into the layout file is
`fullyCaptured ∧ (configRead ∨ kind === 'placeholder')`. Lifecycle reset
moves behind a single orchestrator that enumerates every store touched by
a layout. Document the two named reset paths
(`resetForNewLayout` / `resetForFreshLiveSession`) and the failure mode
they replace. No code in this step.

---

### Step 10 — Effective node facade (TDD)

**File:** `app/src/lib/layout/effectiveNodeStore.svelte.ts` (new),
re-exported from `app/src/lib/layout/index.ts`.

#### Tests first

1. `nodeOrigin(key): 'live-only' | 'layout-only' | 'both' | 'placeholder'`.
2. `isFullyCaptured(key)`: true iff `nodeTreeStore` has the tree AND key
   not in `partialCaptureNodes`. (Pins the existing route-level rule.)
3. `isConfigRead(key)`: thin getter over `configReadNodesStore` with
   canonicalisation via `toCanonicalNodeKey`.
4. `isPersistableInLayout(key)`: true iff
   `isFullyCaptured(key) ∧ (isConfigRead(key) ∨ key.kind === 'placeholder')`.
   **Regression contract for R5:** a node with a tree but absent from
   `configReadNodesStore` returns false.
5. `unsavedInMemoryNodeIds`: live keys that are `isPersistableInLayout`
   AND absent from `layoutStore.activeContext.layoutNodeIds`. Replaces
   the `$derived.by` block in `+page.svelte`.
6. `isDirty`: any persistable in-memory addition OR any draft / metadata
   / offline-change edit. **Regression contract for R6:** an unread real
   node alone does not flip `isDirty`.

#### Implementation

Svelte 5 `$derived` getters reading `nodeTreeStore`, `nodeInfoStore`,
`configReadNodesStore`, `layoutStore.activeContext`, `partialCaptureNodes`,
`configChangesStore`, `bowtieMetadataStore`, `offlineChangesStore`. Inputs
only; no writes.

Migration:

- Replace the `fullyCapturedNodeIds` and `unsavedInMemoryNodeIds`
  derivations in `+page.svelte` with reads through the facade.
- Remove the `layoutStore.setUnsavedInMemoryNodeIds(...)` round-trip;
  `isDirty` derives directly from the facade. `layoutStore.isDirty`
  becomes a passthrough or is deleted in favour of
  `effectiveNodeStore.isDirty`.
- Route `canSaveLayoutAction` (`+page.svelte`), the orange dot
  (`+page.svelte` layout status), the unsaved-changes count
  (`changeTracker.svelte.ts`), and the unsaved-new badge
  (`configSidebarPresenter.shouldShowConfigNotReadBadge` /
  `buildSidebarNodeEntries.isUnsavedNew`) through
  `isPersistableInLayout` / `isDirty`.

---

### Step 11 — Layout lifecycle owner (TDD)

**File:** `app/src/lib/orchestration/layoutLifecycleOrchestrator.ts` (new
— extracted from the two reset functions currently in
`offlineLayoutOrchestrator`).

#### Tests first

1. **Regression contract for R7:** `resetForNewLayout()` clears
   placeholders from `nodeInfoStore` (calls
   `nodeRoster.clearLayoutScope()`, not `nodeRoster.replaceLiveRoster([])`).
   Setup: seed `nodeInfoStore` with one placeholder entry; assert it is
   gone after the call.
2. `resetForNewLayout()` enumerates every store the facade reads:
   `nodeRoster`, `nodeTreeStore`, `configReadNodesStore`,
   `offlineChangesStore`, `bowtieMetadataStore`, `configChangesStore`,
   `connectorSelectionsStore`, `configSidebarStore`, `partialCaptureNodes`,
   `syncSessionOrchestrator.autoTrigger`. Pattern: each store declares a
   `resetForLayoutClose` method (or is registered in an explicit list at
   the top of the orchestrator); the orchestrator iterates the list. A
   test asserts that the list matches the facade's declared input set so
   that adding a new input to the facade without wiring its reset fails
   the test.
3. `resetForFreshLiveSession()` (disconnect / reconnect within the same
   layout) clears live-only state and explicitly preserves placeholders.
4. The two paths are named methods on the orchestrator; callers in
   `+page.svelte` use the named methods. No anonymous reset closures
   remain at call sites.

#### Implementation

Move the two existing reset paths (`resetLayoutStateForNoLayout` and
`resetFreshLiveSessionState`) into `layoutLifecycleOrchestrator` with
explicit, intent-revealing names. The wrong-default in the close path
(`replaceLiveRoster([])` instead of `clearLayoutScope()`) is corrected by
naming the methods after the lifecycle event, not after the mechanism.

---

### Step 12 — Validate + docs

Manual smoke (replaces the R5/R6/R7 reproductions above):

- Empty layout + connect → top bar does not show "N unsaved changes"
  before any config is read. Save is disabled (or, if enabled, refuses to
  promote unread nodes).
- Read one node's config → top bar shows 1 unsaved change. Save promotes
  only that node. Offline reopen shows no "values were not captured"
  warning.
- Click an unread real node → no orange in-memory-changes dot.
- Open a layout containing a placeholder → Close → Create new layout →
  placeholder is gone.

Docs:

- `product/glossary.md`: add `effectiveNodeStore`,
  `isPersistableInLayout`; clarify `fullyCaptured` vs. `configRead` vs.
  `persistable`.
- `aiwiki/owners.md`: point the per-node persistability section at
  `effectiveNodeStore`; remove the route-level derivation pointers and
  the `layoutStore.setUnsavedInMemoryNodeIds` round-trip note.
- `aiwiki/architecture-health.md`: record that the per-node projection
  seam is now owned by the facade and that lifecycle reset has a single
  owner. Note the test pattern that fails when a new facade input is
  added without a reset.

---

### Phase 9 — Key Files Reference

| Area | File | Role |
|------|------|------|
| ADR | `product/architecture/adr/0011-effective-node-facade.md` *(new, Step 9)* | Extends ADR-0004 / ADR-0007. |
| Effective node facade | `app/src/lib/layout/effectiveNodeStore.svelte.ts` *(new, Step 10)* | Per-node projection across three layers; persistability + dirty. |
| Facade re-export | `app/src/lib/layout/index.ts` *(Step 10)* | Surface the new getters alongside the existing `effectiveLayoutStore`. |
| Route | `app/src/routes/+page.svelte` *(Step 10)* | Replaces `fullyCapturedNodeIds` / `unsavedInMemoryNodeIds` derivations; reads facade. |
| Layout store | `app/src/lib/stores/layout.svelte.ts` *(Step 10)* | `setUnsavedInMemoryNodeIds` round-trip removed; `isDirty` derives from facade. |
| Sidebar presenter | `app/src/lib/components/ConfigSidebar/configSidebarPresenter.ts` *(Step 10)* | `shouldShowConfigNotReadBadge` and `isUnsavedNew` reroute through `isPersistableInLayout`. |
| Save controls | `app/src/lib/components/ConfigSidebar/saveControlsPresenter.ts` *(Step 10)* | `canSaveAction` consults facade `isDirty`. |
| Change tracker | `app/src/lib/stores/changeTracker.svelte.ts` *(Step 10)* | Counts mirror facade `isDirty`. |
| Lifecycle owner | `app/src/lib/orchestration/layoutLifecycleOrchestrator.ts` *(new, Step 11)* | `resetForNewLayout` + `resetForFreshLiveSession`. |
| Offline orchestrator | `app/src/lib/orchestration/offlineLayoutOrchestrator.ts` *(Step 11)* | Two reset functions extracted into the lifecycle owner. |
| Glossary | `product/glossary.md` *(Step 12)* | `effectiveNodeStore`, persistability terminology. |

---

### Phase 9 — Out of Scope

- Splitting `effectiveLayoutStore` and `effectiveNodeStore` into separate
  files vs. extending the existing file — design choice deferred to
  Step 10 implementation; either is acceptable as long as the public
  surface lives behind `$lib/layout/index.ts`.
- Removing `layoutStore.isDirty` entirely. Step 10 may leave it as a
  passthrough getter onto `effectiveNodeStore.isDirty` if callers are
  numerous; full removal is cleanup that can land in Step 7 of the
  NodeKey migration or later.
- Auditing every other "lifecycle reset" path (disconnect, transport
  swap, profile reload). Step 11 covers layout-close and new-layout
  only; other lifecycle events stay with their current owners until a
  similar regression surfaces.
- Backfilling tests for `bowties.rs` catalog interactions with the
  facade. The Phase 9 facade tests stop at per-node predicates; bowtie
  preview tests remain in the existing ADR-0004 suite.
