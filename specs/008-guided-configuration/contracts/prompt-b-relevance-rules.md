# Prompt B: Conditional Relevance Rule Extraction

## Context

You are analyzing a node's configuration to identify sections that become irrelevant based on other field values. You will receive:
1. The node's CDI XML (defines the configuration structure — authoritative for element names, paths, and enum values)
2. Extracted text from the node's PDF manual (describes when sections apply and when they don't)

## Task

Identify every configuration relationship where a section (group of fields) becomes **irrelevant** — meaning it has no effect on node behavior — based on the current value of another field. For each such relationship, produce a rule specifying what becomes irrelevant, what controls it, and which values trigger the irrelevance.

## What to Look For

- Fields with a "disabled" or "no function" option (enum value 0 is often this) that make dependent sections meaningless
- Output-related sections that are irrelevant when output is disabled
- Input-related sections that are irrelevant when input is disabled  
- Timing/delay sections that only apply to certain output modes (e.g., delays only matter for pulse/blink modes, not steady)
- Event sections whose source is overridden by another mechanism (e.g., Track Speed is irrelevant when Source uses variable events directly)
- Any section the manual describes as "only used when..." or "does not apply if..."

## Output Format

Produce a JSON object matching this schema exactly:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI>",
    "model": "<from CDI>"
  },
  "rules": [
    {
      "id": "R001",
      "affectedSection": "<CDI path of the section made irrelevant>",
      "controllingField": "<CDI path of the field that determines relevance>",
      "irrelevantWhen": [0],
      "irrelevantValueLabels": ["No Function"],
      "explanation": "<user-facing explanation, e.g., 'Consumer events only apply when an Output Function is set.'>",
      "citation": "<manual section or passage confirming this>"
    }
  ]
}
```

### Path Conventions
- Use `/` to separate hierarchy levels
- For replicated groups, paths are relative to one instance (the rule applies to all instances)
- `controllingField` should be a sibling or ancestor within the same replicated group instance
- `irrelevantWhen` contains the raw `<property>` integer values from the CDI `<map>`
- `irrelevantValueLabels` contains the corresponding `<value>` display labels from the CDI `<map>`

## Important

- Only include rules where the manual explicitly or clearly implicitly describes the dependency. Do not speculate.
- The `explanation` field will be shown to end users in the configuration UI — write it clearly and concisely.
- Each rule should describe a single dependency (one controlling field → one affected section). If a section depends on multiple fields, create separate rules.
- Verify that all `irrelevantWhen` values actually exist in the CDI's `<map>` for the controlling field.
- Number rules sequentially: R001, R002, etc.

## Inputs

[Attach CDI XML and extracted manual text here]
