# Tauri IPC Contracts: Editable Bowties

**Feature Branch**: `009-editable-bowties`  
**Date**: 2026-03-15

This document defines the Tauri command (IPC) contracts between the SvelteKit frontend and the Rust backend. These are the new commands added for Feature 009.

---

## Layout File Commands

### `load_layout`

Load a YAML layout file from disk and merge with discovered node state.

**Invoke**: `invoke('load_layout', { path })`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | `string` | Yes | Absolute filesystem path to the YAML layout file |

**Returns**: `LayoutFile`

```typescript
interface LayoutFile {
  schemaVersion: string;
  bowties: Record<string, BowtieMetadata>;
  roleClassifications: Record<string, RoleClassification>;
}
```

**Errors**:
- File not found → `"Layout file not found: {path}"`
- YAML parse error → `"Failed to parse layout file: {details}"` (FR-026 degraded mode)
- Schema version mismatch → `"Unsupported layout schema version: {version}"`

---

### `save_layout`

Write bowtie metadata and role classifications to a YAML layout file.

**Invoke**: `invoke('save_layout', { path, layout })`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | `string` | Yes | Absolute filesystem path to write |
| `layout` | `LayoutFile` | Yes | Layout data to persist |

**Returns**: `void` (success) or error string

**Behavior**:
- Atomic write: temp file → flush → rename (FR-025)
- Creates parent directories if needed
- Overwrites existing file at path

**Errors**:
- Write failure → `"Failed to save layout file: {details}"`
- Permission denied → `"Cannot write to {path}: permission denied"`

---

### `get_recent_layout`

Retrieve the most recently opened layout file path.

**Invoke**: `invoke('get_recent_layout')`

**Returns**: `RecentLayout | null`

```typescript
interface RecentLayout {
  path: string;
  lastOpened: string;  // ISO 8601
}
```

---

### `set_recent_layout`

Store the most recently opened layout file path.

**Invoke**: `invoke('set_recent_layout', { path })`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | `string` | Yes | Absolute path to remember |

**Returns**: `void`

---

## Bowtie Catalog Commands (Extended)

### `get_bowties` (existing, extended response)

Existing command — response extended to include metadata from loaded layout file.

**Invoke**: `invoke('get_bowties')`

**Returns**: `BowtieCatalog` (extended)

```typescript
interface BowtieCatalog {
  bowties: BowtieCard[];
  built_at: string;
  source_node_count: number;
  total_slots_scanned: number;
}

interface BowtieCard {
  event_id_hex: string;
  event_id_bytes: number[];
  producers: EventSlotEntry[];
  consumers: EventSlotEntry[];
  ambiguous_entries: EventSlotEntry[];
  name?: string;          // NEW: from layout metadata
  tags: string[];         // NEW: from layout metadata
  state: BowtieState;     // NEW: derived from element counts
}

type BowtieState = 'active' | 'incomplete' | 'planning';
```

---

### `build_bowtie_catalog` (existing, extended input)

Existing command — extended to accept layout metadata for merging with discovered state.

**Invoke**: `invoke('build_bowtie_catalog', { layoutMetadata? })`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `layoutMetadata` | `LayoutFile \| null` | No | Loaded layout metadata to merge with discovered data |

**Behavior**:
1. Build catalog from discovered node event data (existing logic)
2. If `layoutMetadata` provided:
   - Merge `roleClassifications` into event role resolution (takes precedence over heuristic, but not profile)
   - Merge `bowties` metadata (names, tags) onto matching `BowtieCard`s by event ID
   - Create `planning` state cards for layout entries with no matching discovered event ID
3. Compute `state` for each card (active/incomplete/planning)

**Returns**: `BowtieCatalog` (extended, emitted via `cdi-read-complete` Tauri event)

---

## Write Commands

### `write_config_value` (existing, no changes)

No changes to the existing command. Used by bowtie creation/editing to write event IDs to node slots.

**Invoke**: `invoke('write_config_value', { nodeId, address, space, data })`

---

### `send_update_complete` (existing, no changes)

No changes. Sent after all writes to a node are complete.

**Invoke**: `invoke('send_update_complete', { nodeId })`

---

## Tauri Events (Backend → Frontend)

### `cdi-read-complete` (existing, extended payload)

Emitted after catalog rebuild. Payload extended with metadata.

```typescript
interface CdiReadCompletePayload {
  catalog: BowtieCatalog;  // now includes name, tags, state
  node_count: number;
}
```

### `layout-loaded` (new)

Emitted when a layout file is successfully loaded.

```typescript
interface LayoutLoadedPayload {
  path: string;
  bowtieCount: number;
  classificationCount: number;
}
```

### `layout-save-error` (new)

Emitted when a layout file save fails (after node writes succeeded).

```typescript
interface LayoutSaveErrorPayload {
  path: string;
  error: string;
}
```

---

## Frontend API Wrappers

New file: `app/src/lib/api/bowties.ts`

```typescript
import { invoke } from '@tauri-apps/api/core';

// Layout file operations
export async function loadLayout(path: string): Promise<LayoutFile> {
  return invoke('load_layout', { path });
}

export async function saveLayout(path: string, layout: LayoutFile): Promise<void> {
  return invoke('save_layout', { path, layout });
}

export async function getRecentLayout(): Promise<RecentLayout | null> {
  return invoke('get_recent_layout');
}

export async function setRecentLayout(path: string): Promise<void> {
  return invoke('set_recent_layout', { path });
}

// Bowtie catalog (extended)
export async function buildBowtieCatalog(layoutMetadata?: LayoutFile): Promise<void> {
  return invoke('build_bowtie_catalog', { layoutMetadata: layoutMetadata ?? null });
}
```

---

## Write Flow Contracts

### Create Connection (Frontend-Orchestrated)

The frontend New Connection dialog orchestrates connection creation:

1. **Resolve event ID** (frontend logic, see FR-002):
   ```typescript
   function resolveEventId(producer: ElementSelection, consumer: ElementSelection): EventIdResolution
   ```

2. **Generate write steps** (frontend):
   ```typescript
   function generateWriteSteps(resolution: EventIdResolution, producer: ElementSelection, consumer: ElementSelection): WriteStep[]
   ```

3. **Execute writes** via existing `writeConfigValue` (sequential, per FR-029a):
   ```typescript
   for (const step of writeSteps) {
     pendingEditsStore.setEdit(key, {
       nodeId: step.nodeId,
       address: step.address,
       space: step.space,
       originalValue: step.originalValue,
       pendingValue: step.newValue,
       // ...
     });
   }
   // Actual writes happen on Save via SaveControls flow
   ```

4. **Track metadata** (frontend):
   ```typescript
   bowtieMetadataStore.createBowtie(eventIdHex, name);
   ```

### Add Element to Existing Bowtie (Frontend-Orchestrated)

1. **Determine event ID**: Use existing bowtie's `event_id_hex`
2. **Find target slot**: Selected element's first unconnected event slot address
3. **Create pending edit**: `pendingEditsStore.setEdit(key, { pendingValue: bowtieEventId })`
4. **Writes happen on Save**

### Remove Element from Bowtie (Frontend-Orchestrated)

1. **Find original value**: `pendingEditsStore` has `originalValue` for the slot
2. **Create pending edit**: `pendingEditsStore.setEdit(key, { pendingValue: originalValue })`
   - This effectively reverts the slot to its pre-connection value
3. **Writes happen on Save**
