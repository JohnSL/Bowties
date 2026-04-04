# Data Model: Offline Layout Editing

**Feature Branch**: `010-offline-layout-editing`  
**Date**: 2026-04-04

## Entities

### 1. LayoutManifest

Top-level metadata for a captured layout directory.

```yaml
schemaVersion: 2
layoutId: "club-layout-east"
capturedAt: "2026-04-04T14:32:11Z"
lastSavedAt: "2026-04-04T18:03:29Z"
activeMode: "offline" # offline | online
matchThresholds:
  likelySame: 80
  uncertainMin: 40
files:
  bowties: "bowties.yaml"
  eventNames: "event-names.yaml"
  offlineChanges: "offline-changes.yaml"
  nodesDir: "nodes"
```

Validation rules:
- `schemaVersion` must be integer `2` for this feature.
- File paths are relative and normalized.
- Threshold values fixed to clarified rules.

### 2. NodeSnapshot

Per-node captured state stored in `nodes/<NODE_ID>.yaml`.

```yaml
nodeId: "0501010114A2B3C4"
capturedAt: "2026-04-04T14:31:54Z"
captureStatus: "complete" # complete | partial
missing:
  - "space=253 offset=0x00000120 length=8"
snip:
  userName: "Yard Switch 3"
  userDescription: "East ladder"
  manufacturerName: "RR-CirKits"
  modelName: "Tower-LCC"
cdiRef:
  cacheKey: "sha256:abc..."
  version: "1.3"
  fingerprint: "d4f98a..."
values:
  "253":
    "0x00000120": "01.02.03.04.05.06.07.08"
producerIdentifiedEvents:
  - "01.02.03.04.05.06.07.08"
  - "01.02.03.04.05.06.07.09"
```

Validation rules:
- Filename must equal canonical node ID (`FR-030`).
- `captureStatus=partial` requires non-empty `missing` list.
- `values` keys are memory space then offset; offsets are canonical hex strings.

### 3. BowtieMetadataFile

Layout-level bowtie metadata and role overrides.

```yaml
bowties:
  "01.02.03.04.05.06.07.08":
    name: "Yard Entry"
    tags: ["yard", "signals"]
roleClassifications:
  "0501010114A2B3C4:Port A/Input 1/Event":
    role: "Producer"
```

Validation rules:
- Event IDs are dotted-hex 8-byte form.
- `role` enum is `Producer|Consumer`.
- Tags are deduplicated and sorted for deterministic output.

### 4. OfflineChangeRow

Persisted planned change for later sync.

```yaml
changeId: "chg-7a0dbf4a"
kind: "config" # config | bowtieMetadata | bowtieEvent
nodeId: "0501010114A2B3C4"
space: 253
offset: "0x00000120"
baselineValue: "01.02.03.04.05.06.07.08"
plannedValue: "01.02.03.04.05.06.07.10"
status: "pending" # pending | conflict | clean | alreadyApplied | skipped | applied | failed
error: null
updatedAt: "2026-04-04T17:45:02Z"
```

Validation rules:
- Identity tuple (`nodeId`,`space`,`offset`,`kind`) unique among pending rows.
- `baselineValue` never mutated after first capture.
- `status=failed` requires `error` text.

### 5. SyncSessionRow (in-memory)

Computed on connect when enough live values exist.

Fields:
- `changeId`
- `baselineValue`
- `plannedValue`
- `busValue`
- `classification`: `conflict|clean|alreadyApplied|nodeMissing|readOnlySuppressed`
- `resolution`: `apply|skip|unresolved`

State transitions:
- `pending -> clean|conflict|alreadyApplied|nodeMissing`
- `clean -> applied|skipped|failed`
- `conflict -> applied|skipped|failed`
- `alreadyApplied -> cleared`
- `failed -> pending` (retry)

### 6. StagedNode

Node added outside original capture.

Fields:
- `nodeId`
- `origin`: `staged`
- `firstSeenAt`
- `validatedOnTargetBus`: boolean
- `snapshotPath`

Validation rules:
- Always persisted as first-class node snapshot file.
- Remains pending/not-validated until observed and acknowledged on target bus.

## Relationships

- `LayoutManifest` 1-to-many `NodeSnapshot`.
- `LayoutManifest` 1-to-1 `BowtieMetadataFile`.
- `LayoutManifest` 1-to-many `OfflineChangeRow`.
- `OfflineChangeRow` many-to-1 `NodeSnapshot` for `kind=config|bowtieEvent`.
- `SyncSessionRow` derives from `OfflineChangeRow` plus live bus reads.

## Deterministic Serialization Rules

- Use `BTreeMap` ordering for map keys.
- Normalize node IDs and event IDs to uppercase hex.
- Preserve stable key ordering in all YAML documents.
- Exclude runtime-only transient fields from persisted node files (FR-029).
