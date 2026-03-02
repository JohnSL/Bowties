# Feature Specification: Profile Content Extraction Skills

**Feature Branch**: `008-guided-configuration`  
**Created**: 2026-02-28  
**Status**: Draft  
**Input**: Phase 1 of Guided Configuration plan (`specs/007-guided-configuration/plans.md`)

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Extract Event Role Classifications (Priority: P1)

A profile author has a node's PDF manual and its CDI XML and needs to determine which event groups are producers and which are consumers. The CDI XML has no explicit producer/consumer distinction, but the manual unambiguously describes each event group's role. The author invokes the `profile-1-event-roles` skill (`.github/skills/profile-1-event-roles/`) to produce a complete mapping from CDI event group path to role (Producer or Consumer), with citations to the manual section confirming each classification.

**Why this priority**: Event role classification is the highest-value structural intelligence the profile provides. It directly replaces the runtime heuristic (which is sometimes wrong) with an authoritative, manually-verified answer. Without accurate roles, conditional relevance rules (Story 2) and the downstream Phase 2 UI improvements cannot function correctly.

**Independent Test**: Run the event role extraction prompt against the Tower-LCC manual and CDI XML. Verify the output contains a role classification for every `<eventid>` group, spot-check 10 entries against the manual text, and confirm 95%+ accuracy.

**Acceptance Scenarios**:

1. **Given** a profile author provides a node's PDF manual and CDI XML to the event role extraction prompt, **When** the prompt completes, **Then** the output contains a role mapping (Producer or Consumer) for every event group in the CDI that contains `<eventid>` fields.

2. **Given** event role extraction output is produced, **When** each mapping entry is reviewed, **Then** it includes a citation referencing the manual section or passage that confirms the classification.

3. **Given** the CDI XML contains two sibling groups with the same name (e.g., two `<group name="Event">` groups under each Line), **When** extraction completes, **Then** the output distinguishes between them using document order or child field name patterns (e.g., Command/Action vs. Indicator/Upon-this-action) so they can be addressed separately.

4. **Given** event role extraction output references a CDI path, **When** that path is checked against the CDI XML, **Then** the path exists in the XML. Any referenced path that does not exist in the XML is flagged as an error.

---

### User Story 2 — Extract Conditional Relevance Rules (Priority: P1)

A profile author needs to identify configuration relationships where entire sections become irrelevant based on other field values — for example, consumer event slots that don't apply when the output function is disabled. The author invokes the `profile-2-relevance-rules` skill (`.github/skills/profile-2-relevance-rules/`), providing the CDI XML, PDF manual, and the event-roles.json output from the prior step as shared context. The skill produces a list of rules, each specifying the section affected, the controlling field, the values that make it irrelevant, and a human-readable explanation.

**Why this priority**: Conditional relevance directly impacts user experience — it prevents users from wasting time configuring sections that have no effect. Tied with event roles as the core structural intelligence of the profile.

**Independent Test**: Run the conditional relevance extraction prompt against the Tower-LCC manual and CDI XML. Verify it produces rules for at least the known cases: consumer events irrelevant when Output Function = No Function, producer events irrelevant when Input Function = Disabled, Track Speed irrelevant when Source uses Variable's Events, and Delay irrelevant for Steady output modes.

**Acceptance Scenarios**:

1. **Given** a profile author provides a node's PDF manual and CDI XML to the relevance extraction prompt, **When** the prompt completes, **Then** each rule specifies: the affected section (by CDI path pattern), the controlling field (by CDI path), the set of enum values that make the section irrelevant, and a human-readable explanation.

2. **Given** relevance extraction output is produced, **When** a rule references a controlling field, **Then** that field exists in the CDI XML and its cited enum values are valid values defined in the CDI's `<map>` for that field.

3. **Given** the Tower-LCC manual describes that "Consumer Event" groups under each Line are only meaningful when an Output Function is configured, **When** extraction completes, **Then** at least one rule captures this relationship with the correct controlling field ("Output Function") and irrelevant value (0 = "No Function").

---

### User Story 3 — Extract Section and Field Descriptions (Priority: P2)

A profile author needs to populate rich descriptions for every segment, group, and leaf field in the CDI. The CDI's own `<description>` elements are often terse, cryptic, or entirely absent. The manual contains detailed explanations for each section and field. The author invokes the `profile-3-section-descriptions` and `profile-4-field-descriptions` skills (`.github/skills/`), providing prior extraction outputs as shared context, to produce structured description content for all levels of the hierarchy.

