# Proposal: Backend `LayoutState` — single owner of the three-layer in-memory layout model

**Status:** Draft proposal (not yet a spec)
**Date:** 2026-06-28
**Trigger:** Online-save data-loss regressions (see "Background" below)
**Supersedes (when accepted):** parts of ADR-0009 (scope of `NodeProxy` ownership)
**Extends (when accepted):** ADR-0005 (`layout/` module surface), ADR-0011 (three-layer projection now exists symmetrically on the backend)

## Goal

Give the backend a single deep module that owns the **persistent in-memory representation of an open layout** across all three layers (saved → drafts → live-derived). Make the save flow read from that module instead of walking node proxies. Reduce `NodeProxy` to a pure bus-IO actor.

Outcome: an entire class of data-loss bugs ("save silently drops nodes whose proxy state happens to be incomplete") becomes structurally impossible.

## Background — what triggered this

On the spec 018 branch, three regressions surfaced (see session of 2026-06-28 in chat history; also `[BUG-INVEST]` instrumentation in `app/src-tauri/src/commands/layout_capture.rs`, `app/src/routes/+page.svelte`, `app/src/lib/orchestration/configReadOrchestrator.ts`):

- **R1.** Open a saved layout (5 nodes on disk) → go online → edit one field → click Save → 4 of 5 nodes are silently deleted from the layout file on disk (their `nodes/<key>.yaml` and `cdi/<key>.xml` files are physically removed). Reproduced and confirmed with byte-level evidence in the `temp/Test 3` vs `temp/Test 3 - Copy` directories.
- **R2.** Tower-LCC silently dropped on every save (regardless of session phase), because its proxy's `cdi_data` is `None` even after a successful config read.
- **R3.** A node that doesn't advertise CDI is still offered "Read Configuration." (Did not reproduce in the current session; deferred — separate investigation.)

