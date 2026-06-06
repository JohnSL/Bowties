# ADR-0008: Unified node-key for real and placeholder boards

Status: Superseded by ADR-0010
Date: 2026-05-24

> ADR-0010 replaces the string-shaped `NodeKey` defined here with a true
> sum type. The unifying *intent* (one identifier for real and placeholder
> nodes through the editor pipeline) is preserved; the *encoding* is
> different. Read ADR-0010 for the current contract.

## Context

Spec 014 introduces **placeholder boards** — layout entries that represent
an instance of a bundled board model with no associated real LCC node.
Placeholders must run through the same guided-configuration editor as a
real node: same CDI tree, same `configChanges` store, same relevance and
event-role annotation, same save-to-layout path. The only behaviors that
differ are (a) the CDI source (bundled XML instead of a live-node fetch)
and (b) that placeholder eventids must never be offered as a binding
source or target anywhere in the app.

The editor pipeline today is keyed by `NodeID` everywhere — stores
(`nodeTree`, `configChanges`, `configEditor`), backend (`node_tree`,
`node_proxy`, `node_registry`), and routes. A naive read of the spec
suggested two pipelines (one for real nodes, one for placeholders), but
that doubles the surface area and creates two homes for every workflow
that already exists once.

## Decision

A single **`NodeKey`** string identifies the node addressed by every
editor-pipeline call, where:

```
NodeKey ::= NodeID                              # canonical LCC node ID, e.g. "05.01.01.01.FF.00.00.01"
         |  "placeholder:" <uuidv4>             # layout-scoped placeholder, e.g. "placeholder:7c9e6b1a-..."
```

The `placeholder:` prefix is the load-bearing seam:

- **Bindable?** `is_placeholder(node_key) ≡ node_key.starts_with("placeholder:")`.
  Every binding-enumeration command in `commands/bowties.rs` gates on this
  one predicate. There is no per-leaf `is_placeholder` annotation in the
  rendered tree — placeholderness is a property of the node.
- **CDI source.** A placeholder NodeKey routes to `load_bundled_cdi`
  instead of the live-node CDI fetch path; the rest of the tree-build is
  unchanged.
- **Stores.** `nodeTree`, `configChanges`, `configEditor`, and
  `effectiveLayoutStore` accept any `NodeKey`. They do not branch on
  kind.
- **`isPlaceholderEventId` is unchanged.** It continues to mean
  "the all-zeros sentinel = unassigned event"; it is orthogonal to the
  placeholder-board concept and was not renamed.

The two duplicate "selected variant per node" maps that would otherwise
have appeared (`connector_selections` on `LayoutFile` for real Tower-LCC
nodes; `modeSelections` under each placeholder) are collapsed into one
top-level `LayoutFile.nodeModeSelections: BTreeMap<NodeKey,
BTreeMap<ModeId, VariantId>>`. Daughterboards have not shipped, so
`LayoutFile.schemaVersion` is bumped to `"2.0"` with no migration code:
older layouts are rejected at load with a clear message.

## Consequences

- Adding placeholder boards requires no new editor pipeline; it
  required only widening every `NodeID` parameter to `NodeKey` and
  routing CDI-fetch to the bundled-XML loader when the prefix matches.
- Deleting a placeholder must also clear its entry in
  `nodeModeSelections` (single extra line in the delta handler).
- A future "reconcile placeholder with discovered node" feature replaces
  the `placeholder:<uuid>` key with the real `NodeID` in
  `placeholderBoards` and `nodeModeSelections` simultaneously; no other
  store or command needs to know.
