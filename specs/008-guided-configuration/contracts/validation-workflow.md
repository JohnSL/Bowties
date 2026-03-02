# Validation Workflow

## Purpose

Cross-reference extraction outputs (from Prompts A–E) against the CDI XML to verify:
1. All referenced CDI paths actually exist in the XML
2. All referenced enum values are valid entries in the CDI's `<map>` definitions
3. All CDI sections are covered by at least one extraction output

## Inputs

- CDI XML file for the target node type
- One or more extraction output JSON files (event-roles.json, relevance-rules.json, section-descriptions.json, field-descriptions.json, recipes.json)

## Validation Steps

### Step 1: Parse CDI XML into a Path Registry

Walk the CDI XML and build a registry of:
- Every segment name → path (e.g., `Port I/O`, `Conditionals`)
- Every group name → path, accounting for nesting and replication (e.g., `Port I/O/Line`, `Port I/O/Line/Event[0-5]`)
- Every leaf field name → path (e.g., `Port I/O/Line/Output Function`)
- Every enum map: field path → set of valid `<property>` integer values
- Total counts: segments, groups, leaf fields, event slots

### Step 2: Validate CDI Path References

For each extraction output file, check every `cdiPath` / `controllingField` / `affectedSection` / `field` / `scope` value:
- Does the path exist in the CDI path registry?
- If the path uses index ranges (e.g., `Event[0-5]`), does the referenced group exist at those indices?

Report every path that does not match as a **path error**.

### Step 3: Validate Enum Value References

For each extraction output that references enum values (`irrelevantWhen`, `options[].value`, `steps[].rawValue`):
- Look up the corresponding field path in the CDI path registry
- Check that the referenced integer value exists in the CDI's `<map>` for that field
- Check that the label (if provided) matches the CDI's `<value>` for that property

Report every invalid value as an **enum error**.

### Step 4: Coverage Analysis

Compare the CDI path registry against all extraction outputs combined:
- Which segments have section descriptions? (from Prompt C)
- Which groups have section descriptions? (from Prompt C)
- Which leaf fields have field descriptions? (from Prompt D)
- Which event groups have role classifications? (from Prompt A)
- Which sections have relevance rules? (from Prompt B — not all sections need rules, so low coverage here is expected)

Produce coverage percentages for each category and an overall percentage.

List all CDI sections with **no extraction output** as uncovered sections.

### Step 5: Produce Validation Report

Output a JSON report matching the schema in [data-model.md](../data-model.md) (Validation Report Structure section).

## Execution Options

### Option A: Manual Validation (Phase 1 minimum)

1. Open the CDI XML and each extraction output side by side
2. For each path in the extraction output, search for the corresponding element in the CDI XML
3. For each enum value, verify it appears in the `<map>` entries
4. Tally covered vs. uncovered sections manually
5. Document findings in `validation-report.md`

### Option B: Script-Assisted Validation (recommended)

Use a lightweight script (Python or TypeScript) that:
1. Parses the CDI XML (using standard XML parser)
2. Loads each extraction JSON file
3. Performs Steps 1–4 programmatically
4. Outputs the validation report JSON

This script is a development aid and does not need to meet production code standards. It can be stored in the specs directory or a utility scripts folder.

## Pass Criteria

- **Path errors**: 0 (all referenced paths must exist)
- **Enum errors**: 0 (all referenced values must be valid)
- **Coverage**: ≥90% of segments, groups, and leaf fields covered by descriptions; 100% of event groups covered by role classifications
