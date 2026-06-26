# NodeKey is a sum type, not a string

Status: accepted (supersedes ADR-0008)
Date: 2026-05-31

## Context

ADR-0008 introduced `NodeKey` as a string convention with two shapes — a
canonical 12-hex live `NodeID` or `placeholder:<uuid>` — and asked every
seam to honour it. Spec 014's regression triage (see
`specs/014-config-modes-placeholders/regression-fix-plan.md`) showed that
honour-system contract is not sustainable: the registry stored canonical
12-hex keys, but the frontend (and `DiscoveredNode` serialization) routinely
passed the dotted display form, so `get_by_node_key` missed and
`get_node_tree` silently fell through to a "build fresh from CDI" path
that returned trees with `None` event-id values. Four user-visible
regressions all trace back to that single lookup miss.

An initial partial fix (Phase 1A/1D — normalize inside the registry,
emit canonical form on discovery events, normalize at `loadTree`) was
attempted. It moved the bug rather than fixing it: the canonical-form
switch on discovery events broke the frontend roster's identity
comparisons, which still operate on the dotted display form. The bug
class is structural — the compiler cannot help while the type is `String`.

## Decision

`NodeKey` becomes a real sum type owned by the **backend application
layer** (not by `lcc-rs`):

```rust
// app/src-tauri/src/node_key.rs
pub enum NodeKey {
    Live(lcc_rs::NodeID),    // serialises as canonical 12-hex
    Placeholder(uuid::Uuid), // serialises as "placeholder:<uuid>"
}
```

The frontend mirrors it as a branded discriminated union with a single
factory:

```ts
// app/src/lib/utils/nodeKey.ts
export type NodeKey =
  | { readonly kind: 'live'; readonly id: string /* canonical 12-hex */ }
  | { readonly kind: 'placeholder'; readonly id: string /* uuid */ };
```

The wire contract is unchanged: live nodes serialise as canonical 12-hex,
placeholders as `placeholder:<uuid>`. Existing layout files and IPC
payloads remain valid. The migration is type-tightening only.

`lcc-rs` is **not** changed. The protocol library only knows `NodeID`;
placeholders are an application concept. Per
`product/architecture/code-placement-and-ownership.md`, pushing
`NodeKey` into `lcc-rs` would be a layering violation.

## Considered alternatives

- **Sprinkle normalization at every seam.** Rejected — that is what
  Phase 1A/1D tried and what failed. The bug class is the missing type.
- **Push `NodeKey` into `lcc-rs`.** Rejected — placeholders are a
  Bowties application concept; `lcc-rs` stays pure.
- **Two-type split (`NodeKey` live-only + `RosterKey` live‑or‑placeholder).**
  Rejected — forces conversions at every store boundary and replicates
  the stringly-typed friction one layer up. A single sum type with one
  factory surface is the deeper module.
- **Extract into a `bowties-domain` crate.** Deferred — no second
  consumer exists. Revisit if a CLI tool or alternate frontend appears.

## Consequences

- Identity has one constructor surface, one serializer, and one equality
  rule on each side of the IPC boundary. The compiler surfaces every site
  that needs to change during the migration.
- `normalize_node_key` (backend) and `normalizeNodeKey` / `PLACEHOLDER_PREFIX`
  (frontend) become unreachable and are deleted.
- `bowties.rs`'s catalog builder gets its first behavioral test as part of
  the migration (Step 4c of the regression plan), narrowing the long-
  standing 0-test gap flagged in `aiwiki/architecture-health.md`.
- ADR-0008's "stringly-typed convention" framing is superseded; the
  rest of ADR-0008 (placeholder seam, single editor pipeline, unified
  `nodeModeSelections`) still holds.

## 2026-06-06 extension: bowties-core crate extraction

