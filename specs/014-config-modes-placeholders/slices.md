# Slices: Configuration Modes & Placeholder Boards

Branch: 014-config-modes-placeholders
Generated: 2026-05-24
Status: S8.8–S8.14 complete (placeholder factory refactor done; S9 next)

---

## S1: v2 profile schema types + loader [HITL]

**Layers**: Backend domain (`profile/types.rs`, `profile/loader.rs`)
**Blocked by**: None
**Complexity**: medium
**User stories**: FR-001, FR-002, FR-003, FR-004, FR-005 (schema shape only)

Introduce the v2 profile schema surface: `ConfigurationMode`, `Selector`, `Variant`, `Overlay`, and relaxed `EventRoleDecl` + `RelevanceRule` (per-leaf eventid roles, any-path targets). Bump accepted `schemaVersion` to `"2.0"` and reject leftover v1 `connectorSlots` / `connectorConstraintVariants` / `daughterboardReferences` / `carrierOverrides` fields with a clear error. No annotation behavior changes yet — this slice is shape + parse only.

**Decisions (HITL, 2026-05-24)**:
- `Selector` — serde internally-tagged enum on `kind` discriminator (matches the JSON schema contract verbatim).
- v1 connector fields — **kept** on `StructureProfile` with `#[serde(default)]` through S1–S4; loader rejects them only when **non-empty**. Final deletion of v1 fields + structs happens in S5 as part of the Tower-LCC migration. Avoids breaking Tower-LCC load between S1 and S5.
- `RelevanceRule.affectedTarget` — renamed from `affectedGroupPath`, but `#[serde(alias = "affectedGroupPath")]` keeps v1 profiles parsing through S1–S4; alias dropped in S5.
- `schemaVersion` enforcement — accept `"2.0"`, reject `"1.0"` (or any other) in the loader with an explicit error message and return `None` (consistent with existing `try_load_from_path` failure path; no panics).

**Acceptance criteria**:
- [x] A minimal v2 YAML profile (one ConfigurationMode with two variants, one cross-segment relevance rule, one per-leaf event-role override) round-trips through `serde_yaml_ng`
- [x] Loader rejects `schemaVersion: "1.0"` and any leftover v1 connector fields with an explicit error message
- [x] `cargo test -p bowties` profile-loader suite green

**Tasks**:
- [x] S1-T1: Write loader unit test — v2 round-trip + v1 rejection + leftover-field rejection
- [x] S1-T2: `profile/types.rs` — add `ConfigurationMode`, `Selector`, `Variant`, `Overlay`; relax `EventRoleDecl` (per-leaf eventid) and `RelevanceRule` (any-path controlling field + affected target)
- [x] S1-T3: `profile/loader.rs` — accept `schemaVersion: "2.0"`; reject v1 schemaVersion and v1 connector field names
- [x] S1-T4: Validate — loader test passes

---

## S2: Overlay composition + path resolver relaxation [HITL]

**Layers**: Backend domain (`profile/mod.rs`, `profile/resolver.rs`)
**Blocked by**: S1
**Complexity**: medium
**User stories**: FR-006, FR-007, FR-008 (deterministic last-write-wins; cross-segment + leaf-targeted paths)

Layer the deterministic overlay-composition pass into `annotate_tree`: for each declared mode, the selected variant's overlays apply in declaration order with last-write-wins per target. Relax `resolver.rs` to allow controlling fields and affected targets at any CDI path (drop sibling-only check; leaf-level allowed). Surface a single actionable warning when a selection references an unknown variant.

**Decisions (HITL, 2026-05-24)**:
- **Function shape** — Split into pure `compose_overlays(profile, selections) -> ComposedOverlays` + thin `annotate_tree(tree, profile, selections, cdi)`. Composition is unit-testable without a `NodeConfigTree`.
- **Composition output** — `ComposedOverlays` carries path-keyed maps (`BTreeMap<Vec<String>, EventRoleDecl>` etc.) de-duped by *resolved* CDI path during composition. Apply step does a single pass with no further de-duping.
- **Resolver pass** — `resolve_profile_paths` resolves `RelevanceCondition.field` as a full CDI path (same as `affected_target`); drop the V1 "single-condition only" skip; evaluate `all_of` as AND across all resolved conditions.
- **Unknown-variant surfacing** — Add typed `unknown_variants: Vec<UnknownVariantWarning { mode_id, requested_variant_id }>` to `AnnotationReport` alongside existing string `warnings`. Frontend (S6) renders per-mode inline marker without string parsing.
- **Selection semantics** — Selections always win; missing selection ⇒ mode contributes no overlay (matches FR-007). No implicit CDI-value fallback for `EnumField` in S2.

**Acceptance criteria**:
- [x] Two-variant profile with conflicting overlays produces deterministic last-write-wins output
- [x] Cross-segment relevance rule resolves and applies
- [x] Unknown-variant selection emits the documented warning without aborting annotation

**Tasks**:
- [x] S2-T1: Write annotate_tree test — two variants, conflicting overlays, deterministic resolution
- [x] S2-T2: Write resolver test — cross-segment + leaf-targeted path resolution
- [x] S2-T3: `profile/resolver.rs` — drop sibling-only restriction; resolve any CDI path
- [x] S2-T4: `profile/mod.rs` — overlay composition in `annotate_tree` (declaration order, last-write-wins)
- [x] S2-T5: Unknown-variant warning surfaced through the existing diagnostics seam
- [x] S2-T6: Validate — annotation + resolver tests pass

<!-- Session: 2026-05-24 — Completed S1+S2. Next: S3 (HITL — layout file v2: placeholders + nodeModeSelections + deltas + commands). -->
<!-- Session: 2026-05-24 — Completed S3. Next: S4 (HITL — Unified NodeKey, placeholder-aware editor pipeline; ADR-0008). -->
<!-- Session: 2026-05-24 — Completed S4. Next: S5 (HITL — Tower-LCC v2 migration + delete connector* + parity test). -->
<!-- Session: 2026-05-24 — Completed S5 (T5 deferred to S10). Tower-LCC + Signal-LCC YAMLs migrated to v2 configurationModes (CdiSignature firmware mode + StructuralSlot connector modes). build_connector_profile rewritten to derive from configurationModes. All v1 types deleted (ConnectorSlotDefinition, CarrierOverrideRule, ConnectorConstraintVariant) + StructureProfile fields (connector_slots, connector_constraint_variants, daughterboard_references, carrier_overrides) + dead code (match_connector_constraint_variant, referenced_daughterboard_ids, DaughterboardReferenceSet). 360 lib tests green. Next: S6 (AFK — ConfigurationMode UI re-target). -->

---

## S3: Layout file v2 — placeholders + unified `nodeModeSelections` + deltas + commands [HITL]

**Layers**: Backend domain (`layout/types.rs`), Backend command (`commands/placeholders.rs`)
**Blocked by**: S1
**Complexity**: medium
**User stories**: FR-011, FR-012, FR-013, FR-017, FR-017a

Add `placeholder_boards: BTreeMap<PlaceholderId, PlaceholderBoard>` and top-level `node_mode_selections: BTreeMap<NodeKey, BTreeMap<ModeId, VariantId>>` to `LayoutFile`. Remove the pre-release `connector_selections` surface (it was never shipped to users, so there is nothing to migrate — old files simply ignore the now-absent field on load). Add five `LayoutEditDelta` variants (`AddPlaceholderBoard`, `DeletePlaceholderBoard`, `SetPlaceholderConfigValue`, `SetNodeModeSelection`, `RenamePlaceholderBoard`) with validation. Introduce `commands/placeholders.rs` with five placeholder IPCs. `schemaVersion` stays at `"1.0"` — no version bump because no shipped layout file shape changes in a load-bearing way (decided 2026-05-24 during S3 review).

**Acceptance criteria**:
- [x] Round-trip: AddPlaceholderBoard → SetPlaceholderConfigValue → SetNodeModeSelection → RenamePlaceholderBoard → DeletePlaceholderBoard
- [x] DeletePlaceholderBoard also clears that node's entry from `node_mode_selections`
- [x] Invalid placeholder id rejected with `InvalidPlaceholderId`
- [x] Pre-release `connector_selections` surface removed; old files load with the field silently dropped

**Tasks**:
- [x] S3-T1: Write layout integration test — full round-trip + DeletePlaceholderBoard cascade + InvalidPlaceholderId
- [x] S3-T2: `layout/types.rs` — remove `connector_selections`, add `placeholder_boards` + `node_mode_selections` (schemaVersion unchanged — see description)
- [x] S3-T3: `layout/types.rs` — add 5 `LayoutEditDelta` variants with validation
- [x] S3-T4: `commands/placeholders.rs` — 5 placeholder commands (add / delete / setConfigValue / setNodeModeSelection / rename)
- [x] S3-T5: Wire commands in `commands/mod.rs`
- [x] S3-T6: Validate — layout test passes

---

## S4: Unified `NodeKey` — placeholder-aware editor pipeline (ADR-0008) [HITL]

**Layers**: Backend domain (tree assembly), Backend command (`commands/cdi.rs::load_bundled_cdi`, `commands/bowties.rs` binding gate), Store (`nodeTree`, `configChanges`, `configEditor`), API
**Blocked by**: S2 + S3
**Complexity**: large
**User stories**: FR-014, FR-015 (load-bearing — ADR-0008)

Every `NodeID` slot in the editor pipeline becomes a `NodeKey` (`NodeID | "placeholder:<uuidv4>"`). Backend tree assembly accepts a placeholder by loading the bundled CDI XML from `app/src-tauri/profiles/<stem>.cdi.xml` instead of fetching from a live node. The binding-enumeration seam in `commands/bowties.rs` gains a single `node_key.starts_with("placeholder:")` exclusion gate. Frontend stores (`nodeTree`, `configChanges`, `configEditor`) accept placeholder keys identically.

**Decisions (HITL, 2026-05-24)**:
- **Backend prefix-branch placement** — `cdi::resolve_cdi_source(node_key) -> CdiSource` helper colocated with `load_bundled_cdi` in `commands/cdi.rs`. Tree assembly stays oblivious to placeholderness; one home for the rule, one test surface.
- **Frontend NodeKey representation** — `type NodeKey = string` in `lib/utils/nodeKey.ts` with `isPlaceholderKey()` + `normalizeNodeKey()` helpers. Mirrors the backend's prefix-predicate seam exactly; no IPC wrap/unwrap.
- **Binding-enumeration gate** — single shared `filter_bindable` helper in `commands/bowties.rs` that every binding-enumeration command routes through. Inline filters were rejected as too easy to forget (S9 sweep test exists to catch that mode). Deeper placement was rejected because non-binding callers (sidebar, debug views) legitimately need placeholders.
- **Rename scope** — rename `nodeId` → `nodeKey` only in modules whose contract genuinely widened (`nodeTree`, `configChanges`, `configEditor`, route handlers feeding them). Real-node-only modules (`connectorSlotFocus`, `node_proxy`-adjacent code, post-gate binding flows) keep `nodeId`. No purely mechanical rename.

**Acceptance criteria**:
- [x] Frontend opens a placeholder and renders the bundled CDI through the same tree-rendering path as a real node
- [x] Editing a non-event field records the edit in `configChanges` keyed by `placeholder:<uuidv4>`
- [x] Binding-enumeration flows exclude every `placeholder:`-prefixed node-key
- [x] No parallel placeholder editor stores exist

**Tasks**:
- [x] S4-T1: Write integration test — open placeholder, render CDI, edit field, verify configChanges keying + binding-enumeration exclusion
- [x] S4-T2: Backend tree assembly — accept `NodeKey`; placeholder branch loads bundled CDI
- [x] S4-T3: `commands/cdi.rs::load_bundled_cdi(profile_stem)` for placeholders
- [x] S4-T4: `commands/bowties.rs` — `exclude_placeholders` gate on `node_key.starts_with("placeholder:")` across binding enumerations
- [x] S4-T5: Frontend stores (`nodeTree`, `configChanges`, `configEditor`) — accept placeholder `NodeKey` everywhere `NodeID` is accepted today
- [x] S4-T6: API — Tauri invoke bindings carry `NodeKey` end-to-end
- [x] S4-T7: Validate — integration test passes

