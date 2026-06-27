---
name: profile-1-event-roles
description: Extract event role classifications (Producer/Consumer) from an LCC node's CDI XML and PDF manual. Use when creating a node profile to classify eventid groups. Keywords -- CDI, event role, producer, consumer, eventid, classification, profile extraction.
---

# Extract Event Role Classifications

Classify every event group in a node's CDI XML as **Producer** or **Consumer** by combining structural analysis of the CDI with contextual information from the node's PDF manual.

## When to Use

Use this skill when you need to determine which event groups in an LCC node's configuration represent events the node **sends** (Producer) versus events the node **listens for** (Consumer). This is typically the first extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML), `pdfFile` (path to PDF manual), and page ranges for each section.

All CDI/PDF paths are read from `manual-outline.json` — do not ask for them.

## Workflow

### Step 1 — generate the skeleton

Run the shared CLI to emit one entry per group containing `<eventid>` children, with `cdiPath` and `childFields` already populated:

```pwsh
uv run .github/skills/_lib/profile_tools.py skeleton events profile-extractions/<node-name>
```

This produces `event-roles.skeleton.json`. The skeleton:

- Includes every segment or group that has at least one direct `<eventid>` child.
- Pre-fills `cdiPath` (with `[N]` / `[N-M]` index suffixes where same-name siblings exist).
- Pre-fills `childFields` with the eventid child names.
- Leaves `role`, `citation`, `confidence`, and `notes` as `TODO` placeholders.

### Step 2 — classify each entry

Use `pdf-utilities.read_pdf` with the page ranges identified as relevant to event-bearing sections (typically Port I/O, Conditionals, Track Circuits) in the outline. For each entry in the skeleton, fill in:

- `role` — `Producer` (the node sends) or `Consumer` (the node listens).
- `citation` — quote or reference from the manual confirming the classification.
- `confidence` — `High` (manual states it clearly) or `Medium` (inferred from CDI hints).
- `notes` — `null` unless there's a useful caveat (e.g., split groups, replicated context).

If a single skeleton group contains *both* producer and consumer eventids as separate children (e.g., a "Rule" group with a consumer `set aspect` and producer `aspect is set` / `aspect cleared`), split it into two entries — one Consumer (for the consumer eventids) and one Producer (for the producer eventids) — and list the relevant `childFields` in each.

### Step 3 — rename and validate

Rename `event-roles.skeleton.json` to `event-roles.json`, then run `profile-6-validate` to confirm every path and child field still resolves.

## Classification Rules

1. **Consumer indicators**: CDI description contains `(C)`, "When this event occurs", "Command", "set true/false"; manual describes the event as something the node *receives* or *reacts to*.
2. **Producer indicators**: CDI description contains `(P)`, "this event will be sent", "Indicator", "Upon this action"; manual describes the event as something the node *emits*, *produces*, or *generates*.
3. If a group contains both producer and consumer eventid elements as separate children, classify each subset separately (as described in Step 2 above).
4. When two sibling groups share the same `<name>`, the skeleton has already distinguished them with `[N]` / `[N-M]` suffixes — list their `childFields` in the skeleton to confirm which is which.

## Output Format

The skeleton — and the final `event-roles.json` — matches this schema:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI <identification>>",
    "model": "<from CDI <identification>>"
  },
  "roles": [
    {
      "cdiPath": "Port I/O/Line/Event[0-5]",
      "role": "Producer | Consumer",
      "childFields": ["<names of eventid child elements>"],
      "citation": "<quote or reference from the manual>",
      "confidence": "High | Medium",
      "notes": "string | null"
    }
  ]
}
```

### CDI Path Conventions

- `/` separates hierarchy levels.
- Replicated groups use the template name (not instance-specific paths).
- Same-named siblings are distinguished by `[N]` / `[N-M]` index suffix (the skeleton emits these automatically).

## Important

- Every group containing `<eventid>` elements MUST appear in the output — the skeleton enforces this; do not delete entries.
- The CDI XML is authoritative for structure and names; the manual is authoritative for role classification.
- If the manual does not clearly state the role, use CDI description hints (`(C)` / `(P)`) and set `confidence` to `Medium`.
- Do NOT invent paths that don't exist in the CDI.

## Output File

`profile-extractions/<node-name>/event-roles.json`. Used as shared context by `profile-2`, `profile-3`, `profile-4`, and `profile-7`.
