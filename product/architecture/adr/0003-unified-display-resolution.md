# Unified display resolution: backend catalog owns resolved baseline, frontend owns draft layer

## Context

ADR-0002 made the backend the sole owner of layout file data. Save commands accept deltas and return the persisted layout. This fixed data loss during save but did not address how **display values** (config field values, display names, role classifications) are resolved for rendering.

The frontend currently has 6+ independent resolution paths. Some use the full draft → offline pending → baseline waterfall (`configChangesStore.visibleValue()`); others read the tree baseline directly (`leaf.value.value`, `leaf.eventRole`). Online, the baseline is live hardware state so both paths agree. Offline, the baseline is the snapshot from the last CDI read — it doesn't reflect saved config changes or user role classifications. This causes names, values, and role tags to diverge between the bowtie cards (which use catalog-resolved data) and the config tree (which reads stale baseline).

The root cause is that there is no single resolution function that all display paths use, and the tree baseline is never updated from the backend catalog after offline saves.

## Decision

**The backend catalog owns the resolved baseline for all display values and role classifications. The frontend owns only the transient draft layer.**

1. **One resolution function for effective values.** All frontend display paths resolve config values through a single function that checks draft → offline pending → baseline. `configChangesStore.visibleValue()` already does this; the remaining call sites that read `leaf.value.value` directly must be wired through it.

2. **One resolution function for effective roles.** All frontend display paths resolve role classifications through a single function that checks pending role edits → catalog-resolved role → CDI baseline. Both bowtie cards and config tree leaf tags use this function.

3. **After catalog rebuild, the frontend baseline reflects resolved values.** When the backend rebuilds the catalog (after save, after CDI read), the catalog response carries resolved values. The frontend updates its baseline from the catalog so that call sites reading the baseline see post-save state, not the stale pre-edit snapshot.

4. **Display name resolution uses the effective-value path everywhere.** `getInstanceDisplayName()`, `buildElementLabel()`, and all group/picker label functions resolve through the value resolution function — never through raw `child.value.value`.

## Considered options

- **Pure backend resolution** — every display value request goes through IPC to the backend. Rejected: frontend config drafts are transient UI state that change on every keystroke. IPC round-trips for display updates during editing would add latency and require the backend to track unsaved drafts, violating ADR-0002's design (backend sees edits only at save time as deltas).

- **Patch each call site independently** — add resolver callbacks to each divergent component. Rejected: this is what S2a-T8 did for `getInstanceDisplayName`, but it doesn't prevent new call sites from reading baseline directly. A centralized resolution function with clear ownership makes the correct path the default path.

## Consequences

- A new resolution utility or store extension provides `resolveValue(nodeId, path)` and `resolveRole(nodeId, path)` as the canonical display-value API.
- Components never import `configChangesStore` for value resolution directly — they use the resolution API.
- The offline tree baseline updates after catalog rebuild, so the bottom layer of the resolution waterfall stays current.
- The backend catalog response may need to carry per-field resolved values (or the frontend updates its baseline by replaying the offline changes store against the snapshot — either approach satisfies the invariant).

## 2026-06-16 extension: node Display Name resolves the User Name edit layer

Point 4 ("display name resolution uses the effective-value path everywhere") was implemented for config-tree labels (`getInstanceDisplayName`, `buildElementLabel`, picker/group labels) but the **node-level Display Name** was the lone holdout: `resolveNodeDisplayName()` read only the SNIP snapshot (`snip_data.user_name`) and never consulted the edit layer. Editing the node's User Name offline therefore did not update the Display Name shown in the sidebar or on bowtie cards until save + re-read.

The editable node name is the ACDI User Name leaf in **memory space 251** (the editable equivalent of `snip_data.user_name`). Its offline edits land in `configChangesStore` as drafts.

Decision: node Display Name resolution now consults the edit layer first, consistent with point 4.

- `resolveEffectiveUserName(tree, resolveValue)` (in `app/src/lib/utils/nodeDisplayName.ts`) locates the User Name leaf (lowest-address `string` leaf in space 251) and resolves it through the draft → offline → baseline waterfall. Pure and store-free; the resolver is injected.
- Node-name surfaces resolve in this order: **effective User Name (edit layer) → SNIP Display Name fallback chain**. Encapsulated in `resolveNodeName(nodeId)` from `$lib/layout` — the canonical single entry point. All node-name surfaces (bowtie store, config sidebar presenter, Element Picker, config-read orchestrator, config-acquisition orchestrator) import and call this function.
- The SNIP-only `resolveNodeDisplayName(nodeId, node)` remains the pure final-tier fallback when no User Name leaf/edit exists. It is not imported directly by display surfaces — they use `resolveNodeName`.
- Consequence: `nodeInfo.updateNodeSnipField()` (a post-save write-through that pushed the edited name back into `snip_data`) is now obsolete — the edit layer surfaces the rename immediately — and was removed as dead code. Stuffing draft state into the SNIP snapshot was the rejected approach here; the snapshot stays a hardware-reported mirror.
- Dead inline fallbacks removed: `NodeList.svelte` `getFriendlyName()`, `getSecondaryInfo()`, `getDisplayName()` (duplicated the SNIP chain); `configAcquisitionOrchestrator` inline `snip_data.user_name || nodeId`.
- Sidebar detail/tooltip now receive the effective node name so subtitle and tooltip decisions respect edit-layer renames.