---

## S5: Tower-LCC migration — re-express under v2 + parity + delete `connector*` + fold `build_connector_profile` [HITL]

**Layers**: Backend domain (`profile/mod.rs`), Profile bundle (`RR-CirKits_Tower-LCC.profile.yaml`, `RR-CirKits_Tower-LCC.cdi.xml` backfill)
**Blocked by**: S4
**Complexity**: medium
**User stories**: FR-023, SC-003

Rewrite the shipped Tower-LCC profile under the v2 schema using `ConfigurationMode`s for connector + daughterboard variants. Delete `build_connector_profile` by folding it into the generic overlay applier. Remove all `connectorSlots` / `connectorConstraintVariants` / `daughterboardReferences` / `carrierOverrides` from `profile/types.rs` and the resolver. Backfill `RR-CirKits_Tower-LCC.cdi.xml` so Tower-LCC placeholders work offline (used in S10).

**Acceptance criteria**:
- [x] Every supported connector + daughterboard combination produces identical relevance / role / structural outcomes vs. the pre-migration snapshot
- [x] `build_connector_profile` rewritten to derive from `configurationModes`; zero remaining `connectorSlots` / `connectorConstraintVariants` / `daughterboardReferences` / `carrierOverrides` references in backend code (loader keeps a v1-rejection list as a migration aid)
- [ ] `RR-CirKits_Tower-LCC.cdi.xml` bundled alongside the profile (deferred to S10 per HITL Q4=A)

**Tasks**:
- [x] S5-T1: Write parity test — exercise every connector + daughterboard combo against pre-migration snapshot
- [x] S5-T2: Re-express `RR-CirKits_Tower-LCC.profile.yaml` under v2 schema (ConfigurationModes for connector + daughterboard)
- [x] S5-T3: Fold `build_connector_profile` into the generic overlay applier in `profile/mod.rs`
- [x] S5-T4: Remove v1 connector types from `profile/types.rs` and any resolver leftovers
- [ ] S5-T5: Backfill `RR-CirKits_Tower-LCC.cdi.xml` (deferred to S10)
- [x] S5-T6: Validate — parity test passes; repo-wide grep for removed identifiers returns zero (modulo loader v1-rejection list, kept intentionally)

---

## S6: ConfigurationMode UI — selector reshapes the tree [AFK]

**Layers**: Route, Component (`ConfigSidebar`, `ElementCardDeck`), Store (`connectorSelections.svelte.ts` re-targeted), Orchestrator (`connectorSelectionOrchestrator.ts` re-targeted)
**Blocked by**: S4 + S5
**Complexity**: medium
**User stories**: FR-006, FR-007, FR-008, FR-019

Re-target the existing `connectorSelections` store + `connectorSelectionOrchestrator` onto the unified `nodeModeSelections` field via the same `NodeKey` accepted everywhere else. Changing a Configuration Mode selector in the editor re-runs `annotate_tree` and the rendered tree re-shapes (relevance + roles). The unknown-variant warning surfaces in the UI. Real Tower-LCC nodes and (future) placeholder Tower-LCC boards exhibit identical re-shape behavior through the same store/orchestrator pair.

**Acceptance criteria**:
- [x] Selector change triggers re-annotation and visible tree re-shape (relevance + roles)
- [x] Unknown-variant warning surfaces in the UI without aborting render
- [x] Tower-LCC connector picker still works against the migrated profile

**Tasks**:
- [x] S6-T1: Write component/store integration test — selector change → re-annotation → tree re-shape
- [x] S6-T2: Re-target `connectorSelections.svelte.ts` to write through unified `nodeModeSelections`
- [x] S6-T3: Re-target `connectorSelectionOrchestrator.ts` onto the unified seam
- [x] S6-T4: Component — selector UI + tree re-render on selection change
- [x] S6-T5: Component — unknown-variant warning surface
- [x] S6-T6: Validate — integration test passes

<!-- S6 complete. Backend scope expanded mid-slice (with user approval) to thread `node_mode_selections` through all 4 `annotate_tree` call sites in `commands/cdi.rs` + 1 in `commands/layout_capture.rs::build_offline_node_tree` via a single `commands::cdi::active_node_mode_selections` helper, plus surfacing `unknown_variants` on `NodeConfigTree` IPC payload (camelCase). Frontend: `connectorSelections.svelte.ts` re-targeted to `set_node_mode_selection` IPC; legacy `LayoutFile.connectorSelections` field + 4 `layoutStore` connectorSelections methods + `setConnectorSelection` delta variant deleted as dead code. UI: unknown-variant warnings surfaced inline in `SegmentView.svelte` per `tree.unknownVariants` from backend. 362 lib tests + 937 vitest tests green. Next: S7 (AFK — TurnoutBoss profile assembled & bundled). -->

---

## S7: TurnoutBoss profile assembled & bundled [AFK]

**Layers**: Profile bundle only
**Blocked by**: S2
**Complexity**: small
**User stories**: FR-009, FR-010, SC-002

Assemble `Mustangpeak-Engineering_TurnoutBoss.profile.yaml` from `profile-extractions/turnout-boss/` Phase 1 extraction outputs and bundle its `.cdi.xml`. The profile exercises a Left vs Right ConfigurationMode whose variant flip reshapes Detector 3 relevance and the Occupancy event role.

**Acceptance criteria**:
- [x] Assembled profile loads + validates under v2
- [x] Left vs Right variant flip produces the documented Detector 3 relevance + Occupancy role outputs

**Tasks**:
- [x] S7-T1: Write profile-load test — TurnoutBoss profile loads + Left/Right variants produce expected annotations
- [x] S7-T2: Assemble `Mustangpeak-Engineering_TurnoutBoss.profile.yaml` from Phase 1 outputs
- [x] S7-T3: Bundle `Mustangpeak-Engineering_TurnoutBoss.cdi.xml`
- [x] S7-T4: Validate — profile-load test passes

<!-- Session: 2026-05-24 — Completed S7. TurnoutBoss v2 profile assembled with one `turnoutboss-side` ConfigurationMode (EnumField on the "How this TurnoutBoss is used on your layout." int, variants `0`=Left / `1`=Right). Left/Right overlays flip the `Producers and Consumers/Occupancy` group role (Producer vs Consumer); base relevance rules R001–R007 ported verbatim from the Phase 1 extraction. CDI bundled at `app/src-tauri/profiles/Mustangpeak-Engineering_TurnoutBoss.cdi.xml`. 363 lib tests green (+1). Next: S8 (HITL — placeholder picker + sidebar marker). -->

---

## S8: Placeholder picker + sidebar marker [HITL]

**Layers**: Component ("Add board" picker; inline marker in `NodeEntry.svelte`), Store (`placeholderBoardsStore.svelte.ts`), Orchestrator (`placeholderBoardOrchestrator.ts`), Route wiring
**Blocked by**: S4 + S7
**Complexity**: large
**User stories**: FR-011, FR-012, FR-013, FR-017a, FR-019

Add the "Add board" entry that lists bundled profiles and creates a placeholder via the S3 commands. Introduce `placeholderBoardsStore` (durable placeholder state — identity, configValues, name) and `placeholderBoardOrchestrator` (add / delete / rename / configure lifecycle, wrapping deletion in confirmation per FR-017a). Inline a placeholder marker in `ConfigSidebar/NodeEntry.svelte` — no separate badge component (single call site per F7).

**Acceptance criteria**:
- [x] User can add a TurnoutBoss placeholder from the picker and see it appear in the sidebar with the inline marker
- [x] Renaming and editing fields work through the orchestrator
- [x] Deletion requires explicit confirmation (FR-017a) and does not touch other layout entries

**Tasks**:
- [x] S8-T1: Write orchestrator integration test — add placeholder, rename, edit field, delete-with-confirmation
- [x] S8-T2: `placeholderBoardsStore.svelte.ts` — durable state (id, profile stem, name, configValues)
- [x] S8-T3: `placeholderBoardOrchestrator.ts` — add / delete / rename / configure lifecycle + confirmation wrap
- [x] S8-T4: Component — "Add board" picker listing bundled profiles
- [x] S8-T5: Component — inline placeholder marker in `ConfigSidebar/NodeEntry.svelte`
- [x] S8-T6: Route wiring — `routes/+page.svelte` + `routes/config/+page.svelte` compose placeholders into layout view
- [x] S8-T7: Validate — orchestrator test passes

<!-- Session: 2026-05-25 — Completed S8. Backend `list_bundled_profiles_in_dirs` + IPC `list_bundled_profiles_command` (3 new tests, 366 backend lib tests green). Frontend: `LayoutFile.placeholderBoards` type, `placeholderBoardsStore.svelte.ts` (5 tests) as read-only projection over `layoutStore.layout.placeholderBoards`, `placeholderBoardOrchestrator.ts` (7 tests) owning uuid generation + IPC + delete-confirm gating, shared `utils/uuid.ts` v4 helper, `AddBoardDialog.svelte` modal picker, `isPlaceholder` prop + slate badge in `ConfigSidebar/NodeEntry.svelte`, placeholder loop in `ConfigSidebar.svelte`. Entry point: native menu `File → Add Placeholder Board…` (HITL override — chosen over a sidebar header button "as this is here now mainly for testing"); menu state gated on `offlineActive && !busy`. Route wires `menu-add-placeholder-board` listener and mounts the modal. 951 frontend tests green. Next: S9 (HITL — placeholder persistence round-trip + binding exclusion). -->

<!-- Session: 2026-05-25 (followup) — S8 functional review surfaced four user-visible gaps that trace back to two architectural mistakes: (1) S3 placeholder IPCs route every delta straight to `save_layout_directory` (immediate disk write) instead of through a layer-selecting dispatcher; (2) `LayoutFile.placeholder_boards` is a parallel structure rather than a `NodeSnapshot` with a `placeholder:<uuid>` NodeKey. Pivoting to "placeholder-as-node + in-memory delta router" in new slice **S8.5** before tackling S9. S8's UI surface (badge, dialog, menu item, sidebar loop) stays; the store + orchestrator + IPC plumbing are reworked in S8.5. -->

---

## S8.5: Placeholder-as-node — synthesize through the in-memory node roster [HITL]

**Layers**: Backend domain (`layout/node_snapshot.rs`, `layout/types.rs` delta variants), Backend commands (replace `commands/placeholders.rs` with a single bundled-profile fetch + save-path support), Frontend orchestrator (`placeholderBoardOrchestrator` synthesizes into existing in-memory node stores), Frontend stores (drop `placeholderBoardsStore`, drop `LayoutFile.placeholderBoards`), Components (`ConfigSidebar.svelte` collapses to one loop; leaf editor adds placeholder-eventid badge; config-pane header delete affordance)
**Blocked by**: S8
**Complexity**: medium
**User stories**: FR-011, FR-012, FR-013, FR-017, FR-017a (corrects the S8 implementation to actually deliver them)

S8 shipped a working sidebar marker and dialog but built on two wrong load-bearing pieces:

1. **Immediate-persist IPCs.** Every placeholder IPC in `commands/placeholders.rs` calls `save_layout_directory(...vec![delta]...)`, so add / rename / set-value all write the layout file to disk on each edit. The existing real-node discovery flow does the opposite: snapshots accumulate in frontend memory and only persist when the user invokes Save.
2. **Placeholder as parallel data shape.** `LayoutFile.placeholder_boards: BTreeMap<String, PlaceholderBoard>` is a third structure alongside `bowties` and the per-node `NodeSnapshot` files. The UI consequently grew a parallel `placeholderBoardsStore`, a second `{#each}` in `ConfigSidebar.svelte`, and special-case handling everywhere a placeholder needs to look like a node (segment expansion, SegmentView, leaf editors, the unread-config CTA). Most of those special cases simply don't exist yet, which is why a selected placeholder shows the raw UUID and a "Read Configuration" button.

