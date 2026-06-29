# ADR-0015: Backend `LayoutState` owns the three-layer in-memory layout model

Status: accepted (extends ADR-0005, narrows ADR-0009, mirrors ADR-0011 onto the backend)
Date: 2026-06-28

## Context

ADR-0002 made the backend the sole owner of layout *file* data: the frontend sends
deltas, the backend persists. ADR-0005 made the `layout/` module the sole owner of
the on-disk file *structure*. ADR-0009 unified real and synthesized nodes behind
`NodeProxyHandle`.

What none of those addressed was **where in the backend the layout's persistent
in-memory state lives while a layout is open**. The shape that grew up incrementally
was three parallel caches, each with a partial view:

1. `LiveNodeProxy::cdi_data` / `cdi_parsed` — per-actor CDI bytes + parse, populated
   only by an explicit CDI download or file-cache hit during the current session.
2. `node_registry.saved_trees` — annotated config trees loaded at layout open,
   consumed once when a fresh proxy spawns for a rediscovered node.
3. `AppState.offline_bowtie_data` — CDI XML + config values + profile roles,
   accumulated by `build_offline_node_tree` for the offline catalog rebuild.

None of the three was the authoritative source for the save flow, which instead
walked per-node proxies via `proxy_snapshot_data` and built a `NodeSnapshot` from
whatever happened to be in the proxy's fields *at save time*. The natural state of
a `LiveNodeProxy` immediately after reconnect is "empty" — SNIP/PIP not yet
requeried, CDI not redownloaded, config_tree not yet read. Save therefore observed
`cdi_xml_len: None` for every saved node whose proxy hadn't been forced to redownload,
and the snapshot builder wrote `cdi_ref.fingerprint = "missing"`. A later filter in
`save_layout_directory` discarded every snapshot with `fingerprint == "missing"`,
silently deleting `nodes/<key>.yaml` and `cdi/<key>.xml` for those nodes.

Two regressions reproduced this on 2026-06-28 at byte level:

- **R1**: Open a saved layout (5 nodes on disk) → go online → edit one field → Save.
  4 of 5 nodes deleted from disk (their `nodes/<key>.yaml` and `cdi/<key>.xml` gone).
- **R2**: Tower-LCC silently dropped on every save, regardless of session phase,
  because its proxy's `cdi_data` was `None` even after a successful config read.

The defensive `.retain(fingerprint != "missing")` filter existed *only because* the
data source was unreliable. The architectural problem was not the filter, the
fingerprint, or any individual proxy — it was that **no single module owned the
persistent in-memory projection of the open layout**.

## Decision

The `layout/` module (ADR-0005) gains a stateful sibling type, `LayoutState`, that
is **the sole in-memory owner of one open layout's three-layer projection**:

- **`saved`** — mirrors what is on disk; one `SavedNode` per persisted node carrying
  the snapshot YAML round-trip shape, resolved CDI XML, and the profile-annotated
  config tree built at open time.
- **`captured`** — partial-data buffer for live reads that have completed but have
  not yet been persisted (a fresh CDI download, a SNIP/PIP query result, a
  completed config read).
- **`drafts`** — frontend-side draft edits mirrored into the backend at save time
  (the existing collect-deltas-at-save pattern from ADR-0012; the struct is sketched
  but not yet populated by callers).

`LayoutState` is owned by `AppState::layout_state: Arc<RwLock<Option<LayoutState>>>`.
`Option` keeps "no layout open" cleanly representable. The struct lives in
`bowties-core/src/layout/state.rs` so the `layout/` module remains the sole owner
of layout-shaped knowledge (ADR-0005 invariant preserved).

### Public surface

The surface is intent-shaped, in the same flavour as ADR-0005's file functions:

**Construction / lifecycle:**
- `LayoutState::from_loaded(root, LayoutDirectoryReadData, cdi_xml_by_key, trees_by_key)` — built once in `open_layout_directory` from the data the file-IO module already loaded.

