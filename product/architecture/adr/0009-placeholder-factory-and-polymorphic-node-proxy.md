# ADR-0009: Placeholder factory and polymorphic node proxy

Status: Accepted
Date: 2026-05-25
Accepted: 2026-05-28

## Context

ADR-0008 introduced `NodeKey` as a unified identifier for real and
placeholder nodes.  That resolved the naming problem but left an
architectural asymmetry in the **in-memory state layer**: real nodes
and placeholders take entirely different paths from creation through
save.

Real nodes flow through a single mechanism:

1. Bus discovery → `register_node` inserts a `NodeProxyHandle` into the
   backend Proxy Registry (keyed by `NodeID`).
2. SNIP/PIP queries + `read_all_config_values` populate the proxy.
3. At save time, `layout_capture` reads the proxy and writes a
   `NodeSnapshot` to disk.

Placeholders bypass that mechanism entirely:

1. A frontend store (`inMemoryPlaceholdersStore`) holds the placeholder
   identity and `profile_stem`.
2. A dedicated IPC (`build_placeholder_tree`) builds a CDI tree from a
   bundled profile — a separate code path from `get_node_tree`.
3. At save time, a `LayoutEditDelta::AddPlaceholderBoard` delta carries
   the full config tree, because the backend has no in-memory state to
   read.  The save command has a placeholder-specific arm that
   synthesizes a `NodeSnapshot` from the delta payload.
4. `replace_offline_changes` validates its parameter as a 12-hex
   `NodeID` via `NodeID::from_hex_string`.  Placeholder keys (UUID-
   shaped) crash this validation — the root cause of the S8.5 bug
   ("Save failed: Invalid NodeID hex string length: 44").

The interim fix (S8.5) made `stageDraftsForOfflineSave` skip placeholder
drafts so save wouldn't crash, but placeholder field edits silently
never persist.

This two-mechanism design forced every downstream module to ask "is this
a placeholder?" and branch: two staging filters, two cleanup methods,
two CDI-tree assembly paths, two delta variants for "add a node," and
`isPlaceholderKey` string-prefix checks scattered across 17+ call sites.

The insight is: **adding a placeholder and discovering a real node are
structurally the same operation** — populate an in-memory state holder
with identity, CDI, SNIP, and config.  The only difference is the data
source: the factory synthesizes what the bus would have read.

## Decision

### 1. Polymorphic `NodeProxyHandle`

The existing `NodeProxyHandle` (which today wraps only live-bus proxies)
becomes an enum:

```rust
enum NodeProxyHandle {
    Live(LiveNodeProxyHandle),
    Synthesized(SynthesizedNodeProxy),
}
```

`Live` wraps the existing CAN-connected proxy (renamed from
`NodeProxy` to `LiveNodeProxy`).  `Synthesized` is a passive holder of
factory-produced state: `node_key`, empty SNIP, bundled `CdiReference`,
pre-populated config (with all-zero EventId leaves), and
`profile_stem`.

Both variants expose the same method set (`node_key()`, `node_id()`,
`snip()`, `cdi_ref()`, `config_tree()`, `producer_identified_events()`).
Every read path (`get_node_tree`, save-time snapshot builder, etc.)
dispatches through these methods and does not know which variant it got.

**Why enum, not trait?** Two known, closed variants.  Exhaustive matching
catches missing cases at compile time; no dynamic dispatch overhead.

### 2. Registry generalization

The Proxy Registry generalizes from `HashMap<NodeID, NodeProxyHandle>`
to `HashMap<NodeKey, NodeProxyHandle>`.  This completes the ADR-0008
`NodeKey` migration into the last `NodeID`-keyed map in the backend.
Callers with a `NodeID` convert to `NodeKey` at the boundary.

### 3. Placeholder factory module

A new top-level backend module (`app/src-tauri/src/placeholder.rs`)
owns all placeholder construction logic:

- Mints `placeholder:<uuid>` node keys.
- Resolves bundled CDI from a `profile_stem`.
- Walks the CDI to find every EventId leaf and pre-populates `[0u8; 8]`
  (all-zero, matching the existing `is_placeholder_event_id` zero-prefix
  convention from the spec research).
- Produces a fully-valid `SynthesizedNodeProxy` and inserts it into the
  registry.

The factory is to "Add Placeholder" what bus discovery is to "Node
Appeared."  No other module knows the placeholder construction
conventions.

**Why top-level, not inside `layout/`?** The factory consumes profile/CDI
knowledge and produces a registry entry — neither is layout-layer logic.
Placing it inside `layout/` would re-couple the layers.

### 4. Layout layer becomes placeholder-agnostic

