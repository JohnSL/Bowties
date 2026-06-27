---
name: profile-7-assemble
description: Assemble a .profile.yaml structure profile from Phase 1 extraction outputs (event-roles.json and relevance-rules.json). Converts CDI paths from index-range notation to '#N' ordinal notation and produces a ready-to-validate profile file. Keywords -- CDI, profile, assemble, .profile.yaml, structure profile, event roles, relevance rules, phase 2, guided configuration.
---

# Assemble Structure Profile from Extraction Outputs

Combine Phase 1 extraction outputs into a complete `.profile.yaml` structure profile file ready for bundling with the Bowties application.

## When to Use

Use this skill after Phase 1 extraction (skills 0–6) is complete and validated. This is the final authoring step before the profile file is placed in `app/src-tauri/profiles/` and bundled with the application.

## Required Inputs

1. **manual-outline.json** — contains `cdiFile` (path to CDI XML) and `nodeType`.
2. **event-roles.json** — produced by `profile-1-event-roles`.
3. **relevance-rules.json** — produced by `profile-2-relevance-rules`.

All paths are read from `manual-outline.json` — do not ask for them.

## How to Run

```pwsh
uv run .github/skills/_lib/profile_tools.py assemble profile-extractions/<node-name>
```

The CLI:

1. Reads `manual-outline.json`, `event-roles.json`, and `relevance-rules.json`.
2. Parses the CDI XML to build a path registry.
3. Converts every extraction path to `#<ordinal>` notation. Same-name siblings get an explicit `#N` suffix (1-based document order); unique names stay unchanged. Ambiguous paths from the extraction (no `[N]` suffix despite a sibling collision) are auto-disambiguated to `#1` to match the loader's default-pick behaviour — add an explicit `[N]` suffix in the source `event-roles.json` / `relevance-rules.json` if you mean a different sibling.
4. Emits `<Manufacturer>_<Model>.profile.yaml` matching the v2 schema at `specs/014-config-modes-placeholders/contracts/profile-yaml-schema-v2.json`:
   - `schemaVersion: "2.0"` (the current Bowties loader rejects `"1.0"`).
   - `affectedTarget` (v2 field name; the loader's `serde(alias)` still accepts `affectedGroupPath` in hand-edited files).
   - Top-level documentation comments and per-segment dividers between entries for human readability.
5. Copies every `explanation` field verbatim from the extraction output.

## Path Conversion: Index-Range to Ordinal Notation

Extraction outputs use index-range bracket notation to identify same-named sibling groups by their instance range (e.g., `Event[0-5]` vs `Event[6-11]`). Profile files use 1-based `#N` ordinal notation (`Event#1` for the first, `Event#2` for the second). The CLI handles this conversion automatically and also adds `#N` to ambiguous bare-name paths; you do not need to compute ordinals by hand.

Single-instance groups (no same-name siblings) stay unchanged — no `#N` suffix is added.

**Examples**:
- `Port I/O/Line/Event[0-5]` → `Port I/O/Line/Event#1` (first `<group name="Event">` under Line)
- `Port I/O/Line/Event[6-11]` → `Port I/O/Line/Event#2` (second)
- `Port I/O/Line` → `Port I/O/Line` (only one `<group name="Line">` under Port I/O)
- `Conditionals/Logic/Action[1-4]` → `Conditionals/Logic/Action#2` (the replicated sibling, when an unreplicated `Action[0]` also exists at the same level)
- `Conditionals/Logic/Action` (no suffix, with two siblings) → `Conditionals/Logic/Action#1` (auto-disambiguated to match the loader's default pick).

## Output Format

The CLI emits a YAML file conforming to the v2 JSON Schema at `specs/014-config-modes-placeholders/contracts/profile-yaml-schema-v2.json`:

```yaml
schemaVersion: "2.0"

nodeType:
  manufacturer: "RR-CirKits"
  model: "Tower-LCC"

eventRoles:
  # ── Port I/O ──
  - groupPath: "Port I/O/Line/Event#1"
    role: Consumer
  - groupPath: "Port I/O/Line/Event#2"
    role: Producer

relevanceRules:
  # ── Port I/O ──
  - id: "R001"
    affectedTarget: "Port I/O/Line/Event#1"
    allOf:
      - field: "Output Function"
        irrelevantWhen: [0]
    explanation: "Consumer events (Commands) that control line output state are irrelevant when no output function is configured. These events only take effect when an Output Function other than 'No Function' is selected."
```

The file includes a top-of-file header comment plus per-segment dividers between entries to make hand-editing easier.

## Key Conventions Enforced by the CLI

- `schemaVersion` is `"2.0"`.
- `groupPath` and `affectedTarget` use `/`-separated CDI group names with `#N` ordinal suffix where same-name siblings exist.
- `allOf` is always a list, even for single-condition rules.
- `irrelevantWhen` contains the raw integer `<property>` values from the CDI `<map>`, not string labels.
- `explanation` is copied **verbatim** from the extraction output's `explanation` field.

`firmwareVersionRange` is omitted unless the extraction outputs declare a version constraint (no extraction file currently produces one).

## Output Location

The CLI writes `<Manufacturer>_<Model>.profile.yaml` into the node directory:

```
profile-extractions/<node-name>/<Manufacturer>_<Model>.profile.yaml
```

Copy to `app/src-tauri/profiles/` when ready to bundle.

## Validation

After assembly, run `profile-6-validate` against the same node directory to confirm all paths and enum values still resolve.

## Configuration Modes (variants, daughterboards)

The v2 schema also supports a `configurationModes` array for firmware-variant auto-detect, structural slots (10-pin daughterboards on Tower-LCC / Signal-LCC / SPROG carriers), and other multi-variant selectors. **The CLI does not generate `configurationModes`** — those decisions are physical product knowledge that does not live in the CDI XML or the manual.

For nodes that need them, hand-author the `configurationModes:` block by appending it to the assembled `.profile.yaml`. Use the Tower-LCC profile (`app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml`) as the worked example. Re-running `assemble` will overwrite the file; keep a copy of the hand-authored section and re-paste it after each assemble, or move it into a sidecar file once the workflow stabilises.