**Why this priority**: Descriptions are the primary content of the companion panel (Phase 3). Without them, the panel has nothing to show. Lower than P1 because the companion panel is a later phase — event roles and relevance rules have immediate impact in Phase 2.

**Independent Test**: Run section and field description extraction prompts against the Tower-LCC manual and CDI XML. Verify descriptions are produced for all 5 segments, all group levels, and all leaf fields — particularly fields that lack CDI descriptions entirely.

**Acceptance Scenarios**:

1. **Given** a profile author provides a node's PDF manual and CDI XML to the section description extraction prompt, **When** the prompt completes, **Then** the output contains a 1–3 sentence purpose statement for every segment and group in the CDI.

2. **Given** a profile author provides a node's PDF manual and CDI XML to the field description extraction prompt, **When** the prompt completes, **Then** the output contains a clear description for every leaf field in the CDI, replacing or supplementing terse CDI descriptions.

3. **Given** an enum field has multiple options (e.g., Output Function with Steady, Pulse, Blink, and Sample modes), **When** field description extraction completes, **Then** each enum option value has a one-line description explaining what it does, optionally grouped by category (e.g., Steady modes, Pulse modes).

4. **Given** a numeric field (e.g., Delay Time), **When** field description extraction completes, **Then** the output includes units, valid range, and typical values where the manual provides this information.

5. **Given** field descriptions reference CDI paths, **When** those paths are validated against the CDI XML, **Then** all referenced paths exist. Any discrepancies are flagged.

---

### User Story 4 — Extract Usage Recipes (Priority: P3)

A profile author identifies common configuration tasks described in the manual — such as "configure a push button input," "set up a blinking LED output," or "wire an occupancy detector" — and invokes the `profile-5-recipes` skill (`.github/skills/profile-5-recipes/`), providing all prior extraction outputs as shared context. The skill produces structured recipes. Each recipe names the task, identifies the applicable CDI scope, and lists the ordered field settings with explanations of why each is needed.

**Why this priority**: Recipes are a value-add for the companion panel (Phase 3) but are not required for the core structural intelligence (roles, relevance) or basic descriptions. They enhance the user experience but can be added incrementally.

**Independent Test**: Run the recipe extraction prompt against the Tower-LCC manual and CDI XML. Verify at least 3 recipes are produced for Port I/O and at least 1 for Conditionals, each with ordered steps referencing valid CDI fields.

**Acceptance Scenarios**:

1. **Given** a profile author provides a node's PDF manual and CDI XML to the recipe extraction prompt, **When** the prompt completes, **Then** each recipe includes: a name, the applicable CDI scope (segment/group), and an ordered list of steps.

2. **Given** a recipe step references a field, **When** the step is reviewed, **Then** it identifies the field by CDI path, the value to set, and a brief explanation of why that setting is needed for the task.

3. **Given** the Tower-LCC manual describes common configurations for Port I/O, **When** recipe extraction completes, **Then** at least 3 distinct recipes are produced (e.g., Push Button, LED Output, Occupancy Sensor).

4. **Given** recipe extraction output references field paths and enum values, **When** those are validated against the CDI XML, **Then** all paths exist and all enum values are valid.

---

### User Story 5 — Validate Extraction Output (Priority: P2)

A profile author has completed one or more extractions and needs to verify that the output is structurally correct before using it to build a profile file. The author invokes the `profile-6-validate` skill (`.github/skills/profile-6-validate/`) which cross-references every CDI path and enum value cited in the extraction output against the actual CDI XML, reporting any mismatches, missing references, or uncovered sections.

**Why this priority**: Validation is essential to prevent downstream errors — an incorrect CDI path in a profile would cause silent failures or incorrect UI behavior. Tied with descriptions because both are needed to produce a usable profile.

**Independent Test**: Take the extraction output from Stories 1–4 and run the validation workflow. Verify that all CDI paths referenced exist in the XML, all enum values are valid, and coverage gaps (CDI sections with no extraction output) are identified.

**Acceptance Scenarios**:

1. **Given** extraction output referencing CDI paths, **When** validation is run against the CDI XML, **Then** every referenced path is confirmed to exist in the XML, and any path that does not exist is reported as an error with the specific extraction entry and the invalid path.

