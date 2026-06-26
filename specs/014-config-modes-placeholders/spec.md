# Feature Specification: Configuration Modes & Placeholder Boards

**Feature Branch**: `014-config-modes-placeholders`
**Created**: 2026-05-24
**Status**: Draft
**Input**: Generalize the structure profile schema with a first-class Configuration Mode concept (subsuming Tower-LCC daughterboards and TurnoutBoss Left/Right) and add support for placeholder boards in layout files so users can preview, configure, and learn any bundled board profile without owning the hardware. Validation case: assemble and ship the TurnoutBoss profile (issue #8) and migrate Tower-LCC to the unified schema without behavioral regression. Designed so a future spec can reconcile placeholder boards with real discovered nodes.

**Background reading**: See [proposal-original.md](./proposal-original.md) for the original problem statement, motivating examples (TurnoutBoss Left/Right; Tower-LCC daughterboards), and a deeper walk-through of the schema gaps in the v1 profile contract. This spec subsumes that proposal and pivots the "Profile Explorer" idea into "placeholder boards in a layout."

## Clarifications

### Session 2026-05-24

- Q: When overlays from multiple Configuration Modes or rules apply to the same target, what is the deterministic composition order? → A: Declaration order in the profile YAML; later overlays win per-field (last-write-wins per target).
- Q: What should happen when a selector's stored value doesn't match any variant declared in the profile (e.g. unknown enum byte, or a variant removed by a profile update)? → A: No overlay applied; UI surfaces a clear "unrecognized variant value" warning and prompts the user to pick a declared variant. The stored byte is preserved until the user changes it.
- Q: What happens when a Configuration Mode selector field is itself made irrelevant by another rule? → A: Out of scope (YAGNI). No current target profile (TurnoutBoss, Tower-LCC) requires this. Revisit if a real profile ever needs it.
- Q: What concrete format should the layout-scoped placeholder identifier take (FR-018)? → A: `placeholder:<uuidv4>` string (e.g. `placeholder:7c9e6b1a-4a8f-4d2e-9d3a-1f5b2c8e9d10`). Prefix makes it trivially distinct from LCC node IDs and gives a one-line "is this a placeholder?" check.
- Q: What stable board-model identity should each placeholder record (FR-019)? → A: The profile filename stem (e.g. `RR-CirKits_Tower-LCC`) from the existing `{Manufacturer}_{Model}.profile.yaml` convention. YAGNI: no new `profileId` or `boardModelKey` field; the filename is already the de-facto stable identity the loader keys on, and a rename would itself constitute a model identity change.
- YAGNI pass (2026-05-24): dropped User Story 4 (covered by FR-018/FR-019 tests), dropped FR-020 (untestable design-advice restatement of FR-018+FR-019), dropped FR-021 (cross-release profile change handling — no placeholder layouts exist yet so the first migration is at least one release away), softened FR-022 to "MUST NOT crash" (full unknown-model UX deferred until layout sharing across versions is real), dropped SC-001's arbitrary 30-second bound, and removed three edge-case bullets that only restated FR-015/FR-016/FR-017a.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Profile author ships a board whose CDI re-shapes based on a configuration enum (Priority: P1)

A profile author for a board like TurnoutBoss can express, in a single structure profile, that:

- A configuration enum (e.g. *How this TurnoutBoss is used on your layout* with values `Left` / `Right`) acts as a **Configuration Mode selector**.
- Each selector value (`Left`, `Right`) defines an **overlay**: which sections become irrelevant, which event leaves flip between Producer and Consumer, and which fields elsewhere in the CDI become required or hidden.
- The same selector mechanism also expresses Tower-LCC's existing "plug a daughterboard into connector A" model.

**Why this priority**: This is the foundational schema work. Without it, neither the TurnoutBoss profile nor the placeholder-board UX can be correct, and Tower-LCC remains a one-off shape.

**Independent Test**: Load the assembled TurnoutBoss `.profile.yaml` against the new schema. The schema validator accepts it, lists `Left` and `Right` as the two declared variants of one Configuration Mode, and reports the per-leaf event-role overrides and cross-segment relevance rules without error. No UI required for this test.

**Acceptance Scenarios**:

1. **Given** a profile that declares a Configuration Mode whose selector is an enum field path with two variants, **When** the schema validator runs, **Then** the profile is accepted and both variants' overlays are reported as parsed.
2. **Given** a profile with a relevance rule whose controlling field lives in a different CDI segment from the affected target, **When** the schema validator runs, **Then** the rule is accepted (the v1 sibling-only restriction is removed).
3. **Given** a profile that assigns Producer to some eventid leaves and Consumer to other eventid leaves within the same CDI group, **When** the schema validator runs, **Then** the per-leaf role overrides are accepted (the v1 group-uniform restriction is removed).
4. **Given** the bundled TurnoutBoss profile assembled from `profile-extractions/turnout-boss/`, **When** loaded through the production profile loader, **Then** it loads without warnings or errors.

---

### User Story 2 - User explores and pre-configures a board they don't own yet, inside a layout (Priority: P1)

A user evaluating an LCC board for their layout can:

- Open (or create) a layout file.
- Add a **placeholder board** of any bundled board model.
- Walk the same guided-configuration screens they would see for a real node of that model — including Configuration Mode selectors that visibly re-shape the configuration surface (e.g. flipping Left↔Right on TurnoutBoss dims Detector 3, re-labels the In-Motion event direction, etc.).
- Edit non-event-ID fields freely (enums, integers, strings, names) and have those edits persisted as part of the layout.
- See every eventid leaf clearly marked as a **placeholder eventid**, with a visible explanation that it does not correspond to a real node and cannot be bound to other nodes.

**Why this priority**: This is the user-visible payoff for the schema work and the answer to "can I see what this board does before I buy one?" It also covers the profile-author preview need without a separate Explorer screen, and persists work the user does so it isn't thrown away when they switch boards or close the app.

**Independent Test**: A user with no LCC hardware adds a TurnoutBoss placeholder to a new layout, flips the Left/Right selector, observes the configuration surface re-shape, edits a few non-event fields, closes and reopens the app, and sees the placeholder board (with its edits and selector state) restored exactly.

**Acceptance Scenarios**:

1. **Given** an empty layout, **When** the user chooses "add a board" and picks a bundled board model, **Then** a placeholder board of that model appears in the layout with the model's default configuration values applied.
2. **Given** a placeholder TurnoutBoss in a layout configured as `Left`, **When** the user changes the selector to `Right`, **Then** the configuration surface re-shapes (Detector 3 becomes irrelevant, the Occupancy event leaves flip Producer↔Consumer per the profile, dependent relevance rules update) without requiring a node connection.
3. **Given** a placeholder board with edits to non-event-ID fields, **When** the user saves and reopens the layout, **Then** the edits are restored exactly.
4. **Given** a placeholder board, **When** the user inspects any eventid leaf, **Then** the UI clearly marks it as a placeholder eventid and prevents any "use this event" action that would otherwise bind a real node to it.
5. **Given** a layout containing both real (discovered) nodes and placeholder boards, **When** any other feature enumerates "nodes available for event binding," **Then** placeholder boards are excluded from that enumeration.

---

### User Story 3 - Tower-LCC daughterboard configuration is correctly expressed under the unified schema (Priority: P1)

Tower-LCC's connector + daughterboard configuration model is re-expressed under the unified Configuration Mode schema as a single source of truth. The bespoke `connectorSlots` / `daughterboardReferences` / `connectorConstraintVariants` shape is removed entirely.

**Why this priority**: Tower-LCC is the only board currently using the connector/daughterboard shape. Daughterboard support has not yet shipped in a released build, so this is a clean re-expression, not a backwards-compatibility migration. It is P1 because the unified schema is incomplete until Tower-LCC's case fits cleanly under it.

**Independent Test**: Open a Tower-LCC node (real or placeholder), install each supported daughterboard variant on each connector, and confirm the configuration surface (relevant groups, event roles, structural constraints) is correct for every variant per the existing daughterboard intent.

**Acceptance Scenarios**:

1. **Given** the Tower-LCC profile re-expressed under the unified Configuration Mode schema, **When** the schema validator runs, **Then** the profile is accepted with no remaining references to the legacy `connectorSlots`, `daughterboardReferences`, or `connectorConstraintVariants` fields.
2. **Given** a Tower-LCC node (real or placeholder), **When** the user installs each supported daughterboard variant on each connector, **Then** the resulting configuration surface (relevance, event roles, structural constraints) matches the daughterboard intent documented in the current Tower-LCC profile.

---

### Edge Cases

- A profile declares two Configuration Modes whose overlays both touch the same field/leaf. Overlays are applied in profile-YAML declaration order; the later overlay wins per affected field (last-write-wins per target).
- A Configuration Mode selector's stored value does not match any variant declared in the profile (unknown enum byte, or a variant removed by a profile update). No overlay is applied; the UI surfaces a clear "unrecognized variant value" indication and prompts the user to pick a declared variant. The stored byte is preserved as-is.

## Requirements *(mandatory)*

### Functional Requirements

**Schema generalization**

- **FR-001**: The structure profile schema MUST support a first-class **Configuration Mode** concept consisting of a selector and one or more named variants, where each variant declares overlays for event roles, relevance, and (where applicable) structural constraints.
- **FR-002**: Configuration Mode selectors MUST support at least two selector kinds: (a) a CDI enum field referenced by full path, and (b) a structural slot whose variant identifies an installed sub-board (the Tower-LCC daughterboard case).
- **FR-003**: The schema MUST allow event-role assignment at the eventid-leaf level, not only the enclosing group level (removing the v1 group-uniform restriction).
- **FR-004**: The schema MUST allow relevance rules whose controlling field references any CDI field by full path, including fields in a different segment from the affected target (removing the v1 sibling-only restriction).
- **FR-005**: The schema MUST allow a relevance rule's affected target to be a group, a single replication instance of a group, or a leaf field/event.
- **FR-006**: When overlays from multiple Configuration Modes or multiple rules apply to the same target, the runtime MUST apply them in the order they are declared in the profile YAML, with later overlays overriding earlier ones on a per-field basis (last-write-wins per target). This order MUST be documented in the schema reference.
- **FR-007**: When a Configuration Mode selector's stored value does not match any variant declared in the profile (e.g. an unknown enum byte, or a variant removed by a profile update), the runtime MUST apply no overlay for that selector, MUST preserve the stored value as-is, and the UI MUST surface a clear "unrecognized variant value" indication prompting the user to pick a declared variant.
- **FR-008**: Tower-LCC's current `connectorSlots`, `daughterboardReferences`, and `connectorConstraintVariants` MUST be re-expressed under the unified Configuration Mode schema and the legacy field names MUST be removed from both the profile and the schema. Daughterboard support has not yet shipped in a released build, so no backwards-compatibility aliases are required.

**TurnoutBoss validation case**

- **FR-009**: The Bowties build MUST bundle a TurnoutBoss structure profile assembled from `profile-extractions/turnout-boss/` that exercises Configuration Modes (Left/Right), per-leaf event-role overrides (Occupancy, Turnout Control, Signal Controls), and all seven relevance rules (R001–R007) including the cross-segment cases.
- **FR-010**: The assembled TurnoutBoss profile MUST load through the production profile loader without warnings or errors and MUST render correctly in both `Left` and `Right` modes in the guided-configuration UI.

**Placeholder boards in layouts**

- **FR-011**: Users MUST be able to add a **placeholder board** of any bundled board model to a layout, without any LCC node being connected.
- **FR-012**: A placeholder board MUST present the same guided-configuration screens as a real node of the same model, including interactive Configuration Mode selectors that re-shape the configuration surface.
- **FR-013**: Non-event-ID fields on a placeholder board (enums, integers, strings, names, including the Configuration Mode selector fields themselves) MUST be editable and persisted as part of the layout.
- **FR-014**: Every eventid leaf on a placeholder board MUST render identically to a real board's EventId field — showing the event ID value and the producer/consumer role badge — but MUST be disabled (not editable) and MUST NOT show the add-connection control. The goal is that a placeholder looks as much like a real board as possible while making clear that its event IDs are not yet bound.
- **FR-015**: Placeholder eventids MUST NOT appear in any "bind / link / connect to this event" enumeration anywhere in the app, and any action that would otherwise create such a binding MUST refuse with a clear message when a placeholder eventid is selected as source or target.
- **FR-016**: The system MUST allow multiple placeholder boards of the same board model in a single layout, each independently configured. Naming is implicit: the sidebar shows `"{manufacturer} {model}"` when the placeholder has no User Name set, and switches to the User Name once the user edits that leaf in the bundled CDI's Identification segment — the same fallback rule a freshly-flashed real node follows before its SNIP User Name has been written. (HITL decision, 2026-05-25: no separate placeholder display-name field.)
- **FR-017**: Layouts containing placeholder boards MUST save and reload such that every persisted configuration value (including the CDI User Name leaf) and Configuration Mode selection is restored exactly.
- **FR-017a**: Users MUST be able to delete a placeholder board from a layout. Deletion MUST remove the placeholder and its persisted configuration from the layout, MUST be confirmable to guard against accidental loss of configuration work, and MUST NOT affect other boards (real or placeholder) in the layout.

**Forward-compatible placeholder identity (design-for, no UX)**

- **FR-018**: Each placeholder board MUST carry a stable layout-scoped identifier of the form `placeholder:<uuidv4>` (e.g. `placeholder:7c9e6b1a-4a8f-4d2e-9d3a-1f5b2c8e9d10`), which is trivially distinguishable from real LCC node ID format, so a future reconciliation feature can replace the identifier with a real node ID without touching the rest of the placeholder's configuration.
- **FR-019**: Each placeholder board MUST record the profile filename stem of the bundled board model it was created from (e.g. `RR-CirKits_Tower-LCC`, from the existing `{Manufacturer}_{Model}.profile.yaml` convention). This is the same key the profile loader uses, so it survives any change to the profile's content. Profile renames are treated as a model identity change and are out of scope for compatibility.

**Compatibility and resilience**

- **FR-022**: Loading a layout that references a board model the current Bowties build does not bundle MUST NOT crash or discard the stored placeholder data. Richer "unknown model" UX is deferred until cross-version layout sharing becomes a real concern.
- **FR-023**: After Tower-LCC is re-expressed under the unified schema, the Tower-LCC guided-configuration behavior (which groups are relevant, which event roles apply, which structural constraints are enforced) MUST match the daughterboard intent documented in the current Tower-LCC profile for every supported connector + daughterboard combination. Tests covering that behavior MUST be updated to target the unified schema and MUST pass.

### Key Entities

- **Structure Profile**: The YAML contract describing a board model's CDI-driven configuration surface. After this spec, includes Configuration Modes alongside the existing event-role and relevance-rule sections.
- **Configuration Mode**: A first-class profile element naming a selector (CDI enum field path or structural slot) and a set of variants, each variant declaring overlays for event roles, relevance, and structural constraints.
- **Configuration Mode Variant**: One legal value of a Configuration Mode selector (e.g. `Left`, `Right`, `BOD4-CP`), with its overlay payload.
- **Overlay**: A bundle of event-role overrides, relevance rule additions, and structural constraints contributed by a single variant when that variant is selected.
- **Placeholder Board**: A layout entry that represents an instance of a bundled board model with no associated real LCC node. Owns its own configuration state, its own Configuration Mode selections, its own placeholder eventids, and a stable layout-scoped identifier. Has no separate display name — the sidebar label falls back to `"{manufacturer} {model}"` until the user edits the CDI's User Name leaf, exactly as a real node behaves pre-naming.
- **Placeholder Eventid**: An eventid leaf on a placeholder board. Visually marked, non-bindable, excluded from cross-node event binding flows.
- **Layout**: The existing user-facing project file. Extended in this spec to hold placeholder boards alongside real-node references.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user with no LCC hardware can add a TurnoutBoss placeholder to a fresh layout, flip the Left/Right selector, and observe the configuration surface reshape (relevance, event roles) visibly and without errors.
- **SC-002**: The bundled TurnoutBoss profile loads, validates, and renders correctly in both `Left` and `Right` modes with zero schema warnings.
- **SC-003**: After Tower-LCC is re-expressed under the unified schema, every supported connector + daughterboard combination produces the configuration surface (relevance, event roles, structural constraints) documented in the current Tower-LCC profile, with zero remaining references to the legacy `connectorSlots` / `daughterboardReferences` / `connectorConstraintVariants` fields in the schema or shipped profile.
- **SC-004**: A layout containing at least one placeholder board can be saved, the app fully closed and reopened, and every configuration value (including the CDI User Name leaf, which the sidebar surfaces as the placeholder's display name) and Configuration Mode selection is restored exactly (zero loss).
- **SC-005**: Across the entire app, no flow that binds one node's event to another node ever offers a placeholder eventid as a valid source or target.
- **SC-006**: A profile author can preview their work on a board by adding it as a placeholder to a scratch layout, with no separate Explorer screen and no hardware, fully demonstrating Configuration Modes, relevance, and event-role overlays.

## Assumptions

- The existing layout file format can be extended to hold placeholder boards alongside real-node references without a breaking on-disk format change. `/speckit.plan` will confirm the extension approach.
- Bundling each board's CDI XML alongside its profile under `app/src-tauri/profiles/` is acceptable. The proposal flagged this as an open question; the assumed answer is yes (bundle at build time) because the placeholder UX needs the CDI available without a network round-trip.
- The TurnoutBoss profile filename will follow the existing `{Manufacturer}_{Model}.profile.yaml` convention used by `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml`. Exact loader matching rules (spaces, casing) will be confirmed in `/speckit.plan`.
- Reconciliation of a placeholder with a real discovered node is **explicitly a future spec** and is not built here. Only the data model is shaped to support it (FR-018, FR-019).
- Per the proposal's non-goals, this spec does not introduce a hardware planner, status modules, a profile-authoring UI beyond placeholder preview, or recipe execution from placeholders.

## Dependencies

- The current v1 schema at [specs/008-guided-configuration/contracts/profile-yaml-schema.json](../008-guided-configuration/contracts/profile-yaml-schema.json) is the baseline being extended.
- The `profile-7-assemble` skill at [.github/skills/profile-7-assemble/SKILL.md](../../.github/skills/profile-7-assemble/SKILL.md) will need updating once the schema lands.
- Tower-LCC's existing profile at [app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml](../../app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml) is the migration source.
- TurnoutBoss source artifacts are staged at `profile-extractions/turnout-boss/` (CDI XML, manual PDF, phase-1 extraction outputs); Tower-LCC's source CDI XML is not currently checked in and should be backfilled during this work.
- The future hardware planner spec ([specs/proposals/app-ux-vision/planner-proposal.md](../proposals/app-ux-vision/planner-proposal.md)) is a downstream consumer of this work, not a dependency.
