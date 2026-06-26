# Plan: Guided Configuration — Node Profile System

**Created**: 2026-02-28
**Status**: Phase 1 Complete (Extraction Tooling); Phase 2-3 Pending
**Input**: Design exploration in `temp/plan-cdiConfigNavigator.prompt.md` and iterative conversation

---

## Problem

The CDI XML read from LCC nodes provides structure (segments, groups, fields, enum maps) but almost no conceptual guidance. Descriptions are terse or absent ("(C) When this event occurs"), enum options are cryptic without domain knowledge ("Sample Steady Active Hi"), and there is no way to express that entire sections are irrelevant given the current settings (consumer events ignored when a line is input-only). Meanwhile, PDF manuals for these nodes contain rich explanations of every option, typical configurations, and wiring guidance — but users must find and cross-reference those documents manually.

Additionally, the CDI XML schema has no explicit producer/consumer distinction for event ID fields. The current app derives event roles at runtime via protocol exchanges and text heuristics, but the results are sometimes ambiguous. The node manufacturer's documentation unambiguously identifies which event groups are producers and which are consumers.

The application has significant unused horizontal space (~50% of a 1080p screen). This space can host a **companion panel** — a scroll-synchronized column of contextual descriptions that mirrors the configuration hierarchy, providing section-level orientation, field-level guidance on focus, and conditional relevance notes — all driven by a per-node-type **profile file**.

## Solution Overview

Create a **node profile** system: a per-node-type data file (matched by manufacturer + model) that layers supplemental knowledge on top of the CDI-driven configuration UI. The profile provides:

1. **Event role declarations** — definitive Producer/Consumer classification for event groups
2. **Conditional relevance rules** — which sections are inapplicable given current field values
3. **Rich descriptions** — section-level purpose statements, field-level explanations, per-enum-option descriptions, usage guidance, and common-task recipes
4. **Companion panel content** — rendered in a scroll-synchronized right column alongside the configuration

The profile is authored once per node type (not per physical node) and can be shipped with the app or contributed by users/manufacturers.

## Phased Approach

Content extraction, schema definition, and UX implementation are separated into three phases. Each phase produces a usable increment: Phase 1 builds the extraction tooling; Phase 2 adds structural intelligence (roles + relevance) to the existing config UI; Phase 3 adds the companion panel with rich contextual content.

---

## Phase 1: Profile Content Extraction Tooling ✓ COMPLETE

**Goal**: Build a repeatable process that can extract all required profile information from a node's PDF manual and CDI XML, producing structured output ready to populate a profile file.

**Why first**: The profile file format (Phase 2) and companion panel content (Phase 3) both depend on knowing *what information is available* and *what structure it takes*. Starting with extraction ensures the schema is grounded in real content rather than speculation. It also creates a reusable pipeline for onboarding new node types.

### Deliverables — Completed

1. **Extraction Skills System** — A set of seven structured LLM-based skills (profile-0 through profile-6) that take a PDF manual + CDI XML and systematically extract profile content. See **[Profile Extraction Guide](../../docs/technical/profile-extraction-guide.md)** for the complete workflow.

   - **Step 0 — profile-0-manual-outline**: Reads the entire PDF once and creates a reusable index of manual sections mapped to page ranges. Output: `manual-outline.json`
   - **Step 1 — profile-1-event-roles**: Classifies every `<eventid>` group as Producer or Consumer using manual text. Output: `event-roles.json`
   - **Step 2 — profile-2-relevance-rules**: Identifies sections that become irrelevant based on field values. Output: `relevance-rules.json`
   - **Step 3 — profile-3-section-descriptions**: Creates 1–3 sentence purpose statements for every segment and group. Output: `section-descriptions.yaml`
   - **Step 4 — profile-4-field-descriptions**: Produces rich descriptions for fields and per-option descriptions for enums. Output: `field-descriptions.yaml`
   - **Step 5 — profile-5-recipes**: Identifies common configuration tasks and produces step-by-step recipes. Output: `recipes.yaml`
   - **Step 6 — profile-6-validate**: Cross-references all extraction outputs against the CDI XML. Output: `validation-report.json`

   **Design**: Each skill requires only the `manual-outline.json` file (which contains file paths and page ranges); subsequent steps' outputs are auto-discovered from the profiles directory. This allows skills to work independently and re-run cleanly when corrections are needed.

