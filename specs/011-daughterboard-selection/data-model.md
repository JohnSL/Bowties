# Data Model

## Overview

This feature adds authored connector-topology data to the existing structure profile system and adds per-node saved hardware assumptions to layout/project persistence.

## Entities

### CarrierBoardProfile

Represents one supported node type's existing `.profile.yaml` file, extended with connector-slot metadata.

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | string | Existing profile schema version, extended for connector support |
| `nodeType.manufacturer` | string | Matching key with model |
| `nodeType.model` | string | Matching key with manufacturer |
| `firmwareVersionRange` | optional object | Advisory only, unchanged |
| `eventRoles` | array | Existing profile data |
| `relevanceRules` | array | Existing profile data |
| `connectorSlots` | array of `ConnectorSlotDefinition` | New connector-slot declarations |
| `daughterboardReferences` | array of string IDs | Optional explicit index of reusable daughterboards referenced by this carrier |
| `carrierOverrides` | array of `CarrierOverrideRule` | Optional targeted overrides for referenced daughterboards |

**Validation rules**

- `(manufacturer, model)` remains unique per carrier profile.
- `connectorSlots[].slotId` must be unique within a carrier profile.
- All referenced daughterboard IDs must resolve to authored reusable definitions.

### ConnectorSlotDefinition

Defines one hardware attachment point on a carrier board.

| Field | Type | Notes |
|---|---|---|
| `slotId` | string | Stable identifier used in persistence and UI |
| `label` | string | User-facing connector label |
| `order` | integer | UI ordering within the carrier board |
| `allowNoneInstalled` | boolean | Enables explicit unpopulated state |
| `supportedDaughterboardIds` | array of string | Allowed reusable daughterboards for this slot |
| `affectedPaths` | array of profile/CDI path refs | Lines/groups/sections governed by the slot |
| `baseBehaviorWhenEmpty` | enum/object | Optional rule for "None installed" behavior; when omitted, empty slots add no extra constraints |

**Relationships**

- Belongs to one `CarrierBoardProfile`.
- References many `DaughterboardProfile` definitions.
- Governs many `AffectedConfigurationTarget` paths.

### DaughterboardProfile

Reusable authored definition for one daughterboard/card type.

| Field | Type | Notes |
|---|---|---|
| `daughterboardId` | string | Stable reusable identifier |
| `displayName` | string | UI label |
| `kind` | enum | Detector, fan-out, relay, driver, isolator, etc. |
| `validityRules` | array of `ConnectorConstraintRule` | Allowed/blocked settings for governed paths |
| `repairRules` | array of `RepairRule` | Preferred auto-stage replacements/resets |
| `defaultsWhenSelected` | optional map | Fallback values when repair rules do not apply |
| `metadata` | optional object | Manual citations, manufacturer tags, notes |

**Validation rules**

- `daughterboardId` must be globally unique within the reusable daughterboard library.
- Every target path in `validityRules` and `repairRules` must resolve against the carrier/CDI context that uses it.

### CarrierOverrideRule

Optional refinement layer that adjusts one reusable daughterboard definition for a specific carrier board or slot.

| Field | Type | Notes |
|---|---|---|
| `carrierKey` | string | Manufacturer/model or normalized key |
| `slotId` | optional string | Narrower scope when only one slot differs |
| `daughterboardId` | string | Reusable daughterboard being refined |
| `overrideValidityRules` | optional array | Adds/replaces validity behavior |
| `overrideRepairRules` | optional array | Adds/replaces repair behavior |
| `notes` | optional string | Documentation/debugging aid |

### ConnectorConstraintRule

Profile-authored rule describing whether a setting/section/choice is valid for a selected daughterboard on a governed connector slot.

| Field | Type | Notes |
|---|---|---|
| `targetPath` | string | Affected CDI/group/leaf path |
| `constraintType` | enum | allow-values, deny-values, show-section, hide-section, readonly, etc. |
| `allowedValues` | optional array | For enum/int filtering |
| `deniedValues` | optional array | For explicit invalid values |
| `explanation` | optional string | UI-facing reason |

