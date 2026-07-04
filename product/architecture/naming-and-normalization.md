# Naming And Normalization

## Purpose

This document specifies the canonical rules for Node ID normalization and node display-name resolution in Bowties. These rules must be applied consistently across all frontend and backend code paths that compare, display, or store node identifiers.

Inconsistent application of these rules has been a direct source of regressions (mixed dotted/canonical forms not matching, display names showing raw IDs instead of human names). Whenever a new path needs to compare or display a node, it must use these shared helpers rather than local ad-hoc logic.

---

## Node ID Normalization

### Canonical Form

The canonical Node ID form used for all comparisons is: **uppercase hex, no separators**.

Example: `05.02.01.02.00.00.00.FF` normalizes to `050201020000FF`.

### Why Two Forms Exist

LCC Node IDs appear in two forms across the codebase:

- **Dotted hex** — `05.02.01.02.00.00.00.FF` — used for display and serialization in layout files, SNIP, and CDI output.
- **Canonical hex** — `050201020000FF` — used for all comparisons, map keys, and identity checks.

The backend may produce either form depending on source (SNIP data vs. node tree IDs). The frontend must normalize before comparing.

### Normalization Helper

All normalization must use `normalizeNodeId()` from `app/src/lib/utils/nodeId.ts`:

```ts
import { normalizeNodeId } from '$lib/utils/nodeId';
// "05.02.01.02.00.00.00.ff" → "050201020000FF"
const key = normalizeNodeId(nodeId);
```

**Implementation:**

```ts
export function normalizeNodeId(nodeId?: string): string {
  return (nodeId ?? '').replace(/\./g, '').toUpperCase();
}
```

### Where To Apply Normalization

| Operation | Rule |
|---|---|
| Map key for node lookup | Always normalize before inserting or looking up |
| Comparing two Node IDs for equality | Always compare normalized forms |
| Storing a Node ID in a store or backend struct | Store in whichever form the API expects; normalize before compare |
| Displaying a Node ID to the user | Use dotted-hex display form (see `formatNodeId()`) |
| Reading a Node ID from SNIP or layout YAML | Normalize immediately before using as a key |
| Writing a Node ID to layout YAML or snapshot | Use the dotted-hex form from the source; do not normalize the stored value |

### Formatting Helper

For display purposes only, convert a 6-byte array to dotted hex:

```ts
import { formatNodeId } from '$lib/utils/nodeId';
// [5, 2, 1, 2, 0, 0, 0, 255] → "05.02.01.02.00.00.FF"
const display = formatNodeId(bytes);
```

For round-trip conversion from a string to bytes:

```ts
import { nodeIdStringToBytes } from '$lib/utils/nodeId';
const bytes = nodeIdStringToBytes('05.02.01.02.00.00.00.FF');
```

---

## Node Display Name Resolution

### Fallback Order

The display name shown for a node in the UI follows this priority order:

1. **User Name (edit layer)** — the effective value of the editable ACDI User Name leaf (memory space 251), resolved through the draft → offline → baseline waterfall. An offline rename is reflected here before save.
2. **User Name (SNIP)** — `snip_data.user_name` (trimmed, non-empty)
3. **Manufacturer + Model** — `snip_data.manufacturer` + `snip_data.model`, formatted as `"Manufacturer — Model"` (both trimmed, non-empty)
4. **Model only** — `snip_data.model` (trimmed, non-empty) when manufacturer is absent
5. **Node ID** — raw Node ID string, as the final fallback when no SNIP data is available or all SNIP name fields are empty

Tier 1 is the editable equivalent of the SNIP user name and takes priority so a pending offline rename updates the UI immediately (ADR-0003 point 4). Tiers 2–5 are the SNIP-only fallback chain implemented by `resolveNodeDisplayName()`.

### Display Name Helpers

The SNIP-only fallback (tiers 2–5) is `resolveNodeDisplayName()` from `app/src/lib/utils/nodeDisplayName.ts`:

```ts
import { resolveNodeDisplayName } from '$lib/utils/nodeDisplayName';

const label = resolveNodeDisplayName(nodeId, node);
// Returns: SNIP user name, or "Manufacturer — Model", or model, or nodeId
```

**Signature:**

