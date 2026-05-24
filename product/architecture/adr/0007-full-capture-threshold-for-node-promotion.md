# ADR-0007: Full-capture threshold for promoting discovered nodes into a layout

Status: Accepted
Date: 2026-05-24

## Context

A Bowties layout is a durable per-layout roster of nodes (S8). When the
user is connected to a bus, the app discovers nodes; some of those nodes
belong to the active layout (they will be added on the next save) and
some are noise (e.g. nodes on a foreign bus the user accidentally
connected to). The save flow must decide which discovered nodes to
promote into the saved roster and which to keep purely in-memory.

The first S8 implementation promoted **every** discovered node not
already in the saved roster. Two defects followed:

1. **Wrong-bus contamination.** A user opens layout *A* and connects to
   bus *B* by accident. Every node on bus *B* lights up as an "unsaved
   change". A reflexive Save permanently writes bus *B*'s roster into
   layout *A*'s file.
2. **Useless stub snapshots.** A freshly discovered node has not yet
   had its CDI fetched or its config values read. Promoting it produces
   a snapshot the backend later filters out (no CDI fingerprint, no
   values to render offline) — and even if it landed on disk, the user
   could not meaningfully edit it offline.

The hotfix added a per-route `discoveredOnlyNodeIds.length > 0` OR to
every dirty-signal call site, but that was a syntactic patch — the
semantic question (when is a discovered node ready to be saved?) was
never answered.

## Decision

A discovered node is considered an **unsaved in-memory addition** only
when it has been **fully captured**:

```
fullyCaptured(nodeId) ≡
  nodeTreeStore.trees.has(nodeId) ∧ ¬ partialCaptureNodes.has(nodeId)
```

That is: the CDI is cached AND every config value has been read. This
condition is computed in `+page.svelte` and pushed into the layout
store via `layoutStore.setUnsavedInMemoryNodeIds(ids)`. The store's
`isDirty` property becomes:

```
isDirty ≡ _hasFileEdits ∨ _unsavedInMemoryNodeIds.length > 0
```

The "unsaved-new" sidebar badge keeps its existing predicate
(`discovered ∧ ¬ inLayoutNodeIds`) — the badge is purely informational,
not a save-readiness signal. The save orchestrator receives only the
threshold-gated list as `AddNode` delta seeds.

## Consequences

Positive:

- A user who accidentally connects to a foreign bus does not see Save
  light up for foreign nodes — none of them are fully captured at
  connect time.
- The save command never writes "missing" or "not_supported" CDI
  snapshots: the threshold filter pairs naturally with the backend's
  fingerprint filter (which would have dropped them anyway, producing a
  silent persistence defect — see S8-T13).
- A single `layoutStore.isDirty` semantic now drives both the Save
  button gate and the unsaved-changes guard on every exit path
  (close-layout, switch-layout, disconnect, app-window-close); no
  consumer has to remember a separate "plus discovered nodes" rule.

Negative / trade-offs:

- The user cannot promote a discovered node into the layout without
  first reading its config. This is intentional — promotion implies
  offline editability, which requires captured values — and is
  surfaced via the restored empty-state "Read all" affordance
  (S8-T16) that runs CDI + read-all across every not-yet-captured
  node in one click.
- Consumers reading `layoutStore.isDirty` see a broader meaning than
  the historical "LayoutFile struct changed". This is the intended
  shift: the property now means "in-memory changes not yet saved",
  mirroring the field-level edit layer model.

## Alternatives considered

- **No threshold (original S8 behavior).** Rejected: caused defect 1
  (wrong-bus contamination) and defect 2 (stub snapshots).
- **Threshold = CDI cached only (no read-all).** Rejected: the resulting
  snapshot has the CDI fingerprint but no captured values, so the user
  still cannot edit the node offline. The save would succeed but offline
  editing would be broken until the next online read.
- **Per-node "promote" toggle in the sidebar.** Rejected as
  over-engineered for the current user base: every workflow we have
  evidence for ends with "save the discovered nodes I just read"; an
  explicit toggle adds a step without solving a real ambiguity.

## Related

- ADR-0002: Backend owns layout file data.
- ADR-0005: Layout module owns file structure.
- ADR-0006: In-place journaled writes.
- specs/013-save-flow-reorder/slices.md — S8 design refinement
  (Session 2026-05-24).
