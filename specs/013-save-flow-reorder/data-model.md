# Data Model: Layout-First Model

## Entity Relationships

```
KnownLayoutRegistry (app prefs)
  └── KnownLayoutEntry[]
        └── references → LayoutManifest (on disk)

LayoutManifest (.layout base file)
  ├── connections: ConnectionDefinition[]
  ├── companionDir → LayoutCompanionDir (.layout.d/)
  │     ├── bowties.yaml → LayoutFile (bowties, roles, connectors)
  │     ├── offline-changes.yaml → OfflineChange[]
  │     ├── nodes/ → NodeSnapshot[] (one per node)
  │     └── cdi/ → CDI XML files
  └── (schema v4)

ConnectionDefinition (in manifest)
  ├── id: UUID
  ├── name: string
  ├── adapterType: Tcp | GridConnectSerial | SlcanSerial
  └── connection params (host/port or serial)

SaveProgress (runtime, frontend)
  ├── phase: SavingLayout | WritingConfig | Reconciling | Complete | Failed
  ├── current / total (for WritingConfig phase)
  └── failures: FailedWrite[]
```

## Entities

### KnownLayoutEntry (NEW)

Stored in `$APPDATA/bowties/known-layouts.json`. App-level registry, not in the layout itself.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| name | String | Required, display-friendly | Layout name shown in picker |
| path | String | Required, absolute path | Path to `.layout` base file |
| lastOpened | String (ISO 8601) | Required | Last time layout was opened |

**Rust struct** (`layout/known_layouts.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownLayoutEntry {
    pub name: String,
    pub path: String,
    pub last_opened: String, // ISO 8601
}
```

### ConnectionDefinition (NEW — layout manifest extension)

Reuses the existing `ConnectionConfig` shape but stored inside the layout manifest.

| Field | Type | Constraints | Notes |
|-------|------|-------------|-------|
| id | String (UUID v4) | Required, unique within layout | Stable identifier |
| name | String | Required | User-visible label (e.g., "Home Workbench") |
| adapter_type | AdapterType | Required | Tcp, GridConnectSerial, SlcanSerial |
| host | Option\<String\> | Required for Tcp | Hostname or IP |
| port | Option\<u16\> | Optional for Tcp | Default 12021 |
| serial_port | Option\<String\> | Required for serial types | e.g., "COM3" |
| baud_rate | Option\<u32\> | Optional for serial types | |

**Rust struct**: Reuse existing `ConnectionConfig` from `commands/connection.rs`. No new struct needed — the same type works in both contexts.

### LayoutManifest (MODIFIED — schema v4)

| Field | Type | Change | Notes |
|-------|------|--------|-------|
| schema_version | u32 | Bumped to 4 | Was 3 |
| layout_id | String | Unchanged | |
| captured_at | String | Unchanged | |
| last_saved_at | String | Unchanged | |
| active_mode | String | Unchanged | "offline" |
| match_thresholds | MatchThresholds | Unchanged | |
| companion_dir | String | Unchanged | |
| **connections** | **Vec\<ConnectionConfig\>** | **NEW** | Connection definitions |

**Migration v3→v4**: Deserialize v3 manifest, add `connections: vec![]`, set `schema_version: 4`.

### LayoutFile (MODIFIED — bowties.yaml)

| Field | Type | Change | Notes |
|-------|------|--------|-------|
| schemaVersion | String | Unchanged | "1.0" |
| bowties | BTreeMap | Unchanged | |
| roleClassifications | BTreeMap | **Semantics extended** | Now includes protocol-resolved roles, not just user overrides |
| connectorSelections | BTreeMap | Unchanged | |

**Role classification change**: During save, all resolved (non-ambiguous) roles from the live bowtie catalog are merged into `roleClassifications`. The existing `RoleClassification` type (`Producer` or `Consumer`) is unchanged.

### SavePhase (NEW — frontend runtime)

Tracks the current phase of a save operation for progress display.

| Variant | Payload | Notes |
|---------|---------|-------|
| Idle | — | No save in progress |
| SavingLayout | — | Phase 1: writing layout to disk |
| WritingConfig | { current: number, total: number } | Phase 2: writing to bus nodes |
| Reconciling | — | Phase 3: re-saving layout after bus writes |
| Complete | { failedCount: number } | Done — 0 failures = full success |

**TypeScript type** (`stores/saveProgress.svelte.ts`):
```typescript
type SavePhase =
  | { kind: 'idle' }
  | { kind: 'saving-layout' }
  | { kind: 'writing-config'; current: number; total: number }
  | { kind: 'reconciling' }
  | { kind: 'complete'; failedCount: number };
```

### WriteModifiedResult (EXISTING — unchanged)

| Field | Type | Notes |
|-------|------|-------|
| total | u32 | Total leaves attempted |
| succeeded | u32 | Successfully written |
| failed | u32 | Write failures |
| read_only_rejected | u32 | 0x1083 rejections (not counted as failed) |

No changes needed — the existing struct supports the three-phase flow.

## State Transitions

### App Lifecycle (SIMPLIFIED)

```
┌──────────────┐     open layout     ┌──────────────┐
│ Layout Picker │ ──────────────────▶ │ Layout Open  │
│ (no layout)  │ ◀────────────────── │ (offline)    │
└──────────────┘     close layout    └──────┬───────┘
                                            │ connect
                                            ▼
                                     ┌──────────────┐
                                     │ Layout Open  │
                                     │ (online)     │
                                     └──────┬───────┘
                                            │ disconnect
                                            ▼
                                     ┌──────────────┐
                                     │ Layout Open  │
                                     │ (offline)    │
                                     └──────────────┘
```

Only 2 operating states: layout-offline and layout-online.
The layout picker is a gate, not a state — it blocks until a layout is chosen.

### Save Flow State Machine

```
┌──────────┐  user Save   ┌────────────────┐  layout saved   ┌──────────────────┐
│   Idle   │ ───────────▶ │ Saving Layout  │ ──────────────▶ │ Writing Config   │
└──────────┘              └────────────────┘                 │ (if online +     │
      ▲                          │                           │  pending changes) │
      │                          │ cancel / no bus writes    └────────┬─────────┘
      │                          ▼                                    │
      │                   ┌──────────┐     writes done        ┌──────┴──────┐
      └───────────────────│ Complete │ ◀──────────────────── │ Reconciling │
                          └──────────┘                        └─────────────┘
```

### Layout Picker States

```
┌──────────────────┐
│ Known Layouts    │ ──── select ────▶ Open layout
│ (list + actions) │ ──── Browse… ───▶ File dialog → Open layout
│                  │ ──── New ───────▶ Name + location → Create + Open
└──────────────────┘
```

## Validation Rules

- **Layout required**: All node browsing, config editing, bowtie creation, and connection activity require `activeContext != null`.
- **Connection names unique**: Within a layout, connection definition names must be unique.
- **Connection IDs unique**: UUIDs ensure uniqueness within and across layouts.
- **One active connection**: `state.active_connection` is `Option<ConnectionConfig>` — at most one.
- **Known layout paths valid**: `get_known_layouts` filters entries whose paths no longer exist on disk (defensive, like `get_recent_layout`).
- **Schema version check**: Accept v3 (migrate) or v4 (current). Reject < v3.
