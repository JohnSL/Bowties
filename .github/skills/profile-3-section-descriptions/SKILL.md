---
name: profile-3-section-descriptions
description: Extract section-level descriptions for every segment and group in an LCC node's CDI XML using the PDF manual. Keywords: CDI, section, segment, group, description, purpose, profile extraction.
---

# Extract Section Descriptions

Create a 1–3 sentence purpose statement for every segment and group in a node's CDI XML, drawing on the PDF manual for context that the CDI's own descriptions often lack.

## When to Use

Use this skill when you need rich, user-facing descriptions for each section of a node's configuration hierarchy. These descriptions populate the companion panel in the configuration UI. This is typically the third extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML), `pdfFile` (path to PDF manual), and page ranges for each section.
2. **event-roles.json** — from `profile-1-event-roles` (optional but recommended). Allows descriptions to reference whether a section contains producer or consumer events.
3. **relevance-rules.json** — from `profile-2-relevance-rules` (optional but recommended). Allows descriptions to note conditional relevance (e.g., "This section only applies when Output Function is set to a non-disabled value").

**No other file paths needed** — read the CDI XML and PDF file paths from `manual-outline.json`, then use the `pdf-utilities` `read_pdf` tool with `pageRange` parameter to extract all configuration sections identified in the outline.

## Task

For every **segment** and **group** in the CDI XML, extract or write a 1–3 sentence purpose statement that explains:
- What this section is for
- What physical function or hardware it corresponds to (if applicable)
- When a user would configure it

## Output Format

Produce a YAML file with embedded Markdown text. The file should be saved as `section-descriptions.yaml` in your profile directory.

```yaml
nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"

sections:
  - cdiPath: "Port I/O"
    level: "segment"
    name: "Port I/O"
    description: |
      Configures the 16 physical I/O lines on the board. Each line can 
      be independently configured as an input (sensing external signals) 
      or output (controlling relays, LEDs, solenoids). This is the heart 
      of the Tower-LCC — where you wire your physical layout devices.
    citation: "Section 3: Port I/O Configuration, pages 15-45"

  - cdiPath: "Port I/O/Line"
    level: "group"
    name: "Line"
    description: |
      Represents one physical I/O line on the board. Configure the output 
      function (how the pin drives external devices), input function 
      (how it reads external sensors), timing delays for pulses/blinks, 
      and the events that trigger state changes. Each of the 16 lines 
      operates independently.
    citation: "Section 3.1: Configuring Individual Lines, pages 16-35"

  - cdiPath: "Conditionals"
    level: "segment"
    name: "Conditionals"
    description: |
      Sets up conditional logic using two variables and combining logic 
      (AND, OR, XOR, etc.). Use this to create complex behaviors — for example, 
      "send this event only if both track circuits are occupied" or 
      "blink this output when either of two conditions is true."
    citation: "Section 4: Conditional Logic, pages 46-80"
```

## Guidelines

- Cover every segment (top-level) and every distinct group template (not every replicated instance — describe the template once)
- For replicated groups (e.g., `Line` with `replication="16"`), describe what one instance represents (e.g., "one physical I/O line on the board")
- **Descriptions support Markdown** — use `**bold**` for emphasis, `*italic*` for alternatives, and line breaks for readability
- Write for model railroad hobbyists, not protocol engineers
- Use present tense and active voice
- If the CDI provides a `<description>` that is already clear and complete, you may use it as-is or enhance it with manual context
- If the CDI provides no `<description>` or only a terse one, write a description from the manual's explanation
- If event-roles.json is available, incorporate role context (e.g., "This group contains the consumer events for this I/O line — the events the node listens for to trigger actions")
- If relevance-rules.json is available, note conditional relevance where applicable (e.g., "This section only applies when the Output Function is set to a pulse or blink mode")

## Important

- Every segment and every group template in the CDI MUST have an entry — do not skip any
- Paths must match CDI element names exactly
- Same-named sibling groups must be distinguished with index ranges (e.g., `Event[0-5]` vs `Event[6-11]`)
- **YAML formatting**: Use the pipe (`|`) syntax for multiline description text to preserve readability. Quote values containing special characters like quotes or colons. Example: `description: |` followed by indented text.

## Output File

Save the output as `profiles/<node-name>/section-descriptions.yaml` (e.g., `profiles/tower-lcc/section-descriptions.yaml`). The .yaml extension enables easy rendering of Markdown content in the UX. This file will be used as shared context by subsequent extraction skills (field descriptions, recipes).
