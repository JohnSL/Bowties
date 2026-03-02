# Node Profile Extraction Workflow

This guide walks through the complete process of creating a node profile by running the seven extraction skills in sequence, with each skill building on the outputs of the previous ones. The first skill (profile-0) indexes the entire manual once; subsequent skills read only the relevant pages they need.

## Overview

A **node profile** is a set of structured JSON files that describe everything about an LCC node's configuration: a map of the manual's structure (outline), what each event does (producer/consumer), which sections are conditionally relevant, what each field and option means, and step-by-step recipes for common tasks.

The extraction process takes two inputs — a node's **CDI XML** and its **PDF manual** — runs them through profile-0 to create a manual outline, then each subsequent profile reads only the JSON file from the previous profile (which contains file references). Outputs are saved to `profiles/<node-name>/` and include seven JSON files.

## Prerequisites

- VS Code with GitHub Copilot (or access to any capable LLM)
- `pdf-utilities` MCP extension installed (for targeted PDF text extraction via `read_pdf` with `pageRange`)
- The node's PDF manual file (text-based PDF, not scanned images)
- The node's CDI XML file

## Step 0: Create Manual Outline

**Skill**: `profile-0-manual-outline`  
**Inputs**: CDI XML file path + PDF manual file path  
**Output**: `profiles/<node-name>/manual-outline.json` (contains file references for all subsequent steps)

Before starting profile extractions, create the output directory `profiles/<node-name>/` and run this step to create a structured index of the manual mapping sections (e.g., "Port I/O Configuration") to page ranges. This is the ONLY step that reads the entire PDF; all subsequent skills use the outline to read only relevant pages.

**What to do**:
1. Invoke the `profile-0-manual-outline` skill
2. Provide the CDI XML and entire PDF manual
3. Review the outline — verify sections and page ranges are accurate
4. Save output to `profiles/<node-name>/manual-outline.json`

**What to check**:
- Every major CDI section (Port I/O, Conditionals, Track Receiver/Transmitter) is identified
- Page ranges are specific and don't overlap unnecessarily
- Topics under each section are relevant to that section's purpose

## Step 1: Extract Event Roles

**Skill**: `profile-1-event-roles`  
**Required input from you**: `manual-outline.json` (step 0 output)
**Optional context** (skill will auto-discover): None (first data extraction step)  
**Output**: `profiles/<node-name>/event-roles.json`

Classify every event group as Producer or Consumer. The skill reads file paths and page ranges from the outline, then extracts only the relevant pages from the manual.

**What to do**:
1. Invoke the `profile-1-event-roles` skill
2. Provide only `manual-outline.json` in your prompt
3. The skill will read CDI XML and PDF file paths and page ranges from the outline
4. Review output for completeness and accuracy
5. Save to `profiles/<node-name>/event-roles.json`

**What to check**:
- Every `<eventid>` group in the CDI has a role classification
- Spot-check 10 entries against the manual
- All same-named sibling groups are correctly disambiguated (e.g., Event[0-5] vs Event[6-11])

## Step 2: Extract Relevance Rules

**Skill**: `profile-2-relevance-rules`  
**Required input from you**: `manual-outline.json`  
**Optional context** (skill will auto-discover in profiles directory): `event-roles.json`  
**Output**: `profiles/<node-name>/relevance-rules.json`

Identify configuration sections that become irrelevant based on other field values.

**What to do**:
1. Invoke the `profile-2-relevance-rules` skill
2. Provide only `manual-outline.json` in your prompt
3. The skill will auto-discover `event-roles.json` from profiles/\<node-name\>/ and use it for context
4. The skill will read file paths and page ranges from the outline
5. Review for known cases (disabled outputs → irrelevant sections)
6. Save to `profiles/<node-name>/relevance-rules.json`

**What to check**:
- Known rules are present (e.g., consumer events irrelevant when Output Function = No Function)
- All field paths and enum values are valid CDI references

## Step 3: Extract Section Descriptions

**Skill**: `profile-3-section-descriptions`  
**Required input from you**: `manual-outline.json`  
**Optional context** (skill will auto-discover in profiles directory): `event-roles.json`, `relevance-rules.json`  
**Output**: `profiles/<node-name>/section-descriptions.yaml`

Create purpose statements for every segment and group.

**What to do**:
1. Invoke the `profile-3-section-descriptions` skill
2. Provide only `manual-outline.json` in your prompt
3. The skill will auto-discover prior outputs from profiles/\<node-name\>/ and use them for richer context
4. The skill will read file paths and page ranges from the outline
5. Review for complete coverage of segments and groups
6. Save to `profiles/<node-name>/section-descriptions.yaml` (note: YAML format, not JSON)

**What to check**:
- All segments have descriptions
- All group templates (not instances) have descriptions
- Descriptions incorporate event role and relevance context where applicable

## Step 4: Extract Field Descriptions

**Skill**: `profile-4-field-descriptions`  
**Required input from you**: `manual-outline.json`  
**Optional context** (skill will auto-discover in profiles directory): `event-roles.json`, `relevance-rules.json`, `section-descriptions.yaml`  
**Output**: `profiles/<node-name>/field-descriptions.yaml`

Create detailed descriptions for every leaf field, including per-option descriptions for enums.

**What to do**:
1. Invoke the `profile-4-field-descriptions` skill
2. Provide only `manual-outline.json` in your prompt
3. The skill will auto-discover prior outputs from profiles/\<node-name\>/ and use them for detail and context
4. The skill will read file paths and page ranges from the outline
5. For large CDIs that may hit output limits, the skill can process per-segment using page ranges from the outline
6. Save to `profiles/<node-name>/field-descriptions.yaml` (note: YAML format, not JSON)