2. **Validation workflow (Integrated)** — Step 6 (`profile-6-validate`) automatically cross-references extraction outputs against the CDI XML. It verifies that every CDI path exists, every enum value is valid, and coverage meets a threshold.

3. **Tower-LCC Profile Reference** — The skills have been tested and validated against the Tower-LCC node (manual + CDI XML). The extraction outputs are ready to seed Phase 2-3 implementation.

### Approach & Implementation

- **Modal-agnostic**: Skills work with any capable LLM (Claude, GPT-4, etc.) via standard LLM interfaces.
- **Page range optimization**: Step 0 reads the entire PDF once, producing page ranges for relevant sections. Subsequent steps use the `pdf-utilities` MCP extension's `read_pdf` tool with `pageRange`, reading only needed pages.
- **Reproducibility**: Each skill produces deterministic, structured output (JSON/YAML) that can be version-controlled and validated.
- **Graceful degradation**: Individual steps can re-run independently; results are auto-discovered and integrated.

### Verification — Complete

✓ profile-0-manual-outline: Reads Tower-LCC manual, produces outline with all major sections and accurate page ranges

✓ profile-1-event-roles: Classifies event groups by producer/consumer with manual citations

✓ profile-2-relevance-rules: Identifies conditional relevance (e.g., consumer events irrelevant when Output Function = No Function)

✓ profile-3-section-descriptions: Produces purpose statements for all segments and groups in YAML format

✓ profile-4-field-descriptions: Generates descriptions for all fields with per-option descriptions for enums in YAML format

✓ profile-5-recipes: Extracts common configuration recipes (button, LED, sensor examples) in YAML format

✓ profile-6-validate: Confirms path validity and coverage against CDI XML

**Documentation**: Complete workflow documented in [Profile Extraction Guide](../../docs/technical/profile-extraction-guide.md)

---

## Phase 2: Profile Schema, Event Roles, and Conditional Relevance

**Goal**: Define the profile file format, implement profile loading, and surface event roles and conditional relevance in the existing configuration UI.

**Why second**: This phase adds structural intelligence that improves the configuration experience immediately — correct event role badges and collapsed irrelevant sections — without requiring the companion panel. It also finalizes the schema that Phase 3's content will populate.

### Deliverables

#### 2A: Profile File Schema

Define a JSON (or YAML) schema for node profile files. The schema must accommodate all five extraction concerns from Phase 1, but Phase 2 implementation focuses on event roles and relevance rules only.

**Top-level structure**:
```
{
  "profileVersion": "1.0",
  "nodeType": {
    "manufacturer": "RR-CirKits",
    "model": "Tower-LCC",
    "firmwareVersionRange": ["rev-C5", "rev-C6"]   // optional
  },
  "manualReference": {
    "title": "Tower-LCC Manual",
    "filename": "TowerLCC-manual-f.pdf"             // for attribution
  },
  "segments": { ... }                               // per-segment content
}
```

**Segment-level structure** (keyed by CDI segment name):
```
"Port I/O": {
  "description": "...",                              // Phase 3
  "recipes": [ ... ],                               // Phase 3
  "groups": {
    "Line": {                                        // matches CDI <group><name>
      "description": "...",                          // Phase 3
      "children": {
        "Event[0-5]": {                              // consumer event group
          "eventRole": "Consumer",                   // Phase 2
          "relevance": {                             // Phase 2
            "field": "Output Function",
            "irrelevantWhen": [0],                   // enum values where irrelevant
            "explanation": "Consumer events only apply when an Output Function is set."
          },
          "description": "...",                      // Phase 3
          "guidance": "...",                         // Phase 3
          "fields": { ... }                          // Phase 3
        },
        "Event[6-11]": {                             // producer event group
          "eventRole": "Producer",
          "relevance": {
            "field": "Input Function",
            "irrelevantWhen": [0],
            "explanation": "Producer events only apply when an Input Function is set."
          }
        },
        "Delay": {
          "relevance": {
            "field": "Output Function",
            "irrelevantWhen": [0],
            "explanation": "Delay values are used by pulse, blink, and sample output modes."
          }
        }
      }
    }
  }
}
```

**Design decisions for the schema**:

- **CDI path matching**: Groups and fields are matched by CDI name, not by memory address. Names are stable across firmware versions; addresses may shift. For replicated groups with identical names (the two "Event" groups in Port I/O), use index ranges (e.g., `Event[0-5]` for the first set, `Event[6-11]` for the second) based on document order in the CDI.
- **Relevance rule expressions**: V1 supports simple rules: a single controlling field (sibling within the same replicated group instance) and a set of enum values that make the section irrelevant. Complex boolean expressions (AND/OR across multiple fields) are deferred. This covers the primary use cases (Output Function, Input Function, Source).
- **Graceful degradation**: Every property in the schema is optional except `nodeType`. A profile with only `eventRole` declarations and no descriptions is valid. A profile with only descriptions and no roles is valid. Missing properties mean "use CDI defaults / show as today."
- **File naming**: `{manufacturer}_{model}.profile.json` (e.g., `RR-CirKits_Tower-LCC.profile.json`). Stored in a `profiles/` directory within the app's data or bundled resources.

#### 2B: Profile Loading and Matching

- On node discovery, after CDI is parsed, look up a matching profile by `manufacturer` + `model` from the node's identification block.
- Profile is loaded once per node type (not per physical node) and cached alongside the CDI cache.
- If no profile exists, the app behaves exactly as today — no empty states, no warnings.
- If the profile declares a `firmwareVersionRange` and the node's software version falls outside it, a subtle note is stored (surfaced in Phase 3's companion panel).

**Implementation scope**:
- Rust: Add profile loading to the Tauri backend (read JSON from bundled resources or user data directory, deserialize, cache by manufacturer+model key).
- TypeScript: Add a `NodeProfile` type mirroring the schema. Expose via a Tauri command (`get_node_profile`) or include in the existing `NodeConfigTree` response.

#### 2C: Event Role Override from Profile

Currently, `LeafConfigNode.eventRole` is classified by the Identify Events protocol exchange (Tier 0) and CDI text heuristic (Tiers 1-2) from spec 006. When a profile exists and declares `eventRole` for a group, the profile's declaration should take precedence over the heuristic — it's authored by someone who read the manual and is always correct.

**Behavior**:
- Profile `eventRole` is **Tier -1** — highest priority, applied before Tier 0 protocol results.
- If the profile declares a role for a group, all `eventid` fields within that group (and its replicated instances) inherit that role.
- If the profile and protocol disagree, the profile wins. (The protocol answer is node-level, not element-level; the profile is element-level and more specific.)
- If no profile exists or the profile doesn't cover a particular group, the existing Tier 0/1/2 pipeline is unchanged.

**Implementation scope**:
- Rust: After CDI parsing and event role classification, apply profile overrides. This is a post-processing step in the config tree builder.
- TypeScript: No changes — `eventRole` is already rendered by `TreeLeafRow`. The values just become more accurate.

#### 2D: Conditional Relevance in the Config UI

When a profile declares a relevance rule for a group, and the controlling field's current value matches the `irrelevantWhen` set, the group's section in the config UI should visually communicate that it's not applicable.

**UX behavior** (per earlier design conversation):
- The section header remains visible with a collapse chevron.
- A muted explanation bar appears below the header: *"Not applicable — [explanation from profile]"*
- The section is collapsed by default when irrelevant; the user can expand it if curious.
- When the controlling field's value changes to make the section relevant again, the section opens and the explanation bar disappears (animated, ~200ms).
- Muted visual treatment (reduced opacity on the header text) reinforces the inapplicable state.

**Implementation scope**:
- TypeScript: `TreeGroupAccordion` needs to accept relevance rules, observe the controlling field's value from the config tree, and conditionally render the collapsed/explained state.
- New component or section within `TreeGroupAccordion`: `RelevanceBar` — renders the explanation text with the muted styling.
- Reactivity: When a field value changes (future write mode, or after a config refresh), relevance state updates. In read-only mode, the value is stable after initial load, so this is evaluated once per render.

### Verification