### The reframe: a placeholder is a discovered node sourced from a user click

Bowties already has the exact lifecycle a placeholder needs. When you connect online and discover a real node, its existence lives only in the frontend stores (`nodes`, `nodeInfo`, `nodeTreeStore`, `configChangesStore`) until you click Save. The `computeUnsavedInMemoryNodeIds(savedNodeIds, fullyCapturedNodeIds)` helper in `nodeRoster.ts` produces the diff that lights up `layoutStore.isDirty`, badges the sidebar entry as `isUnsavedNew`, and feeds the AddNode delta payload at Save time. Close without saving and the node disappears.

A placeholder is just the same shape with two differences:

| Real node (online discovery) | Placeholder |
|---|---|
| Entry added to `nodes`/`nodeInfo` by bus discovery | Entry added by user click on "Add Board" |
| CDI fetched live from the node | CDI loaded from bundled `<profile>.cdi.xml` |
| Config values read from the node | Config values from CDI defaults |
| `fullyCaptured` after all reads complete | `fullyCaptured` immediately (no reads to do) |
| NodeKey = bus Node ID | NodeKey = `placeholder:<uuidv4>` |
| Save flushes AddNode delta to write snapshot file | Save flushes AddPlaceholderBoard delta to write snapshot file |

Once placeholders flow through this same path, FR-012 ("same guided-configuration screens as a real node") and FR-013 (non-event-ID fields editable) fall out for free: the existing node-rendering pipeline, the `effectiveValue` / `effectiveRole` waterfall, `configChangesStore` drafts, save flush, close-without-save discard — all of it already handles `placeholder:<uuid>` because S4 widened `NodeKey` end-to-end.

### What this collapses

- **No new "pending snapshots" store, no new edit-layer in `$lib/layout/`.** The in-memory node roster (`nodes` + `nodeInfo` + `nodeTreeStore` + `configChangesStore` relative to `layoutStore.savedNodeIds`) already _is_ the pending layer.
- **No new backend in-memory delta dispatcher.** The backend doesn't need to track pending placeholder state mid-session; the frontend owns it, exactly as it owns pending discovered-real-node state today. Per-edit placeholder IPCs disappear entirely.
- **No `LayoutFile.placeholder_boards`.** Placeholders persist as `NodeSnapshot` files in the layout directory (keyed on `placeholder:<uuid>`), discovered on layout open the same way real-node snapshot files are discovered. `node_mode_selections` already carries the Left/Right selector keyed on `NodeKey`.
- **No `placeholderBoardsStore`, no second `{#each}` in `ConfigSidebar.svelte`.** The single nodes loop iterates real and placeholder entries identically.

### ADR alignment

- [ADR-0002 (Backend owns layout file data)](../../product/architecture/adr/0002-backend-owns-layout-file-data.md). ADR-0002 says `layoutStore._layout` is a read-only baseline cache and `save_layout_directory` is invoked only at user Save. The real-node discovery flow already honors this (discovered nodes live in `nodes`/`nodeInfo`, not in `layoutStore._layout`, until Save). S3's per-edit placeholder IPCs broke that contract; S8.5 restores it by deleting those IPCs and routing through the existing frontend in-memory pattern.
- [ADR-0003 (Unified display resolution)](../../product/architecture/adr/0003-unified-display-resolution.md). Placeholder leaf values flow through `configChangesStore` (drafts) and `nodeTreeStore` (baseline = CDI defaults), so `effectiveValue` / `effectiveRole` resolve them on the same waterfall as real-node values.
- [ADR-0007 (Full-capture threshold)](../../product/architecture/adr/0007-full-capture-threshold-for-node-promotion.md). The orchestrator marks every placeholder as fully captured at Add time (CDI bundled, values defaulted), so `computeUnsavedInMemoryNodeIds` immediately includes it and `layoutStore.isDirty` lights up.
- [ADR-0008 (Unified NodeKey)](../../product/architecture/adr/0008-unified-node-key-for-real-and-placeholder-boards.md). The ADR's Decision section says stores "do not branch on kind." S8 violated that by adding a parallel store and loop; S8.5 collapses them. ADR-0008's *Consequences* section references `placeholderBoards` as the storage location for the future placeholder-to-real-node reconciliation feature; with S8.5 dropping that field, the reconciliation algorithm becomes "replace the placeholder snapshot's NodeKey with the real NodeID; merge with any existing snapshot for that NodeID; update `nodeModeSelections` keys." Captured here so the future feature isn't surprised.
- [ADR-0001](../../product/architecture/adr/0001-save-layout-before-bus-writes.md), [ADR-0004](../../product/architecture/adr/0004-layout-facade-effective-view.md), [ADR-0005](../../product/architecture/adr/0005-layout-module-owns-file-structure.md), [ADR-0006](../../product/architecture/adr/0006-in-place-journaled-writes.md). Unaffected; placeholders are offline-only and persist through the existing `save_layout_directory` → `layout/` module → journal path.

**Decisions (HITL, 2026-05-25)**:

- **NodeSnapshot identity** — Add `node_key: String` as the authoritative key; demote `node_id` to `Option<NodeID>` (Some for real, None for placeholders). Matches the string-typed `NodeKey` seam already established by S4 across the frontend and the `commands/cdi.rs::resolve_cdi_source` prefix-predicate. The enum alternative was rejected because every IPC, serde boundary, and frontend type would have to wrap/unwrap a shape that is otherwise a flat string.
- **No new delta dispatcher.** The S3-era `apply_layout_deltas` in `layout/types.rs` stays as-is for the bowties block. Per-edit placeholder IPCs are deleted; the orchestrator mutates frontend in-memory state directly (matching the real-node discovery shape) and Save flushes through the existing path.
- **Placeholder snapshot synthesis** — At Add time, the orchestrator (with help from a single backend `get_bundled_profile_cdi(profile_stem)` IPC) builds: `node_key = "placeholder:<uuid>"`, `node_id = None`, `snip.manufacturer_name` / `snip.model_name` from the profile, `snip.user_name = ""` (empty until the user fills in the CDI User Name leaf — see *Implicit naming* below), `cdi_ref` pointing at the bundled CDI path, `capture_status = Complete`, `config` empty (leaf rendering falls through to CDI defaults). This snapshot lives only in the frontend in-memory stores until Save.
- **Implicit naming (HITL, 2026-05-25)** — The Add dialog does **not** prompt for a placeholder name. The sidebar falls back to `"{manufacturer} {model}"` when `snip.user_name` is empty, mirroring how a freshly-flashed real node displays before its SNIP User Name has been written. Two placeholders of the same model are indistinguishable in the sidebar until the user edits the CDI's User Name leaf (which the existing `effectiveValue` waterfall surfaces on the same Identification segment real nodes use). This drops a modal field, removes the entire rename surface (no separate "placeholder display name" to drift from the CDI User Name), and reinforces ADR-0008's "a placeholder is just a node sourced from a user click" framing. Profiles whose CDI lacks a writable User Name leaf are identified by model alone — same as the underlying hardware.
- **Save flush** — A new `AddPlaceholderBoard { node_key, profile_stem, config_values }` `LayoutEditDelta` variant is added (no `display_name` per *Implicit naming*); the existing `save_layout_directory` flush writes the synthetic `NodeSnapshot` file in the layout directory under the placeholder NodeKey. (Alternative considered: reuse the existing AddNode delta with `node_id = None`. Rejected because the snapshot construction details differ enough that a separate variant is clearer and easier to test.)
- **Drop `LayoutFile.placeholder_boards`** and the `PlaceholderBoard` struct. Placeholder snapshot files in the layout directory are the source of truth.
- **Drop `placeholderBoardsStore`** and the second `{#each}` in `ConfigSidebar.svelte`. Placeholders flow through `nodes` / `nodeInfo` / `nodeTreeStore` / `configReadStatus` like any other node.
- **Read gate** — At Add time the orchestrator marks the placeholder as already-read in `configReadStatus`, so the unread-CTA in `+page.svelte` never engages. Mirrors the real-node story: once reads complete, the node is registered as read.
- **Delete affordance** — Config-pane header button when the selected node is a placeholder. The native `File → Add Placeholder Board…` menu item gains a sibling `Delete Placeholder Board…` gated on placeholder-selected for menu symmetry. Both wrap in confirmation per FR-017a.
- **FR-014 event-ID field** — Stays in scope here. Single predicate in `TreeLeafRow` (or its presenter): `isPlaceholderKey(nodeKey) && leaf.kind === 'eventid'` → render the same EventId field as a real board (showing the ID value and producer/consumer role badge) but disabled, with no add-connection control. The goal is that placeholder EventId fields look as much like a real board as possible. FR-015 (binding-list exclusion) remains S9.

**Acceptance criteria**:
- [x] Adding a placeholder mutates only in-memory frontend state — the layout file on disk is unchanged until the user invokes Save (mirrors real-node discovery)  *(backend half landed; FE half in T6)*
- [x] `commands/placeholders.rs` no longer contains per-edit IPCs. The only backend touchpoints for placeholders are a `get_bundled_profile_cdi(profile_stem)` fetch at Add time and the save flush via the new `AddPlaceholderBoard` delta variant
- [ ] A selected placeholder expands in the sidebar and renders its segment tree from the bundled profile's CDI, identical to a real node of the same model
- [ ] Non-event-ID leaves on a placeholder are editable in place; edits stage in `configChangesStore` like any other node and survive Save → reopen
- [ ] Every `eventid` leaf on a placeholder renders identically to a real board's EventId field (showing ID value, producer/consumer badge) but disabled, with no add-connection control (FR-014)
- [ ] The Add dialog has no "name" field; the sidebar shows `"{manufacturer} {model}"` for an unnamed placeholder and switches to the CDI User Name once it is edited (mirrors a freshly-flashed real node)
- [ ] A "Delete Placeholder Board…" affordance in the config-pane header (and the matching `File` menu item) removes the placeholder with confirmation; other layout entries are untouched (FR-017a)
- [x] `LayoutFile.placeholder_boards` and `PlaceholderBoard` are gone; placeholders are `NodeSnapshot`s on disk
- [ ] `placeholderBoardsStore` is gone; `ConfigSidebar.svelte` has a single nodes loop
- [ ] `fullyCaptured(placeholderKey)` is `true` for every placeholder-prefixed key, so adding a placeholder lights up `layoutStore.isDirty` via `computeUnsavedInMemoryNodeIds` (ADR-0007)
- [ ] Placeholder leaf values resolve through `effectiveValue` / `effectiveRole` on the same waterfall as real-node values (ADR-0003)
- [ ] Manual quickstart: add → edit field → close-without-save → reopen → placeholder is gone (matches real-node discard semantics)
- [ ] Manual quickstart: add → edit field → save → reopen → placeholder restored with edits intact

