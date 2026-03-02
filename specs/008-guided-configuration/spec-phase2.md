# Feature Specification: Profile Schema, Event Roles, and Conditional Relevance

**Feature Branch**: `008-guided-configuration`
**Created**: 2026-03-01
**Status**: Draft
**Input**: Phase 2 of Guided Configuration plan (`specs/guided-configuration-plans.md`)

## Clarifications

### Session 2026-03-01

- Q: What serialization format should profile files use? → A: YAML (with a JSON Schema-compatible schema for machine validation). Chosen for consistency with existing Phase 1 extraction outputs (`event-roles.json` aside, `section-descriptions.yaml`, `field-descriptions.yaml`, `recipes.yaml`) and superior readability for community authoring. The plans.md preference for JSON is superseded by this decision.
- Q: How should same-named sibling CDI group *definitions* be addressed in a profile when index ranges and child-field patterns are both fragile? → A: Ordinal among same-named siblings using `#N` suffix notation (e.g., `Event#1`, `Event#2`), where N is the 1-based position of that group definition in CDI document order among siblings sharing the same name. Profile authors determine the mapping by inspecting the app's config view or JMRI without a profile. Human meaning is conveyed by the profile's `eventRole` and `explanation` fields, not the key itself.
- Q: Should the V1 relevance rule schema support compound conditions (multiple controlling fields), or be locked to single-field rules only? → A: The schema MUST support compound `allOf` lists from V1 (forward-compatible structure), but the V1 evaluator processes only single-field rules. Any rule containing more than one condition in its `allOf` list is silently skipped with a log warning until a future release adds multi-field evaluation. All known Tower-LCC use cases are single-field rules, so no V1 functionality is lost.
- Q: Should relevance rule evaluation be reactive to field value changes in Phase 2, or evaluated once on CDI load (deferred until write mode)? → A: Implement reactive wiring in Phase 2. Configuration write mode was merged before this phase began, so the triggering condition (user changing a controlling field value) is live. One-time evaluation on load is insufficient. Assumption 5's read-only claim is corrected accordingly.
- Q: Must the profile be ready before the configuration view first renders, or can it arrive asynchronously and update the view reactively? → A: Synchronous — the profile is resolved and applied in the Rust backend before the `get_node_config` Tauri command returns. The frontend first render always reflects profile data. Asynchronous delivery is excluded to prevent visible badge-switching or section-collapsing flashes on load.
- Post-session: Should Phase 2 and Phase 3 profile content share a single file (with optional Phase 3 fields) or be split into two separate files per node type? → A: Two separate files. A **structure profile** (`.profile.yaml`) covers Phase 2 structural intelligence (event roles + relevance rules) and is lightweight enough for hardware manufacturers or community contributors to author manually or via a CDI template scaffold. A **content profile** (`.content.yaml`) covers Phase 3 rich guidance (descriptions, recipes) and is a separate, independent, optional artifact. Each file is independently optional — having one does not require the other. This split reflects different authoring effort levels, different update triggers (CDI structure changes vs. manual prose updates), and a graduated adoption path for community contributors. Phase 2 defines and implements only the `.profile.yaml` format; the `.content.yaml` format is deferred to Phase 3. Phase 2A tooling also includes: (1) a CDI template generator script that scaffolds an empty `.profile.yaml` skeleton from any CDI XML, and (2) a `profile-7-assemble` skill that produces a `.profile.yaml` from Phase 1 extraction outputs.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Accurate Event Role Labels in the Configuration View (Priority: P1)

A model railroader opens Bowties and connects to a Tower-LCC node. In the Port I/O segment, every Line has two groups of event fields — one group for commanding the output (consumer events) and one group for reporting input state changes (producer events). Today, those groups sometimes show the wrong role label because the app guesses roles from text heuristics. With this feature, the app reads a pre-authored profile for the Tower-LCC type and applies the definitively correct role label (PRODUCER or CONSUMER) to each event group. The user can trust the labels without cross-referencing the manual.

**Why this priority**: Event role labels are already visible in the UI. Incorrect labels cause genuine confusion — a user configuring a PRODUCER event as if it were a CONSUMER one will create a non-functional layout. Fixing this with profile-sourced data is the lowest-risk, highest-confidence improvement in this phase.

