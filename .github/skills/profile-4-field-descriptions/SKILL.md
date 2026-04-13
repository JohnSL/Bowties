---
name: profile-4-field-descriptions
description: Extract detailed field and option descriptions for every leaf field in an LCC node's CDI XML using the PDF manual. Keywords -- CDI, field, option, enum, description, int, string, eventid, profile extraction.
---

# Extract Field & Option Descriptions

Create detailed descriptions for every leaf field in a node's CDI XML, including per-option descriptions for enum fields, units and ranges for numeric fields, and role descriptions for eventid fields.

## When to Use

Use this skill when you need rich, user-facing descriptions for individual configuration fields. These descriptions populate tooltips and the companion panel in the configuration UI. This is typically the fourth extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML), `pdfFile` (path to PDF manual), and page ranges for each section.
2. **event-roles.json** — from `profile-1-event-roles` (optional but recommended). Provides role context for eventid field descriptions (e.g., "This event ID is for a producer event that fires when...").
3. **relevance-rules.json** — from `profile-2-relevance-rules` (optional but recommended). Helps note when a field only applies under certain conditions.
4. **section-descriptions.json** — from `profile-3-section-descriptions` (optional but recommended). Provides section-level context that helps write more coherent field descriptions.

**No other file paths needed** — read the CDI XML and PDF file paths from `manual-outline.json`, then use the `pdf-utilities` `read_pdf` tool with `pageRange` parameter to extract configuration sections identified in the outline. For large CDIs, you may extract per-segment (e.g., Port I/O pages, then Conditionals) to avoid output truncation.

## Task

For every **leaf field** (int, string, eventid, float) in the CDI XML, produce a clear description of what the field controls. For enum fields (fields with `<map>` entries), additionally describe each option value.

## Output Format

Produce a YAML file with embedded Markdown text. The file should be saved as `field-descriptions.yaml` in your profile directory.

```yaml
nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"

fields:
  - cdiPath: "Port I/O/Line/Output Function"
    name: "Output Function"
    elementType: "int"
    description: |
      Controls how this output line drives external devices. **Steady** modes 
      hold the line at a constant level. **Pulse** modes briefly activate 
      the line. **Blink** modes create repeating on/off cycles. Use **Sample** 
      modes to modulate output based on track circuit activity.
    units: null
    validRange: null
    typicalValues: null
    options:
      - value: 0
        label: "No Function"
        description: "Line is disabled and has no effect."
        category: null
      - value: 1
        label: "Steady Active Hi"
        description: "Line is continuously driven high (on state)."
        category: "Steady"
      - value: 2
        label: "Steady Active Lo"
        description: "Line is continuously driven low (off state)."
        category: "Steady"
      - value: 3
        label: "Pulse Active Hi"
        description: "Line briefly pulses high when an event triggers it; timing controlled by Delay Interval 1."
        category: "Pulse"
      - value: 5
        label: "Blink A Active Hi"
        description: "Line blinks continuously with the A timing pattern (on/off intervals from Delay section)."
        category: "Blink"
    citation: "Section 3.1: Output Function Field, pages 17-20"
```

## Guidelines

### For all fields
- `description` should explain what the field does in practical terms, not just repeat the CDI label
- **Descriptions support Markdown** — use `**bold**` for emphasis, `*italic*` for alternatives, and bullet points for lists of options/behaviors
- If the CDI already has a good `<description>`, enhance it with manual context rather than replacing it
- Write for model railroad hobbyists, not protocol engineers
- If prior extraction context is available, use it to write more informed descriptions

### For enum fields (fields with `<map>`)
- Include every option value from the CDI's `<map>` entries — do not skip any
- `value` must exactly match the `<property>` integer from the CDI
- `label` must exactly match the `<value>` string from the CDI
- `description` for each option should explain what it does, not just restate the label (supports **Markdown**)
- Use `category` to group related options when the manual describes them in families (e.g., Steady, Pulse, Blink, Sample mode families)

### For numeric fields
- Include `units` if the manual or CDI specifies them
- Include `validRange` with min/max if the CDI or manual specifies bounds
- Include `typicalValues` if the manual suggests common values

### For eventid fields
- Describe the role this event ID plays in the node's operation
- If event-roles.json is available, reference the producer/consumer classification
- Note whether it's something the user assigns or something that's auto-generated

### For string fields
- Note the maximum length from CDI `size` attribute
- Describe what the string is used for

## Important

- Every leaf field in the CDI MUST have an entry — do not skip any
- For replicated groups, describe the field template once (not per-instance) — it applies to all instances
- Same-named sibling groups must be distinguished with index ranges
- Do NOT invent option values that don't exist in the CDI's `<map>`
- **YAML formatting**: Use the pipe (`|`) syntax for multiline description text. Quote values containing special characters. Use `- ` for bullet lists within description fields (supports Markdown).

## Output File

Save the output as `profiles/<node-name>/field-descriptions.yaml` (e.g., `profiles/tower-lcc/field-descriptions.yaml`). The .yaml extension enables easy rendering of Markdown content in the UX. This file will be used as shared context by the recipes extraction skill.

## Tip: Large CDIs

If the CDI is large (many segments with many fields), the output may be too long for a single response. In that case, run this skill per-segment using page ranges from the outline: extract Port I/O fields first (pages from outline), then Conditionals (pages from outline), etc., and merge the results into a single JSON file.