```ts
function resolveNodeDisplayName(
  nodeId: string,
  node: Pick<DiscoveredNode, 'snip_data'> | null | undefined
): string
```

The edit-layer tier (tier 1) is `resolveEffectiveUserName()` from the same module — pure and store-free; the leaf-value resolver is injected:

```ts
function resolveEffectiveUserName(
  tree: NodeConfigTree | null | undefined,
  resolveValue: (leaf: LeafConfigNode) => TreeConfigValue | null,
): string | null  // trimmed name, or null when no User Name leaf/edit
```

Node-name surfaces compose the two: `resolveEffectiveUserName(...) ?? resolveNodeDisplayName(...)`. This composition is encapsulated in `resolveNodeName(nodeId)` from `$lib/layout`, which is the **canonical single entry point** for all surfaces that display a node name.

**Do not** call `resolveNodeDisplayName` directly from components or orchestrators — it misses the edit layer. Import `resolveNodeName` from `$lib/layout` instead.

**Do not** read `snip_data.user_name` directly for display — it misses the edit layer and the manufacturer/model fallback.

**Do not** implement ad-hoc fallback chains locally in components or stores. If the fallback order changes, it must change in one place.

### When SNIP Data Is Not Yet Available

Before SNIP enrichment completes during discovery, the node has no `snip_data`. `resolveNodeDisplayName()` returns the raw Node ID string in that case. This is expected behavior: names update reactively once SNIP data arrives.

### Resolve Late, On The Consuming Side

Display names must be resolved from the live node info at frontend
derivation/render time — never consumed from a value that was resolved earlier
upstream and shipped along. The backend bowtie catalog computes a `node_name`
for each `EventSlotEntry` once at catalog-build time; if SNIP had not arrived
yet, that value is the raw Node ID. Treat such a pre-baked `node_name` as a
best-effort initial value only. The bowtie preview derivation
(`enrichEntryLabel` in `bowties.svelte.ts`) re-resolves `node_name` through
`resolveNodeDisplayName()` against the live `nodeInfoStore`, so cards update to
the Display Name once SNIP arrives — matching the config sidebar. Trusting a
pre-baked `node_name` directly breaks the "names update reactively" guarantee
above.

---

## Where These Rules Apply

These normalization and display-name rules apply to:

- All store lookups by Node ID (`nodeTreeStore`, `nodeInfo`, `configReadStatus`, `offlineChangesStore`, `syncPanel.svelte.ts`)
- All offline-change row matching (offline change `nodeId` field compared to discovered node IDs)
- Sync session node-match classification (matching offline layout nodes to live discovered nodes)
- Post-apply tree reconciliation (identifying which trees need rebuilding)
- Any component that displays a node name in the sidebar, catalog panel, or sync panel

---

## Anti-Patterns To Avoid

| Anti-pattern | Why it causes bugs |
|---|---|
| `nodeId.replace(/\./g, '')` inline (without uppercase) | Mixed-case IDs do not match canonical form |
| `nodeId.toUpperCase()` without removing dots | Dotted form does not match canonical form |
| Local fallback: `node.snip_data?.user_name \|\| nodeId` | Misses manufacturer+model fallback; diverges if display rules change |
| Storing canonical keys in YAML | Downstream tools and the backend expect dotted hex in layout files |
| Comparing IDs directly without normalizing | Case or separator differences cause silent lookup failures |

---

## Regression Tests

The normalization and display-name helpers are covered by unit tests. Any change to these helpers must include test coverage for:

- Empty and undefined input
- Already-canonical input (no dots, uppercase)
- Dotted-lowercase input
- Mixed-case dotted input
- SNIP data with user name
- SNIP data with only manufacturer+model
- SNIP data with only model
- Missing or null SNIP data

---

## Event ID Normalization

Event IDs (8 bytes / 16 hex chars) follow the same identity-vs-display rule
as Node IDs, and — because they are used as map keys in many places — carry
a compile-time-enforced identity type.

### Canonical Form

**Uppercase hex, no separators.** Example: `02.01.57.00.02.D9.04.D2`
normalizes to `0201570002D904D2`.

### Two Forms

