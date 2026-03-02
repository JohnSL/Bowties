---
name: profile-1-event-roles
description: Extract event role classifications (Producer/Consumer) from an LCC node's CDI XML and PDF manual. Use when creating a node profile to classify eventid groups. Keywords: CDI, event role, producer, consumer, eventid, classification, profile extraction.
---

# Extract Event Role Classifications

Classify every event group in a node's CDI XML as **Producer** or **Consumer** by combining structural analysis of the CDI with contextual information from the node's PDF manual.

## When to Use

Use this skill when you need to determine which event groups in an LCC node's configuration represent events the node **sends** (Producer) versus events the node **listens for** (Consumer). This is typically the first extraction step when building a node profile.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML), `pdfFile` (path to PDF manual), and page ranges for each section.

**No other file paths needed** — read the CDI XML and PDF file paths from `manual-outline.json`, then use the `pdf-utilities` `read_pdf` tool with `pageRange` parameter to extract the relevant pages. For Tower-LCC, this typically means pages covering event slot descriptions (usually in Port I/O and Conditionals sections from the outline).

## Task

For every group in the CDI XML that contains `<eventid>` elements, determine whether the group represents:

- **Producer** events — the node sends these events onto the network when a condition occurs
- **Consumer** events — the node listens for these events from the network and acts on them

## Classification Rules

1. **Consumer indicators**: CDI description contains "(C)", "When this event occurs", "Command", "set true/false"; manual describes the event as something the node *receives* or *reacts to*
2. **Producer indicators**: CDI description contains "(P)", "this event will be sent", "Indicator", "Upon this action"; manual describes the event as something the node *emits*, *produces*, or *generates*
3. If a group contains both producer and consumer eventid elements as separate children (not a mix within one group), classify each child separately
4. When two sibling groups share the same `<name>`, distinguish them by document order (0-based indices) and list their child field names to confirm which is which

## Output Format

Produce a JSON object matching this schema:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI <identification>>",
    "model": "<from CDI <identification>>"
  },
  "roles": [
    {
      "cdiPath": "<slash-separated path, e.g., Port I/O/Line/Event[0-5]>",
      "role": "Producer | Consumer",
      "childFields": ["<names of child elements in this group>"],
      "citation": "<quote or reference from the manual confirming this classification>",
      "confidence": "High | Medium",
      "notes": "string | null"
    }
  ]
}
```

### CDI Path Conventions

- Use `/` to separate hierarchy levels
- For replicated groups, use the template name (not instance-specific)
- For same-named siblings, append index ranges: `Event[0-5]` for the first group (6 replications), `Event[6-11]` for the second
- Index ranges reflect expanded instance indices (0-based, contiguous across the parent's replication)

## Important

- Every group containing `<eventid>` elements MUST appear in the output — do not skip any
- The CDI XML is authoritative for structure and names; the manual is authoritative for role classification
- If the manual does not clearly state the role, use CDI description hints ("(C)" = Consumer, "(P)" = Producer) and set confidence to "Medium"
- Do NOT invent paths that don't exist in the CDI XML

## Output File

Save the output as `profiles/<node-name>/event-roles.json` (e.g., `profiles/tower-lcc/event-roles.json`). This file will be used as shared context by subsequent extraction skills (relevance rules, descriptions, etc.).