**Queries (the save flow and the offline catalog read here):**
- `persisted_node_keys() -> impl Iterator<&NodeKey>` — every node currently persisted in the layout.
- `saved_node(&NodeKey) / captured_node(&NodeKey) -> Option<&...>`
- `cdi_xml(&NodeKey) -> Option<&str>` — resolved CDI XML, **preferring captured over saved**.
- `config_tree(&NodeKey) -> Option<&NodeConfigTree>` — resolved tree, same precedence.
- `snapshot_for_save(&NodeKey) -> Option<NodeSnapshot>` — builder input.

**Mutations:**
- `record_captured(NodeKey, CapturedNode)` — called by `cdi.rs` at the two CDI-download success seams (network download + file-cache hit). Merges field-by-field; a SNIP-only call followed by a CDI-only call leaves both fields populated.
- `merge_drafts(DraftLayer)` — placeholder for the frontend-draft-mirror path; the full draft surface will fill in as the layout layer absorbs channel/facility CRUD inside the atomic save.

The slice-1 captured-vs-saved precedence is **the** semantic the save path relies
on: a freshly-downloaded CDI XML lands in `captured`, and the next save's
`cdi_xml(key)` reads that XML (not the stale one in `saved`) so the snapshot's
fingerprint reflects the new bytes.

### What `NodeProxy` keeps

ADR-0009's polymorphic `NodeProxyHandle` enum survives — that's still the right
shape for "a node the user is interacting with." But the `Live` variant shrinks:
`LiveNodeProxy` no longer holds **persistent** CDI bytes. Specifically removed in
slice 3a (2026-06-28):

- `cdi_data: Option<CdiData>` — and the `GetCdiData` / `SetCdiData` `ProxyMessage`
  variants and accessor methods.
- `cdi_parsed: Option<lcc_rs::cdi::Cdi>` — and the `GetCdiParsed` / `SetCdiParsed`
  variants and accessor methods.

`LiveNodeProxy::snapshot()` now always emits `cdi: None`. `NodeProxyHandle::get_cdi_data`
/ `get_cdi_parsed` return `Ok(None)` for the `Live` variant; the `Synthesized` variant
keeps its `cdi_data` field (placeholder CDI is the proxy's truth — placeholders have
no `LayoutState` entry until a save promotes them).

