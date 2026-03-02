---
name: profile-6-validate
description: Validate extraction outputs against a CDI XML to verify all referenced paths and enum values exist. Cross-reference and coverage check for node profile extraction. Keywords: CDI, validate, verification, coverage, path check, enum check, profile extraction.
---

# Validate Extraction Outputs

Cross-reference all extraction outputs (event roles, relevance rules, descriptions, recipes) against the CDI XML to verify structural correctness and completeness.

## When to Use

Use this skill after completing one or more extraction steps to verify that the output is structurally correct before using it to build a profile file. Run this as the final step in the extraction workflow.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML file to validate against).
2. **One or more extraction output JSON files** — files saved to `profiles/<node-name>/` from any or all of the prior skills:
   - `event-roles.json` (from `profile-1-event-roles`)
   - `relevance-rules.json` (from `profile-2-relevance-rules`)
   - `section-descriptions.json` (from `profile-3-section-descriptions`)
   - `field-descriptions.json` (from `profile-4-field-descriptions`)
   - `recipes.json` (from `profile-5-recipes`)

**No separate CDI XML needed** — read the CDI XML file path from `manual-outline.json` `cdiFile` field and load it for validation.

## Validation Steps

### Step 1: Build CDI Path Registry

Walk the CDI XML and build a registry of:
- Every segment name → path
- Every group name → path (accounting for nesting and replication)
- Every leaf field name → path
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
- Which segments have section descriptions? (from section-descriptions.json)
- Which groups have section descriptions? (from section-descriptions.json)
- Which leaf fields have field descriptions? (from field-descriptions.json)
- Which event groups have role classifications? (from event-roles.json)
- Which sections have relevance rules? (from relevance-rules.json — not all need rules, so low coverage is expected)

List all CDI sections with **no extraction output** as uncovered sections.

### Step 5: Produce Validation Report

```json
{
  "nodeType": {
    "manufacturer": "<from CDI>",
    "model": "<from CDI>"
  },
  "cdiFile": "<path to CDI XML used>",
  "extractionFiles": ["<paths to extraction JSON files validated>"],
  "pathErrors": [
    {
      "extractionFile": "<which file>",
      "entryId": "<identifier of the entry>",
      "referencedPath": "<CDI path cited>",
      "error": "<what's wrong>"
    }
  ],
  "enumErrors": [
    {
      "extractionFile": "<which file>",
      "entryId": "<identifier>",
      "field": "<CDI path of the enum field>",
      "referencedValue": 9,
      "error": "<what's wrong>"
    }
  ],
  "coverage": {
    "totalSegments": 0,
    "coveredSegments": 0,
    "totalGroups": 0,
    "coveredGroups": 0,
    "totalLeafFields": 0,
    "coveredLeafFields": 0,
    "totalEventSlots": 0,
    "coveredEventSlots": 0,
    "overallPercentage": 0
  },
  "uncoveredSections": [
    {
      "cdiPath": "<path>",
      "level": "segment | group | field",
      "name": "<element name>"
    }
  ],
  "summary": "<pass/fail with key statistics>"
}
```

## Pass Criteria

- **Path errors**: 0 (all referenced paths must exist in CDI)
- **Enum errors**: 0 (all referenced values must be valid)
- **Coverage**: ≥90% of segments, groups, and leaf fields covered by descriptions; 100% of event groups covered by role classifications

## Output File

Save the validation report as `profiles/<node-name>/validation-report.json` (e.g., `profiles/tower-lcc/validation-report.json`).
