# IPC Contracts: Information Channels — Tauri Commands

**Feature**: 015-information-channels  
**Date**: 2026-06-24  
**Transport**: Tauri IPC (`invoke`)

## Overview

Channel operations use the existing `apply_layout_deltas` Tauri command for mutations (create, rename, delete) and a new `get_channels` query command for reads. This follows the established pattern where layout state changes go through the delta pipeline.

---

## Commands

### `get_channels`

Retrieve all channels for the currently open layout.

**Direction**: Frontend → Backend  
**Rust function**: `get_channels(state: State<AppState>) -> Result<ChannelsFile, String>`

**Request**: No parameters (operates on the currently loaded layout context).

**Response** (`ChannelsFile`):
```typescript
{
  schemaVersion: "1.0",
  channels: {
    [channelId: string]: {
      name: string,
      channelType: "block-occupancy",
      hardwareRef: {
        nodeKey: string,       // e.g., "05.01.01.01.03.00"
        slotId: string,        // e.g., "connector-a"
        inputOrdinal: number   // 1-based
      },
      createdAt: string        // ISO 8601
    }
  }
}
```

**Error cases**:
- No layout open → `"No layout is currently open"`

**Frontend wrapper** (`app/src/lib/api/channels.ts`):
```typescript
export async function getChannels(): Promise<ChannelsFile> {
  return invoke<ChannelsFile>('get_channels');
}
```

---

### `apply_layout_deltas` (extended)

Existing command — extended with three new delta variants for channel operations.

**Direction**: Frontend → Backend  
**Rust function**: `apply_layout_deltas(state: State<AppState>, deltas: Vec<LayoutEditDelta>) -> Result<(), String>`

#### New Delta: `CreateChannel`

```typescript
{
  type: "createChannel",
  channelId: string,           // UUID v4
  name: string,                // Default name at creation
  channelType: "block-occupancy",
  hardwareRef: {
    nodeKey: string,
    slotId: string,
    inputOrdinal: number
  }
}
```

**Behavior**: Inserts a new channel into `ChannelsFile.channels`. No-op if `channelId` already exists.

#### New Delta: `RenameChannel`

```typescript
{
  type: "renameChannel",
  channelId: string,
  newName: string              // Must be non-empty
}
```

**Behavior**: Updates the `name` field of the specified channel. Error if channel not found. Rejects empty `newName`.

#### New Delta: `DeleteChannel`

```typescript
{
  type: "deleteChannel",
  channelId: string
}
```

**Behavior**: Removes the channel from `ChannelsFile.channels`. No-op if channel not found.

**Error cases** (all deltas):
- No layout open → `"No layout is currently open"`
- Invalid delta → descriptive error message

---

### `get_daughterboard_channel_info`

Query the channel-creation metadata for a specific daughter board. Used by the orchestrator to determine how many channels to auto-create and what type they are.

**Direction**: Frontend → Backend  
**Rust function**: `get_daughterboard_channel_info(state: State<AppState>, daughterboard_id: String) -> Result<Option<DaughterboardChannelInfo>, String>`

**Request**:
```typescript
{ daughterboardId: string }  // e.g., "BOD-8-SM"
```

**Response** (`DaughterboardChannelInfo | null`):
```typescript
{
  channelCount: number,        // e.g., 8
  channelType: "block-occupancy"
} | null  // null if this board doesn't produce channels
```

**Error cases**:
- Unknown `daughterboardId` → returns `null` (not an error — non-channel boards simply return null)

**Frontend wrapper** (`app/src/lib/api/channels.ts`):
```typescript
export interface DaughterboardChannelInfo {
  channelCount: number;
  channelType: ChannelType;
}

export async function getDaughterboardChannelInfo(
  daughterboardId: string
): Promise<DaughterboardChannelInfo | null> {
  return invoke<DaughterboardChannelInfo | null>(
    'get_daughterboard_channel_info',
    { daughterboardId }
  );
}
```

---

## Batch Operations

Channel auto-creation on daughter board selection produces multiple `CreateChannel` deltas in a single `apply_layout_deltas` call:

```typescript
// Example: BOD-8-SM selected → 8 channels created atomically
const deltas: LayoutEditDelta[] = Array.from({ length: 8 }, (_, i) => ({
  type: 'createChannel' as const,
  channelId: crypto.randomUUID(),
  name: `${nodeName} — ${connectorLabel} — Input ${i + 1}`,
  channelType: 'block-occupancy' as const,
  hardwareRef: {
    nodeKey,
    slotId,
    inputOrdinal: i + 1,
  },
}));

await applyLayoutDeltas(deltas);
```

Channel removal on daughter board change produces multiple `DeleteChannel` deltas:

```typescript
const deleteDeltas = affectedChannelIds.map(id => ({
  type: 'deleteChannel' as const,
  channelId: id,
}));

await applyLayoutDeltas(deleteDeltas);
```

---

## Frontend API Module

**File**: `app/src/lib/api/channels.ts`

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { ChannelsFile, DaughterboardChannelInfo } from '$lib/types/channels';

export async function getChannels(): Promise<ChannelsFile> {
  return invoke<ChannelsFile>('get_channels');
}

export async function getDaughterboardChannelInfo(
  daughterboardId: string,
): Promise<DaughterboardChannelInfo | null> {
  return invoke<DaughterboardChannelInfo | null>(
    'get_daughterboard_channel_info',
    { daughterboardId },
  );
}

// Channel mutations go through the existing applyLayoutDeltas pathway
// (already defined in app/src/lib/api/layout.ts or similar)
```
