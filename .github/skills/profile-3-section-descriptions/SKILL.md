---
name: profile-3-section-descriptions
description: Extract section-level descriptions for every segment and group in an LCC node's CDI XML using the PDF manual. Keywords -- CDI, section, segment, group, description, purpose, profile extraction.
---

# Extract Section Descriptions

Create a 1–3 sentence purpose statement for every segment and group in a node's CDI XML, drawing on the PDF manual for context that the CDI's own descriptions often lack.

## When to Use

Use this skill when you need rich, user-facing descriptions for each section of a node's configuration hierarchy. These descriptions populate the companion panel in the configuration UI. This is typically the third extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — produced by `profile-0-manual-outline`. Contains `cdiFile`, `pdfFile`, and page ranges.
2. **event-roles.json** — from `profile-1-event-roles` (optional but recommended). Lets descriptions reference whether a section contains producer or consumer events.
3. **relevance-rules.json** — from `profile-2-relevance-rules` (optional but recommended). Lets descriptions note conditional relevance.

All CDI/PDF paths are read from `manual-outline.json`.

## Workflow

### Step 1 — generate the skeleton

Run the shared CLI to emit one entry per segment and group, with the `cdiPath` / `level` / `name` already populated and TODO placeholders for the narrative fields:

```pwsh
uv run .github/skills/_lib/profile_tools.py skeleton sections profile-extractions/<node-name>
```

This writes `section-descriptions.skeleton.yaml`. The skeleton:

- Emits one entry per top-level segment.
- Emits one entry per group template (not per replicated instance).
- Adds `[N]` / `[N-M]` index suffixes only where same-name siblings exist (e.g., `Conditionals/Logic/Action[0]` vs `Conditionals/Logic/Action[1-4]`).
- Preserves literal `/` inside element names (e.g., `Port I/O-1/Line/Commands/Consumers`).

### Step 2 — fill in narrative fields

Use `pdf-utilities.read_pdf` with `pageRange` values from the outline to read the relevant sections of the manual. Replace each TODO with:

- `description` — a 1–3 sentence purpose statement (Markdown supported).
- `citation` — manual section + page reference.

### Step 3 — rename and validate

Rename `section-descriptions.skeleton.yaml` to `section-descriptions.yaml`, then run `profile-6-validate` against the node directory to confirm every emitted path still resolves.

## Guidelines

- Cover every segment and every distinct group template (the skeleton guarantees this — do not delete entries).
- For replicated groups, describe what one instance represents (e.g., "one physical I/O line on the board").
- **Descriptions support Markdown** — `**bold**` for emphasis, `*italic*` for alternatives, line breaks for readability.
- Write for model railroad hobbyists, not protocol engineers.
- Present tense, active voice.
- If the CDI provides a clear `<description>` already, use it as-is or enhance with manual context.
- If `event-roles.json` is available, mention role context (e.g., "contains the consumer events for this line").
- If `relevance-rules.json` is available, note conditional relevance (e.g., "only applies when Output Function is set to pulse or blink").

## Important

- Every segment and every group template MUST have an entry (the skeleton enforces this).
- Paths must match the skeleton — do not rewrite the `[N]` / `[N-M]` suffixes by hand.
- **YAML formatting**: use the pipe (`|`) syntax for multiline `description` text; quote values containing special characters such as quotes or colons.

## Output File

`profile-extractions/<node-name>/section-descriptions.yaml`. The .yaml extension enables easy rendering of Markdown content in the UX. Used as shared context by field-descriptions and recipes extraction.
