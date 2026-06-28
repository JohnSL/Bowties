# Phase 0 Research — Block Indicator Facility (Spec 018)

Resolutions for design questions left implicit by `spec.md`. Each item names the decision, the rationale that pinned it, and the alternatives considered (and rejected) so the next session can audit the reasoning.

The spec's own *Clarifications* session (2026-06-27) already resolved five framing-level questions; this document covers the remaining design seams that affect file layout, registry shape, and reuse points before implementation begins.

---

## R1 — Role and style registry: profile YAML + tiny in-code registry

**Decision**: Roles are declared **in code** (typed Rust enums + their state vocabularies, mirrored as TS string literal unions on the frontend) because there are exactly two of them in this slice and they are part of the type system that the rest of the code depends on for exhaustive matching. Styles are declared **in profile YAML** under each subsystem's `channelInputs` (existing key, extended) or a new `channelOutputs` key, with a `styleId` string and a `role` string the subsystem catalog binds to. The tiny in-code registry maps `styleId → realisation` (which pins it claims, which event leaves it observes, whether instances are user-creatable, and the constraint contract shape).

**Rationale**:
- This slice has 1 style per role, so an explicit role↔style registry would be premature (YAGNI). The existing profile parser already understands `channelInputs`; extending it is cheaper than introducing a new declarative format.
- Roles encode state vocabularies that *production code* must match exhaustively (e.g., `match channel.role.state { Unknown | Occupied | Clear => ... }`); declaring them in YAML would force a runtime registry lookup at every match site and lose the compile-time exhaustiveness check.
- Styles encode hardware shape that *hardware metadata* must describe (which pins, which leaves, which CDI fields); they belong in profiles where that metadata already lives.

**Alternatives rejected**:
- **Both in YAML** — loses exhaustive match safety; forces every consumer to handle "unknown role at runtime"; defeats Principle I.
- **Both in code** — duplicates hardware metadata that already lives in profiles; forces a code change every time a new board family ships.
- **Explicit registry file (`channel-roles.yaml`)** — introduces a third file with only two entries; promotes the format before the second style realising a single role actually exists (Future Considerations item).

**Promotion trigger**: When the second style realising the same role lands (e.g., a `2-led-bicolor-aspect` style realising a new `signal-aspect-3-color` role on a different node), promote role declarations into a small registry file so multi-style picker UX can branch off declarative metadata.

---

## R2 — Behavior templates: hardcoded module in `bowties-core`

**Decision**: Block Indicator is a single Rust constant in `bowties-core/src/behavior_templates/mod.rs`, exposed via a `pub fn registered_templates() -> &'static [BehaviorTemplate]`. The Tauri command `list_behavior_templates` returns the registry contents serialised to JSON; the frontend caches them in a read-only `behaviorTemplates.svelte.ts` store on app start.