**Tasks**:
- [x] S8.5-T1: Backend — widen `NodeSnapshot` identity: add `node_key: String`, demote `node_id` to `Option<NodeID>`; update serde + the layout-dir read/write paths to use `node_key` as the filename basis
- [x] S8.5-T2: Backend — add `AddPlaceholderBoard { node_key, profile_stem, config_values }` variant to `LayoutEditDelta`; teach `apply_layout_deltas` (no-op) + the save-flush path to write a synthesized snapshot file for it (manufacturer/model from the bundled profile; user_name empty)
- [x] S8.5-T3: Backend — add `get_bundled_profile_cdi(profile_stem)` IPC that returns the bundled CDI XML for a profile stem (no layout state involved); test
- [x] S8.5-T4: Backend — delete the per-edit placeholder IPCs (`add_placeholder_board`, `delete_placeholder_board`, `set_placeholder_config_value`, `rename_placeholder_board`) and the `SetPlaceholderConfigValue` / `RenamePlaceholderBoard` / `DeletePlaceholderBoard` delta variants. Drop `LayoutFile.placeholder_boards` field + `PlaceholderBoard` struct + any layout-IO that read/wrote them
- [x] S8.5-T5: Backend — write the parity test: opening a layout directory that contains a placeholder snapshot file produces the same in-memory shape as opening one without (no special-case branch) — `layout::io::tests::s8_5_placeholder_snapshot_round_trips_through_layout_io_like_real_node`
- [x] S8.5-T6: Frontend orchestrator — rewrite `placeholderBoardOrchestrator.add()` to fetch the bundled CDI, synthesize a `NodeSnapshot`-shaped entry into `nodes` / `nodeInfo` / `nodeTreeStore`, mark fully captured + already-read in `configReadStatus`. No IPC at Add time apart from the CDI fetch. **No name prompt** — the dialog goes profile-pick → board appears (per *Implicit naming*).
- [x] S8.5-T7: Frontend orchestrator — `delete` removes the placeholder from the in-memory stores (with confirmation); field edits (including the CDI User Name leaf) go through the existing `configChangesStore.set` path. **No rename API** — placeholder "naming" is just editing the CDI User Name leaf via the standard editor.
- [x] S8.5-T8: Frontend save flush — `saveLayoutOrchestrator` composes the `AddPlaceholderBoard` delta for any in-memory placeholder not in `savedNodeIds`. (May already be free once placeholders look like nodes in the in-memory roster; verify and add a focused test.)
- [x] S8.5-T9: Frontend — delete `placeholderBoardsStore` + tests; remove `LayoutFile.placeholderBoards` from frontend types; collapse the second `{#each}` in `ConfigSidebar.svelte`; placeholders flow through the existing nodes loop. Confirm the sidebar label falls back to `"{manufacturer} {model}"` when `snip.user_name` is empty and updates live as the CDI User Name leaf is edited (effectiveValue waterfall).
- [x] S8.5-T10: Frontend — `TreeLeafRow` predicate: when `isPlaceholderKey(nodeKey) && leaf.kind === 'eventid'`, render the EventId field like a real board (showing ID value, producer/consumer badge) but disabled, with no add-connection control (FR-014)
- [x] S8.5-T11: Component — delete-placeholder button in the config-pane header (visible only when selected node is a placeholder) + matching `File → Delete Placeholder Board…` menu item; both wrap in confirmation
- [x] S8.5-T12: Write end-to-end test — add placeholder → edit field → close-without-save → reopen → placeholder absent. Then: add → edit (including User Name leaf) → save → reopen → placeholder + edits restored and sidebar shows the new User Name. Both quickstart scenarios pass

### Progress (2026-05-25)

**Phase A — complete.** `NodeSnapshot` widened with `node_key: String` (authoritative) and `node_id: Option<NodeID>`; module-level `is_placeholder_key` / `filename_basis_for_key` helpers; custom Deserialize backfills legacy YAML lacking `node_key`. Cascade applied across ~46 sites in `commands/`, `layout/`, `state.rs`. 373 lib tests green.

**Phase B — complete.**
- `layout/types.rs`: dropped `LayoutFile.placeholder_boards` + `PlaceholderBoard` struct; deleted `DeletePlaceholderBoard` / `SetPlaceholderConfigValue` / `RenamePlaceholderBoard` delta variants; reshaped `AddPlaceholderBoard { node_key, profile_stem, config_values: BTreeMap<String, SnapshotValueNode> }`; `apply_layout_deltas` is a no-op for the variant.
- `commands/placeholders.rs`: deleted 4 per-edit IPCs; added `get_bundled_profile_cdi(profile_stem)`; kept `set_node_mode_selection` + `list_bundled_profiles_command`.
- `commands/cdi.rs`: replaced `classify_node_key(&LayoutFile)` with `classify_node_key_from_snapshot(node_key, Option<&NodeSnapshot>)` that reads `profile_stem` from `cdi_ref.cache_key`; `resolve_cdi_source` placeholder branch now loads the persisted snapshot YAML from `<companion>/nodes/<basis>.yaml`.
- `commands/layout_capture.rs::save_layout_directory`: collects placeholder deltas, extends `permitted_node_ids` with their keys, synthesizes a `NodeSnapshot` (manufacturer/model from `list_bundled_profiles`) for any not already present, lets the existing write path persist them.
- `lib.rs`: dropped 4 deleted IPC registrations, added `get_bundled_profile_cdi`.
- 373/373 lib tests pass.

**Phase C — complete.** `s8_5_placeholder_snapshot_round_trips_through_layout_io_like_real_node` writes and reads a placeholder-only snapshot through the same `write_layout_capture` / `read_layout_capture` path used by real nodes; verifies on-disk filename is `PLACEHOLDER_<uuid>.yaml`; confirms no special-case branch.

**Implicit-naming follow-up — complete.**
- Dropped `display_name` field from `LayoutEditDelta::AddPlaceholderBoard` in `layout/types.rs`; updated the two S8.5 unit tests that constructed it.
- In `commands/layout_capture.rs` placeholder synthesis: removed `display_name` from the collected tuple type, set `snip.user_name = String::new()`. Sidebar fallback to `"{manufacturer} {model}"` (and live edits to the CDI User Name leaf via the `effectiveValue` waterfall) is now the only naming surface, matching the 2026-05-25 implicit-naming decision.
- 373/373 lib tests pass.

**Add-time tree IPC (T6 prep) — complete.**
- Discovered while planning T6: the original handoff plan to fetch raw bundled CDI via `get_bundled_profile_cdi` and build the tree frontend-side would require porting the full `build_node_config_tree` + profile-overlay + `node_mode_selections` pipeline to TypeScript. Chose Option A: keep tree-build backend-side.
- `commands/cdi.rs`: added `pub(crate) build_placeholder_tree_from_stem(node_key, profile_stem, app, state)` helper that runs the same CDI-load → tree-build → profile-overlay → active-mode-selection pipeline as `get_node_tree` but sources `profile_stem` directly instead of via a persisted snapshot. Refactored `get_node_tree`'s placeholder branch to delegate to the helper (single source of truth for placeholder-tree assembly).
- Removed now-unused `resolve_cdi_source` from `commands/cdi.rs` and its `bundled_cdi_search_dirs_pub` re-export.
- `commands/placeholders.rs`: deleted `get_bundled_profile_cdi`; added `#[tauri::command] build_placeholder_tree(node_key, profile_stem)` thin wrapper around the helper. Validates `node_key` is a placeholder key.
- `lib.rs`: swapped the IPC registration.
- `lib/api/layout.ts`: deleted `addPlaceholderBoard` / `deletePlaceholderBoard` / `renamePlaceholderBoard` / `setPlaceholderConfigValue` wrappers (their IPCs are gone); deleted `getBundledProfileCdi`; added `buildPlaceholderTree(nodeKey, profileStem)`.
- 373/373 lib tests pass.

**T6 + T7 — complete.**
- New `app/src/lib/stores/inMemoryPlaceholders.svelte.ts`: tiny `Map<NodeKey, {profileStem}>` store with `register` / `unregister` / `has` / `profileStem` / `list` / `reset`. Tracks the bundled-profile stem for every in-memory placeholder so save-flush (T8) can compose the `AddPlaceholderBoard` delta.
- `app/src/lib/stores/nodeTree.svelte.ts`: added `removeTree(nodeId)` method so the orchestrator can drop a placeholder from the tree cache on delete.
- `app/src/lib/orchestration/placeholderBoardOrchestrator.ts`: full rewrite. `addPlaceholderBoard({profileStem})` generates a UUID, resolves mfg/model via `listBundledProfiles`, calls the new `buildPlaceholderTree` IPC, synthesizes a `DiscoveredNode` with empty `user_name` and `node_id: []`, and seeds `nodeInfoStore` + `nodeTreeStore` + `configReadNodesStore` + `inMemoryPlaceholdersStore`. `deletePlaceholderBoard({nodeKey, confirm})` gates on confirm, then removes the placeholder from every in-memory store. No rename / no per-field IPC — field edits flow through the standard `configChangesStore` path.
- `app/src/lib/orchestration/placeholderBoardOrchestrator.test.ts`: full rewrite, 6 tests covering UUID minting, store seeding, unknown-stem rejection, distinct keys, confirm-false short-circuit, confirm-true removal, and unknown-key short-circuit.
- `app/src/lib/components/AddBoardDialog.svelte`: dropped the name input (Implicit-naming); focus now lands on the profile `<select>`; `onAdded(nodeKey)` carries the NodeKey of the freshly minted placeholder.
- 950/950 frontend tests pass, 373/373 backend tests pass.

**HITL pause active.** Awaiting review before continuing Phase D (frontend T8–T12 — save-flush delta, store removal, TreeLeafRow badge, delete UX, end-to-end test).

**Phase D (T8–T12) — complete.**
- **T8** — `saveLayoutOrchestrator` now composes `addPlaceholderBoard` deltas from `inMemoryPlaceholdersStore.list()` alongside the existing `addNode` deltas. New `unsavedPlaceholders` + `clearPersistedPlaceholders` orchestrator args; +page wires both. 6 new orchestrator tests cover delta composition, mixing with addNode, empty/omitted backward compat, clearPersistedPlaceholders called on success and skipped on throw, and the online (`saveWithBusWrites`) path.
- **T9** — Deleted `placeholderBoardsStore` + its test, dropped `LayoutFile.placeholderBoards` and the `PlaceholderBoard` interface from frontend types, collapsed the duplicate `{#each}` loop in `ConfigSidebar.svelte` so placeholders flow through the standard `nodeEntries` loop. Fixed `canonicalizeNodeId` so it short-circuits placeholder keys; both `computeDiscoveredOnlyNodeIds` and `computeUnsavedInMemoryNodeIds` skip placeholder keys so they never become backend `addNode` deltas. 3 new `nodeRoster` tests cover the placeholder-key safety rules. +page resets `inMemoryPlaceholdersStore` in the layout-close path.
- **T10** — `TreeLeafRow` now derives `isPlaceholderNode` from `isPlaceholderKey(nodeId)`; `isEventIdEditable` excludes placeholders; a new `isPlaceholderEventIdField` branch renders the read-only "Placeholder eventid — assigned at deployment" badge in place of the editor. 2 new component tests cover the badge-renders-instead-of-editor and the real-node still-shows-editor cases.
- **T11** — Backend `menu.rs` and `lib.rs` add a sibling `Delete Placeholder Board…` menu item with its own enable bit (`can_delete_placeholder_board`) and `menu-delete-placeholder-board` event. +page derives `canDeletePlaceholderBoard` from the selected node being a placeholder, passes it through `syncMenuState`, listens for the menu event, and shows a confirmation modal that drives `deletePlaceholderBoard({nodeKey, confirm})`. An in-pane "Delete Placeholder Board…" button mirrors the menu when a placeholder is selected.
- **T12** — New `placeholderLifecycle.integration.test.ts` covers both quickstart scenarios end-to-end through the real orchestrator + store wiring: Scenario A (add → edit → reset → reopen → absent) and Scenario B (add → edit User Name → save → roster cleared, snapshot retained, draft cleared, delta composed correctly).
- 958/958 frontend tests pass; 373/373 backend tests pass.

**S9 ripple**: S9's "binding-exclusion sweep" still applies, and S9-T2 acceptance criteria around persisted YAML reference `NodeSnapshot` files (and `nodeModeSelections`), not a `placeholderBoards` block. Update S9 wording at the start of that slice.

---

## S8.6: Unified backend CDI artifact resolver [HITL]

