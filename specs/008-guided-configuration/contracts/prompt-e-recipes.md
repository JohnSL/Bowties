# Prompt E: Usage Guidance & Recipe Extraction

## Context

You are extracting common configuration tasks ("recipes") from a node's documentation. You will receive:
1. The node's CDI XML (defines the configuration structure — authoritative for field names and enum values)
2. Extracted text from the node's PDF manual (describes common configurations, wiring scenarios, and step-by-step setup procedures)

## Task

Identify every common configuration task described in the manual. For each task, produce a structured recipe that lists the fields to set, the values to use, and why each setting is needed. Recipes should cover the most frequent real-world use cases for this node type.

## What to Look For

- Step-by-step configuration examples in the manual
- Wiring diagrams that correspond to specific field settings
- "How to" or "Getting Started" sections
- Common use cases mentioned in section introductions (e.g., "To configure a push button...", "For a blinking LED output...")
- Tables that show recommended settings for different scenarios
- Conditional logic examples (timer setups, AND/OR conditions)

## Output Format

Produce a JSON object matching this schema exactly:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI>",
    "model": "<from CDI>"
  },
  "recipes": [
    {
      "name": "<short descriptive name, e.g., Push Button Input>",
      "scope": "<CDI path of the applicable segment or group>",
      "description": "<1-2 sentence summary of what this recipe accomplishes>",
      "prerequisites": "<hardware or wiring requirements, or null>",
      "steps": [
        {
          "order": 1,
          "field": "<CDI path of the field to set>",
          "value": "<human-readable value — label for enums, number for integers>",
          "rawValue": 1,
          "rationale": "<why this setting is needed for this recipe>"
        }
      ],
      "citation": "<manual section reference>"
    }
  ]
}
```

## Guidelines

- Each recipe should be a complete, self-contained set of steps — a user following them should achieve the described result
- Field paths must reference actual fields in the CDI XML
- For enum fields, `value` is the human-readable label and `rawValue` is the integer `<property>` value from the CDI `<map>`
- For non-enum fields, `rawValue` can be null
- Steps should be ordered logically (set the mode first, then configure events, then timing)
- Include at least the physical/wiring prerequisites if the manual mentions them
- Prioritize the most common use cases — aim for 3-5 recipes for the main I/O segment, 1-2 for conditional logic if applicable

## Important

- All field paths and enum values must exist in the CDI XML. Do not reference fields or values that aren't in the CDI.
- Recipes describe what to configure and why — they do NOT auto-fill values. The user makes the changes guided by the recipe.
- Write for model railroad hobbyists. Avoid protocol jargon.

## Inputs

[Attach CDI XML and extracted manual text here]
