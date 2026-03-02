# Prompt C: Section Description Extraction

## Context

You are creating section-level descriptions for a node's configuration hierarchy. You will receive:
1. The node's CDI XML (defines the configuration structure — authoritative for element names and hierarchy)
2. Extracted text from the node's PDF manual (provides meaning and purpose for each section)

## Task

For every **segment** and **group** in the CDI XML, extract or write a 1–3 sentence purpose statement that explains:
- What this section is for
- What physical function or hardware it corresponds to (if applicable)
- When a user would configure it

## Output Format

Produce a JSON object matching this schema exactly:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI>",
    "model": "<from CDI>"
  },
  "sections": [
    {
      "cdiPath": "<slash-separated path, e.g., Port I/O or Port I/O/Line>",
      "level": "segment or group",
      "name": "<CDI element name>",
      "description": "<1-3 sentence purpose statement>",
      "citation": "<manual section reference>"
    }
  ]
}
```

## Guidelines

- Cover every segment (top-level) and every distinct group template (not every replicated instance — describe the template once)
- For replicated groups (e.g., `Line` with `replication="16"`), describe what one instance represents (e.g., "one physical I/O line on the board")
- Descriptions should be written for model railroad hobbyists, not protocol engineers
- Use present tense and active voice
- If the CDI provides a `<description>` that is already clear and complete, you may use it as-is or enhance it with manual context
- If the CDI provides no `<description>` or only a terse one, write a description from the manual's explanation

## Important

- Every segment and every group template in the CDI MUST have an entry. Do not skip any.
- Paths must match CDI element names exactly.
- Same-named sibling groups must be distinguished with index ranges (e.g., `Event[0-5]` vs `Event[6-11]`).

## Inputs

[Attach CDI XML and extracted manual text here]