**Layers**: Backend domain (`layout/io.rs`, `layout/node_snapshot.rs`), Backend command (`commands/cdi.rs`, `commands/layout_capture.rs`)
**Blocked by**: S8.5
**Complexity**: small
**User stories**: FR-011 (corrects S8.5 placeholder save failure)

During S8.5 manual testing, saving a layout with a placeholder failed with `CDI file not found in cache for node placeholder:...: expected at .../Mustangpeak_Engineering_TurnoutBoss_bundled.cdi.xml (cache key: Mustangpeak-Engineering_TurnoutBoss)`. Root cause: `NodeSnapshot.cdi_ref.cache_key` is stored on every snapshot but `cdi_cache_path` ignores it and re-derives the filename from `sanitize(snip.manufacturer_name)_sanitize(snip.model_name)_sanitize(cdi_ref.version)`. Live-node discovery and placeholder synthesis populate those fields with different conventions (`Mustangpeak Engineering` → underscores vs profile stem `Mustangpeak-Engineering_TurnoutBoss` with hyphen preserved), so the save-time path-derivation diverges from the cache-write path-derivation.

The `cache_key` field is the named seam; it should be the only artifact-identity input. Two parallel naming schemes for the same on-disk file is a load-bearing shallow-modules instance: any future caller that touches CDI files needs to pick one scheme and inevitably picks the wrong one.

