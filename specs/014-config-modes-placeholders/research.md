# Phase 0 Research: Configuration Modes & Placeholder Boards

All NEEDS CLARIFICATION items from the Technical Context have been resolved here. Open spec questions that the `/speckit.clarify` pass already pinned down are referenced, not re-litigated.

## R1 — Unified selector representation

**Decision**: Model `selector` as a tagged enum with two variants in serialization: `enumField` (CDI enum field path + per-byte variants) and `structuralSlot` (slot id + installed-variant id). Both feed the same overlay-application code path.

**Rationale**:
- TurnoutBoss (`Layout Configuration Setup/How this TurnoutBoss is used on your layout.`) is enum-field driven.
- Tower-LCC daughterboards are structural-slot driven (`connector-a` → `BOD4-CP`).
- A single sum type keeps `annotate_tree` polymorphism cheap (one match arm) and prevents two parallel evaluator code paths from drifting.
- Aligns with the spec's FR-002 ("at least two selector kinds").

**Alternatives considered**:
- Two sibling top-level fields (`enumModes` + `structuralModes`). Rejected: forces duplicate overlay/composition code and complicates FR-006 (declaration-order composition across both).
- Generic "any CDI field" selector. Rejected as YAGNI — no current profile needs non-enum, non-slot selection.

## R2 — Overlay composition order (FR-006)

**Decision**: Apply overlays in profile-YAML declaration order across all Configuration Modes; later overlays win per affected target on a per-field basis (last-write-wins). Within a single variant, the overlay's own `eventRoles` / `relevanceRules` / `structuralConstraints` arrays compose in their listed order.

**Rationale**: Already locked by spec clarification 2026-05-24 (Q1). Declaration order is the simplest, most reviewable rule; profile authors can re-read the file top-to-bottom and predict the result. Implementation is a simple fold over the active overlays in source order.

**Alternatives considered**: Explicit numeric `priority`. Rejected — adds a tunable surface that no current profile needs and invites bug reports about "why isn't my higher priority winning".

## R3 — Unrecognized variant value handling (FR-007)

**Decision**: When a selector's stored value matches no declared variant, apply no overlay for that selector, preserve the stored byte verbatim, and surface a single "unrecognized variant value" warning in the UI that links to a variant picker. The annotation report includes the warning so backend tests can assert it.

**Rationale**: Locked by spec clarification (Q2). Implementation seam: `annotate_tree` returns an `AnnotationReport` already; we add a `unknown_variant_warnings: Vec<UnknownVariantWarning>` field carrying selector path + stored value.

## R4 — Tower-LCC migration strategy

