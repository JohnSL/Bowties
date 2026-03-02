# Prompt D: Field & Option Description Extraction

## Context

You are creating detailed descriptions for every leaf field in a node's configuration. You will receive:
1. The node's CDI XML (defines the configuration structure — authoritative for field names, types, enum maps, and existing descriptions)
2. Extracted text from the node's PDF manual (provides detailed explanations, typical values, and operating guidance)

## Task

For every **leaf field** (int, string, eventid, float) in the CDI XML, produce a clear description of what the field controls. For enum fields (fields with `<map>` entries), additionally describe each option value.

## Output Format

Produce a JSON object matching this schema exactly:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI>",
    "model": "<from CDI>"
  },
  "fields": [
    {
      "cdiPath": "<slash-separated path, e.g., Port I/O/Line/Output Function>",
      "name": "<CDI element name>",
      "elementType": "int | string | eventid | float",
      "description": "<clear explanation of what this field controls>",
      "cdiDescription": "<original CDI <description> text, or null if absent>",
      "units": "<units for numeric fields, e.g., milliseconds, or null>",
      "validRange": {
        "min": null,
        "max": null
      },
      "typicalValues": "<common or recommended values, or null>",
      "options": [
        {
          "value": 0,
          "label": "<CDI map <value> text>",
          "description": "<one-line explanation of what this option does>",
          "category": "<optional grouping name, or null>"
        }
      ],
      "citation": "<manual section reference>"
    }
  ]
}
```

## Guidelines

### For all fields:
- `description` should explain what the field does in practical terms, not just repeat the CDI label
- If the CDI already has a good `<description>`, enhance it with manual context rather than replacing it
- Write for model railroad hobbyists, not protocol engineers

### For enum fields (fields with `<map>`):
- Include every option value from the CDI's `<map>` entries — do not skip any
- `value` must exactly match the `<property>` integer from the CDI
- `label` must exactly match the `<value>` string from the CDI
- `description` for each option should explain what it does, not just restate the label
- Use `category` to group related options when the manual describes them in families (e.g., Output Function has Steady, Pulse, Blink, and Sample mode families)

### For numeric fields:
- Include `units` if the manual or CDI specifies them
- Include `validRange` with min/max if the CDI name or manual specifies bounds (e.g., "Delay Time (1-60000)" → min: 1, max: 60000)
- Include `typicalValues` if the manual suggests common values

### For eventid fields:
- Describe the role this event ID plays in the node's operation
- Note whether it's something the user assigns or something that's auto-generated

### For string fields:
- Note the maximum length from CDI `size` attribute
- Describe what the string is used for

## Important

- Every leaf field in the CDI MUST have an entry. Do not skip any.
- For replicated groups, describe the field template once (not per-instance) — it applies to all instances.
- Same-named sibling groups must be distinguished with index ranges.
- Do NOT invent option values that don't exist in the CDI's `<map>`.

## Inputs

[Attach CDI XML and extracted manual text here]