- **Dotted display** — `02.01.57.00.02.D9.04.D2` — used only for UI display.
  Produced by `lcc_rs::EventID::to_hex_string()` and the frontend
  `formatEventIdKey(key)` / `displayEventIdHex(hex)` helpers.
- **Canonical**      — `0201570002D904D2` — the identity form. Used as map
  keys, set membership, layout YAML keys under `bowties:`, and the wire
  contract for `BowtieCard.event_id_hex`. Produced by
  `lcc_rs::EventID::to_canonical()` and `canonicalEventIdHex(bytes)`.

### `EventIdKey` — the identity type

Frontend code that uses an event ID as identity must use the branded
`EventIdKey` type from `app/src/lib/utils/eventIdKey.ts`:

```ts
import { toEventIdKey, eventIdKeyFromBytes, formatEventIdKey, type EventIdKey } from '$lib/utils/eventIdKey';

// At the IPC / layout-YAML / user-input boundary:
const key: EventIdKey | null = toEventIdKey(rawHex);   // accepts dotted or canonical
const key2: EventIdKey       = eventIdKeyFromBytes(bytes);

// Internal:  identity comparisons, map keys, set membership.
// Display:   render dotted form for humans.
const display: string = formatEventIdKey(key);
```

The phantom brand prevents raw `string` values from being assigned to
`EventIdKey` — the compiler forces every incoming string to go through
`toEventIdKey()`. This is the same discipline as `NodeKey` (ADR-0010),
applied to event IDs.

### Where To Apply

| Operation | Rule |
|---|---|
| Map key or set membership for an event ID | Type as `EventIdKey`; normalize incoming strings via `toEventIdKey` |
| Comparing two event IDs for equality | Compare `EventIdKey` values, or normalize both sides |
| Storing an event ID in a store (e.g. `bowtieMetadataStore` edit prefix) | Canonical form |
| Layout YAML `bowties:` map keys | Canonical form (writer emits canonical; loader normalizes legacy dotted keys) |
| `BowtieCard.event_id_hex` from the backend | Canonical form — see `build_bowtie_catalog` |
| Displaying an event ID to the user | `formatEventIdKey(key)` (dotted) |
| Backend `[u8; 8]` bytes | Already canonical; do not need a string form for identity |

### Backwards Compatibility

Layout YAML files written before the canonical migration have dotted keys
under `bowties:`. The load path (`merge_layout_metadata`,
`buildEffectiveBowtiePreview`) normalizes on read, and the next save writes
canonical form — a silent one-time migration per file.

### Future Extension

The `EventIdKey` type is intentionally a **single-variant branded string**
today (real 8-byte event IDs only). When placeholder proxies get wired up
(ADR-0009 follow-up) and the layout needs to reference "the event slot at
path X of placeholder Y" before the proxy is applied to a real node, we
may widen this to a sum type mirroring `NodeKey` (`Live | Placeholder`).
Keeping identity behind the branded type today means the TypeScript
compiler will enumerate every seam that needs updating when that variant
lands.

### Anti-Patterns

| Anti-pattern | Why it causes bugs |
|---|---|
| `hex.replace(/\./g, '').toUpperCase()` inline | Duplicates `toEventIdKey`; misses invalid input handling |
| Comparing `card.event_id_hex` to `treeConfigValue.hex` as raw strings | Historical duplication bug — dotted vs canonical mismatch produced two preview cards per event ID |
| Writing dotted-form keys into new layout YAML files | Legacy shape; canonical is authoritative going forward |
| Using `event_id_hex` string as a map key without going through `toEventIdKey` | Bypasses the compile-time guard; regressions creep in silently |

---

## Sources

- `app/src/lib/utils/nodeId.ts`
- `app/src/lib/utils/nodeDisplayName.ts`
- `app/src/lib/utils/eventIdKey.ts`
- `bowties-core/src/bowtie/catalog.rs` (`build_bowtie_catalog`, `merge_layout_metadata`)
- `bowties-core/src/node_tree.rs` (`bytes_to_canonical_hex`, `normalize_event_id_hex`)
- ADR-0010 (NodeKey sum type — the precedent this discipline extends)
- `specs/010-offline-layout-editing/refactoring-roadmap.md` (Track A, A2 NodeID Normalization Boundary)