**What to check**:
- All leaf fields have entries
- Enum fields have all options with correct value/label pairs from CDI
- Units and ranges are included for numeric fields

## Step 5: Extract Recipes

**Skill**: `profile-5-recipes`  
**Required input from you**: `manual-outline.json`  
**Optional context** (skill will auto-discover in profiles directory): `event-roles.json`, `relevance-rules.json`, `section-descriptions.yaml`, `field-descriptions.yaml`  
**Output**: `profiles/<node-name>/recipes.yaml`

Identify common configuration tasks and produce step-by-step recipes.

**What to do**:
1. Invoke the `profile-5-recipes` skill
2. Provide only `manual-outline.json` in your prompt
3. The skill will auto-discover prior outputs from profiles/\<node-name\>/ and use them for comprehensive context
4. The skill will read file paths and page ranges from the outline (recipes/examples sections)
5. Review for coverage of main use cases
6. Save to `profiles/<node-name>/recipes.yaml` (note: YAML format, not JSON)

**What to check**:
- At least 3 Port I/O recipes (e.g., button input, LED output, sensor)
- At least 1 Conditionals recipe
- Each recipe step references valid CDI field paths and enum values

## Step 6: Validate

**Skill**: `profile-6-validate`  
**Inputs**: CDI XML + ALL extraction output files from `profiles/<node-name>/`  
**Output**: `profiles/<node-name>/validation-report.json`

Cross-reference all extraction outputs against the CDI XML.

**What to do**:
1. Invoke the `profile-6-validate` skill
2. Provide CDI XML and all JSON files from `profiles/<node-name>/`
3. Review validation report
4. Save to `profiles/<node-name>/validation-report.json`

**Pass criteria**:
- 0 path errors
- 0 enum errors
- ≥90% coverage of CDI sections

If validation fails, fix the identified errors in the relevant extraction output and re-validate.

## File Structure After Completion

```
profiles/tower-lcc/
├── manual-outline.json            ← Step 0
├── event-roles.json               ← Step 1
├── relevance-rules.json           ← Step 2
├── section-descriptions.yaml      ← Step 3
├── field-descriptions.yaml        ← Step 4
├── recipes.yaml                   ← Step 5
└── validation-report.json         ← Step 6
```

## Data Flow

```
CDI XML + PDF Manual
    │
    v
Step 0: manual-outline.json
  (reads entire PDF once, creates section index with file paths)
    │
    ├──> Step 1: event-roles.json
    │    (reads: manual-outline.json only)
    │
    ├──> Step 2: relevance-rules.json
    │    (reads: manual-outline.json + event-roles.json)
    │
    ├──> Step 3: section-descriptions.yaml
    │    (reads: manual-outline.json + relevance-rules.json)
    │
    ├──> Step 4: field-descriptions.yaml
    │    (reads: manual-outline.json + section-descriptions.yaml)
    │
    ├──> Step 5: recipes.yaml
    │    (reads: manual-outline.json + field-descriptions.yaml)
    │
    v
Step 6: validation-report.json
  (reads: CDI XML + all outputs)
```

**Key Design**:
- Step 0 reads the entire PDF once and extracts page ranges into `manual-outline.json`
- Each subsequent step only takes the immediate previous step's output file (plus manual-outline.json which contains file references)
- File paths and page ranges are embedded in `manual-outline.json`, so no manual file path passing is needed
- Each skill reads only the pages it needs from the PDF using the page ranges from the outline
- Prior output provides context for richer results
- Steps 3–5 output YAML format for readability of descriptions; other steps output JSON

## Tips & Troubleshooting

### Page Range Extraction

Each skill automatically extracts page ranges from `manual-outline.json` and uses the `pdf-utilities` `read_pdf` tool with the `pageRange` parameter. You don't need to manually specify page ranges — the skill handles it.

### Merging Per-Segment Field Descriptions

For large CDIs with complex field sections, `profile-4-field-descriptions` may hit output limits. The skill is designed to handle this by processing per-segment using the page ranges from `manual-outline.json`. Results are automatically merged into a single `field-descriptions.yaml`.

### Fixing Validation Errors

If `profile-6-validate` reports path or enum errors:

1. Identify the problematic extraction step from the error report
2. Re-invoke that skill (e.g., `profile-3-section-descriptions`) with corrections noted (e.g., "I found these paths are invalid...")
3. Provide the same inputs as before (manual-outline.json + the previous step's output file)
4. Update the relevant output file
5. Re-validate

### Reusing Outlines Across Nodes

Once you've created `manual-outline.json` for a node, you can reuse it if the manual is updated but sections remain stable. Simply provide the old outline when re-running steps 1-5 with the updated PDF content.

### Understanding Required vs Optional Inputs

**Required input for all steps 1-6**: Always provide `manual-outline.json` in your prompt. This file contains the file paths and page ranges the skill needs.

**Optional context (auto-discovered)**: Previous step outputs are listed as "optional but recommended." You DO NOT need to provide them in your prompt — the skill will automatically discover them in the `profiles/<node-name>/` directory when you run it. Providing `manual-outline.json` tells the skill which directory to search.

This design allows you to:**
- Re-run a step with corrections** without cluttering your prompt with multiple files
- **Skip intermediate steps** if they're already complete
- **Provide richer context** naturally by having all prior outputs in the directory
