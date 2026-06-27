---
name: profile-6-validate
description: Validate extraction outputs against a CDI XML to verify all referenced paths and enum values exist. Cross-reference and coverage check for node profile extraction. Keywords -- CDI, validate, verification, coverage, path check, enum check, profile extraction.
---

# Validate Extraction Outputs

Cross-reference every extraction output (event roles, relevance rules, descriptions, recipes) against the CDI XML to verify structural correctness and completeness.

## When to Use

Use this skill after completing one or more extraction steps to verify that the output is structurally correct before using it to build a profile file. Run as the final step in the extraction workflow.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to the CDI XML to validate against).
2. **One or more extraction output files** — saved to `profile-extractions/<node-name>/` by the prior skills:
   - `event-roles.json` (from `profile-1-event-roles`)
   - `relevance-rules.json` (from `profile-2-relevance-rules`)
   - `section-descriptions.yaml` (from `profile-3-section-descriptions`)
   - `field-descriptions.yaml` (from `profile-4-field-descriptions`)
   - `recipes.yaml` (from `profile-5-recipes`)

The CDI file is read automatically from `manual-outline.json` `cdiFile` field — do not ask for it.

## How to Run

Run the shared CLI from the repo root:

```pwsh
uv run .github/skills/_lib/profile_tools.py validate profile-extractions/<node-name>
```

(See `.github/skills/_lib/README.md` for setup if `uv` is not installed.)

The CLI:

1. Loads the CDI XML referenced by `manual-outline.json`.
2. Walks each extraction file present in the node directory and resolves every `cdiPath`, `controllingField`, `affectedSection`, recipe `field`, recipe `scope`, and `childField` against a CDI path registry that handles literal `/` inside element names, `[N]` / `[N-M]` index suffixes, and `<repname>` collapse.
3. Validates every `irrelevantWhen` value, every `options[].value`, and every recipe `rawValue` against the controlling field's `<map>`.
4. Computes coverage (segments, groups, leaf fields, eventids).
5. Writes `validation-report.json` and exits non-zero on failure.

## Output File

`profile-extractions/<node-name>/validation-report.json` with this shape:

```json
{
  "nodeType": { "manufacturer": "...", "model": "..." },
  "cdiFile": "<path>",
  "extractionFiles": ["event-roles.json", "..."],
  "pathErrors": [
    { "extractionFile": "...", "entryId": "...", "referencedPath": "...", "error": "..." }
  ],
  "enumErrors": [
    { "extractionFile": "...", "entryId": "...", "field": "...", "referencedValue": 9, "error": "..." }
  ],
  "coverage": {
    "totalSegments": 0, "coveredSegments": 0,
    "totalGroups": 0, "coveredGroups": 0,
    "totalLeafFields": 0, "coveredLeafFields": 0,
    "totalEventSlots": 0, "coveredEventSlots": 0,
    "overallPercentage": 0
  },
  "uncoveredSections": [
    { "cdiPath": "...", "level": "segment | group | field", "name": "..." }
  ],
  "summary": "<PASS/FAIL with key statistics>"
}
```

## Pass Criteria

- **Path errors**: 0 — every referenced path must resolve in the CDI.
- **Enum errors**: 0 — every referenced integer must exist in its field's `<map>`; provided labels must match the CDI label exactly.
- **Coverage**: ≥ 90% across segments, groups, and leaf fields; 100% of event groups covered by role classifications.

If the CLI exits non-zero, read the printed `pathErrors` / `enumErrors` and fix the offending extraction file before assembling the profile.