**Independent Test**: Open a Tower-LCC node. Without any change to the hardware or the CDI, verify that the event groups in Port I/O show the correct PRODUCER / CONSUMER badges as specified in the Tower-LCC manual. Accurate role labeling is the pass criterion.

**Acceptance Scenarios**:

1. **Given** a Tower-LCC node is connected and its configuration is loaded, **When** the user views the Port I/O segment, **Then** each event group displays a role badge (PRODUCER or CONSUMER) matching the Tower-LCC manufacturer documentation — regardless of what the runtime protocol heuristic previously inferred.

2. **Given** a Tower-LCC node where the heuristic previously assigned the wrong role to an event group, **When** the profile is present, **Then** the profile-sourced role takes precedence and the badge displays the correct classification.

3. **Given** a node type for which no profile exists (e.g., an Async Blink module), **When** the user views its configuration, **Then** role badges continue to behave exactly as before this feature — the heuristic path is unchanged and no regression is introduced.

4. **Given** multiple physical Tower-LCC nodes are connected, **When** the user switches between them, **Then** each node applies the same Tower-LCC profile (one profile per node type, not per physical device), and all show consistent, correct role labels.

---

### User Story 2 — Irrelevant Configuration Sections Are Visually Suppressed (Priority: P1)

A model railroader is configuring Line 3 on a Tower-LCC node. Line 3 is wired as an input (detecting a block occupancy sensor), so its Output Function is "No Function." The app now recognizes — from the profile's relevance rules — that the consumer event slots ("Command this output On/Off") have no effect in this configuration. Those event slots are visually marked as not applicable. Meanwhile, the producer event slots (reporting Line 3's input state) are configured normally — they are not affected.

The Tower-LCC presents both the consumer and producer event groups under a single "Event" picker. The relevance rule applies only to the consumer subset (items in the picker that belong to the consumer event group). Those items are marked as not applicable in the picker with a muted treatment, and selecting one shows a muted explanation banner: "Not applicable — consumer events only apply when an Output Function is set." The producer event items in the same picker remain fully active and show no banner.

**Why this priority**: LCC nodes have deep, repetitive configuration trees. Irrelevant sections are a primary source of confusion for new users. Calling out which items don't apply — with a clear explanation — is the most direct usability improvement in this phase, and it uses the same profile data as Story 1.

**Independent Test**: Load a Tower-LCC node. Set Output Function on Line 1 to "No Function." Open the Event picker for Line 1 and verify that consumer event items are visually marked as not applicable, while producer event items are unaffected. Select a consumer event item and confirm the explanation banner appears. Then set Output Function to "Pulse Active Hi" and verify the banner disappears and consumer items are no longer marked.

**Acceptance Scenarios**:

1. **Given** a Tower-LCC Line where Output Function is set to "No Function" (value 0), **When** the user views that Line's configuration, **Then** the consumer event items in the Event picker are visually distinguished as not applicable (muted treatment), while producer event items are displayed normally with no such treatment.

2. **Given** a consumer event item is marked not applicable in the picker, **When** the user selects it, **Then** a muted explanation banner is shown beneath the picker containing the `explanation` text from the matching rule in the profile (e.g., *"Consumer events (Commands) that control line output state are irrelevant when no output function is configured…"*). The fields within that item remain visible and inspectable.

3. **Given** a producer event item in the same picker while Output Function is "No Function," **When** the user selects it, **Then** no explanation banner appears and the fields are fully accessible — the producer event relevance rule is independent and not triggered.

4. **Given** a Tower-LCC Line where Input Function is "Disabled" (value 0), **When** the user views that Line's Event picker, **Then** producer event items are marked not applicable, and selecting one shows the `explanation` text from the matching profile rule (e.g., *"Producer event triggers based on input state changes are irrelevant when input function is disabled…"*). Consumer event items are unaffected.

5. **Given** a configuration section where ALL items in a picker fall under a single fired relevance rule (hypothetical — not the Tower-LCC case, but a valid profile configuration), **When** the user views that section, **Then** the entire picker section collapses by default with the explanation banner beneath its header — behaving identically to a non-replicated section under the same rule.

6. **Given** a standalone (non-replicated) accordion section governed by a relevance rule, **When** the rule fires, **Then** the section collapses by default and a muted explanation banner appears beneath the header containing the `explanation` text from the matching profile rule. The user can expand it to inspect the fields. The banner remains visible in the expanded state.

