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
