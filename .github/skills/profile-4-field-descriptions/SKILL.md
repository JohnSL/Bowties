---
name: profile-4-field-descriptions
description: Extract detailed field and option descriptions for every leaf field in an LCC node's CDI XML using the PDF manual. Keywords -- CDI, field, option, enum, description, int, string, eventid, profile extraction.
---

# Extract Field & Option Descriptions

Create detailed descriptions for every leaf field in a node's CDI XML, including per-option descriptions for enum fields, units and ranges for numeric fields, and role descriptions for eventid fields.

## When to Use

Use this skill when you need rich, user-facing descriptions for individual configuration fields. These descriptions populate tooltips and the companion panel in the configuration UI. This is typically the fourth extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — produced by `profile-0-manual-outline`. Contains `cdiFile`, `pdfFile`, and page ranges.
2. **event-roles.json** — from `profile-1-event-roles` (optional but recommended). Provides role context for eventid descriptions.
3. **relevance-rules.json** — from `profile-2-relevance-rules` (optional but recommended). Helps note when a field only applies under certain conditions.
4. **section-descriptions.yaml** — from `profile-3-section-descriptions` (optional but recommended). Provides section-level context.

All CDI/PDF paths are read from `manual-outline.json`.

## Workflow

### Step 1 — generate the skeleton

Run the shared CLI to emit a complete-by-construction scaffold containing one entry per leaf field, with every enum `<map>` entry already enumerated under `options`:

```pwsh
uv run .github/skills/_lib/profile_tools.py skeleton fields profile-extractions/<node-name>
```

This produces `field-descriptions.skeleton.yaml`. The skeleton already contains:

- `cdiPath` (with `[N]` / `[N-M]` suffix where same-name siblings exist)
- `name` (CDI element name)
- `elementType` (`int`, `string`, or `eventid` — refine `int`↔`string` if the CDI uses `float`/`bit`)
- For enum fields: every `value` + `label` from the CDI `<map>`
- `TODO` placeholders for every narrative field you need to fill in

### Step 2 — fill in narrative fields

Use `pdf-utilities.read_pdf` with `pageRange` from the outline to read the relevant sections. Edit each entry's TODO placeholders:

- `description` — what the field does in practical terms (Markdown supported).
- For enum options: each option's `description` and optional `category`.
- `units` / `validRange` / `typicalValues` for numeric fields (leave `null` if not applicable).
- `maxLength` for strings (from CDI `<string size="N">`).
- `role` for eventids (`Producer` | `Consumer` — match `event-roles.json`).
- `citation` — manual section + page.

### Step 3 — rename and validate

Rename `field-descriptions.skeleton.yaml` to `field-descriptions.yaml`, then run `profile-6-validate` to confirm every path and enum value resolves.

## Guidelines

### For all fields
- Explain what the field does in practical terms; do not just repeat the CDI label.
- **Descriptions support Markdown** — `**bold**`, `*italic*`, bullet points.
- If the CDI already has a useful `<description>`, enhance it with manual context.
- Write for model railroad hobbyists, not protocol engineers.

### For enum fields
- Do not change `value` or `label` — the skeleton copied them verbatim from the CDI `<map>`.
- Write the `description` for each option (one short sentence is plenty).
- Use `category` to group related options the manual treats as a family (e.g. Steady, Pulse, Blink, Sample).

### For numeric fields
- `units` only when the manual or CDI states them.
- `validRange: { min, max }` only when the CDI/manual specifies bounds.
- `typicalValues` only when the manual recommends specific values.

### For eventid fields
- Describe the event's role in node operation.
- `role` must match `event-roles.json` (Producer or Consumer).

### For string fields
- `maxLength` from the CDI `<string size="N">` attribute.

## Important

- Every leaf field in the CDI MUST appear (the skeleton guarantees this — do not remove entries).
- For replicated groups, the skeleton emits one template entry — describe it once; it applies to every instance.
- Same-named sibling groups are distinguished with `[N]` / `[N-M]` index suffixes already; do not rewrite the suffix.
- Do NOT add option values that don't exist in the CDI `<map>`.
- **YAML formatting**: use the pipe (`|`) syntax for multiline descriptions; quote values containing special characters.

## Output File

`profile-extractions/<node-name>/field-descriptions.yaml`. Used as shared context by the recipes extraction skill.

## Tip: Large CDIs

If the field list is very long, fill in entries in batches grouped by segment (Port I/O fields, then Conditionals, …). The skeleton groups entries in document order, so segment boundaries are easy to find.