7. **Given** a relevance rule governs a section and the controlling field's value changes such that the section crosses the relevance threshold, **When** the value change takes effect, **Then** the section's treatment (collapsed / marked items / explanation banner) updates reactively within a short transition (approximately 200ms).

8. **Given** a node type with no profile, **When** the user views any section of its configuration, **Then** no sections are collapsed or marked due to relevance rules — all sections and picker items remain fully visible and controllable as today.

---

### User Story 3 — Profile Loads Automatically by Node Type (Priority: P2)

A model railroader connects a Tower-LCC node they have never connected before. Bowties identifies the node's manufacturer and model from its identification data, silently loads the matching profile, and immediately presents the improved experience (correct role labels, collapsed irrelevant sections) — no setup required. The user never sees a "load profile" button or a notification about profiles; the enriched experience is just present.

**Why this priority**: Automatic, invisible profile matching is the prerequisite for Stories 1 and 2 — those features only work if the profile is found and applied. It is listed at P2 here only because it is not user-visible on its own; it is tested as an enabler story.

**Independent Test**: Connect a Tower-LCC to Bowties on a fresh session. Verify that within the time the CDI is loaded (no additional user action), the configuration view reflects the profile-sourced data. Then connect a node type that has no matching profile and verify no profile-related behavior appears.

**Acceptance Scenarios**:

1. **Given** a node that identifies itself as manufacturer "RR-CirKits" and model "Tower-LCC," **When** the user opens its configuration view for the first time, **Then** the profile is applied (event roles and relevance rules are active) without any manual action from the user.

2. **Given** the same Tower-LCC node type connected on two subsequent sessions, **When** the node's configuration is loaded, **Then** the profile is applied consistently in both sessions — no one-time setup is required.

3. **Given** a node whose manufacturer + model combination does not match any known profile, **When** the configuration is loaded, **Then** no profile-related behavior appears. The experience is identical to the pre-feature behavior for that node.

4. **Given** a profile specifies an optional firmware version range, **When** the connected node reports a firmware version outside that range, **Then** the profile's role and relevance data are still applied, but a non-blocking note is stored for potential surfacing in Phase 3's companion panel. The configuration view otherwise behaves normally.

---

### User Story 4 — Ambiguous Bowtie Entries Resolved by Profile (Priority: P2)

A model railroader opens the Bowties tab and sees a connection card for a Tower-LCC event ID. Today, because the Tower-LCC responds to the Identify Events protocol as both a producer and a consumer for the same event (it is the source on one Line and the responder on another), the CDI text heuristic cannot resolve the ambiguity. The event slot appears in the card's "Unknown role — needs clarification" section rather than in the correct Producer or Consumer column. With Phase 2, the profile declares definitively which CDI groups are producers and which are consumers. That declaration resolves the ambiguity: the slot moves into the correct column on the bowtie card, and the "Unknown role" section disappears for that entry.

**Why this priority**: The bowtie diagram is the primary way users understand their layout's wiring. Entries stranded in the "Unknown" section undermine confidence in the diagram and require manual follow-up. The profile data needed to resolve this is the same data already loaded for Stories 1 and 2 — the integration cost is low and the improvement is high-visibility.

**Independent Test**: Open the Bowties tab with a Tower-LCC node on the network. Identify a bowtie card that, without a profile, shows a Tower-LCC slot in the "Unknown role" section. With the profile present, verify that same slot now appears in the Producers or Consumers column of the card, and that the "Unknown role" section is absent for Tower-LCC entries.

**Acceptance Scenarios**:

1. **Given** a Tower-LCC node responds to Identify Events as both Producer and Consumer for the same event ID (same-node Tier 0 case), **When** the bowtie catalog is built and a Tower-LCC profile is present, **Then** the CDI group's profile-declared role is applied to resolve the ambiguity and the entry appears in the correct `producers` or `consumers` list — not in `ambiguous_entries`.

2. **Given** a Tower-LCC bowtie card that previously had entries in the "Unknown role — needs clarification" section, **When** the Phase 2 profile is active during catalog construction, **Then** those entries appear in the correct Producer or Consumer column and the "Unknown role" section is not rendered for those entries.

