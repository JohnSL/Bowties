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