R1 and R2 share one mechanism. The save flow at [layout_capture.rs#L415](app/src-tauri/src/commands/layout_capture.rs#L415):

```rust
snapshots.retain(|s| s.cdi_ref.fingerprint != "not_supported"
                  && s.cdi_ref.fingerprint != "missing");
```

silently drops every snapshot built from a proxy that doesn't currently hold CDI in memory. Fingerprint becomes `"missing"` (a transient "not loaded yet" state) when `data.cdi_xml_len` is `None` on the proxy. For freshly-spawned proxies after reconnect — which is the normal state for every saved-layout node — that's every single one of them.

### Why the existing architecture allowed this

`NodeProxy` was designed (ADR-0009) as "the in-memory state holder for both real and synthesized nodes." The save flow reads from it because that was the design intent. But the design conflates **two unrelated responsibilities** into one struct:

1. **Bus session state** — alias, in-flight SNIP/PIP queries, transport handle, CDI download retries, event subscriptions. *Legitimately owned by the proxy.*
2. **Persistent node data** — CDI XML, config values, SNIP fields, config tree. *Should be owned by the layout, not by a per-node actor whose lifecycle is tied to the bus connection.*

When a layout is open, the persistent layer of every saved node already exists on disk. Spawning a fresh proxy on reconnect should not be the trigger that requires us to reconstruct that data from scratch — yet today the save flow's only source of truth for "what should be in `nodes/<key>.yaml`" is the proxy.

### The half-implementations already present

`NodeRegistry` already has scaffolding that hints at the right architecture but is incomplete:

```rust
// bowties-core/src/node_registry.rs
pub struct NodeRegistry {
    proxies: RwLock<HashMap<NodeKey, NodeProxyHandle>>,
    // ...
    /// Config trees loaded from a saved layout, keyed by NodeKey.
    /// Populated during `open_layout_directory`, consumed during node
    /// discovery to seed freshly-spawned proxies so they start with the
    /// previously-captured config rather than an empty tree.
    saved_trees: RwLock<HashMap<NodeKey, NodeConfigTree>>,
}
```

`get_or_create` seeds the new proxy with the saved config tree on connect. **It does not seed CDI XML, SNIP, or config values.** Those exist in a parallel cache (`AppState.offline_bowtie_data`) used only for the offline bowtie-catalog rebuild and never wired into proxies.

So the current state is: two partial caches (`saved_trees`, `offline_bowtie_data`) plus the proxy registry, none of which has the complete picture, all of which the save flow ignores in favor of reading directly from per-node proxies.

This proposal consolidates all three into one owner with a coherent surface.

## Current architecture inventory (concrete)

| Concern | Today | New owner |
|---|---|---|
| Disk file layout (YAML, CDI XML, journal, atomic writes) | `bowties-core/src/layout/` (ADR-0005). Stateless. | Unchanged. `LayoutState` calls into it. |
| Loaded layout on open | `LayoutDirectoryReadData` returned from `read_capture`, then split across `state.active_layout`, `state.offline_bowtie_data`, `node_registry.saved_trees`, and (indirectly via deltas) the frontend stores | **`LayoutState`** — single in-memory owner |
| Saved CDI XML keyed by `NodeKey` | `state.offline_bowtie_data.cdi_xml` | `LayoutState` |
| Saved config tree per node | `node_registry.saved_trees` | `LayoutState` |
| Saved config values per node | `state.offline_bowtie_data.config_values` | `LayoutState` |
| Saved node snapshot file content | re-read from disk on every save (via `previous = read_capture(target).ok()`) | `LayoutState` (loaded once, kept current) |
| Saved bowties / channels / facilities / offline-changes documents | `LayoutLoaded.bowties / channels / facilities`; offline changes also in `state.offline_changes_cache` | `LayoutState` |
| Active-layout metadata | `state.active_layout: Option<ActiveLayoutContext>` | `LayoutState` (active_layout becomes a thin pointer to the loaded state, or replaced by it) |
| Live bus alias for a node | `LiveNodeProxy.alias` | Unchanged (proxy) |
| Live SNIP/PIP responses | `LiveNodeProxy.snip / pip_flags` | **Stays on proxy for live session**, but **promoted into `LayoutState` when it should update the persisted baseline** (semantics defined in §5) |
| Live CDI download in progress | `LiveNodeProxy.cdi_data` | **Moves to `LayoutState`** as "newly captured for this node" — proxy no longer owns it persistently |
| Live config tree built from current bus values | `LiveNodeProxy.config_tree` | **Moves to `LayoutState`**. Proxy reads from `LayoutState` if asked for "current cached tree." |
| Frontend drafts | `configChangesStore`, `bowtieMetadataStore`, `facilitiesStore`, `channelsStore`, `connectorSelectionsStore`, `offlineChangesStore` | Unchanged (frontend), but **mirrored into `LayoutState` as a drafts layer** before save |

The right mental model: today there are roughly four backend places where layout-derived data lives, and the save flow reads from the *last and least authoritative* of them (the proxy). The new model: one place, fed by the file IO module, mutated by save / read-config / delta application, read by everything else.

## Proposed architecture

### One module: `bowties-core::layout::state` (or `layout_state` sibling — name TBD)

A new submodule of `bowties-core/src/layout/`. Owns a single struct, conceptually:

```rust
pub struct LayoutState {
    /// Folder path of the open layout on disk.
    root: PathBuf,
    /// Last-known on-disk manifest.
    manifest: LayoutManifest,
    /// Saved layer — the bytes most recently written to disk for each node.
    /// One entry per node persisted in the layout roster.
    saved: HashMap<NodeKey, SavedNode>,
    /// Bowties / channels / facilities / offline-changes documents.
    /// Same single-owner principle as `saved`.
    bowties: BowtiesDocument,
    channels: ChannelsDocument,
    facilities: FacilitiesDocument,
    offline_changes: Vec<OfflineChange>,
    /// "Newly-captured" data not yet persisted: result of a fresh
    /// CDI download / config read for a node not yet in `saved`,
    /// or a re-read that produced new values.
    captured: HashMap<NodeKey, CapturedNode>,
    /// Drafts mirrored from the frontend (or applied via deltas).
    /// Cleared on successful save.
    drafts: DraftLayer,
}

pub struct SavedNode {
    pub snapshot: NodeSnapshot,    // already exists
    pub cdi_xml: Option<String>,   // None for `not_supported` nodes; else the loaded XML
    pub tree: NodeConfigTree,      // built from snapshot + CDI at load time
}

pub struct CapturedNode {
    pub snip: Option<SNIPData>,
    pub pip_flags: Option<ProtocolFlags>,
    pub cdi_xml: Option<String>,
    pub cdi_parsed: Option<lcc_rs::cdi::Cdi>,
    pub config_values: HashMap<String, [u8; 8]>,
    pub tree: Option<NodeConfigTree>,
}
```

These struct shapes are sketches — the implementation session refines them.

### Public surface (intent-shaped)

The surface is small and follows the same flavor as ADR-0005's `read_capture` / `save_capture`:

**Loading:**
- `LayoutState::open(folder, app_data_dir, profiles) -> Result<Self>` — read everything from disk into the populated struct. Replaces the inline logic in `open_layout_directory` that today scatters into `offline_bowtie_data` / `saved_trees` / `active_layout`.
- `LayoutState::close(self)` — drop in-memory state, signal proxies to disconnect-from-layout-scope.

**Queries (read paths):**
- `state.persisted_node_keys() -> impl Iterator<&NodeKey>`
- `state.cdi_xml(&NodeKey) -> Option<&str>` — returns saved-or-captured XML; this is what feeds save snapshot building.
- `state.config_tree(&NodeKey) -> Option<&NodeConfigTree>` — saved + captured + drafts merged.
- `state.config_value(&NodeKey, path) -> Option<[u8;8]>` — same.
- `state.is_persistable_in_layout(&NodeKey) -> bool` — backend mirror of the frontend `effectiveNodeStore` predicate.
- `state.snapshot_for_save(&NodeKey) -> NodeSnapshot` — builds the snapshot to write, **never returns fingerprint=missing** because the data either is in `saved` (use it), is in `captured` (use it), or we explicitly skip the node because it has no data anywhere (and we emit a warning the user sees).

**Mutations (write paths, in priority order of who calls them):**
- `state.record_captured(node_key, captured: CapturedNode)` — called by `commands/cdi.rs` when a CDI download completes, when a config read completes, and when SNIP/PIP queries resolve.
- `state.apply_layout_deltas(deltas: Vec<LayoutEditDelta>) -> Result<()>` — replaces today's standalone `apply_layout_deltas` + `apply_facility_deltas` calls inside `save_layout_directory`.
- `state.merge_drafts(drafts: DraftLayer)` — receives frontend draft snapshot as part of the save IPC, populates the drafts layer.
- `state.save(write_data_callback)` — writes the merged result to disk via the existing `layout/save_capture` infrastructure, then transitions `captured` + `drafts` entries that were persisted into `saved`, clears the rest from the drafts layer per save semantics.

**Lifecycle:**
- `state.note_node_present_on_bus(node_key, alias)` — informational; tracks which nodes are currently answering. Does not change persisted data.
- `state.note_node_off_bus(node_key)` — same.

### What `NodeProxy` keeps

Stays:
- `node_id` / `node_key`
- `alias` (current live alias)
- `transport_handle` + `our_alias`
- In-flight SNIP / PIP query state (the actor's mailbox and the spawned tasks they coordinate)
- Connection status + last-verified timestamp
- Event subscription state

Removed (or kept as transient session-only fields that intentionally don't survive layout close):
- `snip: Option<SNIPData>` — moved to `LayoutState` once a query returns
- `pip_flags: Option<ProtocolFlags>` — same
- `cdi_data: Option<CdiData>` — same
- `cdi_parsed: Option<lcc_rs::cdi::Cdi>` — same
- `config_values: HashMap<String, [u8;8]>` — same
- `config_tree: Option<NodeConfigTree>` — same

Methods like `GetCdiData` / `GetConfigTree` on the proxy mailbox either delegate to `LayoutState` (if the proxy is for a node in the open layout) or are removed entirely (callers should ask `LayoutState`, not the proxy).

**Why the proxy shrinks but doesn't disappear:** the actor model still earns its keep for live bus operations — concurrent SNIP/PIP queries, datagram exchanges, write_modified_values, event listening. Those are inherently per-node async work that needs to serialize against the bus. `NodeProxy` is the right home for that. It's the *data ownership* that's wrong, not the existence of the actor.

## Data flow rewrites

### Open a layout

```
user picks folder
  → open_layout_directory IPC
    → LayoutState::open(folder)
        loads manifest, bowties, channels, facilities, offline-changes
        for each saved node:
          loads snapshot YAML, resolves CDI XML, parses CDI, builds tree,
          applies profile annotations
          stores in `saved[node_key]`
    → state.layout = Some(LayoutState)
    → frontend hydrates from the IPC response (unchanged)
```

Today's three parallel populations (`saved_trees`, `offline_bowtie_data`, `active_layout`) collapse into one call.

### Go online (connect, discover nodes)

```
user connects
  → for each lcc-node-discovered event:
       register_node IPC
         → node_registry.get_or_create(node_id, alias)
             spawns LiveNodeProxy with bus session state only
             does NOT seed cdi/tree/values — that lives in LayoutState
         → if state.layout.is_some() and the layout knows this node:
              state.layout.note_node_present_on_bus(node_key, alias)
```

The proxy is empty by design now — it's a session handle, not a data cache. `LayoutState` already has the data for any node previously in the layout.

### Read Configuration on a node

```
user clicks "Read Configuration" for node X
  → read_all_config_values IPC
    → LiveNodeProxy executes the bus reads
    → CDI is downloaded (if not already in LayoutState.saved[X].cdi_xml)
    → config values are read from the bus
    → results are returned to the command handler
    → state.layout.record_captured(X, CapturedNode { cdi_xml, tree, values, ... })
    → frontend updates its tree store as usual
```

The captured layer in `LayoutState` is what feeds the next save. The proxy may briefly hold the in-flight CDI download buffer but doesn't persist it.

### Save

```
user clicks Save
  → frontend builds the draft layer (configChanges + metadata + channels + facilities + connector selections + offline drafts)
  → save_layout_with_bus_writes IPC
    → state.layout.merge_drafts(drafts_from_frontend)
    → state.layout.apply_layout_deltas(deltas)   # AddNode / RemoveNode / facility / channel / bowtie / role
    → for online: write_modified_values bus writes via proxies (Phase 2)
    → state.layout.save() writes the merged result through layout/save_capture
        snapshot_for_save(node_key) is called per persisted node
        — no `.retain(fingerprint != "missing")` is needed, because
          we already know whether data exists at the point of building
          the snapshot; a node with no saved-and-no-captured data is
          not in `persisted_node_keys()` to begin with
    → returns persisted_node_ids, warnings, etc.
```

`.retain(fingerprint != "missing")` disappears. The defensive filter exists today only because the data source is unreliable (the proxy). With `LayoutState` as the source, the filter has nothing to filter — by construction.

### Disconnect

```
user disconnects
  → registry shuts down LiveNodeProxy actors
  → LayoutState is unchanged
  → frontend stays in the same layout, just sees "off-bus" status for each node
```

This already mostly works (the layout file stays open across disconnects). The new model makes the property explicit: the layout doesn't need the bus to exist.

### Close layout

```
user closes layout
  → registry.clear_layout_scope() (placeholders dropped)
  → LayoutState drops; everything in memory for that layout is gone
```

## Public IPC surface (frontend ↔ backend) — minimal impact

The IPC contracts that frontend sees mostly do not change. Some examples:

- `save_layout_with_bus_writes(path, deltas)` → unchanged signature; internally backed by `LayoutState`.
- `open_layout_directory(path)` → unchanged signature; internally constructs `LayoutState`.
- `read_all_config_values(node_id, ...)` → unchanged result shape; side effect changes from "writes into proxy" to "writes into `LayoutState.captured`".
- `get_node_tree(node_id)` → reads from `LayoutState` if the node is in the open layout; otherwise asks the proxy / returns empty.
- New optional command (if helpful): `get_layout_state_summary` for diagnostics.

ADR-0002 is preserved — backend still owns layout file data, frontend still sends deltas — but with an explicit in-memory owner behind the IPC instead of an implicit scatter.

## Migration plan — three slices, each leaves the app fully working

### Slice 1 — Introduce `LayoutState` and parallel-populate; no caller switches yet

- Create `bowties-core/src/layout/state.rs` (or sibling module) with the struct + public API.
- `open_layout_directory` constructs and stores `LayoutState` on `AppState`, **in parallel with** the existing population of `offline_bowtie_data`, `saved_trees`, `active_layout`. Nothing reads from it yet.
- All existing call sites unchanged.
- Tests: a focused integration test that opens a layout and asserts `LayoutState` contents match the union of today's three caches.

End-state: code compiles, no behavior change, `LayoutState` exists and is correct.

### Slice 2 — Switch the save path to read from `LayoutState`

- `save_layout_directory` stops walking `node_registry.get_all_handles()` and stops calling `.retain(fingerprint != "missing")`. Instead it asks `LayoutState` for snapshots per persisted node.
- `record_captured` is wired into the relevant command paths (CDI download completion, config read completion, SNIP/PIP query resolution).
- The frontend draft mirror is sent as part of the save IPC; `merge_drafts` is wired in.
- Tests: the R1 scenario as an integration test — open, connect, edit, save, reopen, assert all nodes still present. The R2 scenario — open, connect, read all configs, save, assert all nodes including Tower-LCC persisted.

End-state: the bug class is structurally eliminated. `.retain` and the silent drop are gone.

### Slice 3 — Shrink `NodeProxy`

- Remove `snip`, `pip_flags`, `cdi_data`, `cdi_parsed`, `config_values`, `config_tree` from `LiveNodeProxy`. Remove the corresponding `ProxyMessage` variants. Callers that asked the proxy for these now ask `LayoutState`.
- `node_registry.saved_trees` is removed (its job is done by `LayoutState`).
- `AppState.offline_bowtie_data` is removed; consumers (offline catalog rebuild) read from `LayoutState`.
- Tests: confirm no caller still asks the proxy for persistent data; confirm the offline catalog rebuild still works against `LayoutState`.

End-state: `NodeProxy` is a clean bus-IO actor; persistent data has one owner.

## Tests

Significant test surface that becomes possible (and didn't exist before):

- **`LayoutState` round-trip:** open a layout, mutate nothing, save, assert byte-for-byte identical files on disk. Catches "save drops data" regressions in their entire family.
- **Open → connect → save → reopen invariance:** open layout with N nodes, simulate going online (without reading any config), save (with no edits), reopen. Must still have N nodes on disk. This is the test that catches R1.
- **Read-configures-only-the-node-you-asked-for:** read config on node X; `LayoutState.captured` has only X; other nodes are unchanged.
- **Save merges drafts:** apply a draft edit to one leaf of one node; save; assert only that leaf's value changed on disk, all other YAML is byte-identical.
- **Save with explicit `RemoveNode` delta:** the only way to reduce the persisted node count. Verified by an invariant test that asserts `persisted_node_keys.len() >= previous_persisted_node_keys.len() - removed_node_keys.len()`.
- **Disconnect preserves layout:** open, connect, disconnect; `LayoutState` is unchanged.
- **Concurrent draft and save:** a draft arriving mid-save is either fully included or fully excluded — no torn writes.

These tests can be largely Rust-side integration tests against a fake transport and a fake disk (`tempfile`-based). They don't require a running Tauri app.

## ADR / documentation impact

When this lands, the following docs change:

- **New ADR:** "Backend `LayoutState` owns the three-layer in-memory layout model" — accepts this proposal, supersedes parts of ADR-0009.
- **ADR-0009 amendment:** narrow the scope of `NodeProxy` to bus-session state only. Mark the persistent-data fields as removed. The polymorphic `NodeProxyHandle` (Live vs Synthesized) survives — that's still the right shape for bus session handles vs synthesized handles for placeholders.
- **ADR-0005 amendment:** the `layout/` module gains a stateful surface (`LayoutState`) alongside the existing intent-shaped file functions. Both are still owned by `layout/`.
- **ADR-0011 reflection:** the backend now mirrors the frontend's three-layer projection. The two facades have different shapes (backend is for persistence/IO, frontend is for rendering) but the layering is symmetric.
- **`aiwiki/owners.md`:** new entry under bowties-core for `LayoutState`. Update Backend section to reflect the proxy slimming.
- **`aiwiki/seams.md`:** the "what's the source of truth for node CDI/values" seam gets a clean Owner (`LayoutState`) and a small Contributor/Consumer list.
- **Frontend ADR-0004 / ADR-0011 docs:** unchanged — the frontend facade keeps doing what it does. Note in `aiwiki/architecture-health.md` that the backend now has a symmetric structure.

## Risks and watch-outs

- **Concurrency.** `LayoutState` is shared mutable state. It needs the same `RwLock` discipline as `AppState`'s existing caches. Save must take a consistent snapshot of `saved + captured + drafts` even while CDI downloads complete concurrently. Pattern: `record_captured` takes a write lock briefly; `save` takes a write lock for the whole save (the existing save is already serialized via the active_layout lock — same pattern applies).
- **Memory.** Large CDI XMLs (Tower-LCC ~25-30 KB; modulino ~9 KB; LT-50 ~30 KB) and per-node config trees live in memory for the whole open-layout duration. For typical layouts (5-20 nodes), this is well under 1 MB total. Not a concern; flagged for awareness.
- **Captured-vs-saved promotion semantics.** When a captured node is included in a save (via AddNode delta or as part of a re-save of an already-saved node), `LayoutState.save()` must move it from `captured` into `saved`. Tested in slice 2.
- **Live SNIP/PIP refresh paths.** Today, SNIP/PIP query handlers update the proxy. In the new model, they update `LayoutState` (and possibly the proxy's transient cache of "what we last heard from the bus" for status display). The implementation session must decide whether SNIP changes from the live bus *update* the persisted baseline silently or surface as drift. Suggested default: update the saved layer (SNIP is identity, not configuration; the user expects it to match the bus).
- **Test infrastructure.** `LayoutState` needs a way to be constructed from in-memory data for tests (without going through the file IO). The implementing session should add a `LayoutState::for_test(...)` constructor or builder pattern.
- **The `[BUG-INVEST]` instrumentation already added on 2026-06-28** stays in tree during implementation as verification scaffolding, then gets removed in the final slice. The backlog entry tracking its removal is already in `specs/backlog.md`.

## Out of scope (deliberately not addressed)

- **R3** (no-CDI node still offers Read Configuration). Different root cause, didn't reproduce. Separate investigation.
- **The deeper R2 question** (why Tower-LCC's CDI doesn't end up in proxy.cdi_data after a successful read). This proposal makes it not a data-loss issue; if it still matters for live-state correctness after `LayoutState` lands, separate investigation.
- **Channel/facility persistence atomicity** (the post-orchestrator IPCs for channel CRUD that violate ADR-0002's atomic-save promise). Adjacent but separate; can be folded into a follow-on slice if appetite is there, or stay as its own backlog entry.
- **Layout file format changes.** This proposal does not change the on-disk format. `LayoutState::save()` writes the same YAML the current code writes.
- **Sync / drift detection between disk and bus.** The user mentioned this as a future goal: read live bus values, compare against saved values, let user reconcile. `LayoutState` makes this future much easier (live reads land in `captured`, comparison against `saved` is straightforward) — but the UX and the sync semantics are a separate spec.

## Open questions for the implementation session

1. **Module name and location.** `bowties-core::layout::state` (submodule)? Or top-level `bowties-core::layout_state`? Or rename `bowties-core::layout` to `bowties-core::layout::io` and let `LayoutState` own the top-level surface? Lean: submodule, name TBD by the implementer.
2. **Should `LayoutState` itself be the thing on `AppState`, or stay accessed via `AppState.layout: Option<LayoutState>`?** Lean: the latter — keeps option semantics for "no layout open" clean.
3. **`record_captured` granularity.** One call per node, or one call per data kind (CDI / SNIP / values)? Lean: per node, with a partial `CapturedNode` struct (Options for each field) — keeps call sites simple and avoids transient inconsistent states.
4. **Frontend draft mirroring.** Does the frontend send the full draft layer as part of the save IPC, or do we maintain a backend mirror that's kept in sync incrementally? Today's pattern is "send everything as part of save deltas." Lean: keep that pattern.
5. **Live PIP/SNIP changes.** Do they update `LayoutState.saved` directly, or land in `captured` until the next save? Decision will surface during slice 2 implementation.

## Where to start (for the implementing session)

1. Read this proposal end-to-end.
2. Re-read ADR-0005, ADR-0009, ADR-0011.
3. Skim:
   - `bowties-core/src/layout/mod.rs` (current public surface)
   - `bowties-core/src/layout/capture.rs` (`build_node_snapshot` + `ProxySnapshotData`)
   - `bowties-core/src/node_registry.rs` (`saved_trees`, `get_or_create`)
   - `bowties-core/src/node_proxy.rs` (the field set we're shrinking)
   - `app/src-tauri/src/state.rs` (`offline_bowtie_data`, `active_layout`)
   - `app/src-tauri/src/commands/layout_capture.rs` (`open_layout_directory`, `save_layout_directory`, `save_layout_with_bus_writes`)
4. Open a new branch off `main` (or off `018-block-indicator-facility` if 018 is finishing) named `layout-state-deep-module` (or similar).
5. Start with slice 1: introduce the struct + parallel population + a single round-trip test. Land it. Move on to slice 2.
6. After slice 2 lands, the `[BUG-INVEST]` instrumentation can be removed; the integration tests are the durable replacement.
7. After slice 3, write the new ADR + amend ADR-0009 + update aiwiki.