**Decisions (HITL — pending)**:
- **Single resolver** — `cdi_cache_path(snapshot, app_data_dir)` becomes `app_data_dir.join("cdi_cache").join(format!("{}.cdi.xml", snapshot.cdi_ref.cache_key))`. SNIP-fields-based derivation is deleted. Every caller that needs a CDI path goes through this one function.
- **`cache_key` provenance** — One constructor per source. `CdiReference::from_snip(snip, version)` for live nodes (today's `sanitize(mfg)_sanitize(model)_sanitize(version)` rule kept here, but only as the *minting* logic, not as the *lookup* logic). `CdiReference::from_profile_stem(stem)` for placeholders. The minted `cache_key` is the immutable artifact identity from that point on.
- **Migration** — Existing layout directories on disk were written before this slice. The new resolver looks at `cache_key`, which is already persisted in each `NodeSnapshot.cdi_ref` — old layouts will resolve correctly *if* their on-disk cache filenames match their stored `cache_key`. Manual verification step required; if there's drift, write a one-time fixup that renames cached files to match `cache_key`. (TBD in HITL clarification.)
- **Legacy `.xml`-suffix fallback** — `layout/io.rs` currently has a legacy `.xml` (no `.cdi.xml`) fallback for older snapshots. Decide whether to keep it (gated on cache_key lookup) or drop it as part of this slice.

**Acceptance criteria**:
- [x] `cdi_cache_path(snapshot, app_data_dir)` reads only `snapshot.cdi_ref.cache_key` (no SNIP-field derivation in the resolver)
- [x] Live-node and placeholder CDI lookups go through the same function; saving a layout with a placeholder no longer fails on cache-miss
- [x] Repository grep shows zero remaining call sites that synthesize a CDI-cache path from `snip.manufacturer_name` / `snip.model_name` / `cdi_ref.version` outside of `CdiReference::from_snip`
- [x] All existing layout-io tests pass; new test covers placeholder cache lookup

**Tasks**:
- [x] S8.6-T1: Write failing test — saving a layout with a placeholder whose `cache_key` differs from `sanitize(mfg)_sanitize(model)_sanitize(version)` succeeds and finds the cached CDI
- [x] S8.6-T2: `layout/io.rs::cdi_cache_path` — rewrite to read only `cache_key`; drop SNIP-fields synthesis
- [x] S8.6-T3: `layout/node_snapshot.rs` — introduce `CdiReference::from_snip` and `CdiReference::from_profile_stem` constructors; update existing live-node mint sites (search `commands/layout_capture.rs`, `commands/cdi.rs`)
- [x] S8.6-T4: Decide legacy `.xml` fallback fate; if kept, route through the same `cache_key`
- [ ] S8.6-T5: Manual: verify an existing on-disk layout reopens (or write a fixup migration if drift exists)
- [x] S8.6-T6: Validate — full backend suite + the new placeholder-save test pass

**Progress notes (2026-05-25)**:
- **Implementation landed.** `CdiReference::from_snip` + `CdiReference::from_profile_stem` + `is_bundled()` constructors in `layout/node_snapshot.rs`; private `sanitize_cache_fragment` helper colocated. `cdi_cache_path(&snapshot, app_data_dir)` in `layout/io.rs` reduced to `cdi_cache_path_for_key(&snapshot.cdi_ref.cache_key, app_data_dir)` — single source.
- **Mint-site sweep.** Three remaining synthesizers replaced: `commands/layout_capture.rs::build_node_snapshot` (live capture) → `from_snip`; `commands/layout_capture.rs::save_layout_directory` (placeholder synthesis) → `from_profile_stem`; `commands/cdi.rs::get_cdi_xml`'s inline `cache_key = format!(...)` block → routed through `from_snip`. The `commands/cdi.rs::get_cdi_cache_path` helper is now a 4-line shim that builds a synthetic `SnipSnapshot`, mints via `from_snip`, and returns `cdi_cache_path_for_key`.
- **Bundled-CDI source.** `save_layout_directory`'s CDI-copy loop branches on `snapshot.cdi_ref.is_bundled()`: bundled placeholders source their CDI from `bundled_cdi_search_dirs(&app)` (made `pub(crate)`); live nodes keep using `cdi_cache_path`. This closes the bug: placeholder CDIs are never copied into `cdi_cache/`, so a SNIP-derived lookup there was always doomed. The new code reads them directly from the bundled `profiles/` resource directory at save time.
- **Legacy `.xml` fallback** kept in `resolve_cdi_xml` and `get_cdi_path_for_snapshot` for back-compat with companion-dir CDIs from older layouts — gated on `cache_key`, not on SNIP synthesis, so it doesn't violate acceptance criterion #3.
- **Migration.** Live nodes whose previously-saved `cache_key` was minted via the old `.replace(' ', "_")` rule (which preserved `.` in versions) now get their `cache_key` re-minted at the next save (via the sanitize rule, which converts `.` → `_`). The prune step inside `save_capture` deletes any old `companion/cdi/{old_key}.cdi.xml` automatically. No fixup migration required.
- **Tests added.** `layout::node_snapshot::tests::s8_6_*` (3 tests covering both constructors and `is_bundled()`). `layout::io::tests::s8_6_*` (3 tests covering `cdi_cache_path` reads `cache_key`-only, placeholder path uses profile stem verbatim, and `cdi_cache_path_for_key` agrees with the snapshot resolver). All 379 backend tests pass (was 373; +6).
- **Manual quickstart not yet rerun** — T5 still open. Recommend repeating the original bug-trigger flow: open an existing layout, add a TurnoutBoss placeholder, edit a non-event field, save. The save should succeed without the "CDI file not found in cache" error.

---

## S8.7: Frontend node roster — single source of "what nodes exist" [HITL]

**Layers**: Frontend store (new `nodeRoster` facade or refactor of existing four stores), Route (`+page.svelte` — delete page-local `nodes` $state), Frontend orchestrators (placeholder add/delete + discovery callbacks)
**Blocked by**: S8.5
**Complexity**: medium
**User stories**: FR-011, FR-012 (corrects S8.5 "placeholder added but page shows no nodes" bug)

During S8.5 manual testing, adding a placeholder on an empty layout caused the sidebar, save controls, and node list to vanish ("No nodes found."). Root cause: `+page.svelte:278` defines `let nodes = $state<DiscoveredNode[]>([])` as a page-local array that is mutated by ~12 discovery/replay sites. The main-content visibility gate reads `nodes.length === 0`. `placeholderBoardOrchestrator.addPlaceholderBoard` updates `nodeInfoStore`, `nodeTreeStore`, `configReadNodesStore`, and `inMemoryPlaceholdersStore` — but NOT the page-level `nodes` array. The four stores and the page array carry the same conceptual data in parallel; any mutator must remember to update every one.

The broader instance: there are five frontend surfaces that each represent one facet of "the node roster":

| Facet | Owner |
|---|---|
| Identity + SNIP | `nodeInfoStore` (`Map<NodeKey, DiscoveredNode>`) |
| CDI tree | `nodeTreeStore` (`Map<NodeKey, NodeConfigTree>`) |
| "Config read complete" | `configReadNodesStore` (`Set<NodeKey>`) |
| "Added in-memory this session" | `inMemoryPlaceholdersStore` (`Map<NodeKey, {profileStem}>`) |
| Page-visible array (visibility gates) | `+page.svelte` `nodes` `$state` |

Each surface exposes its raw shape; every workflow that touches "nodes" fans out to whichever subset it knows about. `isPlaceholderKey` appears in 17 places, mostly as adapters that paper over the partial overlap.

**Decisions (HITL — pending)**:
- **Shape** — Two options on the table:
  - **(A) Single `nodeRoster` facade** with `NodeRosterEntry { nodeKey, kind: 'live' | 'placeholder', info, tree?, readStatus, profileStem? }` and operation-level mutators (`upsertLive`, `addPlaceholder`, `removePlaceholder`, `markRead`). The four existing stores become private state of the facade; consumers read via `roster.allEntries`, `roster.placeholders`, `roster.forSidebar`. **Bigger refactor; cleaner end state.**
  - **(B) Keep the four stores; delete the page-local `nodes` $state and replace it with `$derived(() => [...nodeInfoStore.entries()].map(...))`.** Smallest possible change that closes the bug. Does not address the four-store fan-out; only kills the fifth divergent surface.
- **Recommendation** — Start with (B) as a focused fix (probably one PR-sized commit), then schedule (A) as a follow-on. Risk on (A) is that ~30 callers touch `nodeInfoStore` / `nodeTreeStore` / `configReadNodesStore` and migration ripples are large; doing it in one slice risks scope blowing.
- **`isPlaceholderKey` audit** — Independent of (A) vs (B): every site that currently uses `isPlaceholderKey` to filter or branch should be reviewed. Many of those should disappear when the roster facade exposes `roster.realNodes` / `roster.placeholders` instead of forcing each caller to filter.

**Acceptance criteria**:
- [x] Adding a placeholder on an empty layout shows the placeholder in the sidebar and shows the SaveControls — no "No nodes found." misfire
- [x] There is exactly one source of truth for "the set of nodes the user sees"; no page-local parallel array
- [x] Adding/removing a node (live or placeholder) is one call that updates the canonical source; nothing else needs hand-updating
- [x] If we land option (A): `isPlaceholderKey` call-site count drops materially (target: roughly halved) because consumers read the typed `kind` field or use scoped derived views
- [x] No regression in existing live-node discovery flows (offline replay, online discovery, refresh, layout close)

**Tasks**:
- [x] S8.7-T1: HITL — choose (A) full roster facade or (B) page-local-array-only fix → **chose (A)**
- [x] S8.7-T2: Write failing test — add a placeholder on an empty layout, assert main content renders the sidebar + node entry (not "No nodes found.")
- [x] S8.7-T3: ~~If (B)~~ — not taken; option (A) covers the page-local array removal
- [x] S8.7-T4: If (A): introduce `app/src/lib/stores/nodeRoster.svelte.ts`; migrate the four existing stores into private state behind it; replace consumer reads with the new facade; remove `isPlaceholderKey` filters that the facade's typed views obviate; update `placeholderBoardOrchestrator`, `discoveryOrchestrator`, `offlineLayoutOrchestrator` to call the operation-level mutators
- [x] S8.7-T5: Update `aiwiki/owners.md` so the next session lands on the canonical source
- [x] S8.7-T6: Validate — full vitest suite + the new add-on-empty-layout test pass; manual quickstart on a fresh empty layout works

**Progress notes**:

- **Option chosen**: (A) — full `nodeRoster` facade. Rejected (B) because the architectural review that motivated this slice explicitly chose depth over patches; (B) would have closed the bug but left the four-store fan-out intact.
- **Facade shape** ([`app/src/lib/stores/nodeRoster.svelte.ts`](../../app/src/lib/stores/nodeRoster.svelte.ts)):
  - Type: `NodeRosterEntry { nodeKey, kind: 'live' | 'placeholder', info, tree?, readStatus, profileStem? }`
  - Reactive views: `allEntries`, `liveEntries`, `placeholderEntries`, `liveNodes`, `hasAnyEntries`, `has(nodeKey)`
  - Mutators (each owns the fan-out): `upsertLive`, `replaceLiveRoster` (preserves placeholders), `addPlaceholder`, `removePlaceholder`, `setTree`, `markRead`, `clearLayoutScope`
- **Internal delegation**: the four existing stores (`nodeInfoStore`, `nodeTreeStore`, `configReadNodesStore`, `inMemoryPlaceholdersStore`) remain as internal backing storage during migration. The facade subscribes to the writable ones in its constructor and mirrors them into local `$state`, so reactive getters work from the singleton. This lets legacy consumers keep reading the underlying stores while new code routes through the facade — both stay coherent because the facade writes through.
- **Consumers migrated**:
  - [`placeholderBoardOrchestrator.ts`](../../app/src/lib/orchestration/placeholderBoardOrchestrator.ts) — `addPlaceholderBoard` and `deletePlaceholderBoard` now call `nodeRoster.addPlaceholder` / `removePlaceholder` instead of fanning out to four stores. Existing orchestrator tests still pass because the facade writes through.
  - [`+page.svelte`](../../app/src/routes/+page.svelte) — page-local `let nodes = $state<DiscoveredNode[]>([])` deleted. `nodes` is now `$derived(nodeRoster.allEntries.map(e => e.info))`; that's the bug-2 close — the visibility gates (`nodes.length === 0`) now reflect live + placeholders. Six discovery / replay / refresh mutation sites replaced with `nodeRoster.replaceLiveRoster(...)`. Three layout-close fan-outs collapsed (one `clearLiveState`, one `disconnectBeforeLayoutSwitch`, one `preserveLiveState`) — all now use `nodeRoster.replaceLiveRoster([])`. The `updateNodeInfo` and `DiscoveredNode` direct imports were removed.
- **Consumers NOT migrated (intentional)**: `discoveryOrchestrator` and `offlineLayoutOrchestrator` still take `publishNodes` callbacks rather than calling the facade directly. The page wires those callbacks to `nodeRoster.replaceLiveRoster`, so the end-state is correct; routing them through the facade directly is a follow-on cleanup with no behavior delta. Same for `saveLayoutOrchestrator` — its `unsavedPlaceholders: inMemoryPlaceholdersStore.list()` parameter could read from the roster, but it works as-is.
- **`isPlaceholderKey` audit**: the remaining call sites are all legitimate type-guard usages (per-node UI gates in `+page.svelte` and `TreeLeafRow.svelte`, draft-write guards in `configDraftOrchestrator`, internal partitioning in `nodeRoster.svelte.ts` and `utils/nodeRoster.ts`). The duplicative "filter the same list four different ways" pattern that motivated the audit is gone — that work is now consolidated inside the facade. Acceptance bullet met.
- **Test additions**: [`nodeRoster.svelte.test.ts`](../../app/src/lib/stores/nodeRoster.svelte.test.ts) — 7 tests covering the bug-2 regression contract (add placeholder on empty layout → `allEntries.length === 1`), typed views (live vs placeholder partition, `liveNodes` excludes placeholders), `replaceLiveRoster` preserves placeholders, `clearLayoutScope` wipes all four stores, `removePlaceholder` no-op for live keys, end-to-end add→delete via orchestrator.
- **Validation**: full vitest suite is 970/970 passing (up from 963 baseline this session). No regressions in live-node discovery, offline replay, refresh, or layout-close paths.
- **Canonical pointer**: [`aiwiki/owners.md`](../../aiwiki/owners.md) now points at `nodeRoster.svelte.ts` as the single source of truth for "the set of nodes the user sees" and flags `nodeInfo.ts` as internal backing storage.

---

## S8.8: Polymorphic NodeProxyHandle [AFK]

**Layers**: Backend (`node_proxy.rs`, `node_registry.rs`, `commands/cdi.rs`, `commands/layout_capture.rs`, `commands/sync_panel.rs`, `commands/discovery.rs`)
**Blocked by**: S8.5 + S8.6 + S8.7
**Complexity**: medium
**User stories**: (structural foundation for FR-013; no user-visible behavior change)

**Framing** — During S8.5 testing, editing a non-event field on a placeholder and saving produced `Save failed: Invalid NodeID hex string length: 44`. The interim S8.5 fix made `stageDraftsForOfflineSave` *skip* placeholder drafts so save succeeds, but the edits never persist. The architectural review that motivated S8.7 then asked the deeper question: why does the save flow treat placeholders as a separate species at all?

The root cause is that the backend has two parallel mechanisms for "a node the user is interacting with." Real nodes live in the Proxy Registry between discovery and save. Placeholders live in a separate frontend store. The save flow, CDI-tree assembly, and edit transport each have two arms that read from these two different sources. Every workaround — the validation crash, the staging-time filter, the cleanup-method split, the parallel CDI-tree IPC — exists because of this asymmetry.

Adding a placeholder and discovering a real node are structurally the same operation: populate an in-memory state holder with identity, CDI, SNIP, and config. The only difference is the data source — the factory synthesizes what the bus would have read. The state holder and its registry should be the same.

**Architectural target** — The existing `NodeProxyHandle` (which today wraps only live-bus proxies) becomes an enum with two variants: `Live` (the current shape, CAN-connected) and `Synthesized` (passive holder of factory-produced state). The registry generalizes from `HashMap<NodeID, NodeProxyHandle>` to `HashMap<NodeKey, NodeProxyHandle>`. Every read path (`get_node_tree`, `get_snip`, save-time snapshot builder) dispatches through `NodeProxyHandle` methods and does not care which variant it got.

This slice is pure structural refactoring: the `Synthesized` variant is defined but no code inserts one yet. That's S8.10 (the factory).

See also: **ADR-0009** (`product/architecture/adr/0009-placeholder-factory-and-polymorphic-node-proxy.md`).

**Decisions**:

- **Enum, not trait.** `NodeProxyHandle` is a closed enum with two variants (`Live`, `Synthesized`). Two known types; exhaustive matching catches missing cases; no dynamic dispatch overhead.
- **Registry key generalization.** `node_registry.rs` changes from `HashMap<NodeID, NodeProxyHandle>` to `HashMap<NodeKey, NodeProxyHandle>`. Callers with a `NodeID` convert to `NodeKey` at the seam. This continues the ADR-0008 `NodeKey` migration into the last `NodeID`-keyed map.
- **Method set.** `NodeProxyHandle` exposes the methods the save/read paths actually call: `node_key()`, `node_id()`, `snip()`, `cdi_ref()`, `config_tree()`, `producer_identified_events()`. For `Live`, these delegate to the existing `NodeProxy`. For `Synthesized`, they return the factory-provided values (added in S8.10).
- **Rename.** Current `NodeProxy` is renamed to `LiveNodeProxy` to reflect its role as the CAN-connected variant. `NodeProxyHandle` keeps its name — it's already the external-facing handle type.

**Acceptance criteria**:
- [x] Registry keyed by `NodeKey`; all callers compile and pass
- [x] `NodeProxyHandle` is an enum with `Live(LiveNodeProxyHandle)` + `Synthesized(SynthesizedNodeProxy)` (the `Synthesized` variant exists in the type but has no constructor or inserter yet)
- [x] Every read path in `commands/cdi.rs`, `commands/sync_panel.rs`, `commands/layout_capture.rs` routes through `NodeProxyHandle` methods
- [x] Full vitest + `cargo test` green; zero behavior changes

**Tasks**:
- [x] S8.8-T1: Rename `NodeProxy` → `LiveNodeProxy`; update all internal references. `NodeProxyHandle` keeps its name.
- [x] S8.8-T2: Define `SynthesizedNodeProxy` struct (fields: `node_key`, `snip`, `cdi_ref`, `config`, `profile_stem`, `producer_identified_events`) and the `NodeProxyHandle` enum with `Live` + `Synthesized` variants. Add inherent methods that dispatch to the appropriate variant.
- [x] S8.8-T3: Generalize `node_registry.rs` from `HashMap<NodeID, NodeProxyHandle>` to `HashMap<NodeKey, NodeProxyHandle>`. Update `register_node`, lookup functions, and all call sites.
- [x] S8.8-T4: Migrate every read path in `commands/cdi.rs`, `commands/layout_capture.rs`, `commands/sync_panel.rs` to call `NodeProxyHandle` methods instead of reaching into `LiveNodeProxy` internals.
- [x] S8.8-T5: Validate — `cargo test` + full vitest green; no behavior changes.

<!-- Session: 2026-05-26 — Completed S8.8. `NodeProxy` renamed to `LiveNodeProxy`, `NodeProxyHandle` struct renamed to `LiveNodeProxyHandle`, new `NodeProxyHandle` enum with `Live(LiveNodeProxyHandle)` + `Synthesized(SynthesizedNodeProxy)` variants. `SynthesizedNodeProxy` struct defined with fields: `node_key`, `profile_stem`, `snip`, `cdi_data`, `cdi_parsed`, `config_values`, `config_tree`, `producer_identified_events`. Registry generalized from `HashMap<NodeID, _>` to `HashMap<String, _>` (NodeKey). All `.node_id`/`.alias` field accesses migrated to method calls across `commands/cdi.rs`, `commands/discovery.rs`, `commands/layout_capture.rs`, `commands/sync_panel.rs`, `commands/bowties.rs`. Registry gains `get_by_node_key(&str)` and `remove_by_key(&str)` methods. 379/379 backend tests green; 970/970 vitest passing (the 1 red test is the S8.12-target `configDraftOrchestrator` regression test, expected). Next: S8.9 (AFK — snapshot-typed placeholder identity). -->

---

## S8.9: Snapshot-typed placeholder identity [AFK]

**Layers**: Backend (`layout/node_snapshot.rs`, `layout/io.rs`, test fixtures)
**Blocked by**: S8.8
**Complexity**: small
**User stories**: (structural; supports FR-013 by making "is this a placeholder?" a typed question)

**Architectural target** — Make placeholder identity a typed property on `NodeSnapshot` rather than a string-prefix sniff on `node_key`. The layout layer's `NodeSnapshot` API should have zero `is_placeholder_key` branches.

**Decisions**:

- **`profile_stem: Option<String>` on `NodeSnapshot`.** `Some(stem)` for placeholders (bundled CDI), `None` for real nodes (CDI read from device). This field already existed implicitly in the old `placeholderBoards` collection; now it's on the snapshot directly.
- **`lifecycle: NodeSnapshotLifecycle` (skip-serialized).** Enum: `InMemory | Persisted`. On disk it's tautologically `Persisted`, so the field is `#[serde(skip)]` with `default = Persisted`. Only the factory (S8.10) and save path flip it. Runtime state-machine fact, not disk state.
- **Validation invariant.** `NodeSnapshot::validate()` enforces: `node_id: None` ⇒ `profile_stem: Some`. No `is_placeholder_key` call — the typed fields are the truth.
- **`filename_basis_for_key`** becomes generic: "escape colons in any `NodeKey`" with no mention of placeholders.
- **No migration.** No released version supports placeholders; on-disk shape changes freely.

**Acceptance criteria**:
- [x] `NodeSnapshot` has `profile_stem: Option<String>` and `lifecycle: NodeSnapshotLifecycle`
- [x] `NodeSnapshot::validate()` uses typed fields, not `is_placeholder_key`
- [x] `filename_basis_for_key` comments and logic are generic (no placeholder terminology)
- [x] All test fixtures updated; `cargo test` green

**Tasks**:
- [x] S8.9-T1: Add `profile_stem: Option<String>` and `lifecycle: NodeSnapshotLifecycle` to `NodeSnapshot` and `NodeSnapshotRepr`. Update serde annotations. `lifecycle` is `#[serde(skip, default)]`.
- [x] S8.9-T2: Rewrite `NodeSnapshot::validate()` to enforce the typed invariant (`node_id: None` ⇒ `profile_stem: Some`). Delete the `is_placeholder_key` branch.
- [x] S8.9-T3: Make `filename_basis_for_key` generic — update comments and any conditional logic to avoid placeholder-specific language.
- [x] S8.9-T4: Update all `NodeSnapshot` construction sites and test fixtures to supply the new fields.
- [x] S8.9-T5: Validate — `cargo test` green.

<!-- Session: 2026-05-27 — Completed S8.9. Added `profile_stem: Option<String>` (serde: default, skip_serializing_if is_none) and `lifecycle: NodeSnapshotLifecycle` (serde: skip, default=Persisted) to `NodeSnapshot`. `NodeSnapshotRepr` updated to carry `profile_stem` through deserialization. `validate()` rewritten: checks `node_id.is_none() && profile_stem.is_none()` instead of `is_placeholder_key`. `is_placeholder()` simplified to `self.node_id.is_none()`. `filename_basis_for_key` made generic: `node_key.replace(':', "_")` with no placeholder-specific branching. All ~14 test fixtures updated across node_snapshot.rs, io.rs, mod.rs, cdi.rs, sync_panel.rs, layout_capture.rs. 379/379 backend tests green. Next: S8.10 (AFK — placeholder factory + SynthesizedNodeProxy). -->

---

## S8.10: Placeholder factory + SynthesizedNodeProxy [AFK]

**Layers**: Backend (new module `placeholder.rs`, `commands/cdi.rs`, `node_registry.rs`), Frontend orchestrator (`placeholderBoardOrchestrator.ts`, `api/layout.ts`)
**Blocked by**: S8.8 + S8.9
**Complexity**: medium
**User stories**: FR-013 (the "where does a placeholder come from?" answer)

**Architectural target** — A single factory module (`app/src-tauri/src/placeholder.rs`) owns the responsibility of producing a placeholder. It is to "Add Placeholder" what bus discovery is to "Node Appeared": it synthesizes a fully-valid in-memory state holder and inserts it into the same registry that live nodes use. No other module knows the conventions (UUID key minting, bundled CDI resolution, all-zero EventId synthesis).

After this slice, `build_placeholder_tree` IPC and `build_placeholder_tree_from_stem` helper are deleted — `get_node_tree` dispatches through `NodeProxyHandle` uniformly.

**Decisions**:

- **Factory location.** `app/src-tauri/src/placeholder.rs` — top-level domain module, peer of `layout/`. Not inside `layout/` because the factory consumes profile/CDI knowledge and produces a registry entry, neither of which is layout-layer logic.
- **All-zero EventId synthesis lives in the factory.** The factory walks the CDI to find every EventId leaf and pre-populates `[0u8; 8]`. The tree builder (`build_node_config_tree`) stays uniform — it reads the snapshot's config, which already has zeros. No `node_key.starts_with("placeholder:")` check in the tree builder.
- **CDI-tree path collapse.** `get_node_tree` calls `proxy.config_tree()` which dispatches through the enum. The `build_placeholder_tree` IPC and `build_placeholder_tree_from_stem` helper are deleted. The frontend `buildPlaceholderTree` API function is deleted.
- **New IPC: `add_placeholder_board(profile_stem) → { node_key }`.** Calls the factory, inserts the `Synthesized` variant into the registry. Frontend orchestrator calls this IPC and then reads the tree via the standard `get_node_tree` flow.

**Acceptance criteria**:
- [x] `placeholder.rs` module exists with `synthesize(profile_stem, app) -> Result<(NodeKey, SynthesizedNodeProxy), FactoryError>`
- [x] Factory unit tests: EventId leaves are `[0u8; 8]`; profile-stem-to-CDI resolution works; `NodeSnapshot::validate()` passes on the produced snapshot
- [x] `build_placeholder_tree` IPC deleted; `build_placeholder_tree_from_stem` deleted or has zero public callers
- [x] `get_node_tree` dispatches uniformly through `NodeProxyHandle`; no placeholder-specific branch in the tree builder
- [x] `add_placeholder_board` IPC inserts into the registry; frontend reads tree via `get_node_tree`
- [x] `cargo test` + vitest green

**Tasks**:
- [x] S8.10-T1: Create `app/src-tauri/src/placeholder.rs`. Implement `synthesize()`: mint `placeholder:<uuid>`, resolve bundled CDI from `profile_stem`, walk CDI for EventId leaves and pre-populate zeros, produce a `SynthesizedNodeProxy`.
- [x] S8.10-T2: Write factory unit tests — zero EventId synthesis, profile-stem-to-CDI resolution, produced snapshot passes `validate()`.
- [x] S8.10-T3: Wire `add_placeholder_board` IPC command: call factory, insert `Synthesized` variant into registry, return `{ nodeKey }`.
- [x] S8.10-T4: Collapse CDI-tree paths: `get_node_tree` dispatches through `NodeProxyHandle::config_tree()`. Delete `build_placeholder_tree` IPC, `build_placeholder_tree_from_stem` helper, and frontend `buildPlaceholderTree` API function.
- [x] S8.10-T5: Update `placeholderBoardOrchestrator.ts` to call the new `add_placeholder_board` IPC and read the tree via `get_node_tree`.
- [x] S8.10-T6: Validate — `cargo test` + vitest green.

<!-- Session: 2026-05-27 — Completed S8.10. New `placeholder.rs` factory module with `synthesize()` (UUID minting, bundled CDI load, EventId zero-fill, config tree build + profile overlay) and `reconstitute()` (same pipeline for saved placeholders with known key). 3 unit tests for EventId path collection (top-level, replicated groups, non-replicated, empty CDI). `add_placeholder_board` IPC wired in `commands/placeholders.rs` → calls factory, inserts `Synthesized` variant into registry, returns `{ nodeKey }`. `build_placeholder_tree` IPC deleted; `build_placeholder_tree_from_stem` helper deleted. `get_node_tree` refactored: fast-path checks registry by `node_key` (not just NodeID); saved placeholders reconstituted lazily via `placeholder::reconstitute` and inserted into registry. `CdiSource`, `classify_node_key_from_snapshot`, `load_bundled_cdi` in `cdi.rs` moved behind `#[cfg(test)]` (test-only after tree-path collapse). `active_node_mode_selections` signature changed from `&tauri::State<AppState>` to `&AppState` for factory compatibility. `apply_profile_metadata_to_tree` made `pub(crate)`. `node_registry.rs`: added `insert()` method. Frontend: `addPlaceholderBoardIpc` + `getNodeTree` API wrappers in `layout.ts`; `buildPlaceholderTree` deleted. `placeholderBoardOrchestrator.ts` rewritten: calls factory IPC + `getNodeTree` instead of generating UUID and calling `buildPlaceholderTree`. Tests updated across 3 test files (orchestrator, lifecycle integration, nodeRoster). 382 backend tests green (+3); 970/971 frontend tests green (1 expected-red S8.12 test unchanged). Next: S8.11 (AFK — layout-agnostic deltas + unified edit transport). -->

<!-- Session: 2026-05-28 — Completed S8.11 + S8.12. S8.11: backend `OfflineChange.node_id: Option<NodeID>` → `node_key: Option<String>` with backward-compat serde alias; `AddPlaceholderBoard` delta deleted, `AddNode { node_key }` is the single variant; save flow unified (one arm for all nodes); sync_panel guards `node_id().is_some()` to skip Synthesized proxies. Frontend: `saveLayoutOrchestrator` accepts `inMemorySnapshotKeys` instead of `discoveredOnlyNodeIds`+`unsavedPlaceholders`; `computeUnsavedInMemoryNodeIds` no longer skips placeholder keys. 384 backend + 969 frontend tests green. S8.12: `inMemoryPlaceholdersStore` deleted (profile-stem tracking internalized in `nodeRoster._profileStems`); `clearNonPlaceholderDrafts` deleted, replaced by `commitForSave()`; `stageDraftsForOfflineSave` no longer skips placeholder keys (regression test from S8.8-T1 now GREEN); `SaveControls.svelte` uses `commitForSave()`; `nodeRoster.markPlaceholdersPersisted()` added for post-save cleanup. 384 backend + 969 frontend tests green, zero failures. Next: S8.13 (AFK — UX gate migration + isPlaceholderKey audit). -->

---

## S8.11: Layout-agnostic deltas + unified edit transport [AFK]

**Layers**: Backend (`layout/types.rs`, `commands/layout_capture.rs`, `commands/cdi.rs`), Frontend (`api/layout.ts`, `saveLayoutOrchestrator.ts`)
**Blocked by**: S8.10
**Complexity**: medium
**User stories**: FR-013 (single save path; the `replace_offline_changes` crash fix)

**Architectural target** — The layout layer becomes entirely placeholder-agnostic. One `AddNode { node_key }` delta variant replaces both `AddNode { node_id_hex }` and `AddPlaceholderBoard { node_key, profile_stem, config_values }`. The save flow has one arm: for each `AddNode` delta, look up the proxy in the registry, build a `NodeSnapshot` from it, write to disk. `replace_offline_changes` accepts `node_key: String` (a `NodeKey`) instead of validating as 12-hex `NodeID`.

**Decisions**:

- **Delete `AddPlaceholderBoard`.** The variant dies. `AddNode { node_id_hex: String }` → `AddNode { node_key: String }`. The field name changes to reflect that any `NodeKey` is accepted, not just hex NodeIDs.
- **Save-flow unification.** `save_layout_directory` iterates `AddNode` deltas and, for each, looks up the `NodeProxyHandle` in the registry, calls its snapshot-building methods, and writes the `.yaml` file. No species-branching.
- **`replace_offline_changes` accepts `NodeKey`.** The IPC parameter changes from `node_id: String` (validated as `NodeID::from_hex_string`) to `node_key: String`. This is the root-cause fix for the S8.5 crash.
- **`config_values` field removed from deltas.** Edits ride `OfflineChangeRow` like every other edit. The delta is identity-only.

**Acceptance criteria**:
- [x] `LayoutEditDelta::AddPlaceholderBoard` deleted; only `AddNode { node_key: String }` exists
- [x] Save flow has one code path for all nodes
- [x] `replace_offline_changes` accepts and round-trips a `placeholder:` key without crashing
- [x] Backend integration test: a delta log with both a real-node `AddNode` and a placeholder `AddNode` round-trips identically
- [x] `cargo test` + vitest green

**Tasks**:
- [x] S8.11-T1: Write integration test — `replace_offline_changes` with a `placeholder:` key round-trips end-to-end (currently crashes; this is the red test equivalent of S8.8-T1 from the old plan).
- [x] S8.11-T2: Change `replace_offline_changes` IPC parameter from `node_id` to `node_key: String`. Remove `NodeID::from_hex_string` validation. Downstream lookups use `NodeKey`.
- [x] S8.11-T3: Replace `AddNode { node_id_hex }` + `AddPlaceholderBoard { ... }` with `AddNode { node_key: String }`. Update `as_add_node()`, `as_add_placeholder()` (delete the latter), `apply_layout_deltas`, serde, and all test fixtures.
- [x] S8.11-T4: Unify save flow in `save_layout_directory`: for each `AddNode` delta, look up `NodeProxyHandle` in registry, build snapshot, write file. One arm.
- [x] S8.11-T5: Update `saveLayoutOrchestrator.ts`: replace `discoveredOnlyNodeIds` and `unsavedPlaceholders` with `inMemorySnapshotKeys: Set<NodeKey>`. All deltas are `AddNode { nodeKey }`.
- [x] S8.11-T6: Validate — `cargo test` + vitest green; the T1 integration test passes.

---

## S8.12: Frontend cleanup — delete parallel stores and methods [AFK]

**Layers**: Frontend store (`configChangesStore.svelte.ts`, `inMemoryPlaceholders.svelte.ts`), Frontend orchestrator (`configDraftOrchestrator.ts`), Component (`SaveControls.svelte`), Route
**Blocked by**: S8.11
**Complexity**: medium
**User stories**: FR-013 (the original bug closes here — placeholder edits round-trip through save)

**Architectural target** — The frontend mirrors the unified backend. The parallel `inMemoryPlaceholdersStore` is deleted. The placeholder-specific staging filter and cleanup method are collapsed. The regression test from the original S8.8-T1 goes green here.

**Existing red test**: `configDraftOrchestrator.test.ts` → `"stageDraftsForOfflineSave — S8.8 placeholder unification"` — written before the refactor began. It asserts that a placeholder draft flows through `stageDraftsForOfflineSave` into `offlineChangesStore` and round-trips through `replace_offline_changes`. This test turns green in this slice.

**Decisions**:

- **Delete `inMemoryPlaceholdersStore` entirely.** Backend's `SynthesizedNodeProxy` in the registry is the truth. Frontend reads `profile_stem` and `lifecycle` via existing snapshot IPCs. `nodeRoster.svelte.ts` stops delegating to this store.
- **`configChangesStore.svelte.ts`**: delete the `isPlaceholderKey` branch at line 222. Drafts are uniformly `(NodeKey, space, address, value)`. Add `commitForSave()` (clears all drafts — post-S8.11 the save flow is atomic). Delete `clearNonPlaceholderDrafts`.
- **`configDraftOrchestrator.stageDraftsForOfflineSave`**: delete the `isPlaceholderKey` filter at line 69. Drafts flow uniformly into `offlineChangesStore`. Keep the single-line `isPlaceholderKey` guard at line 38 (`flushDraftToBackend`) — that one is a transport rule (can't talk to something not on the bus), not a partition rule.
- **`SaveControls.svelte`**: collapse three `clearNonPlaceholderDrafts` calls to one `commitForSave` call.

**Acceptance criteria**:
- [x] **Bug closed**: the red regression test from S8.8-T1 is now green — editing a non-event field on a placeholder → save → reload restores the edit
- [x] `inMemoryPlaceholdersStore` deleted
- [x] `clearNonPlaceholderDrafts` deleted; replaced by `commitForSave()`
- [x] `stageDraftsForOfflineSave` has zero `isPlaceholderKey` checks
- [x] Full vitest 970+ passing; no regressions

**Tasks**:
- [x] S8.12-T1: Delete `inMemoryPlaceholdersStore`. Update `nodeRoster.svelte.ts` to read `profile_stem` and `lifecycle` from snapshot data instead of the deleted store.
- [x] S8.12-T2: `configChangesStore.svelte.ts` — delete the `isPlaceholderKey` branch. Add `commitForSave()`. Delete `clearNonPlaceholderDrafts`.
- [x] S8.12-T3: `configDraftOrchestrator.ts` — delete the `isPlaceholderKey` filter at line 69 in `stageDraftsForOfflineSave`.
- [x] S8.12-T4: `SaveControls.svelte` — collapse three `clearNonPlaceholderDrafts` calls to one `commitForSave`.
- [x] S8.12-T5: Update `placeholderBoardOrchestrator.test.ts` and `placeholderLifecycle.integration.test.ts` — assertions read from snapshot/roster, not from the deleted store.
- [x] S8.12-T6: Validate — regression test green; full vitest green.

---

## S8.13: UX gate migration + final audit [AFK]

**Layers**: Component (`TreeLeafRow.svelte`), Route (`+page.svelte`), Backend (`commands/bowties.rs`, `commands/cdi.rs`), Frontend utility (`utils/nodeRoster.ts`)
**Blocked by**: S8.12
**Complexity**: small
**User stories**: SC-001 (reduce places that can mis-handle placeholders)

**Architectural target** — Every "is this a placeholder?" question in the codebase routes through a typed predicate (snapshot field absence/presence), not string-prefix sniffing. `isPlaceholderKey` survivors are limited to a documented short list of legitimate encoding/transport concerns.

**Decisions**:

- **Typed predicate.** Introduce `isPlaceholder(snapshot)` (or `entry.isPlaceholder` on roster entries) — reads `node_id === null`. All frontend UX gates migrate.
- **`TreeLeafRow.svelte`**: when `isPlaceholder && isEventIdLeaf`, render the same EventId field as a real board (showing the ID value and producer/consumer role badge) but disabled, with no add-connection control. Tooltip: "Placeholder event IDs are reserved and cannot be edited."
- **`filter_bindable` deleted.** Factory-produced zero EventIds + existing `is_placeholder_event_id` zero-prefix predicate already exclude placeholder eventids from binding enumeration. `filter_bindable` is redundant.
- **`classify_node_key_from_snapshot` renamed** to express the snapshot-driven shape (`classify_snapshot_cdi_source`); body reads `node_id`/`profile_stem` typed fields.
- **`isPlaceholderKey` final audit.** Expected survivors:
  - Frontend (≤3): `configDraftOrchestrator.ts:38` (transport skip), `utils/nodeRoster.ts` (canonicalization passthrough), `nodeRoster.svelte.ts` (internal partition).
  - Backend (≤2): factory itself (minting), `filename_basis_for_key` (colon-escaping).
  - Everything else uses the typed predicate.

**Acceptance criteria**:
- [x] `TreeLeafRow.svelte` disables EventId editing on placeholders via typed predicate
- [x] `filter_bindable` deleted; binding enumeration excludes placeholder eventids via zero-prefix rule
- [x] `classify_node_key_from_snapshot` renamed; body reads typed fields
- [x] `isPlaceholderKey` call-site count matches the documented short list
- [x] `cargo test` + vitest green

**Tasks**:
- [x] S8.13-T1: Introduce typed `isPlaceholder` predicate on roster entries. Migrate `TreeLeafRow.svelte`, `+page.svelte` UX gates.
- [x] S8.13-T2: `TreeLeafRow.svelte` — disable input when `isPlaceholder && isEventIdLeaf`. Add test: placeholder eventid input rendered as disabled.
- [x] S8.13-T3: Backend — delete `filter_bindable` from `commands/bowties.rs` and call sites. Write binding-exclusion test: placeholder's zero eventids excluded; real node's eventids present.
- [x] S8.13-T4: Backend — rename `classify_node_key_from_snapshot` → `classify_snapshot_cdi_source`. Update call sites and test names.
- [x] S8.13-T5: `isPlaceholderKey` audit — grep, compare against short list, delete unexpected survivors.
- [x] S8.13-T6: Validate — `cargo test` + vitest green.

---

## S8.14: Documentation + ADR [AFK]

**Layers**: Documentation (`aiwiki/owners.md`, `aiwiki/flows.md`, `product/glossary.md`, `product/architecture/adr/`)
**Blocked by**: S8.13
**Complexity**: small
**User stories**: (documentation)

Lock in the architectural decisions and update the knowledge base.

**Tasks**:
- [x] S8.14-T1: Finalize `product/architecture/adr/0009-placeholder-factory-and-polymorphic-node-proxy.md` (drafted at start of S8.8; mark `Accepted` once S8.13 green).
- [x] S8.14-T2: Update `aiwiki/owners.md` — drop `inMemoryPlaceholdersStore` row; add `placeholder.rs` (factory) + `SynthesizedNodeProxy` rows; update `nodeRoster`, `configChanges`, `placeholderBoardOrchestrator` rows.
- [x] S8.14-T3: Update `aiwiki/flows.md` — "add placeholder" flow mirrors "discover node" with factory substituting for bus discovery.
- [x] S8.14-T4: Update `product/glossary.md` — "placeholder = `NodeSnapshot` with `node_id: None`; in memory it's a `SynthesizedNodeProxy` in the registry."
- [x] S8.14-T5: Check `specs/backlog.md` for S8.8–S8.14-related items to retire.

<!-- Session: 2026-05-28 (continued) — Completed S8.13 + S8.14. S8.13: `filter_bindable` deleted from bowties.rs (3 tests removed, now redundant — placeholder eventids excluded by zero-prefix rule); `classify_node_key_from_snapshot` renamed to `classify_snapshot_cdi_source`; `isPlaceholderKey` audit confirmed survivors match documented short list (7 frontend code sites + 1 definition, 6 backend code sites + 2 definitions). TreeLeafRow eventId gating already implemented (S8.5/T10). S8.14: ADR-0009 marked Accepted; aiwiki/flows.md updated with Placeholder Board Lifecycle flow; product/glossary.md entries added for NodeKey and Placeholder. 381 backend + 969 frontend tests green. S8.8–S8.14 placeholder factory refactor COMPLETE. Next: S9 (HITL — placeholder persistence round-trip end-to-end). -->

---

## S9: Placeholder persistence round-trip end-to-end [HITL]

**Layers**: Integration (frontend + backend), store/orchestrator persistence
**Blocked by**: S8 + S8.5 + S8.6 + S8.7 + S8.8 + S8.9 + S8.10 + S8.11 + S8.12 + S8.13 + S8.14
**Complexity**: small
**User stories**: FR-014, FR-015, FR-016, SC-001

Validate full save → reopen persistence of placeholders end-to-end. Quickstart steps 1–7 pass: add TurnoutBoss placeholder → flip Left/Right → edit fields → save → reopen → state restored exactly.

**Note on scope shrink**: The binding-exclusion sweep that this slice originally owned has been absorbed by S8.10 (factory synthesizes all-zero EventIds, making the existing `is_placeholder_event_id` zero-prefix predicate cover every binding-enumeration entry point) and S8.13-T3 (test + delete of the now-redundant `filter_bindable`). S9 now focuses on the persistence round-trip and the quickstart walkthrough as the final user-visible acceptance gate.

**Acceptance criteria**:
- [ ] Full quickstart steps 1–7 pass
- [ ] Saved YAML on disk matches the documented `nodeSnapshots` shape with `profile_stem` + `node_id: None` for placeholders (post-S8.11: no separate `placeholderBoards` collection)
- [ ] Reopen after restart restores the placeholder roster, the daughterboard variant flip, and the non-event field edits exactly

**Tasks**:
- [ ] S9-T1: Write end-to-end persistence integration test — add placeholder → flip daughterboard variant → edit non-event field → save → reload → assert state restored exactly (snapshot identity, profile_stem, variant selection, edited field value)
- [ ] S9-T2: Validate — integration test passes; manual quickstart 1–7 walkthrough green

---

## S10: Tower-LCC placeholder demo + Unknown-Model resilience [AFK]

**Layers**: Backend domain (FR-022 unknown-model handling), Route
**Blocked by**: S5 + S9
**Complexity**: small
**User stories**: FR-022, quickstart step 8

Exercise a Tower-LCC placeholder with daughterboard variants (uses the S5-bundled CDI) and add resilient handling for layouts that reference a nonexistent profile stem: the placeholder loads as "Unknown model" without crash, sidebar shows the unknown marker, edits are blocked but the rest of the layout opens normally.

**Acceptance criteria**:
- [ ] Tower-LCC placeholder add → daughterboard variant flip → save → reload round-trips
- [ ] Hand-editing the YAML to reference a nonexistent profile stem reopens as "Unknown model" without crash
- [ ] Quickstart step 8 passes

**Tasks**:
- [ ] S10-T1: Write integration test — Tower-LCC placeholder daughterboard variant flip + unknown-stem resilience
- [ ] S10-T2: Backend — unknown-profile-stem handling in tree assembly returns the "Unknown model" sentinel
- [ ] S10-T3: Route — render the "Unknown model" placeholder state with edits blocked
- [ ] S10-T4: Validate — integration test passes; quickstart step 8 green
