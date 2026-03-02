# Research: Profile Content Extraction Tooling

**Feature**: 008-guided-configuration (Phase 1)  
**Date**: 2026-02-28

## 1. CDI Path Addressing for Extraction Outputs

### Decision
Use CDI element names joined by `/` as the canonical path format, with 0-based index suffixes for replicated groups and ambiguous siblings. Example: `Port I/O/Line[0]/Event[0-5]/Command`.

### Rationale
- CDI element names (`<name>` tags) are stable across firmware versions, unlike memory addresses which shift when firmware adds/removes fields
- The codebase already uses this convention: `node_tree.rs` builds `path` fields by joining group/element names with `/` and appending instance indices for replicated groups
- The existing `merge_event_roles` function matches by path key (joined by `/`), confirming this is the established pattern
- Index ranges (e.g., `Event[0-5]`) disambiguate same-named sibling groups (the two `<group name="Event">` groups under each Line in Tower-LCC)

### Alternatives Considered
- **Memory addresses**: Fragile across firmware versions; the CDI `offset` model is complex with relative offsets and cursor-based calculation. Would require recalculating addresses per firmware version.
- **XPath expressions**: Too verbose for prompt outputs and profile files. Not aligned with existing codebase conventions.
- **Child field name patterns** (e.g., "the Event group containing Command"): More readable but not systematic; breaks down for groups with similar child structures.

---

## 2. Distinguishing Same-Named Sibling Groups

### Decision
Use document order (0-based index) to distinguish same-named siblings. For Tower-LCC's two `<group name="Event">` groups under each Line: `Event[0-5]` (replication 6, consumer — contains Command/Action) and `Event[6-11]` (replication 6, producer — contains "Upon this action"/Indicator).

### Rationale
- The CDI XML schema allows multiple sibling groups with identical `<name>` values. Tower-LCC has exactly this: two `<group name="Event" replication="6">` groups per Line.
- Document order is deterministic from the CDI XML and is how the CDI parser (`lcc-rs/src/cdi/parser.rs`) processes elements.
- The config tree builder already assigns instance indices by document order (0-based), making index-range notation natural.
- For extraction prompts, the index range also communicates the event slot count (6 consumer events at indices 0-5, 6 producer events at indices 6-11).

### Alternatives Considered
- **Child field name matching** ("Event group containing Command" vs. "Event group containing Indicator"): More human-readable in isolation but not systematic — would require per-node-type heuristics.
- **1-based indexing**: The codebase consistently uses 0-based; aligning with that avoids off-by-one confusion.

---

## 3. PDF Text Extraction Strategy

### Decision
Use the `pdf-utilities` MCP extension's `read_pdf` tool to pre-extract manual text, then provide the extracted text (not the raw PDF) alongside CDI XML to extraction prompts.

### Rationale
- The `read_pdf` tool supports `pageRange` parameters, enabling targeted extraction of specific manual sections rather than processing the entire document each time
- Pre-extracted text is inspectable — the profile author can verify the LLM will see correct content (especially important for tables, diagrams with text, multi-column layouts)
- Text tokens are more predictable and typically fewer than PDF binary tokens when attached directly
- The `pdf-utilities` extension is already installed and available in the workspace

### Alternatives Considered
- **Attaching raw PDF to each prompt**: Simpler workflow but no visibility into what the LLM actually parses from the PDF; tables and multi-column layouts may be misread with no way to correct. Also processes the full PDF on every prompt invocation.
- **External PDF-to-text tools** (e.g., `pdftotext`): Would work but adds a dependency outside the VS Code environment. The MCP extension already provides this capability.

---

## 4. Extraction Output Format

### Decision
Use JSON for extraction outputs. Each prompt produces a single JSON file with a consistent top-level structure.

### Rationale
- JSON is machine-parseable, enabling automated validation (cross-referencing CDI paths against the XML)
- The Phase 2 profile file will be JSON (per plans.md recommendation), so extraction outputs feed directly into profile population with minimal transformation
- JSON Schema can formally validate extraction output structure
- The codebase already uses JSON for all data interchange (Tauri IPC, configuration)

### Alternatives Considered
- **Structured markdown**: More readable for manual review but harder to validate programmatically. Would require parsing markdown to check CDI path references.
- **YAML**: Slightly more readable than JSON but adds a format dependency not used elsewhere in the project. JSON is the established interchange format per constitution.

---

## 5. Validation Approach

### Decision
Build a lightweight validation script (Python or TypeScript) that parses the CDI XML and each extraction JSON output, cross-references all CDI paths and enum values, and produces a coverage report. Phase 1 validation can also be performed manually by visual inspection.

### Rationale
- Automated validation catches path typos and invalid enum values that are tedious to spot manually, especially across hundreds of fields
- The Tower-LCC CDI has ~400 lines of XML with dozens of enum maps — manual cross-referencing is error-prone
- A script is reusable when extracting profiles for additional node types
- The script is a development aid, not production code — it doesn't need to meet the constitution's Rust/testing requirements

### Alternatives Considered
- **Purely manual validation**: Feasible for a small CDI but doesn't scale; Tower-LCC has 16 replicated Lines × 2 Event groups × 6 events = 192 event slots plus Conditionals, Track Receiver/Transmitter.
- **LLM-assisted validation** (another prompt that checks the output): Creative but circular — if the LLM made extraction errors, it may also miss them in validation.

---

## 6. Prompt Modularity vs. Consolidation

### Decision
Keep 5 independent prompts (A through E) as designed in the spec. Do NOT consolidate into fewer prompts.

### Rationale
- Each prompt has a focused output schema — mixing concerns (e.g., roles and descriptions in one prompt) produces harder-to-validate output
- Independent prompts can be iterated individually — if event role accuracy is poor, only Prompt A needs rework
- Different prompts may benefit from different context windows — Prompt B (relevance) needs the full manual to find cross-field dependencies, while Prompt D (field descriptions) can be run section-by-section using `pageRange`
- The spec requires this modularity (FR-003: "Each extraction category MUST be producible independently")

### Alternatives Considered
- **Single mega-prompt**: Would produce all outputs at once but exceeds practical output length for most models; harder to debug; one error contaminates everything.
- **Merging C + D** (section + field descriptions): These are the most similar prompts and could theoretically merge, but the output schemas differ (section descriptions are short purpose statements; field descriptions include per-option details, units, ranges). Keeping them separate produces cleaner output.

---

## 7. Tower-LCC CDI Structure Summary (Reference)

For extraction prompt design, the Tower-LCC CDI contains:

| Segment | Groups | Replications | Event Groups | Key Observations |
|---------|--------|-------------|--------------|------------------|
| NODE ID | 1 | None | 0 | User name/description strings |
| Node Power Monitor | 0 (direct elements) | None | 2 eventids (Power OK/Not OK) | Segment contains elements directly (no groups) |
| Port I/O | Line (×16) | 16 Lines, each with 2 Delay, 2×6 Events | 192 eventids (96 consumer + 96 producer) | Two same-named Event groups per Line; consumer has Command/Action, producer has "Upon this action"/Indicator |
| Conditionals | Logic (×32) | 32 Logic units, each with Variable #1, Variable #2, Action (×4) | 192 eventids (128 consumer + 64 producer) | Complex nesting; Variables have Source/Track Speed; Actions have Condition/Destination |
| Track Receiver | Circuit (×8) | 8 circuits | 8 eventids (consumer — Link Address) | Track speed inter-mast communication |
| Track Transmitter | Circuit (×8) | 8 circuits | 8 eventids (producer — Link Address) | Read-only link addresses |

**Total event slots**: ~400 across all segments.