The "extract into a `bowties-domain` crate" alternative has been un-deferred
and executed — under the name `bowties-core` (not `bowties-domain`, since
"core" better describes the app's business-logic layer).

**Trigger.** The Windows `cargo test` DLL issue (`STATUS_ENTRYPOINT_NOT_FOUND
0xc0000139`, caused by WebView2Loader linkage) made it impossible to run
any backend tests. 237 inline tests existed but had never executed. A pure-
Rust crate with no `tauri` dependency sidesteps the DLL problem entirely.

**What moved.** Every domain module whose code has zero `tauri` imports:
`node_key`, `node_tree`, `node_proxy`, `node_registry`, `layout/*`,
`profile/` (types + resolver + annotation logic). Modules that depend on
`tauri::AppHandle` (`placeholder`, `profile/loader`) remain in `src-tauri`
and can be trait-injected in a follow-up.

**Pattern.** `bowties-core` sits beside `lcc-rs` as a sibling path-dep crate.
`src-tauri` depends on both and re-exports bowties-core modules through thin
shim files so existing `crate::node_tree` paths compile without churn.

**NodeRoles.** The `NodeRoles` struct (producer/consumer sets per event)
moved from `state.rs` to `bowties_core::node_tree` — it is pure data with
no Tauri coupling and is the only state.rs type that domain modules reference.

**Test fixes discovered.** Four `node_key` tests and two `node_registry` tests
had incorrect 14-char NodeID expectations (should be 12-char). These were
never caught because the tests had never run. Fixed as part of the extraction.

## 2026-06-07 extension: Phases 3–4 extraction (snapshot builder + sync domain logic)

Continued the bowties-core extraction with two more batches of pure domain
logic, completing the four-phase plan.

**Phase 3 — Snapshot builder** (`bowties_core::layout::capture`). The
tree-walking logic that populates a `NodeSnapshot` from a config tree —
`collect_leaf_values`, `group_key`, and the core `build_node_snapshot`
algorithm — moved from `commands/layout_capture.rs` into a new
`bowties-core/src/layout/capture.rs`. A `ProxySnapshotData` input struct
decouples the builder from `NodeProxyHandle` and `AppState`. The src-tauri
command handler became a thin adapter: `proxy_snapshot_data()` fetches from
the proxy, delegates to the pure builder, relays log messages via `bwlog!`.
8 unit tests cover placeholder CdiRef, complete/partial/missing captures,
SNIP fallback, and producer-event propagation.

**Phase 4 — Sync domain logic** (`bowties_core::sync`). Three submodules:

- `sync/changes.rs` — `same_change_target`, `remove_changes_by_id` (5 tests).
- `sync/field_meta.rs` — CDI field metadata resolution (`find_field_meta_in_cdi`,
  `walk_elements_for_meta`), value conversion (`raw_bytes_to_value_string`,
  `string_to_config_value`), synthetic leaf construction (`field_meta_to_leaf`),
  snapshot label helpers (`find_snapshot_field_label`, `fallback_field_label`,
  `resolve_snapshot_node_name`) (10 tests).
- `sync/classifier.rs` — layout match scoring (`compute_layout_match`), sync
  row classification (`classify_sync_row` returning an enum instead of inline
  `if/else`), and IPC types (`SyncSession`, `SyncRow`, `ApplySyncResult`,
  `ApplySyncFailure`, `LayoutMatchStatus`) (7 tests).

`commands/sync_panel.rs` shrank from 1,337 to 959 lines. It now imports and
re-exports bowties-core types, keeping only AppState coordination, bus I/O,
and CDI path resolution (which depend on `tauri::AppHandle`).

**Net result.** bowties-core grew from 287 to 310 tests (+23). The four
extraction phases together brought the previously-untestable backend from
0 runnable tests to 310 passing tests across domain modules.

## 2026-06-25 extension: canonical form for persisted node references

Any field that stores a node reference in a persisted file (YAML, JSON) or
in an in-memory domain struct MUST use the canonical NodeKey wire form
(`020157000002D9` for live, `placeholder:<uuid>` for placeholders) — never
the dotted display form (`02.01.57.00.02.D9`).

Where the branded `NodeKey` type cannot be used directly (e.g., serde struct
fields that must remain `String` for schema compatibility), the code that
**populates** the field MUST normalize at the boundary via
`normalizeNodeId()` (frontend) or the equivalent backend normalization.
Consumers SHOULD still normalize when comparing, as a safety net for legacy
data written before this rule was codified.

**Trigger.** Spec 015 S5: `HardwareReference.node_key` (a raw `String` field)
was populated with `document.nodeId` in dotted format. When the sidebar
emitted a connector-selection-change event with the canonical form, the
`===` comparison silently failed — no confirmation dialog appeared and
channel removal never executed. The bug persisted across sessions because
the dotted value was written to `channels.yaml`.

**Rule.** Treat any `String`-typed field whose semantic role is "identifies a
node" as a deferred migration site for the branded `NodeKey` type. Until
that migration reaches the field, normalize at the write site — do not rely
on downstream normalization alone.

## 2026-06-26 extension: canonical contiguous hex for event IDs

The same canonical-vs-display split that applies to Node IDs now applies to
Event IDs. The two identifier types follow the same convention:

| Identifier | Canonical (storage / comparison / IPC) | Display (UI labels, tooltips) |
|------------|----------------------------------------|-------------------------------|
| Node ID    | 12-char contiguous: `020157000​2D9`     | Dotted: `02.01.57.00.02.D9`  |
| Event ID   | 16-char contiguous: `0201570002D90100` | Dotted: `02.01.57.00.02.D9.01.00` |

**Canonical form.** `ConfigValue::EventId.hex`, IPC payloads, map keys, and
any field whose semantic role is "identifies an event" MUST store the
16-character contiguous uppercase hex form. Parsing functions MUST accept
both dotted and contiguous input and normalize to contiguous on output.

**Display form.** UI components that show event IDs to users MUST convert to
dotted hex at the display boundary using `displayEventIdHex()` (frontend) or
`bytes_to_display_hex()` (backend). The dotted form matches the OpenLCB
convention users see in manuals and other tools.

**API surface.** `lcc-rs::EventID` now mirrors `NodeID` with both
`to_hex_string()` (dotted, display) and `to_canonical()` (contiguous,
storage). `bowties-core::node_tree` exports `parse_event_id_hex()` (accepts
both formats → bytes), `normalize_event_id_hex()` (any format → canonical
string), and `bytes_to_display_hex()` (bytes → dotted display).

**Trigger.** Spec 016 S1: the PCER event handler (`handle_pcer` in
`router.rs`) formatted event IDs as contiguous hex, while the config tree
resolution path (`bytes_to_dotted_hex` in `node_tree.rs`) produced dotted
hex. `deriveChannelState()` performed direct string equality between the
two, so occupancy indicators never left the "unknown" state despite events
arriving on the bus. The same bug class as the Node ID canonical-form
mismatch in the 2026-06-25 extension.

**Backward compatibility.** Existing layout files and snapshots that contain
dotted event ID strings remain valid — all parsers accept both formats and
normalize to contiguous on load. No data migration is required.

## 2026-06-26 extension: shared HexId helpers (DRY follow-up)

The canonical-form decision above produced three near-identical hex
formatting implementations in Rust and several inline parsers in TypeScript.
A subsequent DRY pass consolidated them.

**Rust (`lcc-rs/src/types.rs`).** Module-private generic helpers own the
rule once, parameterised by byte count:

- `format_canonical_hex<const N: usize>(&[u8; N]) -> String` — uppercase contiguous
- `format_dotted_hex<const N: usize>(&[u8; N]) -> String` — uppercase, `.`-separated
- `parse_hex_id<const N: usize>(&str) -> Result<[u8; N], String>` — strips `.`/`-`/space, validates length

`NodeID` and `EventID` remain as distinct newtype structs (so signatures
keep their semantic distinction), but their `to_canonical`, `to_hex_string`,
and `from_hex_string` methods now delegate to the generic helpers. Adding
a third ID width (e.g. a future 12-byte UUID-like ID) is a one-line
addition, not a re-implementation.

**Rust app/backend.** `bowties-core::node_tree`'s
`bytes_to_canonical_hex`, `bytes_to_display_hex`, `parse_event_id_hex`,
and `normalize_event_id_hex` are kept as named entry points but delegate
to `lcc_rs::EventID`. Inline `format!("{:02X}", b)` event-id sites in
`bowties-core::sync::field_meta` and `bowties-core::placeholder` were
replaced with calls to `lcc_rs::EventID::{to_canonical, to_hex_string}`.

**TypeScript (`app/src/lib/utils/hexId.ts`).** Mirror of the Rust helpers:

- `formatCanonicalHex(bytes)` / `formatDottedHex(bytes)` / `parseHexId(input, expectedBytes)`

`serialize.ts` (`parseEventIdHex`, `canonicalEventIdHex`, `formatEventIdHex`,
`normalizeEventIdHex`) and `nodeId.ts` (`formatNodeId`, `nodeIdToDisplayHex`,
`nodeIdStringToBytes`) became thin wrappers. The duplicate `formatEventId`
in `formatters.ts` was removed; its single caller now uses `formatEventIdHex`.
Inline parsers in `editKey.ts::parseOfflineValueString`,
`treeConfigValuePersistence.ts::parseOfflineStoredValueForLeaf`,
`offlineLayoutOrchestrator.ts::parseOfflineValue`,
`bowties.svelte.ts::eventIdHexToBytes`, and
`eventIds.ts::generateFreshEventIdForNode` now route through the shared
helpers.

**Rule going forward.** Do not write a new `bytes.map(b => b.toString(16)…)`
or `format!("{:02X}", b)` pair at any call site. Either reuse an existing
named wrapper or add a new one in the appropriate domain module that
delegates to `hexId.ts` / `lcc_rs::types`.