- Load Tower-LCC profile → profile loads and matches the Tower-LCC node by manufacturer + model.
- Event role badges on consumer event groups show "CONSUMER" (blue); producer event groups show "PRODUCER" (green) — matching the screenshot's existing badge style but now driven by profile data rather than heuristic.
- With Output Function = "No Function" on Line 1: the consumer event section for Line 1 shows collapsed with explanation bar. Expanding it shows the fields with muted opacity.
- With Input Function = "Disabled" on Line 1: the producer event section for Line 1 shows collapsed with explanation bar.
- With Output Function = "Pulse Active Hi": consumer events section is fully visible (no relevance bar), delay section is fully visible.
- A node with no profile (e.g., "Async Blink" from the screenshot) renders identically to today — no profile-related UI elements appear.

---

## Phase 3: Companion Panel with Rich Contextual Content

**Goal**: Add a scroll-synchronized companion panel to the right of the configuration area, populated with section descriptions, field-level guidance, enum option explanations, usage notes, and common-task recipes — all driven by the profile's Phase 3 content fields.

**Why third**: This is the highest-value, highest-effort phase. It depends on the profile schema being finalized (Phase 2) and content being extracted (Phase 1). It transforms the right half of the screen from empty space into a persistent contextual knowledge layer.

### Deliverables

#### 3A: Populate Profile with Rich Content

Using the extraction tooling from Phase 1, populate the Tower-LCC profile with all Phase 3 content fields:

- **Segment descriptions**: 1-3 sentence purpose statement for each of the 5 segments (Node Power Monitor, Port I/O, Conditionals, Track Receiver, Track Transmitter).
- **Group descriptions**: Purpose and physical mapping for each group level (Line, Delay, Event, Logic, Variable, Action, Circuit).
- **Field descriptions**: Rich explanations for every leaf field, replacing terse CDI descriptions. Priority on fields with no CDI description at all (Output Function, Input Function, all enum ints).
- **Enum option descriptions**: One-line explanation for each enum value, grouped by category where appropriate (Steady modes, Pulse modes, Blink modes, Sample modes for Output Function).
- **Usage guidance notes**: Brief notes at the sub-group level (e.g., "For simple on/off, use Event 1 = On, Event 2 = Off").
- **Recipes**: At least 3 common-task recipes for Port I/O (Push Button, LED Output, Occupancy Sensor) and 1 for Conditionals (Simple Timer).

**Output**: Updated `RR-CirKits_Tower-LCC.profile.json` with all content fields populated.

#### 3B: Companion Panel Layout

Add a third column to the configuration layout — the companion panel — visible when a profile exists for the selected node type.

**Layout behavior**:
- Three-column layout: Sidebar (~160px) | Config (flex: 1) | Companion (flex: 1, max ~500px)
- When no profile exists: two-column layout (Sidebar | Config), config area takes full remaining width.
- On narrow screens (<1000px): companion panel hidden; config area takes full width; a toggle button (📖) in the toolbar can overlay the companion panel.
- Companion panel has a subtle background differentiation (1-2% darker or faint warm tint) and a left border separator.

**Implementation scope**:
- Modify the config layout container (in `+page.svelte` or `config/+page.svelte`) to conditionally render the companion column.
- New component: `CompanionPanel.svelte` — receives the profile data and current config tree, renders content blocks.
- CSS: three-column flex layout with responsive breakpoints.

#### 3C: Section Content Blocks

Each segment/group in the CDI that has a matching description in the profile gets a **content block** in the companion panel, vertically aligned with its corresponding config section.

**Scroll synchronization**: Sticky stacking — each companion block uses `position: sticky` to remain visible while its corresponding config section is in the viewport. When the next section scrolls up, its companion block pushes the previous one out. This ensures the user always sees guidance for the section they're currently looking at.

**Content block structure**:
```
┌─ [Section Name] ─────────────────┐
│                                   │
│  [Section description text]       │
│                                   │
│  ┌─ [Field Name] ─────────────┐  │  ← appears on field focus
│  │  [Field description]        │  │
│  │  [Typical values / hints]   │  │
│  └─────────────────────────────┘  │
│                                   │
│  [Relevance note if applicable]   │
│                                   │
└───────────────────────────────────┘
```

**Implementation scope**:
- New component: `CompanionBlock.svelte` — renders one section's description, optional field detail, and optional relevance note.
- New component: `FieldDetail.svelte` — renders field-level detail card; shown/hidden based on a `focusedFieldPath` store.
- Focus tracking: a shared Svelte store (`focusedFieldPath`) updated by `TreeLeafRow` on field focus events (`focusin`). `CompanionPanel` subscribes and reveals the appropriate `FieldDetail`.
- For enum fields with option descriptions: `FieldDetail` renders grouped option explanations when the field has focus or its dropdown is open.

