---
name: profile-5-recipes
description: Extract usage recipes and step-by-step configuration tasks from an LCC node's CDI XML and PDF manual. Keywords: CDI, recipe, configuration, howto, step-by-step, wiring, setup, profile extraction.
---

# Extract Usage Recipes

Identify common configuration tasks described in the node's manual and produce structured recipes — each listing the fields to set, the values to use, and why each setting is needed.

## When to Use

Use this skill when you need to create step-by-step configuration guides for common real-world tasks (e.g., "Configure a push button input", "Set up a blinking LED output"). These recipes populate the guided configuration panel. This is typically the fifth and final extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML), `pdfFile` (path to PDF manual), and topics that identify pages with examples/recipes.
2. **event-roles.json** — from `profile-1-event-roles` (optional but recommended). Helps recipes correctly reference producer vs consumer events.
3. **relevance-rules.json** — from `profile-2-relevance-rules` (optional but recommended). Helps recipes avoid configuring sections that would be irrelevant for the task.
4. **section-descriptions.json** — from `profile-3-section-descriptions` (optional but recommended). Provides context about what each section does.
5. **field-descriptions.json** — from `profile-4-field-descriptions` (optional but recommended). Provides field-level detail including valid options and their meanings.

**No other file paths needed** — read the CDI XML and PDF file paths from `manual-outline.json`, then use the `pdf-utilities` `read_pdf` tool with `pageRange` parameter to extract pages with "example", "recipe", "step-by-step", or "howto" topics (identified in the outline).

## Task

Identify every common configuration task described in the manual. For each task, produce a structured recipe that lists the fields to set, the values to use, and why each setting is needed.

## What to Look For

- Step-by-step configuration examples in the manual
- Wiring diagrams that correspond to specific field settings
- "How to" or "Getting Started" sections
- Common use cases mentioned in section introductions (e.g., "To configure a push button...", "For a blinking LED output...")
- Tables that show recommended settings for different scenarios
- Conditional logic examples (timer setups, AND/OR conditions)

## Output Format

Produce a YAML file with embedded Markdown text. The file should be saved as `recipes.yaml` in your profile directory.

```yaml
nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"

recipes:
  - name: "Push Button Input"
    scope: "Port I/O/Line"
    description: |
      Configure a physical push-button switch to send an event when pressed. 
      This is the foundation for operator controls and sensor inputs.
    prerequisites: |
      * Wire a normally-open push button to a digital input pin (lines 0–15)
      * Ground the other side of the button, or wire to +5V depending on desired polarity
    steps:
      - order: 1
        field: "Port I/O/Line/Input Function"
        value: "Active Hi"
        rawValue: 1
        rationale: |
          Enables the input to read the button state. "Active Hi" means the line reads 
          high (1) when pressed, low (0) when released. Choose based on your wiring polarity.
      - order: 2
        field: "Port I/O/Line/Event[0]/Command"
        value: "<copy your desired EventID from the Node ID section>"
        rawValue: null
        rationale: |
          Assign the event that fires when the button is pressed. This event can trigger 
          other nodes or conditionals. Copy the EventID from your node's ID assignment.
      - order: 3
        field: "Port I/O/Line/Event[0]/Action"
        value: "On (Line Active)"
        rawValue: 1
        rationale: "The line stays active (high) while the button is pressed."
    citation: "Section 3.3: Input Examples, page 28"

  - name: "Blinking LED Output"
    scope: "Port I/O/Line"
    description: |
      Configure an output line to blink an LED with adjustable on/off timing. 
      Use for status indicators or attention-getting effects.
    prerequisites: |
      * Wire an LED (with current-limiting resistor) to an output pin
      * Ensure the pin can source/sink sufficient current for your LED
    steps:
      - order: 1
        field: "Port I/O/Line/Output Function"
        value: "Blink A Active Hi"
        rawValue: 5
        rationale: |
          Enables continuous blinking using timing pattern A. The LED will cycle 
          on and off based on the delays configured below.
      - order: 2
        field: "Port I/O/Line/Delay/Interval 1/Delay Time"
        value: 500
        rawValue: 500
        rationale: "Controls the on-time duration for the blink cycle (milliseconds)."
      - order: 3
        field: "Port I/O/Line/Delay/Interval 1/Units"
        value: "Milliseconds"
        rawValue: 0
        rationale: "Specifies that the delay time is in milliseconds (most responsive)."
      - order: 4
        field: "Port I/O/Line/Delay/Interval 2/Delay Time"
        value: 500
        rawValue: 500
        rationale: "Controls the off-time duration. Equal on/off times create a balanced blink."
    citation: "Section 3.2: Output Examples, pages 23-25"
```

## Guidelines

- Each recipe should be a complete, self-contained set of steps — a user following them should achieve the described result
- **`description`, `prerequisites`, and `rationale` fields support Markdown** — use `**bold**` for emphasis, `*italic*` for alternatives, and bullet lists for multi-step requirements
- Field paths must reference actual fields in the CDI XML
- For enum fields, `value` is the human-readable label and `rawValue` is the integer `<property>` value from the CDI `<map>`
- For non-enum fields, `rawValue` can be null
- Steps should be ordered logically (set the mode first, then configure events, then timing)
- Include at least the physical/wiring prerequisites if the manual mentions them
- Prioritize the most common use cases — aim for 3–5 recipes for the main I/O segment, 1–2 for conditional logic if applicable
- If relevance-rules.json is available, ensure recipes don't include steps for sections that would be irrelevant given the recipe's own settings
- If field-descriptions.json is available, use it to verify field paths and option values are correct

## Important

- All field paths and enum values must exist in the CDI XML — do not reference fields or values that aren't in the CDI
- Recipes describe what to configure and why — they do NOT auto-fill values. The user makes the changes guided by the recipe
- Write for model railroad hobbyists — avoid protocol jargon
- **YAML formatting**: Use `|` for multiline text (descriptions, prerequisites, rationales). Use `- ` for bullet lists within these fields (supports Markdown). Preserve indentation for readability.

## Output File

Save the output as `profiles/<node-name>/recipes.yaml` (e.g., `profiles/tower-lcc/recipes.yaml`). The .yaml extension enables easy rendering of Markdown content in the UX.
