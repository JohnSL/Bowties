# Layout facade: effective view store + save orchestrator behind one import surface

## Context

ADR-0002 made the backend the sole owner of layout file data. ADR-0003 unified value/role resolution at the **leaf** level via `resolveValue`/`resolveRole` helpers in `app/src/lib/utils/displayResolution.ts`. S2b wired six divergent call sites through those helpers.

After S2b, three bugs remained and all share one root cause:

1. **Bowtie diagram goes blank during save (and sometimes stays blank).** `EditableBowtiePreviewStore` in `bowties.svelte.ts` has fast/slow paths gated on `configChangesStore.hasDraftsForNode`. Drafts are not cleared after a persisted offline save, so the preview is stuck on the slow path while tree/catalog are mid-rebuild.

2. **ElementPicker shows "?" badges and no role filtering when offline.** `PickerTreeNode` reads `leaf.eventRole` directly for filtering and badges. Offline, the saved `roleClassifications` in `layoutStore.layout` never reach the tree, so `leaf.eventRole` is null. `resolveRole` exists but is only called in `handleSelect`, not in the filter/badge code.

3. **Deleting a bowtie leaves a stale card on screen until save.** Both `_buildPreviewFromCatalog` and `_buildPreviewWithTreeScanning` iterate the catalog without consulting `bowtieMetadataStore`'s pending `delete:${eventIdHex}` edits.

The pattern is the same as S2b's pre-fix state, one level up: each display surface re-derives an "effective view" from raw inputs (`bowtieCatalogStore`, `layoutStore`, `bowtieMetadataStore`, `configChangesStore`, `nodeTreeStore`) and each omits a different layer. Leaf-level helpers cannot prevent this because the divergence is at the bowtie-list and slot-filter levels, not just at the leaf.

A secondary problem: components import four different stores to read display data and a fifth module to perform writes. There is no single entry point that says "this is how the UI talks to layout state," so new components reach for whichever store they discover first.

## Decision

**Introduce a `$lib/layout` facade module as the only layout-state import surface for components.** Internally the facade composes two implementations that stay separate because they have opposite shapes:

1. **`effectiveLayoutStore`** — a Svelte 5 `$derived` read model that projects (`bowtieCatalogStore`, `layoutStore`, `bowtieMetadataStore`, `configChangesStore`, `nodeTreeStore`) into the single set of values the UI renders:
   - `effectiveBowties` — bowtie cards with pending deletions removed and pending entry edits merged.
   - `effectiveRole(nodeId, path)` — replaces `leaf.eventRole` for every display and filter site (subsumes `resolveRole`).
   - `effectiveValue(nodeId, path)` — subsumes `resolveValue` from ADR-0003.
   - `slotsByRole(nodeId, role)` — pre-filtered slot lists for the ElementPicker.
   - `isSlotFree(nodeId, path)` — replaces ad-hoc occupancy checks.

2. **`saveLayoutOrchestrator`** (existing, extended) — owns the multi-step save workflow and the state transitions that surround it. Specifically, it clears `configChangesStore` drafts that have been persisted and atomically swaps the catalog so the read model never observes an intermediate blank state.

The facade module re-exports the read API from `effectiveLayoutStore`, the write entry points from the orchestrator, and a small set of edit-recording commands (record a bowtie deletion, record a role classification, record a draft value) that delegate to the existing edit-layer stores. Components import only from `$lib/layout`.

The four edit-layer stores (`bowtieMetadataStore`, `configChangesStore`, `bowtieCatalogStore`, `layoutStore`) become **internal**. They keep their current responsibilities (recording edits, holding rebuilt catalogs, holding loaded layout files) but are not imported by routes or components.

## Considered options

- **Stay with helper functions (extend `displayResolution.ts`).** Add `resolveEffectiveBowties`, `resolveRoleFilter`, `isBowtiePendingDeletion` next to the existing helpers and wire every call site. Rejected: S2b was this approach and three new divergent sites still surfaced. Helpers do not prevent new code from reading raw stores; the structural guarantee is missing. The helper count will keep growing.

- **One combined `layoutStore` module owning both reads and writes.** Put the derived projection and the async save workflow in the same module. Rejected: reads (reactive, fine-grained, synchronous) and writes (coarse, async, transactional) have opposite shapes. A combined module becomes a god-object, forces awkward compromises between Svelte `$derived` and async orchestration, and violates the durable boundary in `product/architecture/code-placement-and-ownership.md` between stores and orchestrators.

- **Backend-authoritative effective view.** Move projection into Rust; frontend mirrors the result. Rejected: drafts-in-progress are inherently UI state. IPC round-trips for every keystroke or filter recompute would either be noisy or require debouncing that interferes with responsive editing. ADR-0002 deliberately keeps drafts on the frontend until save time.

## Consequences

- Components and routes import from `$lib/layout` only. The four edit-layer stores are not part of the public surface.
- The fast/slow path branch in `EditableBowtiePreviewStore` collapses into a single derivation in `effectiveLayoutStore`. `configChangesStore` drafts contribute through the same merge regardless of save state.
- `PickerTreeNode` filter and badge code calls `effectiveRole`/`slotsByRole` instead of reading `leaf.eventRole`. The "?" badge appears only when the effective role is genuinely unknown.
- `saveLayoutOrchestrator` clears persisted drafts on successful save completion. The read model never sees a window where catalog has been rebuilt but drafts still mask it.
- `resolveValue`/`resolveRole` from ADR-0003 become the **internals** of `effectiveLayoutStore`. They do not move; the facade exposes the higher-level API.
- Tests targeting `EditableBowtiePreviewStore`'s preview shape retarget to `effectiveLayoutStore`. Tests for the orchestrator's transition rules stay where they are.
- The aiwiki `owners.md` Stores section gains a "Layout facade" subsection that names the public surface and lists the internal stores it composes.

## Invariants

Structured testable rules for the `/design` audit. Each invariant resolves to OK / Drift / Unknown with file:line evidence.

- `$lib/layout` is the only layout-state import surface for components and routes. The four edit-layer stores (`bowtieMetadataStore`, `configChangesStore`, `bowtieCatalogStore`, `layoutStore`) are not imported by files under `app/src/lib/components/**` or `app/src/routes/**`. Audit: grep imports of those store paths from those directories.
- All display sites for bowtie cards, role filters, slot occupancy, and effective values read through `effectiveLayoutStore`'s composed projections (`effectiveBowties`, `effectiveRole`, `effectiveValue`, `slotsByRole`, `isSlotFree`) — never directly from raw stores. The fast/slow path branch in `EditableBowtiePreviewStore` does not return; any new equivalent is a regression.
- Write entry points (including draft-recording commands like "record a bowtie deletion", "record a role classification", "record a draft value") are re-exported through the facade. Components do not call edit-layer store mutation methods directly.

When extending this ADR, add or amend invariants in this section rather than scattering them across the file.
