# Quickstart: Profile Content Extraction

**Feature**: 008-guided-configuration (Phase 1)

## Overview

This guide walks through extracting profile content from a node's PDF manual and CDI XML using the five extraction prompts. The process produces structured JSON outputs that feed into the Phase 2 profile file.

## Prerequisites

- VS Code with Copilot Chat (or access to any capable LLM)
- `pdf-utilities` MCP extension installed (for PDF text extraction)
- The node's PDF manual file
- The node's CDI XML file (e.g., `specs/archive/003-miller-columns/Tower-LCC.xml`)

## Step-by-Step Process

### 1. Extract PDF Manual Text

Use the `pdf-utilities` MCP extension to extract text from the PDF manual:

```
Read the PDF at <absolute-path-to-manual.pdf>
```

For targeted extraction (recommended for large manuals), extract by section:

```
Read pages 5-15 of <absolute-path-to-manual.pdf>
```

Save the extracted text for use with extraction prompts. You can extract the full manual once and reuse the text across all five prompts.

### 2. Run Extraction Prompts

Run each prompt independently. For each prompt:

1. Open the prompt file from `specs/008-guided-configuration/contracts/`
2. Copy the prompt content into Copilot Chat
3. Attach or paste the CDI XML and extracted manual text as inputs
4. Review the output for obvious errors
5. Save the output as JSON in `specs/008-guided-configuration/extractions/tower-lcc/`

**Recommended order** (but any order works):

| Order | Prompt | Output File | Time Estimate |
|-------|--------|-------------|---------------|
| 1 | [prompt-a-event-roles.md](contracts/prompt-a-event-roles.md) | `event-roles.json` | ~10 min |
| 2 | [prompt-b-relevance-rules.md](contracts/prompt-b-relevance-rules.md) | `relevance-rules.json` | ~10 min |
| 3 | [prompt-c-section-descriptions.md](contracts/prompt-c-section-descriptions.md) | `section-descriptions.json` | ~15 min |
| 4 | [prompt-d-field-descriptions.md](contracts/prompt-d-field-descriptions.md) | `field-descriptions.json` | ~20 min |
| 5 | [prompt-e-recipes.md](contracts/prompt-e-recipes.md) | `recipes.json` | ~15 min |

### 3. Review and Refine

After each prompt run:

- **Spot-check accuracy**: Pick 5–10 entries and verify against the manual
- **Check completeness**: Are all CDI sections covered? The prompt asks for exhaustive output, but LLMs may miss some entries
- **Fix errors**: If the output has mistakes, re-run the prompt with a clarifying note (e.g., "You missed the Track Receiver segment" or "The Event[0-5] group contains Command/Action, not Indicator")
- Expect 1–3 refinement rounds per prompt

### 4. Validate

Run the validation workflow ([contracts/validation-workflow.md](contracts/validation-workflow.md)):

- Cross-reference all CDI paths in extraction outputs against the CDI XML
- Verify all enum values are valid
- Check coverage — aim for ≥90% of CDI sections covered

Save findings in `extractions/tower-lcc/validation-report.md`.

### 5. Done

When validation passes (0 path errors, 0 enum errors, ≥90% coverage), the extraction outputs are ready for Phase 2 profile population.

## Tips

- **Use `pageRange` for Prompt D**: Field descriptions is the largest output. If the model truncates, run it per-segment: extract Port I/O fields, then Conditionals fields, then Track Receiver/Transmitter fields, and merge the results.
- **CDI XML is always attached**: Every prompt needs the CDI XML alongside the manual text. The CDI is the structural authority; the manual provides the meaning.
- **Copy CDI path conventions**: All prompts use the same path format (`/`-separated names, `[index-range]` for disambiguation). Consistent paths across outputs make validation straightforward.
- **JSON output**: If the model produces markdown tables instead of JSON, ask it to "reformat the output as JSON matching the schema in the prompt."

## File Structure After Completion

```text
specs/008-guided-configuration/
├── extractions/
│   └── tower-lcc/
│       ├── event-roles.json
│       ├── relevance-rules.json
│       ├── section-descriptions.json
│       ├── field-descriptions.json
│       ├── recipes.json
│       └── validation-report.md
├── contracts/
│   ├── prompt-a-event-roles.md
│   ├── prompt-b-relevance-rules.md
│   ├── prompt-c-section-descriptions.md
│   ├── prompt-d-field-descriptions.md
│   ├── prompt-e-recipes.md
│   └── validation-workflow.md
├── data-model.md
├── quickstart.md
├── research.md
├── plan.md
└── spec.md
```