**Rationale**:
- Spec 018 ships exactly one template. A declarative DSL is Future Considerations; committing to a format now would be premature (matches the spec's "commit only when there are three or four templates of varying complexity").
- A function-returning-`&'static [T]` is the smallest abstraction that already supports a multi-template future — no schema change required when the second template lands as another Rust constant, and the IPC boundary is stable.

**Alternatives rejected**:
- **Inline literal in the command handler** — ties the registry to IPC layout; later templates would force handler bloat.
- **Templates in profile YAML now** — locks in a DSL with one example. Spec explicitly defers this.

---

## R3 — Facility persistence: new `facilities.yaml` next to `bowties.yaml` and `channels.yaml`

**Decision**: Add `facilities.yaml` to the layout folder, sibling of the existing YAML files. Read/write through `bowties-core/src/layout/facilities.rs`, which uses the existing journal (ADR-0006). Schema captures `{ facilityId, templateId, name, slotBindings: { slotLabel → channelId | null } }` plus a layout-wide schema-version marker.

**Rationale**:
- Splitting facility persistence away from `bowties.yaml` keeps the bowtie file as the canonical record of *what bowties exist* (still the unit of producer/consumer wiring) and the facility file as the canonical record of *why those bowties exist* (the user's declared intent). This matches the spec's "facility persistence stores the declaration, not just the resulting event wiring."
- Reusing the journaled writer means atomic save semantics come for free (ADR-0006).
- A future YAML-defined template registry can live in the same file family without restructuring.

**Alternatives rejected**:
- **Embed inside `bowties.yaml`** — couples two concerns that have different lifecycle owners (bowties are mechanical wiring; facilities are user intent). Hardens migration if either schema evolves.
- **Embed inside `manifest.yaml`** — wrong layer (manifest is metadata about the layout folder, not layout content).
- **Per-facility files (`facilities/<id>.yaml`)** — premature for the expected scale (tens of facilities) and adds directory-walk cost to every load.

**Schema-version field**: One `schemaVersion: "1.0"` line at the top of `facilities.yaml`. Read path treats missing file as "no facilities" (Slice 1+). No back-compat code for pre-018 layouts — see FR-009.

---

## R4 — Reuse the existing bowtie creation mechanism via in-process orchestration

**Decision**: When a facility becomes Wired, `facilityOrchestrator` (frontend) calls the same Tauri commands that the existing "+ New Connection" dialog calls in `BowtieCatalogPanel.svelte` — `query_event_roles` and the catalog/metadata writers that `NewConnectionDialog` already exercises. The atomic step is "build the bowtie spec from the template + slot bindings, then invoke the existing bowtie creation pipeline." No new IPC command for "create-bowtie-for-facility"; the facility orchestrator composes existing ones.

**Rationale**:
- Spec is explicit: "spec 018 introduces no new sync, persistence, or deployment machinery." The cheapest way to honour that is to *literally call* the existing entry points, not to mirror them in new code.
- Each created bowtie carries a back-reference (`createdByFacility: <facilityId>`) on its metadata so the slot-detach pipeline knows which bowties to free on a slot-empty transition.

**Alternatives rejected**:
- **New `create_facility_bowties` backend command** — re-implements the bowtie creation mechanism; doubles the surface area to maintain when bowtie creation evolves.
- **Direct YAML writes from facility orchestrator** — bypasses the draft layer and ADR-0012.

---

## R5 — Slot-detach pipeline reuse for the Wired → Incomplete transition

**Decision**: When a slot empties, `facilityOrchestrator` looks up the bowties owned by the affected facility (`createdByFacility` back-reference), then invokes the existing per-bowtie deletion path the user already has access to (the same call backing the bowtie-card delete action). No new backend command. The "slot-detach pipeline" the spec names is the chain that already runs when a user deletes a bowtie or removes a slot from a bowtie today; the facility orchestrator just initiates that flow programmatically.

**Rationale**:
- Honours the spec's "spec 018 introduces no facility-specific undeploy mechanism."
- Reuses the well-tested existing flow; any future improvements to event-id field default-value handling on slot-detach automatically benefit facility-driven detaches.

**Alternatives rejected**:
- **Bulk facility-aware detach command** — re-implements the slot-detach pipeline; the existing per-bowtie path is already correct and tested.

---

## R6 — Style constraint contract is the existing relevance/validity mechanism, repointed to style

**Decision**: The constraint contract is the same shape Bowties already uses for daughter-board `validityRules` and node-level `relevanceRules` — `{ targetPath, constraintType: 'allowValues' | 'hideSection', allowedValues, ... }`. For BOD-family channels, the existing daughter-board `validityRules` entries are moved into the style's `constraints` block in the profile YAML (no UX change for the user). For `single-led-direct-lamp`, the style declares a new `constraints` block that, at minimum, fixes/restricts `Lamp Selection` on the claimed row to not be "Used by Mast". The Config tab's existing relevance/validity renderer is the only surface that applies these constraints; no new Config-tab UI.

**Rationale**:
- The mechanism already exists end-to-end (parser → backend resolution → frontend renderer). Repointing the *source* of constraints from daughter-board entries to style entries is a YAML refactor plus a small backend change (look up constraints by the row's claiming style instead of by the connector's selected board).
- Avoids a parallel constraint mechanism the user would have to learn.

**Alternatives rejected**:
- **Build a new constraint renderer specific to channels** — duplicates work; violates DRY; breaks the visual consistency users already have for daughter-board constraints.
- **Hardcode the lamp-selection check in the AddChannelPicker** — leaves drift detection out (FR-030's compatibility check is only one half of the story).

**Drift detection from external edits**: Explicitly deferred. The constraint contract prevents the most damaging edits from inside Bowties; out-of-band edits (JMRI, raw CDI tools) are a Future Considerations item.

---

## R7 — Channels panel: restructure existing `RailroadPanel` grouping

**Decision**: The existing `RailroadPanel` is restructured so its top-level grouping is **by node + subsystem** (hardware-organised) rather than by `channelType`. Each entry shows ownership badge, role, style, identity (pin), name, live state, and binding column. The component file `RailroadPanel.svelte` keeps its name and route wiring; its children gain new columns. This is the Channels panel the spec describes — not a separate component.

**Rationale**:
- Spec FR-031 calls out hardware-organised grouping explicitly: "displays every channel in the layout in a hardware-organised list, grouped by node and subsystem." That is a single-panel surface — splitting it into "old channel list" + "new channels panel" doubles the surface and would force the Slice 6 retirement to be a directory delete instead of a grouping change.
- Existing tests on `ChannelCard` and `RailroadPanel` continue to apply; new column tests extend rather than replace them.

**Alternatives rejected**:
- **Add a parallel `ChannelsPanel.svelte` component** — duplicates rendering logic; Slice 6 (FR-009) would become a delete-and-rewire instead of a grouping change.
- **Keep the channelType grouping and add hardware grouping as a toggle** — UX clutter; not in the spec; over-engineering.

---

## R8 — Channel ownership field and schema migration

**Decision**: The channel schema gains three new fields: `role: "block-occupancy" | "lamp-indicator"`, `style: "bod-block-detector-input" | "single-led-direct-lamp"`, `ownership: "hardware-owned" | "user-owned"`. The existing `channelType` field is **retired** in the same change set (Slice 2 introduces the new fields; Slice 6 removes the now-redundant `channelType` and the legacy per-input channel inventory entirely). The existing `hardwareRef` field stays but becomes one shape among many — for `single-led-direct-lamp` channels the binding target is a Direct Lamp Control row identifier (node + row ordinal), captured under a discriminated `binding: { kind: 'connectorInput', ... } | { kind: 'lampRow', ... }` shape.

**Rationale**:
- Per FR-001 ("the pre-018 channel-to-hardware-input shape MUST be retired; no parallel persistence shape is acceptable") and FR-009 ("no migration code; user is responsible for ensuring only post-018 layouts are opened"), there is **no compatibility path** to preserve. The cleanest implementation is to land the new shape and remove the old shape in the final cleanup slice without a migration layer.
- A discriminated `binding` field generalises to future styles (e.g., multi-pin signal aspects) without another schema change.

**Alternatives rejected**:
- **Keep `channelType` and `hardwareRef` and add new fields alongside** — accumulates dead state; future readers will not know which field to trust.
- **Migration code** — spec explicitly forbids (FR-009); single-user pre-1.0 context.

**Slice 2 transitional state**: BOD channels appear in *both* the legacy per-input inventory (kept by spec 015) and the new Channels panel grouping. That duplication is intentional and is the entire purpose of Slice 6.

---

## R9 — Lamp-row sub-picker: enumerate from CDI tree + apply style constraint filter

**Decision**: `AddChannelPicker` (the lamp-row sub-picker) enumerates candidate Direct Lamp Control rows by walking the live (and offline-rehydrated) CDI tree of every Signal LCC node, finding the `Direct Lamp Control/Lamp` repeated group, and filtering out:
1. Rows currently bound by some other user-owned `single-led-direct-lamp` channel.
2. Rows whose `Lamp Selection` field is currently "Used by Mast" (the style's constraint contract gate; rows that fail compatibility are *hidden*, per FR-030).

Identity of a lamp row is `{ nodeKey, rowOrdinal }`. The picker shows `<node name> — Direct Lamp Control — Row N` plus the row's current `Lamp Description` value if non-empty.

**Rationale**:
- Reuses the already-walked CDI tree in `nodeTreeStore`; no new backend call for enumeration.
- Filter rule (2) is the FR-030 compatibility gate; rule (1) enforces the "channel bound to at most one slot" invariant *and* the "lamp row backs at most one user-owned channel" implicit invariant.

**Alternatives rejected**:
- **Backend command that returns the list** — overengineered for data already on the client.
- **Show ineligible rows greyed-out** — spec FR-030 explicitly chose hidden over disabled-with-confirmation.

---

## R10 — Facility status derivation is a pure function, computed in `effectiveLayoutStore`

**Decision**: `facilityStatus(facility) → 'Incomplete' | 'Wired'` is a pure helper in `utils/facilityStatus.ts`. The merged facility view exposed by `effectiveLayoutStore` applies it once per facility on read; components consume the derived status. There is no stored `status` field on facility persistence — it is always derived from slot fullness.

**Rationale**:
- ADR-0004 (single-merge derivation) forbids scattered re-computation. Status is a deterministic function of slot bindings — storing it would introduce inconsistency risk.
- A pure helper is trivially unit-testable.

**Alternatives rejected**:
- **Stored `status` on persistence** — duplicates state; can drift; fights ADR-0004.

---

## R11 — Hardware-owned channel lifecycle: extend `connectorSelectionOrchestrator` Step 4

**Decision**: The existing Step 4 in `connectorSelectionOrchestrator` ("auto-create/delete channels on BOD daughter-board selection") gains the new fields (`role`, `style`, `ownership = 'hardware-owned'`) when constructing the channel objects, and the existing delete path already handles "channel disappears when board cleared/changed." The orchestrator also publishes a `channels-deleted` event the new `facilityOrchestrator` subscribes to, so any slot bound to a deleted channel becomes empty and the facility (if Wired) returns to Incomplete via the existing slot-detach pipeline (R5) in the same atomic step.

**Rationale**:
- Step 4 is the right ownership layer (orchestrator owns multi-step async workflows per `frontend-orchestration.instructions.md`). Adding a notification edge is small.
- The new `facilityOrchestrator` listens; existing orchestrators don't grow facility knowledge.

**Alternatives rejected**:
- **Make `connectorSelectionOrchestrator` aware of facilities** — violates separation of concerns; bloats the existing orchestrator with a concept it should not own.

---

## R12 — Slice 8 (FR-036, top-level tab chrome) is fully isolated

**Decision**: Slice 8 is shipped as the **last optional slice**, touches only `+page.svelte`'s top-level segmented-button-group rendering, and has no dependencies on any other slice. If it slips, the rest of the feature still ships.

**Rationale**:
- Spec explicitly calls it out as "isolated chrome refresh" and "safely deferrable to a later release."

**Alternatives rejected**:
- **Bundle with Slice 1** — couples facility delivery to a chrome decision that has nothing to do with facilities.

---

## R13 — `lcc-rs` gets zero changes

**Decision**: All facility, channel-role, channel-style, behavior-template, and slot-binding logic lives in `bowties-core` and the frontend. `lcc-rs` does not learn any of these concepts.

**Rationale**:
- Constitution Principle IV: protocol library prioritises protocol correctness over app-specific convenience. Facilities are an app-level convenience layer atop event semantics; they belong above `lcc-rs`.
- `frontend-stores.instructions.md`, `backend-tauri.instructions.md`, and `lcc-rs.instructions.md` collectively pin this separation.

**Alternatives rejected**:
- **Add a "facility" abstraction to `lcc-rs`** — would leak Bowties UI semantics into a library other LCC/OpenLCB consumers might use.

---

## Open items (none blocking)

All NEEDS CLARIFICATION items in the Technical Context resolved through R1–R13. None remain.