**Decision**: Inline rewrite. Remove `connectorSlots`, `connectorConstraintVariants`, `daughterboardReferences`, `carrierOverrides` from both the shipped Tower-LCC `.profile.yaml` and from `profile::types::StructureProfile`. Re-express each connector as one `ConfigurationMode` whose selector is a `structuralSlot` (slot id from today's `connectorSlots[].slotId`) and whose variants are the supported daughterboards. The shared daughterboard library (`RR-CirKits.shared-daughterboards.yaml`) stays — its rules are referenced from variant overlays instead of from `carrierOverrides`.

**Rationale**:
- FR-008 explicitly forbids backwards-compatibility aliases (daughterboards haven't shipped).
- One shape means one evaluator; deleting `connector*` fields removes ~150 LoC of Tower-LCC-specific types and the parallel `build_connector_profile` path can be folded into the generic overlay applier.
- Test parity is enforced by the existing connector/daughterboard test suite — those tests get rewritten to assert against the new shape (FR-023) but the *observable behavior* they validate is unchanged.

**Alternatives considered**: Loader-level alias normalization (translate legacy v1 fields into v2 shape on read). Rejected — adds dead-code burden for a shape that has zero installed users.

## R5 — Placeholder identity (FR-018, FR-019)

**Decision**:
- **Layout-scoped id**: string of the form `placeholder:<uuidv4>` (e.g. `placeholder:7c9e6b1a-4a8f-4d2e-9d3a-1f5b2c8e9d10`). Generated frontend-side at "Add board" time using `crypto.randomUUID()` (Tauri webview is Chromium-based; `randomUUID()` is available). Backend validates the `placeholder:` prefix + UUID v4 shape on load and on every delta accepting an id.
- **Board-model key**: profile filename stem (e.g. `RR-CirKits_Tower-LCC`, `Mustangpeak-Engineering_TurnoutBoss`), matching the existing `{Manufacturer}_{Model}.profile.yaml` loader convention. No new `profileId` field is introduced.

**Rationale**: Both locked by spec clarifications (Q4, Q5). The `placeholder:` prefix gives the spec's required "trivially distinguishable from a real LCC node ID" check (`s.startsWith("placeholder:")`) without forcing every consumer to parse UUID structure.

**Alternatives considered**: Numeric/auto-increment id (rejected — collides across layouts and complicates a future reconciliation spec); embedding board model into the id (rejected — couples identity to model rename).

## R6 — Bundled CDI XML strategy

**Decision**: Bundle each placeholder-capable board's CDI XML alongside its `.profile.yaml` under `app/src-tauri/profiles/` as `{Manufacturer}_{Model}.cdi.xml`. New backend command `load_bundled_cdi(profile_stem)` resolves the stem to the bundled file and returns the parsed CDI in the same shape the live-node CDI fetch returns. The existing CDI annotation pipeline runs unchanged.

**Rationale**:
- Spec Assumption #2 explicitly accepts bundling at build time.
- Avoids a "no CDI yet, load asynchronously over the network" lifecycle for a use case (placeholders) that is fundamentally local.
- Same search-path logic as `load_profile` (debug-build override → resource dir).
- Tower-LCC's CDI XML is currently not checked in — backfill is in scope here (proposal Pointers; FR-009 covers the TurnoutBoss bundle).

**Alternatives considered**: Load CDI on demand from `profiles/<node-name>/` source tree (rejected — that tree isn't shipped to end users; would require a second runtime data-discovery mechanism).

## R7 — Placeholder eventid representation & binding exclusion

**Decision**:
- Reserve all-zeros (`00.00.00.00.00.00.00.00`) as the canonical placeholder eventid in the in-memory CDI tree built from a bundled profile. Frontend renders them with the new `PlaceholderBoardBadge` component and an inline "placeholder eventid" tooltip.
- The backend tags every eventid leaf belonging to a placeholder board with `is_placeholder: true` in the rendered tree payload (one boolean field on the leaf annotation). Every existing cross-node binding flow already has a single seam (event enumeration) — add an `excludePlaceholders: true` parameter (default true everywhere except the placeholder board's own view) and short-circuit there.
- Any attempt to set/promote a placeholder eventid into a real binding returns a typed `Error::PlaceholderEventNotBindable` from the relevant Tauri command.

**Rationale**: One tag, one exclusion seam, zero scattered checks — satisfies FR-014 and FR-015 with the smallest possible blast radius. The all-zeros sentinel is already used elsewhere as a "no event assigned" marker and `is_placeholder_event_id` (see `commands/bowties.rs:38`) is the seam to extend.

**Alternatives considered**: Per-placeholder synthetic node IDs masking as real ones. Rejected — would require teaching every binding flow to recognize the synthetic prefix and defeats the purpose of the one-line `placeholder:` check.

## R8 — `uuid` dependency in backend (validation only)

**Decision**: Add `uuid = { version = "1", features = ["v4"] }` to `app/src-tauri/Cargo.toml` only if a Rust-side generator is needed. **Confirmed not needed**: generation happens in the webview (`crypto.randomUUID()`); the backend only validates shape via a small hand-rolled check (`placeholder:<8-4-4-4-12 hex>` with version nibble = 4). Avoids adding a dependency.

**Rationale**: Validation is trivial; pulling a 600 KB crate for a regex check fails YAGNI.

## R9 — Cross-segment / leaf-targeted relevance

**Decision**: `relevanceRules[].allOf[].field` and `relevanceRules[].affectedTarget` both become full CDI paths in `'/' + '#N'` notation (same path syntax as `eventRoles[].groupPath`). The path resolver in `profile::resolver.rs` already handles cross-segment lookups; the v1 sibling-only restriction was a *check*, not a *limit*. Removing it requires deleting the constraint and adding tests for cross-segment + leaf-targeted cases.

**Rationale**: Required for TurnoutBoss R001–R007 (per proposal). The path resolver is the natural owner — no new module needed.

## R10 — Test seams & matrix

**Decision**: Four backend test sites + two frontend test sites cover the spec:

| Seam | Tests |
|------|-------|
| `profile/types.rs` round-trip | v2 schema serializes + deserializes; v1 connector* fields are rejected (not silently ignored) |
| `profile/mod.rs::annotate_tree` | Per-leaf event-role overrides apply; overlay composition order (declaration order, last-write-wins); unknown-variant warning surfaced |
| Tower-LCC integration (existing test suite, rewritten) | Every supported connector + daughterboard combo produces identical relevance / role / structural outcomes vs. today |
| TurnoutBoss bundled load | Schema-accept; Left vs Right reshape (Detector 3 relevance, Occupancy role flip) |
| `placeholderBoardsStore` (Vitest) | Add / delete / configure / mode-select; persistence round-trip via mocked save_layout |
| Binding enumeration | Placeholder eventids are never offered as source/target; setting one returns `PlaceholderEventNotBindable` |

**Rationale**: Aligns each FR with a single owning test file (Constitution III).

---

## Open Items (none blocking)

- The Mustangpeak-Engineering filename stem: spec uses `Mustangpeak Engineering` as the manufacturer; the loader pattern is `{Manufacturer}_{Model}.profile.yaml`. Existing loader normalizes manufacturer + model via lowercase + trim (`make_profile_key`), but the *filename* preserves the SNIP-reported manufacturer. **Resolution**: bundle as `Mustangpeak-Engineering_TurnoutBoss.profile.yaml` (replace internal space with `-`) and document the rule in the v2 schema reference. Confirmed against the existing `RR-CirKits_*` precedent (hyphen, not space).
- Tower-LCC's source CDI XML is missing from the repo. **Resolution**: backfill from the connected hardware's `<cdi>` segment dump into `profiles/tower-lcc/tower-lcc_cdi.xml` and bundle into `app/src-tauri/profiles/RR-CirKits_Tower-LCC.cdi.xml` during this work (per proposal Pointers + R6).