**Retained on `LiveNodeProxy`** (deliberately, not duplication): `snip`, `pip_flags`,
`config_values`, `config_tree`. These are working buffers for in-progress bus
operations — partial-read accumulators, `set_modified_value` pending writes, SNIP/PIP
query dedup state. Moving them into `LayoutState` would introduce a new concurrency
surface for in-flight reads versus save snapshots, with no offsetting benefit (the
save flow does not consult them; they cannot drift into a data-loss regression
because the save path's source is `LayoutState`). See ADR-0009's 2026-06-28
extension for the narrowed scope.

### What the existing parallel caches become

- **`AppState::OfflineBowtieData`** — deleted. The offline catalog rebuild in
  `commands/bowties.rs` derives its projections (config_values, profile_roles,
  synthetic `DiscoveredNode` list for slot walking) directly from
  `LayoutState.saved` + `LayoutState.cdi_xml(key)`.
- **`node_registry.saved_trees`** — retained as a **load-once seeding cache** for
  freshly-spawned proxies. Populated by the same loop in `open_layout_directory`
  that feeds `LayoutState`, so the two cannot drift; consumed exactly once when
  `get_or_create` spawns a `LiveNodeProxy` for a rediscovered node. **Not** a
  parallel source of truth for the save flow — `LayoutState` is.

### Read paths through `LayoutState`

- `commands/layout_capture.rs::proxy_snapshot_data` falls back to
  `LayoutState::cdi_xml` when the proxy lacks CDI; this is the structural cure for
  R1/R2. The save-path `.retain(fingerprint != "missing")` filter is kept as a
  defence-in-depth log site for the truly-no-data case, but is no longer the
  primary failure mode.
- `commands/cdi.rs::get_cdi_xml` consults `LayoutState.cdi_xml(key)` before any
  disk-based fallback (replacing the prior "scan active_layout's companion
  directory" path).
- `commands/bowties.rs::build_bowtie_catalog_command` derives every offline
  projection from `state.layout_state`.

## Considered options

### A. Keep the scattered caches; fix the `.retain` filter

Make the save filter authoritatively decide which nodes to keep based on
fingerprint + saved-on-disk presence rather than dropping silently.

**Rejected.** A symptom patch. The architectural problem is that the save flow's
data source is unreliable; tightening the filter just moves the failure mode (e.g.,
a future regression that writes a wrong fingerprint, instead of a missing one).

### B. `BTreeMap<NodeKey, NodeSnapshot>` on `ActiveLayoutContext`

Add a per-layout snapshot map alongside the existing context.

**Rejected.** This is the same shape ADR-0009 rejected for placeholders, in a new
clothing: every save-flow lookup site would still ask "is this node in the parallel
map?" and the question "which cache has the truth?" reappears. The whole point of
the consolidation is that there is no such question.

### C. Move all of `LiveNodeProxy`'s working state into `LayoutState` too

Empty out `LiveNodeProxy` to a pure bus-IO actor (no `config_tree`, `config_values`,
`snip`, `pip_flags`).

**Deferred.** These are working buffers for in-progress bus operations, not
duplicates of persistent state. Moving them would introduce a new concurrency
surface (LayoutState as the writeable target for in-flight CDI reads, partial
captures, and pending writes — torn-read risk between in-progress reads and
save snapshots) with no offsetting bug class prevented. The slice-3a removals
target *only* fields that the save flow could read out-of-sync. If a future driver
appears (concurrency bug, new feature requiring a writeable in-flight surface in
`LayoutState`), revisit then. See ADR-0009's 2026-06-28 extension.

### D. Frontend mirror only

Have the frontend hold the canonical in-memory layout state and the backend re-read
from disk at save time.

**Rejected.** Backend-owned layout data is the explicit ADR-0002 commitment. Moving
the source-of-truth to the frontend would invert the trust direction and force
disk-roundtrips on every save, and the bus-side actor still needs the data for
write-modified-values phase.

## Consequences

- **R1 / R2 are structurally impossible**, not merely filter-guarded. The save
  flow's data source is the canonical `LayoutState`, populated once at open time
  from disk and updated via `record_captured` as live reads complete. Verified by
  the behaviour pins `r1_every_persisted_node_resolves_cdi_xml_after_open` and
  `r2_captured_cdi_resolves_for_unsaved_node` in `bowties-core::layout::state`,
  plus the capture-layer fingerprint pins (`cdi_xml_len_some_produces_len_fingerprint_not_missing`,
  `cdi_xml_len_none_with_unknown_pip_falls_through_to_missing`) in
  `bowties-core::layout::capture`.
- **The backend now mirrors the frontend's three-layer projection** (ADR-0011's
  `saved` / `captured` / `drafts` shape). Different facades — backend is for
  persistence/IO, frontend is for rendering — but the layering is symmetric.
- **One read path per concern.** "Where is this node's CDI?" → `LayoutState.cdi_xml`.
  "What goes on disk for this node?" → `LayoutState.snapshot_for_save`. "What is
  the offline catalog made from?" → `LayoutState.saved`.
- **`NodeProxy` is more honest about its job.** Per-actor working buffers for live
  bus operations (which require serialization against the bus mailbox) remain on the
  actor. Persistent state lives in `LayoutState`. The "is this a duplicate cache of
  persistent data?" principle test is the criterion.
- **No on-disk format change.** This is purely an in-memory consolidation.
  `LayoutState::save()` writes the same YAML the prior code wrote, through the same
  ADR-0006-journaled writer.

## Migration trace

This ADR records a three-slice migration that landed across two sessions:

- **Slice 1 (2026-06-28)**: `LayoutState` introduced and parallel-populated alongside
  the existing scatter; no callers switched. 6 unit tests including the slice-1
  captured-vs-saved precedence contract.
- **Slice 2 (2026-06-28)**: `proxy_snapshot_data` falls back to `LayoutState::cdi_xml`
  when the proxy lacks CDI. `record_captured` wired at both CDI-download success
  seams. R1 and R2 verified fixed on real hardware (Tower-LCC + saved layout, byte
  level diff against `temp/Test 3` / `Test 3 - Copy`).
- **Slice 3a (2026-06-28)**: Duplicate caches deleted (`LiveNodeProxy::cdi_data` /
  `cdi_parsed` + their `ProxyMessage` variants; `AppState::OfflineBowtieData` +
  field). `commands/bowties.rs` offline path rewritten to read from `LayoutState`.
  `commands/cdi.rs::get_cdi_xml` consults `LayoutState` before disk fallbacks.
- **Slice 3b (deferred)**: Moving `LiveNodeProxy::config_tree` / `config_values` /
  `snip` / `pip_flags` into `LayoutState` is not adopted — see Option C above.

## Invariants

Structured testable rules for the `/design` audit. Each invariant resolves to
OK / Drift / Unknown with file:line evidence.

- `bowties_core::layout::state::LayoutState` is the sole in-memory owner of an open
  layout's persistent CDI XML, profile-annotated config trees, and saved bowtie /
  channels / facilities / offline-changes documents. No other module holds a
  duplicate cache of these for save-flow consumption. Audit: grep for
  `cdi_xml: HashMap` / `saved_trees:` / `config_values:` declarations across
  `bowties-core/` and `app/src-tauri/`; the only legitimate matches are
  `LayoutState`, `node_registry.saved_trees` (proxy-seeding cache; see below), and
  `SynthesizedNodeProxy` (placeholder-local CDI; see ADR-0009 amendment).
- `commands/layout_capture.rs::proxy_snapshot_data` falls back to
  `LayoutState::cdi_xml` when the proxy lacks in-memory CDI bytes. The
  `.retain(fingerprint != "missing")` filter in `save_layout_directory` is a
  defence-in-depth log site, not the structural protection. Audit: bowties-core
  pins `r1_every_persisted_node_resolves_cdi_xml_after_open` and
  `r2_captured_cdi_resolves_for_unsaved_node` exercise the fallback.
- `LayoutState::cdi_xml(key)` and `LayoutState::config_tree(key)` always prefer
  the `captured` layer over the `saved` layer. A fresh CDI download or config
  read recorded via `record_captured` is visible to the next save before any
  persistence step. Audit: `LayoutState` unit tests assert the precedence
  directly.
- `LiveNodeProxy::snapshot()` always emits `cdi: None`. `NodeProxyHandle::get_cdi_data`
  for the `Live` variant returns `Ok(None)`. CDI bytes for live nodes flow through
  `LayoutState`. Audit: grep `bowties-core/src/node_proxy.rs` for any new `cdi:`
  populating arm on `LiveNodeProxy`; grep for `set_cdi_data` / `set_cdi_parsed` —
  must be zero call sites against `LiveNodeProxy`.
- `node_registry.saved_trees` is a load-once seeding cache. Populated only by
  `open_layout_directory` from the same loop that feeds `LayoutState`; read only
  by `NodeRegistry::get_or_create` to seed a fresh `LiveNodeProxy::config_tree` at
  spawn time. Not consulted by `save_layout_directory` or any other save-path
  reader. Audit: grep for `saved_trees` outside `bowties-core/src/node_registry.rs`
  and `app/src-tauri/src/commands/layout_capture.rs::open_layout_directory`; any
  other reader is drift.
- `AppState::OfflineBowtieData` is gone. The offline branch of
  `commands/bowties.rs::build_bowtie_catalog_command` derives every projection
  (per-node config values, profile group roles, synthetic `DiscoveredNode` list)
  from `state.layout_state` directly. Audit: grep `offline_bowtie_data` /
  `OfflineBowtieData` — must return zero matches in production source.