3. **Given** a node type with no profile that produces a same-node Tier 0 ambiguity, **When** the bowtie catalog is built, **Then** the existing heuristic pipeline (Tier 1/2) operates unchanged and entries remain in `ambiguous_entries` as before. Profile resolution does not affect nodes without a matching profile.

4. **Given** a same-node event ID where the profile declares a role for the CDI group containing the slot, **When** that declaration conflicts with what the Tier 1/2 heuristic would have inferred, **Then** the profile declaration wins and the heuristic result is discarded.

---

### User Story 5 — Profile File Format Enables Community Authoring (Priority: P3)

A manufacturer or community member who has read a node's manual wants to create a profile that enriches the Bowties configuration experience for that node type. They can write a profile file following a documented format and place it in a designated location. Bowties reads it on the next launch and applies it to matching nodes — no code change, no app rebuild required. The author can validate their profile against the documented schema before distributing it.

**Why this priority**: Without a stable, documented file format, every profile is fragile and private. A clear format enables the library of supported nodes to grow beyond what the core team can author. This is the foundation, but it has no direct user-visible effect on its own — it enables Stories 1–3 and future node types.

**Independent Test**: Author a minimal profile (just event roles for a hypothetical node) following the documented schema. Place it in the user data directory. Restart Bowties, connect a matching node, and verify the roles appear. Confirm the app ignores a malformed profile gracefully (no crash, no unexpected behavior).

**Acceptance Scenarios**:

1. **Given** a community author has written a profile file in the documented format and placed it in the designated user data directory, **When** Bowties launches, **Then** the profile is discovered and applied to any connected node matching its manufacturer + model — with no code change or app rebuild.

2. **Given** a user data profile and a built-in profile exist for the same node type, **When** the configuration is loaded, **Then** the user-placed profile takes precedence (allowing user corrections to ship a built-in profile).

3. **Given** a profile file with structural errors (missing required fields, invalid format), **When** Bowties loads it, **Then** the app ignores the malformed profile and logs a warning, but continues running normally. No node's configuration is disrupted.

4. **Given** a valid profile that omits some optional fields (e.g., contains event roles but no relevance rules), **When** the profile is applied, **Then** the available data (event roles) is applied and the missing sections silently fall back to defaults — no empty-state UI or error messages appear.

---

### Edge Cases

