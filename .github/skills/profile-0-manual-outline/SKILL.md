---
name: profile-0-manual-outline
description: Create a structured outline of an LCC node's PDF manual with section names and page ranges. Use as the first step to index the manual for targeted reading by subsequent extraction skills. Keywords -- CDI, manual, outline, index, page ranges, profile extraction.
---

# Create Manual Outline

Analyze an LCC node's PDF manual to produce a structured index that maps manual sections (e.g., "Port I/O Configuration") to their page ranges. This allows subsequent extraction skills to read only the pages relevant to their task rather than processing the entire manual.

## When to Use

Use this skill as the **first step** of the profile extraction workflow. Run it once per node type to create a map of the manual's structure. Later skills reference this outline to extract targeted page ranges via `pdf-utilities` `read_pdf` with the `pageRange` parameter.

## Required Inputs

1. **PDF manual** — the complete node's PDF manual file (text-based, not scanned images)
2. **CDI XML** — the node's CDI XML. Used to understand the major configuration sections (Port I/O, Conditionals, Track Receiver/Transmitter, etc.) so you can match them to manual sections.

## Implementation

**IMPORTANT**: Use the `pdf-utilities` MCP server's `read_pdf` function to extract the full text of the PDF manual. This is the only step that reads the entire PDF; pass the extracted text to the task below for analysis.

Before producing the output JSON, **capture the full absolute paths** to:
1. The CDI XML file (pass via input or capture from the provided file)
2. The PDF manual file (the file being read)

These paths **must** be included in the JSON output as `cdiFile` and `pdfFile` fields so that subsequent extraction skills can locate and read them automatically.

Example: `read_pdf(filePath="D:/src/github/LCC/Bowties.worktrees/008-guided-configuration/temp/TowerLCC-manual-f.pdf")` to get the complete manual text.

## Task

Read the entire PDF manual and produce a structured outline identifying:
- Major sections (e.g., "Port I/O Configuration", "Conditional Logic")
- Page ranges for each section
- Subsection topics within each range
- Any appendices or references

Use the CDI XML as a guide to what sections should exist and help you identify them in the manual.

## Output Format

Produce a JSON object matching this schema:

```json
{
  "nodeType": {
    "manufacturer": "<from CDI <identification>>",
    "model": "<from CDI <identification>>"
  },
  "cdiFile": "<full absolute path to CDI XML file>",
  "pdfFile": "<full absolute path to PDF manual file>",
  "totalPages": 250,
  "generatedDate": "2026-02-28",
  "sections": [
    {
      "order": 1,
      "name": "<section title, e.g., Port I/O Configuration>",
      "pages": "<page range, e.g., 5-80>",
      "topics": ["<topic1>", "<topic2>", "<topic3>"],
      "relevantTo": ["<profile-skill names, e.g., profile-1, profile-3, profile-4>"],
      "notes": "string | null"
    },
    {
      "order": 2,
      "name": "Conditional Logic",
      "pages": "81-150",
      "topics": ["logic units", "variables", "actions", "conditions"],
      "relevantTo": ["profile-1", "profile-2", "profile-3", "profile-4", "profile-5"],
      "notes": null
    }
  ],
  "appendices": [
    {
      "name": "<appendix name>",
      "pages": "<page range>",
      "purpose": "<brief description>"
    }
  ]
}
```

## Guidelines

- **Page ranges**: Use format `"5-80"` for contiguous sections or `"150-160, 220-240"` for non-contiguous
- **Topics**: List 3–5 key topics covered in each section
- **relevantTo**: Identify which extraction skills (profile-1 through profile-5) will need to read this section:
  - `profile-1`: event roles → needs sections describing what events the node sends/receives
  - `profile-2`: relevance rules → needs sections describing conditional field dependencies
  - `profile-3`: section descriptions → needs all sections describing configuration purpose
  - `profile-4`: field descriptions → needs all sections describing individual fields
  - `profile-5`: recipes → needs sections with step-by-step configuration examples
- **Appendices**: Include reference tables, firmware version details, wiring diagrams with text descriptions

## Important

- Cover the entire manual — every page should belong to at least one section or appendix
- Be specific about page ranges — subsequent skills will use these to extract targetted text
- Identify sections by how they're labeled in the manual (table of contents, chapter titles, section headers)
- Total pages in all ranges should sum to the actual total page count (allowing for some overlap if a page belongs to multiple sections)

## Output File

Save the output as `manual-outline.json` in your profile directory (e.g., `profiles/tower-lcc/manual-outline.json`). All subsequent extraction skills will reference this file as shared context.

## Downstream Usage

After this skill produces the outline, subsequent skills will:

```
profile-1 (event roles):
  Read pages 5-80 and 151-170 from the manual (from outline)
  
profile-2 (relevance rules):
  Read pages 81-150 and 5-80 (from outline)

profile-3 (section descriptions):
  Read pages 5-150 (from outline)
  
profile-4 (field descriptions):
  Read pages 5-150 optimally per-section (from outline)
  
profile-5 (recipes):
  Read pages with "step-by-step", "example", "recipe" topics (from outline)
```

The outline enables efficient, targeted PDF reading across the entire extraction workflow.