#### 3D: Enum Option Descriptions in Companion Panel

When a field with an enum type receives focus, the companion panel's field detail card shows the option descriptions from the profile — grouped by category if the profile defines categories.

**Rationale**: Putting full descriptions inside the dropdown itself makes the dropdown visually heavy and hard to scan. The companion panel is the natural home for this detail — the user opens the dropdown on the left to select, and reads the explanations on the right.

**Implementation scope**:
- `FieldDetail.svelte` checks if the focused field is an enum with profile option descriptions.
- If yes, renders a categorized list of option labels with their descriptions.
- If the profile groups options (e.g., "Steady modes", "Pulse modes", "Blink modes"), render with category headers.

#### 3E: Usage Guidance Notes

At the sub-group level, the profile can provide a guidance note that appears in the companion panel's content block above any field detail.

**Behavior**:
- Always visible when the section is in view (not dependent on field focus).
- Preceded by an ℹ icon to distinguish from the section description.
- Dismissible per-session (small × button) — dismissed state stored in component-level state, not persisted.

**Implementation scope**:
- Rendered as part of `CompanionBlock.svelte` when the profile includes `guidance` for the corresponding group.

#### 3F: Recipe Chips and Recipe Panel

Segments with associated recipes show small clickable chips in the companion panel's top-level segment block.

**Behavior**:
- Recipe chips appear below the segment description: `Push Button` · `LED Output` · `Occupancy Sensor`
- Clicking a chip expands a recipe card within the companion block, showing ordered steps with field references and explanations.
- The recipe doesn't auto-fill values — it tells the user what to set and why.
- While a recipe is open, referenced fields in the config area could optionally receive a subtle highlight (stretch goal).
- Only one recipe open at a time; clicking another chip switches to it; clicking the active chip closes it.

**Implementation scope**:
- New component: `RecipeCard.svelte` — renders recipe steps.
- Recipe chips rendered in `CompanionBlock.svelte` at the segment level.
- Optional: field highlight via the `focusedFieldPath` store or a separate `highlightedPaths` store.

#### 3G: Firmware Version Mismatch Note

If the profile declares a `firmwareVersionRange` and the node's software version falls outside it, a subtle note appears in the companion panel above all section blocks:

*"This guidance was written for firmware rev-C5–C6. Your node reports rev-D1 — some details may differ."*

**Implementation scope**:
- Version comparison logic in profile loading (Rust or TypeScript).
- Rendered as a muted banner at the top of `CompanionPanel.svelte`.

### Verification

- Tower-LCC node selected, Port I/O segment: companion panel appears on the right with three content blocks (Line, Delay, Event) aligned to their config sections.
- Scroll the config area: companion blocks stick and transition smoothly.
- Click into "Delay Time" field: a field detail card appears in the Delay companion block showing range, units, and typical values.
- Click into "Output Function": companion panel shows categorized option descriptions (Steady, Pulse, Blink, Sample modes with Hi/Lo explanations).
- Click "Push Button" recipe chip: recipe card expands showing 5 steps. Config proceeds normally — no values auto-filled.
- Tab through fields with keyboard: companion panel updates field detail smoothly without jarring transitions.
- A node with no profile (e.g., "Async Blink"): no companion panel rendered; config area takes full width.
- Narrow window (<1000px): companion panel hidden; 📖 button in toolbar toggles an overlay.
- Profile with only Phase 2 data (roles + relevance, no descriptions): companion panel does not appear (no content to show), but event roles and relevance rules still apply from Phase 2.

---

## Cross-cutting Concerns

### Profile Authoring and Distribution

- **Built-in profiles**: Ship with the app for known hardware (Tower-LCC, Signal-LCC, etc.). Bundled as resources in the Tauri app.
- **User-contributed profiles**: Users can place profile files in a designated directory (platform-specific app data folder). The app scans both built-in and user directories on startup.
- **Schema documentation**: The profile JSON schema is documented with examples so that hardware manufacturers or community members can author profiles for new node types.
- **Profile version**: The `profileVersion` field allows the app to handle schema evolution. Unknown fields are ignored (forward compatibility).

