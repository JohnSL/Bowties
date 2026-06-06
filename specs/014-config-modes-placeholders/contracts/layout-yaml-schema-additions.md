# Layout YAML v2 — `placeholderBoards` + `nodeModeSelections`

Bowties layout files (`.bowties.yaml`) gain two new top-level fields and lose one. `schemaVersion` is bumped from `"1.0"` to `"2.0"`.

Removed: `connectorSelections` (folded into `nodeModeSelections`; see ADR-0008 and the F2 finding in `plan.md`). Daughterboards have not shipped, so no migration code is provided — `validate()` rejects `"1.0"` layouts with a clear message.

## Example

```yaml
schemaVersion: "2.0"
bowties: {}
roleClassifications: {}

# One home for "which Configuration Mode variant is currently selected".
# Keys are NodeKey (real NodeID or placeholder:<uuidv4>); see ADR-0008.
nodeModeSelections:
  "05.01.01.01.FF.00.00.01":                    # real Tower-LCC node
    connector-a: BOD4-CP
    connector-b: __none__
  "placeholder:c1d2e3f4-5678-4abc-9def-0123456789ab":   # placeholder Tower-LCC
    connector-a: BOD4-CP
  "placeholder:7c9e6b1a-4a8f-4d2e-9d3a-1f5b2c8e9d10":   # placeholder TurnoutBoss
    turnoutboss-side: "0"                       # variant id for Left

placeholderBoards:
  "placeholder:7c9e6b1a-4a8f-4d2e-9d3a-1f5b2c8e9d10":
    id: "placeholder:7c9e6b1a-4a8f-4d2e-9d3a-1f5b2c8e9d10"
    profileStem: "Mustangpeak-Engineering_TurnoutBoss"
    name: "Yard Throat (left)"
    configValues:
      "Hardware Configuration/Detector Sensitivity": 4
    createdAt: "2026-05-24T18:00:00Z"

  "placeholder:c1d2e3f4-5678-4abc-9def-0123456789ab":
    id: "placeholder:c1d2e3f4-5678-4abc-9def-0123456789ab"
    profileStem: "RR-CirKits_Tower-LCC"
    name: "Staging Yard Tower"
    configValues: {}
    createdAt: "2026-05-24T18:01:00Z"
```

## Field reference

| Field | Type | Required | Validation |
|---|---|---|---|
| `schemaVersion` | `"2.0"` | yes | `validate()` rejects any other value with `"This layout was created by an older Bowties build that did not ship daughterboard support; no migration path exists."` |
| `nodeModeSelections` | object (NodeKey → (ModeId → VariantId)) | no, defaults to `{}` | Each key MUST be either a canonical `NodeID` (`^[0-9A-F]{2}(\.[0-9A-F]{2}){7}$`) or a placeholder id (regex below). Reserved variant id `"__none__"` permitted when the targeted ConfigurationMode has `allowNoneInstalled: true`. |
| `placeholderBoards` | object (string → PlaceholderBoard) | no, defaults to `{}` | Map key MUST equal `<entry>.id`. |
| `PlaceholderBoard.id` | string | yes | Regex `^placeholder:[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$` (UUID v4, lowercase). |
| `PlaceholderBoard.profileStem` | string | yes | Matches `^[A-Za-z0-9._-]+$`. Bundled profile lookup uses this exact stem. Load MUST NOT fail if the bundle is missing (FR-022); the placeholder is marked "unknown model" in the rendered payload. |
| `PlaceholderBoard.name` | string | yes | User-facing label (default `"<Model> #<n>"`). |
| `PlaceholderBoard.configValues` | object (string → scalar or YAML node) | no | Keys are CDI paths in `'/' + '#N'` notation. Values are the raw stored representation (integer enum byte, string, etc.) for that field. |
| `PlaceholderBoard.createdAt` | RFC3339 string | no | Audit only. |

## Backend command contract

| Command | Direction | Payload | Behavior |
|---|---|---|---|
| `add_placeholder_board` | FE → BE | `{ id, profileStem, name }` | Validates id + profileStem; emits `LayoutEditDelta::AddPlaceholderBoard`. |
| `delete_placeholder_board` | FE → BE | `{ id }` | Emits `DeletePlaceholderBoard`. Backend also clears `nodeModeSelections[id]` in the same delta. FE wraps with confirmation per FR-017a. |
| `set_placeholder_config_value` | FE → BE | `{ id, cdiPath, value }` | Emits `SetPlaceholderConfigValue`. |
| `set_node_mode_selection` | FE → BE | `{ nodeKey, modeId, variantId }` | Emits `SetNodeModeSelection`. Accepts both real `NodeID` and `placeholder:<uuidv4>` (ADR-0008). |
| `rename_placeholder_board` | FE → BE | `{ id, newName }` | Emits `RenamePlaceholderBoard`. |
| `load_bundled_cdi` | FE → BE | `{ profileStem }` | Returns the parsed CDI XML bundled alongside the profile, or `Err("unknown profile stem")`. |

## Error envelopes (new)

| Error | When |
|---|---|
| `InvalidPlaceholderId` | id does not match the UUID v4 regex. |
| `InvalidNodeKey` | A `nodeKey` parameter is neither a canonical NodeID nor a placeholder id. |
| `UnknownPlaceholderId` | id not present in `placeholderBoards`. |
| `PlaceholderEventNotBindable` | Any binding-creation command receives an eventid whose owning node has a `placeholder:` prefix (FR-015). |
| `UnknownBundledProfile` | `load_bundled_cdi` called with a profileStem that has no matching `.profile.yaml` / `.cdi.xml` pair. |

