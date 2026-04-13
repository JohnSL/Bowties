---
name: profile-2-relevance-rules
description: Extract conditional relevance rules from an LCC node's CDI XML and PDF manual. Identifies configuration sections that become irrelevant based on other field values. Keywords -- CDI, relevance, conditional, irrelevant, disabled, profile extraction.
---

# Extract Conditional Relevance Rules

Identify every configuration relationship where a section (group of fields) becomes **irrelevant** — meaning it has no effect on node behavior — based on the current value of another field.

## When to Use

Use this skill when you need to determine which configuration sections can be hidden or greyed out in a configuration UI because they have no effect given the current settings. This is typically the second extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML), `pdfFile` (path to PDF manual), and page ranges for each section.
2. **event-roles.json** — from `profile-1-event-roles` (optional but recommended). Helps identify rules like "consumer events are irrelevant when output function is disabled" by knowing which groups are consumer vs producer.

**No other file paths needed** — read the CDI XML and PDF file paths from `manual-outline.json`, then use the `pdf-utilities` `read_pdf` tool with `pageRange` parameter to extract pages describing conditional field dependencies and disabled/no-function modes.

## Task

For each dependency relationship found, produce a rule specifying:
- What section becomes irrelevant
- What field controls the relevance
- Which values of that field trigger the irrelevance
- A user-facing explanation

## What to Look For

- Fields with a "disabled" or "no function" option (enum value 0 is often this) that make dependent sections meaningless
- Output-related sections that are irrelevant when output is disabled
- Input-related sections that are irrelevant when input is disabled
- **Mutually exclusive function fields**: when the manual says a line must be configured as *either* X or Y (not both), and setting X makes Y irrelevant — even if Y is not zero. For example: setting an output function to Steady, Pulse, or Blink makes the input function irrelevant because the hardware prioritizes the output. In this case `irrelevantWhen` lists all the active output-mode values (e.g., `[1,2,3,4,5,6,7,8]`), not just zero. Look for manual language like "takes priority", "overrides", "must only have one", or "does not apply when [other field] is active".
- **Exceptions within a mutually-exclusive relationship**: some values legitimately use both fields simultaneously (e.g., Sample modes that both drive output and read input). Those values must be *excluded* from `irrelevantWhen` even though they are non-zero.
- Timing/delay sections that only apply to certain output modes (e.g., delays only matter for pulse/blink modes, not steady)
- Event sections whose source is overridden by another mechanism
- Any section the manual describes as "only used when..." or "does not apply if..."

## Output Format

Produce a JSON object matching this schema:

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
    },
    {
      "id": "R002",
      "affectedSection": "<CDI path of section made irrelevant by an active — not disabled — controlling value>",
      "controllingField": "<CDI path of controlling field>",
      "irrelevantWhen": [1, 2, 3, 4],
      "irrelevantValueLabels": ["Mode A", "Mode B", "Mode C", "Mode D"],
      "explanation": "<user-facing explanation, e.g., 'When a Steady or Pulse output is active, the hardware prioritizes the output and the input function has no effect. To use this line as an input, first set Output Function to No Function.'>",
      "citation": "<manual section confirming the priority/override rule>"
    }
  ]
}
```

### CDI Path Conventions

- Use `/` to separate hierarchy levels
- For replicated groups, paths are relative to one instance (the rule applies to all instances)
- `controllingField` should be a sibling or ancestor within the same replicated group instance
- `irrelevantWhen` contains the raw `<property>` integer values from the CDI `<map>`
- `irrelevantValueLabels` contains the corresponding `<value>` display labels

## Important

- Only include rules where the manual explicitly or clearly implicitly describes the dependency — do not speculate
- The `explanation` field will be shown to end users in the configuration UI — write it clearly and concisely, including what the user should do if they want to use the suppressed section (e.g., "To use this line as an input, first set Output Function to 'No Function'")
- Each rule should describe a single dependency (one controlling field → one affected section). If a section depends on multiple fields, create separate rules
- `irrelevantWhen` can contain **any set of enum values** — not only zero. For mutually exclusive functions, list every value of the controlling field that makes the affected section irrelevant, and carefully exclude values where both fields are legitimately active simultaneously (e.g., Sample modes)
- Verify that all `irrelevantWhen` values actually exist in the CDI's `<map>` for the controlling field; verify that excluded values are also real map entries
- Number rules sequentially: R001, R002, etc.

## Output File

Save the output as `profiles/<node-name>/relevance-rules.json` (e.g., `profiles/tower-lcc/relevance-rules.json`). This file will be used as shared context by subsequent extraction skills (descriptions, recipes, etc.).