### Accessibility

- Companion panel uses `role="complementary"` with `aria-label="Configuration guidance"`.
- Companion content is not in the tab order (read-only reference).
- Field-level detail cards use `aria-live="polite"` for screen reader announcement on focus.
- Relevance bars from Phase 2 are focusable and expandable via keyboard (Enter/Space).
- Event role badges use text labels ("Producer", "Consumer"), not just color.
- Collapsed relevance sections announce their state for screen readers.

### Performance

- Profile files are small (tens of KB) and loaded once per node type — no performance concern.
- Companion panel rendering is lightweight — text content with CSS sticky positioning.
- Focus tracking uses standard DOM events (`focusin`/`focusout`) — no polling.
- Scroll synchronization via CSS `position: sticky` — no JavaScript scroll listeners needed.

### Testing Strategy

- **Phase 1**: Manual validation — review extraction output against manual for accuracy. Prompt refinement is iterative.
- **Phase 2**: Unit tests for profile loading and matching. Unit tests for relevance rule evaluation. Component tests (Vitest) for `RelevanceBar` rendering states. Integration test with Tower-LCC profile + CDI.
- **Phase 3**: Component tests for `CompanionPanel`, `CompanionBlock`, `FieldDetail`, `RecipeCard`. Visual regression tests for three-column layout at different widths. Manual testing with real hardware for scroll sync behavior.

---

## Dependencies and Sequencing

```
Phase 1 (Extraction Tooling)
  │
  ├──→ Prompt A (Event Roles)     ──→ Phase 2C (Role Override)
  ├──→ Prompt B (Relevance Rules) ──→ Phase 2D (Relevance UI)
  │
  │    Phase 2A (Schema) ←── informed by Phase 1 outputs
  │    Phase 2B (Loading) ←── depends on 2A
  │    Phase 2C (Roles)   ←── depends on 2B + Prompt A output
  │    Phase 2D (Relevance) ←── depends on 2B + Prompt B output
  │
  ├──→ Prompt C (Section Descriptions) ──→ Phase 3A (Content Population)
  ├──→ Prompt D (Field Descriptions)   ──→ Phase 3A
  └──→ Prompt E (Recipes)              ──→ Phase 3A
       │
       Phase 3A (Content) ←── depends on 2A schema + Prompts C/D/E
       Phase 3B (Layout)  ←── independent of content, can start with 2A
       Phase 3C-F (Components) ←── depends on 3A + 3B
       Phase 3G (Version Note) ←── depends on 2B
```

**Parallel opportunities**:
- Phase 2A (schema design) can begin in parallel with Phase 1 extraction, then refine based on outputs.
- Phase 3B (companion panel layout) can begin as soon as Phase 2A schema is stable, before Phase 3A content is complete — using placeholder content.
- Prompts A and B feed Phase 2; Prompts C, D, E feed Phase 3. All prompts can be developed and run in parallel.

---

## Open Questions

1. **Profile file format**: JSON vs. YAML? JSON is more natural for TypeScript consumption and has better schema tooling (JSON Schema). YAML is more readable for human authoring. Recommendation: JSON with JSON Schema for validation, plus good documentation with examples.

2. **Replicated group disambiguation**: The Tower-LCC CDI has two sibling `<group name="Event">` elements within each Line — one for consumer events (Command + Action), one for producer events (Upon this action + Indicator). The profile needs to address them separately. Index-based addressing (`Event[0-5]` / `Event[6-11]`) works but is fragile if firmware reorders groups. Alternative: match by child field name patterns ("Command" → consumer group, "Indicator" → producer group). Needs decision during Phase 2A.

3. **Relevance rule complexity**: V1 supports only "single field, set of values" rules. Are there known cases requiring compound rules (field A = X AND field B = Y)? If so, should the schema allow it from the start (even if the UI only evaluates simple rules initially)?

4. **Recipe field highlighting**: When a recipe step references a field, should that field receive a visual highlight in the config area? This adds delight but requires a cross-panel communication mechanism. Could defer to a later iteration.

5. **Profile editor**: Should Phase 3 or a later phase include a visual editor for creating/editing profile files within Bowties? Or is a JSON file with documentation sufficient for the foreseeable future?