2. **Given** extraction output referencing enum values for a field, **When** validation is run, **Then** every cited enum value is confirmed to be a valid entry in the CDI's `<map>` for that field, and any invalid value is reported.

3. **Given** a complete CDI XML and extraction outputs from all prompts, **When** a coverage check is run, **Then** a report is produced listing: CDI sections covered by extraction output, CDI sections with no corresponding extraction output, and the overall coverage percentage.

4. **Given** an extraction output with zero validation errors and 100% coverage, **When** the validation completes, **Then** the output is marked as ready for profile population.

---

### Edge Cases

- What happens when the PDF manual covers multiple firmware versions with differing CDI structures? The extraction prompts should use the provided CDI XML as the authoritative structure. Descriptions from the manual that refer to features not present in the given CDI XML version are excluded, and a note is added indicating the manual may cover additional firmware features.
- What happens when a CDI field has no corresponding section in the PDF manual? The field is listed as "no manual documentation found" in the extraction output and flagged in the validation coverage report. The profile author can provide a description manually or leave it empty (the profile schema allows missing descriptions).
- What happens when the manual uses different terminology than the CDI XML for the same concept? The extraction prompts are instructed to map manual terminology to CDI element names, using the CDI XML as the canonical reference. If the mapping is ambiguous, the extraction output notes the discrepancy for manual review.
- What happens when multiple prompts produce conflicting information about the same CDI element? Each prompt is scoped to a specific concern (roles, relevance, descriptions, recipes) and they do not overlap in output type. If a description prompt and a recipe prompt both describe the same field, they are complementary — descriptions explain what it does, recipes explain how to use it in a task context.
- What happens when the extraction prompt produces output for a CDI path that exists in the XML but has a different structure than expected (e.g., the prompt says it's an enum but the CDI defines it as an integer)? The validation step checks structural consistency — element type mismatches are reported as warnings for manual review.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The extraction process MUST accept two inputs for each node type: the node's PDF manual and its CDI XML file.

- **FR-002**: The extraction process MUST produce five independent categories of structured output via five individual Copilot skills: event role classifications (`profile-1-event-roles`), conditional relevance rules (`profile-2-relevance-rules`), section descriptions (`profile-3-section-descriptions`), field and option descriptions (`profile-4-field-descriptions`), and usage recipes (`profile-5-recipes`).

- **FR-003**: Each extraction category MUST be producible independently — extracting event roles MUST NOT require running the description extraction first, and vice versa. However, skills SHOULD accept shared context from prior extraction outputs to produce better results. This allows iterative refinement of individual skills without re-running the full set.

- **FR-004**: Event role extraction output MUST map every `<eventid>` group in the CDI to either Producer or Consumer, with a citation to the manual passage confirming each classification.

- **FR-005**: Conditional relevance extraction output MUST specify: the affected CDI section (by path pattern), the controlling field (by CDI path), the set of values that make the section irrelevant, and a human-readable explanation suitable for display to end users.

- **FR-006**: Section description extraction output MUST provide a 1–3 sentence purpose statement for every segment and every group in the CDI hierarchy.

- **FR-007**: Field description extraction output MUST provide a clear description for every leaf field in the CDI. For enum fields, it MUST include a per-option description. For numeric fields, it MUST include units and valid range where available in the manual.

- **FR-008**: Recipe extraction output MUST include: a recipe name, applicable CDI scope, and an ordered list of steps — each step identifying a field by CDI path, the value to set, and a rationale.

- **FR-009**: All extraction outputs MUST use CDI element names and paths as the canonical identifiers, not memory addresses or manual page numbers. CDI paths are stable across firmware versions; memory addresses may shift.

- **FR-010**: The validation workflow MUST cross-reference every CDI path cited in any extraction output against the actual CDI XML and report any path that does not exist in the XML.

- **FR-011**: The validation workflow MUST cross-reference every enum value cited in extraction output against the CDI's `<map>` definitions and report any invalid values.

- **FR-012**: The validation workflow MUST produce a coverage report showing which CDI sections have extraction output and which do not, including an overall coverage percentage.

- **FR-013**: The extraction prompts MUST produce output in a consistent, machine-parseable format (structured markdown or JSON) with explicit format instructions so that outputs are reproducible across runs.

- **FR-014**: The extraction skills MUST be implemented as GitHub Copilot skills (`.github/skills/{name}/SKILL.md`) so they are permanently discoverable and reusable across sessions. Skills MUST be usable with any capable large language model and work within Copilot Chat when the PDF and XML are attached.

- **FR-014a**: A workflow guide (`docs/technical/profile-extraction-guide.md`) MUST document the recommended sequencing of skills and show how shared context files flow between extraction steps.

- **FR-015**: A reference extraction MUST be completed for the Tower-LCC node type (using `TowerLCC-manual-f.pdf` and Tower-LCC CDI XML) to serve as both a validation of the skill set and the seed content for the Phase 2 profile.

### Key Entities

- **CDI XML**: The Configuration Description Information XML document that describes a node's configuration structure — segments, groups, fields, enum maps. This is the authoritative source for structure and element names.
- **Node Manual**: The PDF documentation from the hardware manufacturer describing the node's configuration options, wiring diagrams, and operating modes. This is the authoritative source for meaning, intent, and guidance.
- **Extraction Skill**: A GitHub Copilot skill (`.github/skills/{name}/SKILL.md`) designed to take a PDF manual, CDI XML, and optionally prior extraction outputs as inputs and produce a specific category of structured output (roles, relevance, descriptions, field details, or recipes). Skills are permanently stored in the repository and auto-discoverable by Copilot.
- **Extraction Output**: The structured, machine-parseable JSON result of running one extraction skill. Contains CDI path references, classifications or descriptions, and manual citations. Outputs are saved as files and serve as shared context for subsequent skills.
- **Validation Report**: The result of cross-referencing extraction outputs against the CDI XML, listing path errors, enum value errors, and coverage gaps.
- **Profile Content**: The aggregate of all validated extraction outputs for a node type, ready to populate a profile file (defined in Phase 2).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A profile author can produce a complete set of extraction outputs (all five categories) for a new node type within 2 hours, given the node's PDF manual and CDI XML.

- **SC-002**: Event role extraction achieves 95% or higher accuracy when spot-checked against the manual for a minimum of 10 randomly selected event groups.

- **SC-003**: Validation confirms that 100% of CDI paths cited in extraction outputs exist in the CDI XML — zero false path references.

- **SC-004**: Validation confirms that 100% of enum values cited in extraction outputs are valid entries in the CDI's enum maps — zero invalid value references.

- **SC-005**: The coverage report for the Tower-LCC reference extraction shows extraction output for at least 90% of all CDI segments, groups, and leaf fields.

- **SC-006**: Two different operators running the same extraction prompts on the same inputs produce substantially similar outputs — key facts (roles, relevance rules, enum descriptions) agree on at least 90% of entries.

- **SC-007**: At least 3 usage recipes are successfully extracted from the Tower-LCC manual for Port I/O, each with valid CDI field references confirmed by validation.

## Assumptions

1. The Tower-LCC PDF manual (`TowerLCC-manual-f.pdf`) and CDI XML (`Tower LCC CDI.xml` or `Tower-LCC.xml`) are available and accessible to the profile author during extraction. These are the reference inputs for the first extraction run.
2. The extraction prompts are used interactively by a profile author within Copilot Chat or a similar LLM interface — this is not a fully automated pipeline. The author reviews outputs and may re-run prompts with adjustments.
3. The CDI XML is well-formed and follows the OpenLCB CDI schema. The extraction process does not need to handle malformed XML.
4. The PDF manual is a text-based PDF (not scanned images) and is readable by the LLM when attached to a chat session.
5. The validation workflow can be performed manually (by visual inspection or simple scripting) in Phase 1. A fully automated validation tool is a potential future enhancement but is not required for this phase.
6. Prompt refinement is iterative — the first run may not achieve target accuracy, and 2–3 rounds of prompt tuning per category are expected before the outputs are production-ready.

## Scope Boundaries

### In Scope

- Extraction skill design and implementation for all five categories (`.github/skills/profile-{1..5}-*/`)
- Validation skill (`profile-6-validate`)
- Workflow guide (`docs/technical/profile-extraction-guide.md`)
- Reference extraction run against Tower-LCC (as test case for the skills)
- Output format specification (JSON schemas) embedded in each skill

### Out of Scope

- Profile file schema definition (Phase 2)
- Profile loading or matching in the application (Phase 2)
- Any changes to the Bowties application UI (Phases 2 and 3)
- Automated extraction pipeline or CI integration
- Extraction for node types other than Tower-LCC (additional nodes use the same skills but are separate efforts)
