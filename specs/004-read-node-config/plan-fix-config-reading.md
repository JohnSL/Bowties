# Plan: Fix Config Reading Performance & Correctness

## Context

The app's batch config reading is functionally correct — it makes the same 3 round trips
as JMRI for the async_blink node. However, it is ~230ms slower due to IPC overhead from
progress events emitted inside the read loop. There is also a correctness issue: if a
node's CDI has never been downloaded (fresh install, or new node), `read_all_config_values`
will fail with `CdiNotRetrieved`. We do NOT want to auto-download CDI during discovery —
that is handled separately. Instead, we silently skip config reading for nodes that don't
yet have CDI in cache.

## Root Causes

### 1. Inter-read latency (~110ms per batch gap)

In `read_all_config_values` ([cdi.rs](../../../app/src-tauri/src/commands/cdi.rs)), the
batch loop emits a Tauri event at `batch_idx % 10 == 0 || batch_idx == total_batches - 1`.
Even when the condition fires only at batch 0 and batch 2 (3-batch case), the
`app_handle.emit()` IPC call causes the async executor to yield, introducing ~110ms latency
before the next `read_memory` call. This is why the gap between successive LCC datagrams
is ~114ms in Bowties vs ~4ms in JMRI.

### 2. Silent failure when CDI absent

When `read_all_config_values` is called for a node whose CDI is not in cache, the function
returns a `CdiNotRetrieved` error, which propagates as an exception in the frontend — caught
and logged per-node, but still noisy. The discover flow should simply skip nodes without
CDI rather than attempting and failing.

## Changes

### Change 1 — Move progress events outside the batch read loop

**File:** `app/src-tauri/src/commands/cdi.rs`  
**Function:** `read_all_config_values`

Remove the per-batch progress emit from inside the batch loop:

```rust
// REMOVE this block from inside the batch loop:
if batch_idx % 10 == 0 || batch_idx == total_batches - 1 {
    let _ = app_handle.emit("config-read-progress", ReadProgressUpdate { ... });
}
```

Replace with a single emit **before** the loop starts:
```rust
let _ = app_handle.emit("config-read-progress", ReadProgressUpdate {
    ...
    status: ProgressStatus::ReadingNode { node_name: node_name.clone() },
});

for (batch_idx, batch) in batches.iter().enumerate() {
    // cancellation check only — no progress emit inside loop
    ...
}
```

The completion emit after the loop stays as-is.

**Expected result:** Time between successive LCC datagrams drops from ~114ms to ~5ms.

### Change 2 — Skip nodes without CDI in the discover flow

**File:** `app/src/routes/+page.svelte`  
**Function:** `discover()`

Before calling `readAllConfigValues(nodeId)`, check whether CDI is available in cache using
`getCdiXml`. If the call returns a `CdiNotRetrieved` error (or succeeds with no content),
skip that node silently. Do **not** call `downloadCdi`.

```typescript
// Before (current):
const response = await readAllConfigValues(nodeId);

// After:
let hasCdi = false;
try {
  const cdiCheck = await getCdiXml(nodeId);
  hasCdi = cdiCheck.xmlContent !== null;
} catch {
  // CdiNotRetrieved or similar — CDI not available yet
}

if (!hasCdi) {
  console.log(`Skipping config read for ${nodeName} — CDI not yet downloaded`);
  continue;
}

const response = await readAllConfigValues(nodeId);
```

This change applies to **both** code paths in `discover()`:
- The fresh discovery path (inside the `else` branch, ~line 200)
- The refresh path (inside the `if (nodes.length > 0)` branch, ~line 130)

**Import to add** to `app/src/routes/+page.svelte`:
```typescript
import { readAllConfigValues, cancelConfigReading, getCdiXml } from '$lib/api/cdi';
```

## What Is Not Changed

- The batch algorithm (zero-gap only) is correct and produces equivalent round trips to JMRI.
- `DetailsPanel.svelte` already only reads from cache (no per-click network calls) — no change needed.
- CDI download is intentionally NOT triggered automatically during discovery.

## Verification

1. Run app, connect, click **Discover Nodes**.
2. In LCC Traffic Monitor: the gap between successive read datagrams should be ~5ms (not ~114ms).
3. For a node with no CDI in file cache: no config-read attempt, no error logged.
4. Navigate Miller Columns to a leaf node → zero new LCC traffic.
5. Refresh button in DetailsPanel still reads a single value on demand.
