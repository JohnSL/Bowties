# Tauri IPC Contracts: Offline Layout Editing

**Feature Branch**: `010-offline-layout-editing`  
**Date**: 2026-04-04

This document defines frontend/backend IPC contracts for capture, offline editing persistence, and sync resolution flows.

## Layout Capture and File Operations

### `capture_layout_snapshot`

Capture currently discovered/read node state into layout model in memory.

Invoke: `invoke('capture_layout_snapshot', { includeProducerEvents: true })`

Returns:

```typescript
interface CaptureSummary {
  capturedAt: string;
  nodeCount: number;
  completeCount: number;
  partialCount: number;
}
```

### `save_layout_directory`

Atomically persist full layout directory (manifest, node files, metadata, pending offline changes).

Invoke: `invoke('save_layout_directory', { path, overwrite: true })`

Parameters:
- `path: string` absolute directory path
- `overwrite: boolean`

Returns:

```typescript
interface SaveLayoutResult {
  manifestPath: string;
  nodeFilesWritten: number;
  warnings: string[];
}
```

Errors:
- permission denied
- invalid schema
- staging swap failed

### `open_layout_directory`

Load a previously captured layout from disk without requiring bus connection.

Invoke: `invoke('open_layout_directory', { path })`

Returns:

```typescript
interface OpenLayoutResult {
  layoutId: string;
  capturedAt: string;
  offlineMode: boolean;
  nodeCount: number;
  partialNodes: string[];
  pendingOfflineChangeCount: number;
}
```

### `close_layout`

Close active layout context.

Invoke: `invoke('close_layout', { decision })`

Parameters:
- `decision: 'save' | 'discard' | 'cancel'`

Returns: `CloseLayoutResult`

```typescript
interface CloseLayoutResult {
  closed: boolean;
  reason?: string;
}
```

### `create_new_layout_capture`

Start fresh empty layout context.

Invoke: `invoke('create_new_layout_capture')`

Returns:

```typescript
interface NewLayoutResult {
  layoutId: string;
  createdAt: string;
}
```

## Offline Change Commands

### `set_offline_change`

Create/update a pending offline change row.

Invoke: `invoke('set_offline_change', { change })`

```typescript
interface OfflineChangeInput {
  kind: 'config' | 'bowtieMetadata' | 'bowtieEvent';
  nodeId?: string;
  space?: number;
  offset?: string;
  baselineValue: string;
  plannedValue: string;
}
```

Returns: `changeId: string`

### `revert_offline_change`

Revert one pending offline change back to baseline.

Invoke: `invoke('revert_offline_change', { changeId })`

Returns: `{ removed: boolean }`

### `list_offline_changes`

List persisted pending rows.

Invoke: `invoke('list_offline_changes')`

Returns:

```typescript
interface OfflineChangeRow {
  changeId: string;
  kind: 'config' | 'bowtieMetadata' | 'bowtieEvent';
  nodeId?: string;
  space?: number;
  offset?: string;
  baselineValue: string;
  plannedValue: string;
  status: 'pending' | 'conflict' | 'clean' | 'alreadyApplied' | 'skipped' | 'applied' | 'failed';
  error?: string;
}
```

## Matching and Sync Commands

### `compute_layout_match_status`

Compute preliminary bus-to-layout match from discovered node IDs.

Invoke: `invoke('compute_layout_match_status', { discoveredNodeIds })`

Returns:

```typescript
interface LayoutMatchStatus {
  overlapPercent: number;
  classification: 'likely_same' | 'uncertain' | 'likely_different';
  expectedThresholds: {
    likelySameMin: 80;
    uncertainMin: 40;
  };
}
```

### `build_sync_session`

Compare pending offline rows against current live bus values and classify each row.

Invoke: `invoke('build_sync_session')`

Returns:

```typescript
interface SyncSession {
  conflictRows: SyncRow[];
  cleanRows: SyncRow[];
  alreadyAppliedCount: number;
  nodeMissingRows: SyncRow[];
}

interface SyncRow {
  changeId: string;
  nodeId?: string;
  baselineValue: string;
  plannedValue: string;
  busValue?: string;
  resolution: 'unresolved' | 'apply' | 'skip';
  error?: string;
}
```

### `set_sync_mode`

Set user-selected mode for uncertain/different match states.

Invoke: `invoke('set_sync_mode', { mode })`

Parameters:
- `mode: 'target_layout_bus' | 'bench_other_bus'`

Returns: `{ mode: string }`

### `apply_sync_changes`

Apply resolved conflict rows and selected clean rows.

Invoke: `invoke('apply_sync_changes', { applyChangeIds, skipChangeIds })`

Returns:

```typescript
interface ApplySyncResult {
  applied: string[];
  skipped: string[];
  failed: Array<{ changeId: string; reason: string }>;
  readOnlyCleared: string[];
}
```

Behavior:
- Continues independent rows after non-fatal failures.
- Read-only write replies clear row and restore latest bus value.
- Successful rows removed from pending set.

## CDI Portability Commands

### `export_cdi_bundle`

Export CDI references used by active layout to a portable bundle.

Invoke: `invoke('export_cdi_bundle', { outputPath })`

### `import_cdi_bundle`

Import CDI bundle so missing references in layout become resolvable.

Invoke: `invoke('import_cdi_bundle', { bundlePath })`

## Tauri Events

- `layout-opened`: `{ layoutId, path, capturedAt, offlineMode }`
- `layout-save-progress`: `{ phase, filesWritten, totalFiles }`
- `sync-session-ready`: `{ conflictCount, cleanCount, alreadyAppliedCount }`
- `sync-apply-progress`: `{ completed, total, currentChangeId }`
- `sync-apply-failed`: `{ changeId, reason }`
- `cdi-reference-missing`: `{ nodeId, cacheKey }`