- What happens when two built-in profiles claim the same manufacturer + model identity? The app must detect the conflict at load time and ignore both, logging a warning. This prevents silent precedence bugs.
- What happens when both Output Function and Input Function are set to non-zero values simultaneously? The profile's relevance rules handle this explicitly — it is not app-hardcoded logic. When a Steady, Pulse, or Blink output function (values 1–8) is active, the Tower-LCC hardware prioritizes the output and the input function has no effect; the profile expresses this as a relevance rule with `irrelevantWhen: [1,2,3,4,5,6,7,8]` on the Input Function section, and the `explanation` field carries the manual-sourced text shown to the user. Sample output modes (values 9–16) are the intentional exception — they drive the output while simultaneously reading the input — so no rule fires for those values and both sections remain fully visible. Any node type with similar mutually-exclusive function fields can express the same pattern in its own profile.
- What happens when a relevance rule's controlling field has no current value (e.g., the CDI was only partially read)? The rule is treated as indeterminate and no section is collapsed or marked — defaulting to "show everything" is always safer than incorrectly hiding content.
- What happens when a CDI has two sibling groups with the same name (e.g., two "Event" groups under each Line in the Tower-LCC)? The profile must address them separately using an unambiguous identifier (such as child field name patterns or document-order index ranges). The format must be specified and tested against the Tower-LCC CDI before the schema is finalized.
- What happens when a relevance rule targets a *subset* of instances in a replicated group set that is presented as a picker — specifically, when the consumer event instances (0–5) and producer event instances (6–11) share the same picker? The instances covered by the rule are individually marked in the picker; the uncovered instances are unaffected. The rule must identify which instances it covers (e.g., by index range or by the profile's group path pattern) so the UI can apply the treatment selectively.
- What happens when the firmware version check produces a range mismatch but the profile data is actually still accurate? The note is advisory only and does not suppress any profile behavior — it is stored for potential Phase 3 surfacing, never shown as a blocking error.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST define two distinct, documented, machine-validatable YAML file formats for node profile content, each validated against its own JSON Schema-compatible schema file:
  - **Structure profile** (`.profile.yaml`, e.g., `RR-CirKits_Tower-LCC.profile.yaml`): Phase 2 scope. Contains node type identification (manufacturer + model, optional firmware version range), event role declarations per CDI path, and conditional relevance rules. Sufficient on its own to deliver full Phase 2 behavior.
  - **Content profile** (`.content.yaml`, e.g., `RR-CirKits_Tower-LCC.content.yaml`): Phase 3 scope. Contains descriptions, field guidance, enum option explanations, and recipes. Format definition is deferred to Phase 3.
  Both files are independently optional for any given node type. Phase 2 MUST define and implement only the `.profile.yaml` format and schema. Phase 2A tooling MUST also deliver: (1) a CDI template generator script that scaffolds an empty `.profile.yaml` skeleton from any CDI XML, enabling manual authoring without schema knowledge; and (2) a `profile-7-assemble` skill that assembles a completed `.profile.yaml` from Phase 1 extraction outputs.

- **FR-002**: When a node is identified by manufacturer and model, the system MUST automatically locate, load, and apply the matching profile in the Rust backend before the `get_node_tree` command returns its response to the frontend. The profile MUST be fully applied on first render — no asynchronous update or secondary command is permitted. If no profile exists for the node type, the command returns without profile data and the frontend behaves identically to pre-feature behavior.

- **FR-003**: Profile matching MUST use manufacturer and model as the matching key. If a profile also specifies a firmware version range, the match MUST succeed regardless of version match — the firmware range is advisory information only, not a gating condition for profile application.

- **FR-004**: If no profile exists for a connected node's type, the system MUST behave identically to pre-feature behavior — no empty states, no warnings, no UI elements related to profiles or guidance.

- **FR-005**: The system MUST support profile files from two sources: built-in profiles bundled with the application, and user-placed profiles in a platform-appropriate user data directory. Both `.profile.yaml` and `.content.yaml` files are discoverable from both sources. User-placed files MUST take precedence over built-in files of the same type when both match the same node type.

- **FR-006**: A profile file that cannot be parsed or is structurally invalid MUST be silently skipped. The application MUST continue loading and operating normally. A warning MUST be written to the application log identifying the invalid file.

- **FR-007**: When a profile declares an event role for a CDI group, that declaration MUST override any role inferred by the existing heuristic or runtime protocol exchange. The profile role applies to all `<eventid>` fields within that group and its replicated instances.

- **FR-008**: When no profile role declaration exists for a given event group, the existing event role classification pipeline MUST remain fully active and unchanged.

- **FR-009**: When a profile declares a relevance rule for a CDI group, and the controlling condition evaluates as irrelevant, the system MUST apply one of two behaviors depending on how that group is presented in the UI:
  - **Standalone section**: The section MUST be collapsed by default and display a muted explanation banner beneath its header. The banner text MUST be the `explanation` field from the matching profile rule verbatim — the app MUST NOT generate, paraphrase, or substitute the text.
  - **Within a picker (replicated group set)**: The items in the picker that belong to the affected group instances MUST be visually distinguished as not applicable (muted treatment). Selecting an affected item MUST display the explanation banner beneath the picker, again using the `explanation` field from the matching profile rule verbatim. Unaffected items in the same picker MUST be presented normally with no banner.
  - When ALL items in a picker are covered by the same fired relevance rule, the entire picker section MUST collapse with the explanation banner, behaving identically to a standalone section.

- **FR-009a**: The relevance rule schema MUST express conditions as an `allOf` list of one or more `{field, irrelevantWhen}` entries, enabling compound multi-field rules in the format from V1 onward. The V1 evaluator MUST process only rules whose `allOf` list contains exactly one entry (single-field rules). A rule with two or more entries in its `allOf` list MUST be silently skipped and a warning written to the application log identifying the rule and its location in the profile file. This skip MUST NOT prevent other rules in the same profile from applying.

- **FR-010**: An irrelevant standalone section MUST remain user-expandable. An irrelevant item within a picker MUST remain user-selectable. In both cases the user MAY inspect the fields inside even though they have no effect. The explanation banner MUST remain visible when the user has navigated into the irrelevant content.

- **FR-011**: When the controlling field's value changes such that a previously irrelevant section or item set becomes relevant (or vice versa), the section's collapsed/expanded state, the picker items' not-applicable treatment, and the explanation banner MUST all update reactively within a visual transition of approximately 200ms. This is a Phase 2 requirement — configuration write mode is active and users can change controlling field values during a session.

- **FR-012**: A relevance rule that references a controlling field or CDI path not found in the node's actual CDI MUST be silently ignored. Partial profile data MUST NOT prevent other profile data from applying.

- **FR-013**: A `.profile.yaml` (Phase 2 structure data) and a `.content.yaml` (Phase 3 content data) are each independently optional for any node type. The presence or absence of either file MUST NOT affect the behavior delivered by the other. A node type with only a `.profile.yaml` MUST receive full Phase 2 behavior with no missing-content UI. A node type with only a `.content.yaml` and no `.profile.yaml` MUST deliver Phase 3 companion panel content while Phase 2 behavior falls back to heuristics — this MUST NOT produce errors or missing-state UI elements.

- **FR-014**: A Tower-LCC profile MUST be authored and bundled with the application, containing at minimum: event role declarations for all event groups in the Tower-LCC CDI, and conditional relevance rules for consumer event groups (controlled by Output Function), producer event groups (controlled by Input Function), and the Delay group (controlled by Output Function).

- **FR-015**: The profile file format MUST distinguish same-named sibling CDI group *definitions* using an ordinal `#N` suffix notation (e.g., `Event#1`, `Event#2`), where N is the 1-based position of that group definition in CDI document order among siblings sharing the same name within the same parent. This notation applies to group keys in the profile schema wherever sibling name collisions exist. Groups with unique names within their parent require no suffix. The same ordinal logic MUST be applied consistently by the profile loader when matching profile keys to CDI groups.

- **FR-016**: When the bowtie catalog is built and a profile is present for a node, profile-declared event role for a CDI group MUST be applied during the same-node ambiguity resolution step of catalog construction. A slot in a group with a profile-declared role of Producer MUST be placed in `producers`; a slot with a declared role of Consumer MUST be placed in `consumers`. Neither slot MAY appear in `ambiguous_entries` when the group's role is declared in the profile.

- **FR-017**: Profile-based resolution of bowtie ambiguity MUST NOT affect nodes for which no matching profile exists. The existing Tier 0/1/2 classification pipeline MUST remain the sole mechanism for those nodes, and their `ambiguous_entries` behavior MUST be unchanged.

### Key Entities

- **Structure Profile** (`.profile.yaml`): A YAML file, one per node type, containing Phase 2 structural intelligence: node type identification (manufacturer + model, optional firmware version range), event role declarations per CDI path, and conditional relevance rules. Keyed by manufacturer + model. Machine-validated against a JSON Schema-compatible schema. Sufficient on its own to deliver full Phase 2 behavior. May be authored manually from a CDI template scaffold or assembled from Phase 1 extraction outputs via the `profile-7-assemble` skill.
- **Content Profile** (`.content.yaml`): A YAML file, one per node type, containing Phase 3 guidance content: section descriptions, field descriptions, enum option explanations, and recipes. Format defined in Phase 3. Either file may exist independently of the other.
- **Event Role Declaration**: A profile entry that classifies a CDI group containing event ID fields as either PRODUCER or CONSUMER. Applies to all event ID fields within that group and its replicated instances.
- **Relevance Rule**: A profile entry expressed as an `allOf` list of one or more `{field, irrelevantWhen}` conditions, plus a human-readable `explanation`. In V1, only single-entry `allOf` rules are evaluated; multi-entry rules are skipped with a log warning. Each condition specifies a controlling field (by CDI path, sibling within the same replicated group instance) and a set of enum index values that render the section irrelevant. Also specifies the affected CDI section (by path pattern using `#N` ordinal notation for same-named siblings).
- **Explanation Banner**: A muted UI element that communicates why a section or picker item is not applicable, displaying the profile's human-readable explanation text. Shown beneath a collapsed standalone section header, or beneath a picker when the user selects a not-applicable item. Visible whether the user has expanded the section or navigated into the irrelevant picker item.
- **Profile Source**: Either "built-in" (bundled with the app) or "user-placed" (in the platform-specific user data directory). User-placed sources take precedence over built-in for the same node type.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user opening a Tower-LCC node's configuration sees PRODUCER/CONSUMER badges on 100% of event groups that have profile-declared roles, matching the Tower-LCC manufacturer documentation — verified by spot-checking all Port I/O Lines.

- **SC-002**: With Output Function set to "No Function" on any Tower-LCC Line, the consumer event items in the Event picker are visually marked as not applicable within one rendering cycle (no perceptible delay), and an explanation banner appears when one is selected. Producer event items in the same picker are unaffected. Tested across all Lines simultaneously to confirm consistent behavior.

- **SC-003**: A user who places a valid manually-authored profile in the user data directory, then launches the app and connects a matching node, sees the profile applied within the normal CDI load time — no additional wait, no manual "load" action.

- **SC-004**: A node type with no profile produces zero visible profile-related UI elements. Verified by inspection of the full configuration view for at least two non-Tower-LCC node types.

- **SC-005**: A malformed profile file in the user data directory causes zero visible disruption — the app loads, other nodes work correctly, and the malformed file is noted in the application log.

- **SC-006**: A profile covering only event roles (no relevance rules) applies correctly — event role badges update, no relevance UI elements appear. A profile covering only relevance rules (no event roles) applies correctly — sections collapse as specified, role badges remain heuristic-driven.

- **SC-007**: The Tower-LCC profile, when loaded against the Tower-LCC CDI, produces no "rule skipped due to missing path" log warnings — all declared paths exist in the CDI.

- **SC-008**: After Phase 2, zero Tower-LCC event slots appear in the "Unknown role — needs clarification" section of any bowtie card — all same-node Tower-LCC ambiguities are resolved by the profile. Verified by opening the Bowties tab with at least one Tower-LCC node on the network and inspecting all generated cards.

## Assumptions

1. The Tower-LCC profile content (event role classifications and relevance rules) was produced and validated during Phase 1 using the profile extraction skills. This content is ready to populate the Phase 2 profile bundle.
2. The CDI XML structure for the Tower-LCC node does not change between Phase 1 extraction and Phase 2 implementation. If firmware updates change the CDI structure, the profile must be re-validated against the new CDI before shipping.
3. A node's manufacturer and model are reliably available in its identification data after CDI loading — the Phase 2 matching mechanism can depend on these fields being present and accurate.
4. The app's current event role classification pipeline (Tiers 0–2) remains in place unchanged. Profile role declarations act as a Tier -1 override; they do not replace the pipeline — nodes without profiles continue to use Tier 0–2.
5. Configuration write mode is active in Phase 2 — the app supports reading and writing node configuration values. Relevance rules are therefore evaluated reactively against the current field values as they change during a session, not just once on initial CDI load. The `TreeGroupAccordion` relevance state MUST subscribe to the controlling field's current value and update whenever it changes.
6. The profile file format version field enables future schema evolution. Phase 2 defines version "1.0". Fields unknown to the app MUST be ignored (forward compatibility), and the app MUST NOT reject a profile solely because it contains unrecognized fields.

## Scope Boundaries

### In Scope

- Structure profile file format definition and documentation (`.profile.yaml` schema + human-readable authoring guide)
- CDI template generator script (scaffolds an empty `.profile.yaml` skeleton from any CDI XML for manual authoring)
- `profile-7-assemble` skill (assembles a `.profile.yaml` from Phase 1 extraction outputs)
- Profile loading from built-in app resources and user data directory (`.profile.yaml` only in Phase 2)
- Profile matching by manufacturer + model
- Event role override: applying profile-declared roles to CDI event groups
- Conditional relevance: collapsing sections with explanation banners based on relevance rules
- Tower-LCC built-in structure profile (event roles + relevance rules)
- Firmware version range storage (stored in `.profile.yaml` metadata, advisory note logged if mismatch — companion panel surfacing is Phase 3)
- Profile-based resolution of same-node bowtie ambiguity (applying profile role declarations during bowtie catalog construction to move previously-ambiguous Tower-LCC slots into the correct Producer or Consumer column)

### Out of Scope

- Content profile file format definition (`.content.yaml`) — deferred to Phase 3
- Companion panel and rich contextual descriptions (Phase 3)
- Recipes and usage guidance notes (Phase 3)
- Field-level description display in the configuration view (Phase 3)
- Profile editor or authoring tool within the application
- Automated profile validation tooling (beyond the Phase 1 extraction skills)
- Any "profile file not found" notification or discovery wizard in the UI
- Profiles for node types other than Tower-LCC (same format, separate effort)
