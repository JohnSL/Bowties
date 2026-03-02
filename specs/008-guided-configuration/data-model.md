# Data Model: Extraction Output Schemas

**Feature**: 008-guided-configuration (Phase 1)  
**Date**: 2026-02-28

This document defines the JSON output schemas for each extraction prompt. These schemas are the "data model" for Phase 1 — they describe the structured output that extraction prompts must produce and that the validation workflow will check.

---

## Prompt A Output: Event Role Classifications

```json
{
  "$schema": "extraction-event-roles",
  "nodeType": {
    "manufacturer": "string — e.g., RR-CirKits",
    "model": "string — e.g., Tower-LCC"
  },
  "roles": [
    {
      "cdiPath": "string — e.g., Port I/O/Line/Event[0-5]",
      "role": "Producer | Consumer",
      "childFields": ["string — fields within the group, e.g., Command, Action"],
      "citation": "string — manual section/passage confirming classification",
      "confidence": "High | Medium",
      "notes": "string | null — any ambiguity or special case"
    }
  ]
}
```

**Conventions**:
- `cdiPath` uses `/`-separated CDI element names
- For replicated groups, the path refers to the group template (e.g., `Port I/O/Line/Event[0-5]`), not individual instances
- Index ranges in brackets (e.g., `[0-5]`) disambiguate same-named sibling groups by document order
- `childFields` lists the fields that identify the group (aids disambiguation)

---

## Prompt B Output: Conditional Relevance Rules

```json
{
  "$schema": "extraction-relevance-rules",
  "nodeType": {
    "manufacturer": "string",
    "model": "string"
  },
  "rules": [
    {
      "id": "string — unique rule identifier, e.g., R001",
      "affectedSection": "string — CDI path pattern of section made irrelevant",
      "controllingField": "string — CDI path of the field that determines relevance",
      "irrelevantWhen": ["number — enum values that make the section irrelevant"],
      "irrelevantValueLabels": ["string — human-readable labels for the values"],
      "explanation": "string — user-facing explanation suitable for UI display",
      "citation": "string — manual section confirming the relationship"
    }
  ]
}
```

**Conventions**:
- `controllingField` is a sibling or ancestor field within the same replicated group instance
- `irrelevantWhen` contains the raw integer property values from CDI `<map>` entries
- `irrelevantValueLabels` contains corresponding human-readable labels (parallel array)
- `explanation` is written for end users, not developers

---

## Prompt C Output: Section Descriptions

```json
{
  "$schema": "extraction-section-descriptions",
  "nodeType": {
    "manufacturer": "string",
    "model": "string"
  },
  "sections": [
    {
      "cdiPath": "string — e.g., Port I/O or Port I/O/Line",
      "level": "segment | group",
      "name": "string — CDI element name",
      "description": "string — 1-3 sentence purpose statement",
      "citation": "string — manual section reference"
    }
  ]
}
```

---

## Prompt D Output: Field & Option Descriptions

```json
{
  "$schema": "extraction-field-descriptions",
  "nodeType": {
    "manufacturer": "string",
    "model": "string"
  },
  "fields": [
    {
      "cdiPath": "string — e.g., Port I/O/Line/Output Function",
      "name": "string — CDI element name",
      "elementType": "int | string | eventid | float",
      "description": "string — clear explanation of what the field controls",
      "cdiDescription": "string | null — original CDI description for comparison",
      "units": "string | null — for numeric fields",
      "validRange": {
        "min": "number | null",
        "max": "number | null"
      },
      "typicalValues": "string | null — common or recommended values",
      "options": [
        {
          "value": "number — enum property value from CDI map",
          "label": "string — CDI map display label",
          "description": "string — one-line explanation of what this option does",
          "category": "string | null — optional grouping, e.g., Steady, Pulse, Blink, Sample"
        }
      ],
      "citation": "string — manual section reference"
    }
  ]
}
```

**Conventions**:
- `options` array is present only for enum fields (fields with `<map>` in CDI)
- `value` matches the `<property>` in the CDI's `<relation>` entries
- `label` matches the `<value>` in the CDI's `<relation>` entries
- `category` groups related options for display (optional; used when the manual describes option families)
- `validRange` is present only for numeric fields without enums
- `cdiDescription` preserves the original CDI description for comparison during review

---

## Prompt E Output: Usage Recipes

```json
{
  "$schema": "extraction-recipes",
  "nodeType": {
    "manufacturer": "string",
    "model": "string"
  },
  "recipes": [
    {
      "name": "string — e.g., Push Button Input",
      "scope": "string — CDI path of applicable segment or group",
      "description": "string — what this recipe accomplishes",
      "prerequisites": "string | null — any wiring or hardware requirements",
      "steps": [
        {
          "order": "number — 1-based step number",
          "field": "string — CDI path of the field to set",
          "value": "string — value to set (label for enums, number for integers)",
          "rawValue": "number | null — integer value for enum fields",
          "rationale": "string — why this setting is needed"
        }
      ],
      "citation": "string — manual section reference"
    }
  ]
}
```

---

## Validation Report Structure

```json
{
  "$schema": "extraction-validation-report",
  "nodeType": {
    "manufacturer": "string",
    "model": "string"
  },
  "cdiFile": "string — path to CDI XML used for validation",
  "extractionFiles": ["string — paths to extraction JSON files validated"],
  "pathErrors": [
    {
      "extractionFile": "string",
      "entryId": "string — identifier of the extraction entry",
      "referencedPath": "string — CDI path cited in extraction",
      "error": "string — e.g., path not found in CDI XML"
    }
  ],
  "enumErrors": [
    {
      "extractionFile": "string",
      "entryId": "string",
      "field": "string — CDI path of the enum field",
      "referencedValue": "number",
      "error": "string — e.g., value 9 not in CDI map for this field"
    }
  ],
  "coverage": {
    "totalSegments": "number",
    "coveredSegments": "number",
    "totalGroups": "number",
    "coveredGroups": "number",
    "totalLeafFields": "number",
    "coveredLeafFields": "number",
    "totalEventSlots": "number",
    "coveredEventSlots": "number",
    "overallPercentage": "number"
  },
  "uncoveredSections": [
    {
      "cdiPath": "string",
      "level": "segment | group | field",
      "name": "string"
    }
  ],
  "summary": "string — pass/fail with key statistics"
}
```

---

## Entity Relationships

```
CDI XML (source of truth for structure)
  │
  ├── Prompt A output (event-roles.json)
  │     references: cdiPath → groups containing <eventid> elements
  │
  ├── Prompt B output (relevance-rules.json)
  │     references: affectedSection → CDI groups
  │                 controllingField → CDI leaf fields with <map>
  │                 irrelevantWhen → <property> values from CDI <map>
  │
  ├── Prompt C output (section-descriptions.json)
  │     references: cdiPath → segments and groups
  │
  ├── Prompt D output (field-descriptions.json)
  │     references: cdiPath → leaf fields
  │                 options[].value → <property> values from CDI <map>
  │                 options[].label → <value> strings from CDI <map>
  │
  └── Prompt E output (recipes.json)
        references: scope → segments/groups
                    steps[].field → leaf field CDI paths
                    steps[].rawValue → <property> values from CDI <map>
        │
        All ──→ Validation Report
                  cross-references paths and enum values against CDI XML
                  produces coverage statistics
                  │
                  └──→ Phase 2 Profile File (future consumer)
```
