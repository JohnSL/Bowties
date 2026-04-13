---skill
---
name: profile-7-assemble
description: Assemble a .profile.yaml structure profile from Phase 1 extraction outputs (event-roles.json and relevance-rules.json). Converts CDI paths from index-range notation to '#N' ordinal notation and produces a ready-to-validate profile file. Keywords -- CDI, profile, assemble, .profile.yaml, structure profile, event roles, relevance rules, phase 2, guided configuration.
---

# Assemble Structure Profile from Extraction Outputs

Combine Phase 1 extraction outputs into a complete `.profile.yaml` structure profile file ready for bundling with the Bowties application.

## When to Use

Use this skill after Phase 1 extraction (skills 0–6) is complete and validated. This is the final authoring step before the profile file is placed in `app/src-tauri/profiles/` and bundled with the application.

## Required Inputs

1. **manual-outline.json** — the structured index produced by `profile-0-manual-outline`. Contains `cdiFile` (path to CDI XML), `nodeType` (manufacturer, model).
2. **event-roles.json** — produced by `profile-1-event-roles`. Contains classified event role groups.
3. **relevance-rules.json** — produced by `profile-2-relevance-rules`. Contains conditional relevance rules.

**Read all file paths from `manual-outline.json`** — do not ask for or assume file locations.

## Task

Produce a single `.profile.yaml` file by:
1. Extracting `nodeType` from `manual-outline.json`.
2. Converting every entry in `event-roles.json` to a `eventRoles` list entry.
3. Converting every entry in `relevance-rules.json` to a `relevanceRules` list entry.
4. Converting CDI path notation from extraction format to profile format (see Path Conversion below).

## Path Conversion: Index-Range to Ordinal Notation

Extraction outputs use index-range bracket notation to identify same-named sibling groups by their instance range (e.g., `Event[0-5]` vs `Event[6-11]`).

Profile files use `#N` ordinal notation (1-based) to identify which sibling group is meant (e.g., `Event#1` for the first, `Event#2` for the second).

**Conversion rule**:
1. Read the CDI XML file (path from `manual-outline.json` `cdiFile` field).
2. For each group in the extraction path with an `[low-high]` bracket suffix:
   - Find all sibling groups at the same CDI level with the same `<name>`.
   - Determine which ordinal (1-based position in document order) this is.
   - Replace `[low-high]` with `#N` (where N is the ordinal). If N=1 and there is only one such group (no same-named siblings), the suffix can be omitted.
3. Groups with no index-range suffix and no same-named siblings: leave the name unchanged (no `#N`).

**Example for Tower-LCC**:
- `Port I/O/Line/Event[0-5]` → the first `<group name="Event">` under Line → `Port I/O/Line/Event#1`
- `Port I/O/Line/Event[6-11]` → the second `<group name="Event">` under Line → `Port I/O/Line/Event#2`
- `Port I/O/Line` → only one `<group name="Line">` under Port I/O → `Port I/O/Line` (no suffix needed)

**Converting `controllingField`** in relevance rules:
- The `controllingField` in extraction outputs is already a CDI element name (e.g., `Output Function`). Use it directly as the `field` value in `allOf`.
- The extraction `affectedSection` path also needs index-range-to-ordinal conversion.

## Output Format

Produce a YAML file conforming to the JSON Schema at `specs/008-guided-configuration/contracts/profile-yaml-schema.json`.

```yaml
schemaVersion: "1.0"

nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"
  # firmwareVersionRange:  # Omit if no firmware version constraint is known
  #   min: null
  #   max: null

eventRoles:
  - groupPath: "Port I/O/Line/Event#1"
    role: Consumer
  - groupPath: "Port I/O/Line/Event#2"
    role: Producer
  # ... one entry per event-bearing group from event-roles.json ...

relevanceRules:
  - id: "R001"
    affectedGroupPath: "Port I/O/Line/Event#1"
    allOf:
      - field: "Output Function"
        irrelevantWhen: [0]
    explanation: "Consumer events (Commands) that control line output state are irrelevant when no output function is configured. These events only take effect when an Output Function other than 'No Function' is selected."
  # ... one entry per rule from relevance-rules.json ...
```

## Key Conventions

- `schemaVersion` must be `"1.0"`.
- `groupPath` must use `/`-separated CDI group names with `#N` ordinal suffix where needed.
- `allOf` must be a list even for single-condition rules: `allOf: [{ field: ..., irrelevantWhen: [...] }]`.
- `irrelevantWhen` must contain **raw integer `<property>` values** from the CDI `<map>`, not string labels.
- `explanation` must be copied **verbatim** from the extraction output's `explanation` field. Do not paraphrase.
- Omit `firmwareVersionRange` unless the extraction outputs include version-specific notes.
- Profile YAML must be valid YAML 1.1 (serde_yaml_ng compatibility).

## Output Location

Save the assembled profile to:
```
profiles/<node-name>/RR-CirKits_Tower-LCC.profile.yaml
```
(where `<node-name>` matches the directory used for the extraction outputs)

Then copy or move to:
```
app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml
```
when ready to bundle with the application.

## Validation

After assembly, run the `profile-6-validate` skill against the assembled `.profile.yaml` to confirm:
- All `groupPath` and `affectedGroupPath` values resolve in the CDI XML.
- All `irrelevantWhen` values exist as `<property>` entries in the controlling field's CDI `<map>`.
- No missing required fields.
