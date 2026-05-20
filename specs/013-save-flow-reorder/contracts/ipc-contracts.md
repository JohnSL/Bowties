# IPC Contracts: Layout-First Model

All commands follow the existing Tauri IPC pattern: `invoke<ReturnType>('command_name', { params })`.

## New Commands

### Known Layout Registry

#### `get_known_layouts`
Returns the list of known layouts from the app-level registry.

**Frontend**:
```typescript
export async function getKnownLayouts(): Promise<KnownLayoutEntry[]> {
  return invoke<KnownLayoutEntry[]>('get_known_layouts');
}
```

**Backend**:
```rust
#[tauri::command]
pub async fn get_known_layouts(app: tauri::AppHandle) -> Result<Vec<KnownLayoutEntry>, String>
```

**Behavior**: Reads `$APPDATA/bowties/known-layouts.json`. Filters entries whose paths no longer exist on disk. Returns empty vec if file doesn't exist.

---

#### `add_known_layout`
Adds a layout to the known-layout registry (or updates lastOpened if already present).

**Frontend**:
```typescript
export async function addKnownLayout(entry: KnownLayoutEntry): Promise<void> {
  return invoke<void>('add_known_layout', { entry });
}
```

**Backend**:
```rust
#[tauri::command]
pub async fn add_known_layout(
    app: tauri::AppHandle,
    entry: KnownLayoutEntry,
) -> Result<(), String>
```

**Behavior**: Loads existing registry, upserts by path (case-insensitive on Windows), saves atomically.

---

#### `remove_known_layout`
Removes a layout from the known-layout registry without deleting files.

**Frontend**:
```typescript
export async function removeKnownLayout(path: string): Promise<void> {
  return invoke<void>('remove_known_layout', { path });
}
```

**Backend**:
```rust
#[tauri::command]
pub async fn remove_known_layout(
    app: tauri::AppHandle,
    path: String,
) -> Result<(), String>
```

**Behavior**: Loads registry, removes entry matching path, saves atomically. No error if path not found.

---

### Layout Connection Management

#### `get_layout_connections`
Returns connection definitions from the active layout's manifest.

**Frontend**:
```typescript
export async function getLayoutConnections(): Promise<ConnectionConfig[]> {
  return invoke<ConnectionConfig[]>('get_layout_connections');
}
```

**Backend**:
```rust
#[tauri::command]
pub async fn get_layout_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionConfig>, String>
```

**Behavior**: Reads connections from active layout manifest. Errors if no layout is active.

---

#### `save_layout_connections`
Persists updated connection definitions to the active layout's manifest.

**Frontend**:
```typescript
export async function saveLayoutConnections(connections: ConnectionConfig[]): Promise<void> {
  return invoke<void>('save_layout_connections', { connections });
}
```

**Backend**:
```rust
#[tauri::command]
pub async fn save_layout_connections(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    connections: Vec<ConnectionConfig>,
) -> Result<(), String>
```

**Behavior**: Updates connections in the active layout manifest and writes the manifest file atomically. Errors if no layout is active.

---

### Save Flow

#### `save_layout_with_bus_writes` (NEW — replaces the current two-step frontend orchestration)

**Frontend**:
```typescript
export async function saveLayoutWithBusWrites(
  path: string,
  overwrite: boolean,
  layout: LayoutFile | null,
): Promise<SaveWithBusWriteResult> {
  return invoke<SaveWithBusWriteResult>('save_layout_with_bus_writes', {
    path,
    overwrite,
    layout,
  });
}
```

**Backend**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveWithBusWriteResult {
    pub layout_saved: bool,
    pub bus_writes: Option<WriteModifiedResult>,
    pub reconciled: bool,
    pub catalog_rebuilt: bool,
}

#[tauri::command]
pub async fn save_layout_with_bus_writes(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    path: String,
    overwrite: bool,
    layout: Option<LayoutFile>,
) -> Result<SaveWithBusWriteResult, String>
```

**Behavior** (three-phase):
1. Save layout (calls existing `save_layout_directory` internally)
2. If connected and there are pending modified values, write them to bus (calls existing `write_modified_values` internally)
3. If any bus writes succeeded, reconcile: clear applied offline changes, re-save layout
4. Rebuild bowtie catalog
5. Emit progress events throughout

**Progress events** (Tauri events, not return values):
```typescript
// Event: 'save-progress'
type SaveProgressEvent =
  | { phase: 'saving-layout' }
  | { phase: 'writing-config'; current: number; total: number }
  | { phase: 'reconciling' }
  | { phase: 'complete'; failedCount: number };
```

---

## Modified Commands

### `save_layout_directory` (MODIFIED)

**Change**: Now merges all resolved event role classifications from the live bowtie catalog into `roleClassifications` during save, not just user overrides.

**Existing signature unchanged**:
```rust
pub async fn save_layout_directory(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    path: String,
    overwrite: bool,
    layout: Option<LayoutFile>,
) -> Result<SaveLayoutResult, String>
```

---

### `open_layout_directory` (MODIFIED)

**Change**: Now also:
- Migrates v3 manifests to v4 (adds empty `connections` section) on load
- Adds the layout to the known-layout registry on successful open

**Existing signature unchanged**.

---

### `create_new_layout_capture` (MODIFIED)

**Change**: Now also adds the newly created layout to the known-layout registry.

**Existing signature unchanged**.

---

## Existing Commands (Unchanged)

These commands are used in the save flow but require no changes:

- `write_modified_values` — writes pending config to bus nodes
- `set_modified_value` — marks a single leaf as modified
- `discard_modified_values` — discards all pending modifications
- `build_bowtie_catalog_command` — rebuilds bowtie catalog
- `connect_lcc` — establishes LCC connection
- `disconnect_lcc` — drops LCC connection
- `capture_layout_snapshot` — snapshots live node state
- `close_layout` — closes active layout and clears state
- `load_connection_prefs` / `save_connection_prefs` — global connection prefs (retained for import into layout)

## Frontend Types

```typescript
// New types
export interface KnownLayoutEntry {
  name: string;
  path: string;
  lastOpened: string; // ISO 8601
}

export interface SaveWithBusWriteResult {
  layoutSaved: boolean;
  busWrites: WriteModifiedResult | null;
  reconciled: boolean;
  catalogRebuilt: boolean;
}

export interface WriteModifiedResult {
  total: number;
  succeeded: number;
  failed: number;
  readOnlyRejected: number;
}

// Existing types (unchanged)
export interface ConnectionConfig {
  id: string;
  name: string;
  adapterType: 'tcp' | 'gridConnectSerial' | 'slcanSerial';
  host?: string;
  port?: number;
  serialPort?: string;
  baudRate?: number;
}
```