### RepairRule

Profile-authored rule that tells Bowties how to stage a compatible follow-up edit after a connector selection changes.

| Field | Type | Notes |
|---|---|---|
| `targetPath` | string | Leaf or group path requiring repair |
| `whenInvalid` | object | Condition describing invalid current state |
| `replacementStrategy` | enum | set-explicit, reset-default, clear-empty |
| `replacementValue` | optional scalar | Used for explicit replacement |
| `priority` | integer | Lower values win if multiple rules match |

### NodeHardwareSelectionSet

Saved per-node connector assumptions persisted with the layout/project context.

| Field | Type | Notes |
|---|---|---|
| `nodeId` | string | Canonical node identifier |
| `carrierKey` | string | Manufacturer/model key used for validation |
| `slotSelections` | map of `slotId -> ConnectorSelectionRecord` | One record per chosen slot |
| `updatedAt` | timestamp | Last user-confirmed edit |

**Validation rules**

- `slotSelections` may only contain slot IDs declared by the active carrier profile.
- Unknown daughterboard IDs are preserved as unknown records, not remapped.

### ConnectorSelectionRecord

One persisted connector selection.

| Field | Type | Notes |
|---|---|---|
| `slotId` | string | Stable connector-slot ID |
| `selectedDaughterboardId` | string | Reusable daughterboard ID or sentinel for none/unknown |
| `status` | enum | selected, none, unknown |
| `sourceProfileVersion` | optional string | Helps diagnostics if profile files later change |

### ConnectorCompatibilityPreview

Computed view model produced when a selection changes.

| Field | Type | Notes |
|---|---|---|
| `nodeId` | string | Node being edited |
| `slotId` | string | Changed connector slot |
| `selectedDaughterboardId` | string | Candidate selection |
| `filteredTargets` | array | Paths/fields affected by the selection |
| `stagedRepairs` | array of `StagedCompatibilityRepair` | Auto-generated pending edits |
| `warnings` | array of string | Unknown daughterboard/profile issues |

### StagedCompatibilityRepair

Pending configuration edit automatically staged to keep the config compatible.

| Field | Type | Notes |
|---|---|---|
| `nodeId` | string | Node owning the edit |
| `targetPath` | string | Leaf path |
| `space` | integer | Memory space when applicable |
| `offset` | string | Existing offline-change identifier field |
| `baselineValue` | string | Current value before repair |
| `plannedValue` | string | Compatible staged value |
| `reason` | string | User-facing explanation |
| `originSlotId` | string | Connector slot that triggered the repair |

## Relationships

- One `CarrierBoardProfile` has many `ConnectorSlotDefinition` entries.
- One `ConnectorSlotDefinition` references many reusable `DaughterboardProfile` entries.
- One `DaughterboardProfile` can be reused by many carrier profiles through slot references.
- One `CarrierOverrideRule` refines one reusable `DaughterboardProfile` in one carrier or slot context.
- One `NodeHardwareSelectionSet` belongs to one node instance in one saved layout/project context.
- One connector selection change may produce many `StagedCompatibilityRepair` records.

## State Transitions

### Connector Selection Lifecycle

1. `restored` -> loaded from layout metadata when a saved context opens
2. `edited` -> user changes a slot selection in the UI
3. `previewed` -> compatibility filtering and repair preview recomputed locally
4. `staged` -> auto-generated config changes inserted into the existing pending/offline change workflow
5. `saved` -> selection set persisted with layout/project metadata
6. `reopened` -> saved selection set restored and revalidated against the active profile
7. `unknown` -> previously saved daughterboard no longer resolves; selection is preserved and flagged

### Repair Lifecycle

1. `detected` -> connector selection invalidates one or more current values
2. `resolved` -> repair rule or fallback chooses compatible replacement/reset
3. `staged` -> generated pending change is visible before apply
4. `applied` -> normal sync/apply workflow writes the config
5. `cleared` -> if the user changes the connector or affected value again, obsolete staged repairs are recomputed or removed