- `LayoutEditDelta::AddPlaceholderBoard` is deleted.
  `AddNode { node_id_hex: String }` generalizes to
  `AddNode { node_key: String }`.  One variant for "add a node."
- The save flow has one arm: for each `AddNode` delta, look up the
  `NodeProxyHandle` in the registry, build a `NodeSnapshot`, write to
  disk.  No species-branching.
- `replace_offline_changes` accepts `node_key: String` instead of
  validating as 12-hex `NodeID`.  This is the root-cause fix for the
  S8.5 crash.
- `NodeSnapshot` gains `profile_stem: Option<String>` (bundled CDI
  stem, `None` for real nodes) and
  `lifecycle: NodeSnapshotLifecycle` (`InMemory | Persisted`,
  skip-serialized — on disk it's tautologically `Persisted`).
  `validate()` enforces the typed invariant
  (`node_id: None` ⇒ `profile_stem: Some`) instead of sniffing the
  key prefix.

### 5. Frontend mirrors the unified backend

- `inMemoryPlaceholdersStore` is deleted.  The backend registry is the
  truth.
- `configChangesStore.commitForSave()` replaces
  `clearNonPlaceholderDrafts`.
- `stageDraftsForOfflineSave` has zero `isPlaceholderKey` filters.
- `saveLayoutOrchestrator` takes one `inMemorySnapshotKeys: Set<NodeKey>`
  input instead of separate `discoveredOnlyNodeIds` +
  `unsavedPlaceholders`.

### 6. UX gates use typed predicates

Frontend "is this a placeholder?" questions route through a typed
predicate (`snapshot.node_id === null` / `entry.isPlaceholder`) instead
of `isPlaceholderKey` string-prefix checks.  `isPlaceholderKey`
survives only where it's a legitimate encoding or transport concern:

- `configDraftOrchestrator.ts:38` — `flushDraftToBackend` transport
  skip (can't talk to something not on the bus).
- `utils/nodeRoster.ts` — canonicalization passthrough (dotted-hex
  normalization doesn't apply to UUID-shaped keys).
- `nodeRoster.svelte.ts` — internal partition into typed views.
- Backend: factory (minting), `filename_basis_for_key` (colon-escaping).

## Alternatives considered

### A. Parallel snapshot map on `ActiveLayoutContext`

Add `in_memory_snapshots: BTreeMap<NodeKey, NodeSnapshot>` to the
existing layout context.  The factory inserts there; the save flow reads
from there.

**Rejected.** This is the same "two parallel registries" problem in a new
shape.  Every lookup site must now check two maps; the "which map does
this node live in?" question reappears.  The existing Proxy Registry
already serves the in-memory-state-holder role — generalize it rather
than duplicating it.

### B. Snapshot-in-delta

`AddNode { snapshot: NodeSnapshot }` — the delta carries the complete
snapshot for both real and placeholder nodes.

**Rejected.** This breaks the intuition that a delta is a small mutation
record.  It creates two ways for a snapshot to land in the backend:
via delta payload and via proxy-to-snapshot conversion at save time.
Identity-only deltas are sufficient; the save flow reads state from the
registry.

### C. Frontend-only factory

The factory runs as a thin Rust function or even purely in TypeScript.
Frontend stores the synthesized state.

**Rejected.** This keeps the backend proxy registry ignorant of
placeholders, re-creating the asymmetry.  The backend is the authority
on node state; the frontend is a read-through view.

### D. Trait object (`Box<dyn NodeProxy>`)

`NodeProxyHandle` wraps a trait object instead of an enum.

**Rejected.** Two known, closed variants.  Enum gives exhaustive matching
and zero dynamic-dispatch overhead.  Traits are better for open sets of
implementors.

## Consequences

- **One mechanism for "a node the user is interacting with":** the
  Proxy Registry, populated by two sources (bus discovery, factory).
- **One save path, one staging path, one cleanup method:** no
  placeholder-specific arms anywhere in the layout, draft, or save
  flows.
- **`isPlaceholderKey` call-site count drops** from 17+ to ≤6, each in
  a documented encoding, transport, or minting role.
- **Factory is the single owner** of placeholder conventions (UUID
  minting, bundled CDI resolution, zero-EventId synthesis).  Adding a
  new board model requires only adding a bundled CDI + profile; no save
  or delta code changes.
- **The layout layer is node-agnostic.**  It persists snapshots and
  replays deltas without knowing what kind of node produced them.
- **Renames:** `NodeProxy` → `LiveNodeProxy`.  This is a one-time
  migration touching internal references only; the external-facing
  `NodeProxyHandle` name is unchanged.
