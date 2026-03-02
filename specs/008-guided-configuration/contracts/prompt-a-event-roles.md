# Prompt A: Event Role Extraction

## Context

You are analyzing a node's configuration to classify event groups as Producer or Consumer. You will receive:
1. The node's CDI XML (defines the configuration structure — authoritative for element names, paths, and enum values)
2. Extracted text from the node's PDF manual (provides meaning, role descriptions, and operational context)

## Task

For every group in the CDI XML that contains `<eventid>` elements, determine whether the group represents **Producer** events (the node sends these events onto the network when a condition occurs) or **Consumer** events (the node listens for these events from the network and acts on them).

## Classification Rules

- **Consumer**: The CDI description contains patterns like "(C)", "When this event occurs", "Command", "set true/false", or the manual describes the event as something the node *receives* or *reacts to*.
- **Producer**: The CDI description contains patterns like "(P)", "this event will be sent", "Indicator", "Upon this action", or the manual describes the event as something the node *emits*, *produces*, or *generates*.
- If a group contains both producer and consumer eventid elements as separate children (not a mix within one group), classify each child separately.
- When two sibling groups share the same `<name>`, distinguish them by document order (0-based indices) and list their child field names to confirm which is which.

## Output Format

Produce a JSON object matching this schema exactly:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI <identification>",
    "model": "<from CDI <identification>"
  },
  "roles": [
    {
      "cdiPath": "<slash-separated CDI element names, e.g., Port I/O/Line/Event[0-5]>",
      "role": "Producer or Consumer",
      "childFields": ["<names of child elements in this group>"],
      "citation": "<quote or reference from the manual confirming this classification>",
      "confidence": "High or Medium",
      "notes": null
    }
  ]
}
```

### Path Conventions
- Use `/` to separate hierarchy levels
- For replicated groups, use the template name (not instance-specific)
- For same-named siblings, append index ranges: `Event[0-5]` for the first group (6 replications), `Event[6-11]` for the second
- Index ranges reflect the expanded instance indices (0-based, contiguous across the parent's replication)

## Important

- Every group containing `<eventid>` elements MUST appear in the output — do not skip any.
- The CDI XML is the authoritative source for structure and names. The manual is the authoritative source for role classification.
- If the manual does not clearly state the role, use the CDI description hints ("(C)" = Consumer, "(P)" = Producer) and set confidence to "Medium".
- Do NOT invent paths that don't exist in the CDI XML.

## Inputs

[Attach CDI XML and extracted manual text here]
