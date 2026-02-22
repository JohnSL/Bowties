# Developer Quickstart: Read Node Configuration

**Feature**: 004-read-node-config  
**Target Audience**: Developers implementing configuration value reading  
**Prerequisites**: Familiarity with Tauri, Rust, and SvelteKit

---

## Overview

This feature adds configuration value reading to the Bowties LCC application. It consists of:

1. **Backend** (Rust): Two new Tauri commands that read configuration values using Memory Configuration Protocol
2. **Frontend** (TypeScript/Svelte): UI integration for progress indication and value display
3. **State Management**: Config value cache and progress tracking in Miller Columns store

---

## Architecture Summary

```
┌─────────────────────────────────────────────────────────────┐
│ Frontend (SvelteKit)                                        │
│                                                             │
│  +page.svelte ──> refreshNodes() ──> readAllConfigValues() │
│      │                                         │            │
│      v                                         v            │
│  ProgressIndicator <── store.readProgress     │            │
│                                                │            │
│  DetailsPanel <── store.configValues          │            │
│                                                │            │
└────────────────────────────────────────────────┼────────────┘
                                                 │
                                    Tauri IPC    │
┌────────────────────────────────────────────────┼────────────┐
│ Backend (Rust)                                 v            │
│                                                             │
│  read_all_config_values(node_id) ──> emit progress events  │
│      │                                                      │
│      v                                                      │
│  Get CDI ──> Extract elements ──> For each element:        │
│                                     1. Calculate address    │
│                                     2. Read from 0xFD       │
│                                     3. Parse typed value    │
│                                     4. Store in cache       │
│                                     5. Emit progress        │
│      │                                                      │
│      v                                                      │
│  Return Map<path, value>                                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Implementation Steps

### Step 1: Backend - Add Tauri Commands

**File**: `app/src-tauri/src/commands/cdi.rs`

#### Command 1: `read_config_value`

Reads a single configuration value from a specified element.

```rust
use lcc_rs::protocol::memory_config::{MemoryConfigCmd, AddressSpace};
use lcc_rs::cdi::Element;

#[tauri::command]
pub async fn read_config_value(
    state: tauri::State<'_, AppState>,
    node_id: String,
    element_path: Vec<String>,
    timeout_ms: Option<u64>,
) -> Result<ConfigValueWithMetadata, String> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(2000));
    
    // 1. Get CDI for node
    let cdi = get_cdi_from_state(&state, &node_id)?;
    
    // 2. Navigate to element using existing get_element_details logic
    let element = navigate_to_element(&cdi, &element_path)?;
    
    // 3. Calculate absolute address
    let (segment_origin, element_offset) = extract_address_info(&element)?;
    let absolute_address = segment_origin + element_offset;
    
    // 4. Get connection and node alias
    let conn_lock = state.connection.read().await;
    let connection = conn_lock.as_ref().ok_or("Not connected")?;
    let connection = connection.lock().await;
    let node = get_node_by_id(&state, &node_id)?;
    let alias = node.alias;
    
    // 5. Build memory read request
    let size = get_element_size(&element)?;
    let read_cmd = MemoryConfigCmd::build_read(
        connection.alias,  // source
        alias,             // destination
        AddressSpace::Configuration as u8,
        absolute_address,
        size as u8,
    )?;
    
    // 6. Send datagram and wait for response
    let response = connection.send_datagram_and_wait(alias, &read_cmd, timeout).await?;
    
    // 7. Parse response
    let read_reply = MemoryConfigCmd::parse_read_reply(&response)?;
    
    // 8. Convert bytes to typed value
    let typed_value = parse_config_value(&element, &read_reply.data)?;
    
    // 9. Return with metadata
    Ok(ConfigValueWithMetadata {
        value: typed_value,
        memory_address: absolute_address,
        address_space: AddressSpace::Configuration as u8,
        element_path,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}
```

#### Command 2: `read_all_config_values`

Reads all configuration values from a node with progress indication.

```rust
#[tauri::command]
pub async fn read_all_config_values(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    node_id: String,
    timeout_ms: Option<u64>,
    emit_progress: Option<bool>,
) -> Result<ReadAllConfigValuesResponse, String> {
    let start_time = std::time::Instant::now();
    let should_emit = emit_progress.unwrap_or(true);
    
    // 1. Get CDI and extract all elements with memory addresses
    let cdi = get_cdi_from_state(&state, &node_id)?;
    let elements = extract_all_elements_with_addresses(&cdi)?;
    let total_elements = elements.len();
    
    // 2. Get node info for progress display
    let node = get_node_by_id(&state, &node_id)?;
    let node_name = get_node_display_name(&node);  // SNIP priority cascade
    
    // 3. Initialize progress
    let mut values = HashMap::new();
    let mut successful = 0;
    let mut failed = 0;
    
    // 4. Read each element
    for (index, (element, path)) in elements.iter().enumerate() {
        // Emit progress event
        if should_emit {
            let percentage = ((index as f32 / total_elements as f32) * 100.0) as u8;
            app.emit_all("config-read-progress", ReadProgressUpdate {
                total_nodes: 1,
                current_node_index: 0,
                current_node_name: node_name.clone(),
                current_node_id: node_id.clone(),
                total_elements,
                elements_read: successful,
                elements_failed: failed,
                percentage,
                status: ProgressStatus::ReadingNode { node_name: node_name.clone() },
            })?;
        }
        
        // Read value for this element
        match read_config_value(
            state.clone(),
            node_id.clone(),
            path.clone(),
            timeout_ms,
        ).await {
            Ok(value) => {
                values.insert(path.join("/"), value);
                successful += 1;
            }
            Err(e) => {
                // Store as Invalid value
                values.insert(path.join("/"), ConfigValueWithMetadata {
                    value: ConfigValue::Invalid { error: e },
                    memory_address: 0,
                    address_space: AddressSpace::Configuration as u8,
                    element_path: path.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
                failed += 1;
            }
        }
    }
    
    // 5. Emit completion event
    if should_emit {
        app.emit_all("config-read-progress", ReadProgressUpdate {
            // ... (same as above but with status: Complete)
        })?;
    }
    
    Ok(ReadAllConfigValuesResponse {
        node_id,
        values,
        total_elements,
        successful_reads: successful,
        failed_reads: failed,
        duration_ms: start_time.elapsed().as_millis() as u64,
    })
}
```

#### Helper Functions

```rust
fn get_node_display_name(node: &DiscoveredNode) -> String {
    node.snip_data.as_ref()
        .and_then(|snip| {
            if !snip.user_name.is_empty() {
                Some(snip.user_name.clone())
            } else if !snip.user_description.is_empty() {
                Some(snip.user_description.clone())
            } else if !snip.model_name.is_empty() {
                Some(snip.model_name.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| format_node_id(&node.node_id))
}

fn parse_config_value(element: &Element, data: &[u8]) -> Result<ConfigValue, String> {
    match element {
        Element::Int { size, .. } => {
            let value = match size {
                1 => data[0] as i64,
                2 => i16::from_be_bytes([data[0], data[1]]) as i64,
                4 => i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as i64,
                8 => i64::from_be_bytes(data[0..8].try_into().unwrap()),
                _ => return Err(format!("Invalid int size: {}", size)),
            };
            Ok(ConfigValue::Int { value, size_bytes: *size })
        }
        Element::String { size, .. } => {
            let s = String::from_utf8(data.to_vec())
                .map_err(|e| format!("Invalid UTF-8: {}", e))?;
            Ok(ConfigValue::String { value: s.trim_end_matches('\0').to_string(), size_bytes: *size })
        }
        Element::EventId { .. } => {
            if data.len() != 8 {
                return Err(format!("EventId must be 8 bytes, got {}", data.len()));
            }
            Ok(ConfigValue::EventId { value: data.try_into().unwrap() })
        }
        Element::Float { .. } => {
            if data.len() != 4 {
                return Err(format!("Float must be 4 bytes, got {}", data.len()));
            }
            let value = f32::from_be_bytes(data.try_into().unwrap());
            Ok(ConfigValue::Float { value })
        }
        _ => Err(format!("Unsupported element type for config reading")),
    }
}
```

#### Register Commands

**File**: `app/src-tauri/src/lib.rs`

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    read_config_value,
    read_all_config_values,
])
```

---

### Step 2: Frontend - Add API Wrappers

**File**: `app/src/lib/api/cdi.ts`

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { ConfigValueWithMetadata } from './types';

export interface ReadAllConfigValuesResponse {
    node_id: string;
    values: Record<string, ConfigValueWithMetadata>;
    total_elements: number;
    successful_reads: number;
    failed_reads: number;
    duration_ms: number;
}

export async function readConfigValue(
    nodeId: string,
    elementPath: string[],
    timeoutMs?: number
): Promise<ConfigValueWithMetadata> {
    return await invoke<ConfigValueWithMetadata>('read_config_value', {
        node_id: nodeId,
        element_path: elementPath,
        timeout_ms: timeoutMs,
    });
}

export async function readAllConfigValues(
    nodeId: string,
    timeoutMs?: number,
    emitProgress: boolean = true
): Promise<ReadAllConfigValuesResponse> {
    return await invoke<ReadAllConfigValuesResponse>('read_all_config_values', {
        node_id: nodeId,
        timeout_ms: timeoutMs,
        emit_progress: emitProgress,
    });
}
```

**File**: `app/src/lib/api/types.ts`

```typescript
export type ConfigValue = 
    | { type: 'Int'; value: number; size_bytes: number }
    | { type: 'String'; value: string; size_bytes: number }
    | { type: 'EventId'; value: number[] }
    | { type: 'Float'; value: number }
    | { type: 'Invalid'; error: string };

export interface ConfigValueWithMetadata {
    value: ConfigValue;
    memory_address: number;
    address_space: number;
    element_path: string[];
    timestamp: string;
}

export interface ReadProgressState {
    totalNodes: number;
    currentNodeIndex: number;
    currentNodeName: string;
    currentNodeId: string;
    totalElements: number;
    elementsRead: number;
    elementsFailed: number;
    percentage: number;
    status: ProgressStatus;
}

export type ProgressStatus = 
    | { type: 'Starting' }
    | { type: 'ReadingNode'; node_name: string }
    | { type: 'NodeComplete'; node_name: string; success: boolean }
    | { type: 'Cancelled' }
    | { type: 'Complete'; success_count: number; fail_count: number };
```

---

### Step 3: Frontend - Extend Miller Columns Store

**File**: `app/src/lib/stores/millerColumns.ts`

```typescript
import { writable } from 'svelte/store';

interface MillerColumnsState {
    // ... existing fields ...
    configValues: Map<string, ConfigValueWithMetadata>;
    readProgress: ReadProgressState | null;
}

function createMillerColumnsStore() {
    const { subscribe, update } = writable<MillerColumnsState>({
        // ... existing fields ...
        configValues: new Map(),
        readProgress: null,
    });

    return {
        subscribe,
        
        // ... existing methods ...
        
        setConfigValue: (nodeId: string, elementPath: string[], value: ConfigValueWithMetadata) => {
            update(state => {
                const key = `${nodeId}:${elementPath.join('/')}`;
                state.configValues.set(key, value);
                return state;
            });
        },
        
        setConfigValues: (nodeId: string, values: Record<string, ConfigValueWithMetadata>) => {
            update(state => {
                for (const [path, value] of Object.entries(values)) {
                    const key = `${nodeId}:${path}`;
                    state.configValues.set(key, value);
                }
                return state;
            });
        },
        
        getConfigValue: (nodeId: string, elementPath: string[]): ConfigValueWithMetadata | null => {
            let result: ConfigValueWithMetadata | null = null;
            update(state => {
                const key = `${nodeId}:${elementPath.join('/')}`;
                result = state.configValues.get(key) ?? null;
                return state;
            });
            return result;
        },
        
        setReadProgress: (progress: ReadProgressState | null) => {
            update(state => ({ ...state, readProgress: progress }));
        },
        
        clearConfigValues: () => {
            update(state => ({ ...state, configValues: new Map() }));
        },
    };
}

export const millerColumnsStore = createMillerColumnsStore();
```

---

### Step 4: Frontend - Integrate into Refresh Flow

**File**: `app/src/routes/+page.svelte`

```typescript
import { listen } from '@tauri-apps/api/event';
import { readAllConfigValues } from '$lib/api/cdi';
import { millerColumnsStore } from '$lib/stores/millerColumns';

let isReadingConfig = false;

// Listen to progress events
onMount(() => {
    const unlistenProgress = listen('config-read-progress', (event) => {
        millerColumnsStore.setReadProgress(event.payload as ReadProgressState);
    });
    
    return () => {
        unlistenProgress.then(fn => fn());
    };
});

async function handleRefreshNodes() {
    isRefreshing = true;
    isReadingConfig = false;
    
    try {
        // 1. Refresh nodes (existing)
        nodes = await refreshAllNodes(timeoutMs);
        
        // 2. Clear old config values
        millerColumnsStore.clearConfigValues();
        
        // 3. Read config values from all nodes
        isReadingConfig = true;
        for (const node of nodes) {
            try {
                const result = await readAllConfigValues(node.node_id, timeoutMs);
                millerColumnsStore.setConfigValues(node.node_id, result.values);
            } catch (err) {
                console.error(`Failed to read config from ${node.node_id}:`, err);
                // Continue with other nodes
            }
        }
        
        // 4. Update Miller Columns
        if (millerColumnsNav) {
            millerColumnsNav.refreshNodes();
        }
    } finally {
        isRefreshing = false;
        isReadingConfig = false;
        millerColumnsStore.setReadProgress(null);
    }
}
```

#### Add Progress Indicator UI

```svelte
{#if isReadingConfig && $millerColumnsStore.readProgress}
    <div class="progress-indicator">
        <div class="progress-text">
            Reading {$millerColumnsStore.readProgress.currentNodeName} config... 
            {$millerColumnsStore.readProgress.percentage}%
        </div>
        <div class="progress-bar">
            <div 
                class="progress-fill" 
                style="width: {$millerColumnsStore.readProgress.percentage}%"
            />
        </div>
    </div>
{/if}
```

---

### Step 5: Frontend - Display Values in Details Panel

**File**: `app/src/lib/components/MillerColumns/DetailsPanel.svelte`

```svelte
<script lang="ts">
    import { millerColumnsStore } from '$lib/stores/millerColumns';
    import type { ConfigValue } from '$lib/api/types';
    
    $: selectedDetails = $millerColumnsStore.selectedElementDetails;
    $: selectedNode = $millerColumnsStore.selectedNode;
    
    $: configValue = selectedNode && selectedDetails 
        ? millerColumnsStore.getConfigValue(
              selectedNode.node_id, 
              selectedDetails.elementPath
          )
        : null;
    
    function formatConfigValue(value: ConfigValue): string {
        switch (value.type) {
            case 'Int':
                return `${value.value} (${value.size_bytes} bytes)`;
            case 'String':
                return value.value;
            case 'EventId':
                return value.value.map(b => b.toString(16).padStart(2, '0')).join('.');
            case 'Float':
                return value.value.toFixed(4);
            case 'Invalid':
                return `Error: ${value.error}`;
        }
    }
</script>

<!-- Existing detail rows -->
<div class="detail-row">
    <div class="detail-label">Memory Address:</div>
    <div class="detail-value">{formatMemoryAddress(details.memoryAddress)}</div>
</div>

<!-- NEW: Display config value -->
{#if configValue}
    <div class="detail-row">
        <div class="detail-label">Current Value:</div>
        <div class="detail-value" class:error={configValue.value.type === 'Invalid'}>
            {formatConfigValue(configValue.value)}
        </div>
    </div>
    <div class="detail-row">
        <div class="detail-label">Last Read:</div>
        <div class="detail-value">{new Date(configValue.timestamp).toLocaleString()}</div>
    </div>
{:else}
    <div class="detail-row">
        <div class="detail-label">Current Value:</div>
        <div class="detail-value text-muted">(not yet read)</div>
    </div>
{/if}
```

---

## Testing Strategy

### Unit Tests (Backend)

**File**: `app/src-tauri/src/commands/cdi.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_int_value_1_byte() {
        let element = Element::Int { size: 1, /* ... */ };
        let data = vec![42];
        let result = parse_config_value(&element, &data).unwrap();
        assert!(matches!(result, ConfigValue::Int { value: 42, size_bytes: 1 }));
    }
    
    #[test]
    fn test_parse_eventid_value() {
        let element = Element::EventId { /* ... */ };
        let data = vec![0x05, 0x01, 0x01, 0x01, 0x03, 0x01, 0x00, 0x00];
        let result = parse_config_value(&element, &data).unwrap();
        if let ConfigValue::EventId { value } = result {
            assert_eq!(value, [0x05, 0x01, 0x01, 0x01, 0x03, 0x01, 0x00, 0x00]);
        } else {
            panic!("Expected EventId");
        }
    }
    
    #[test]
    fn test_snip_priority_cascade() {
        let node = DiscoveredNode {
            snip_data: Some(SNIPData {
                user_name: "".to_string(),
                user_description: "My Controller".to_string(),
                model_name: "GenericNode".to_string(),
                // ...
            }),
            // ...
        };
        assert_eq!(get_node_display_name(&node), "My Controller");
    }
}
```

### Integration Tests

**File**: `app/src-tauri/tests/config_reading.rs`

```rust
#[tokio::test]
async fn test_read_all_config_values_with_mock_node() {
    // Setup mock connection that responds to memory reads
    let mock_conn = MockLccConnection::new();
    mock_conn.expect_send_datagram()
        .returning(|_, data, _| {
            // Return mock config value bytes
            Ok(vec![0x54, 0x6F, 0x77, 0x65, 0x72]) // "Tower"
        });
    
    // Test read_all_config_values command
    let state = create_test_app_state(mock_conn);
    let result = read_all_config_values(
        app_handle,
        state,
        "05.01.01.01.03.01".to_string(),
        None,
        Some(false),  // Don't emit progress in test
    ).await;
    
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.successful_reads > 0);
}
```

### Frontend Tests

**File**: `app/src/lib/stores/millerColumns.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { get } from 'svelte/store';
import { millerColumnsStore } from './millerColumns';

describe('Config Value Cache', () => {
    it('stores and retrieves config values by cache key', () => {
        const value: ConfigValueWithMetadata = {
            value: { type: 'String', value: 'Tower LCC', size_bytes: 32 },
            memory_address: 16,
            address_space: 253,
            element_path: ['Settings', 'User Name'],
            timestamp: '2026-02-19T14:32:00Z',
        };
        
        millerColumnsStore.setConfigValue('05.01.01.01.03.01', ['Settings', 'User Name'], value);
        
        const retrieved = millerColumnsStore.getConfigValue('05.01.01.01.03.01', ['Settings', 'User Name']);
        expect(retrieved).toEqual(value);
    });
    
    it('calculates percentage correctly in progress updates', () => {
        const progress: ReadProgressState = {
            totalNodes: 3,
            currentNodeIndex: 1,
            currentNodeName: 'Tower LCC',
            currentNodeId: '05.01.01.01.03.01',
            totalElements: 100,
            elementsRead: 50,
            elementsFailed: 0,
            percentage: 50,
            status: { type: 'ReadingNode', node_name: 'Tower LCC' },
        };
        
        millerColumnsStore.setReadProgress(progress);
        const state = get(millerColumnsStore);
        expect(state.readProgress?.percentage).toBe(50);
    });
});
```

---

## Common Patterns & Best Practices

### SNIP Data Priority Cascade
Always use this order when displaying node names:
1. `user_name` (user-configured, highest priority)
2. `user_description` (user-configured description)
3. `model_name` (manufacturer-provided)
4. `node_id` (always available, fallback)

### Error Handling
- **Timeout**: Continue with next element/node, store as `ConfigValue::Invalid`
- **Parse Error**: Store as `ConfigValue::Invalid` with error message
- **Connection Lost**: Fail entire `read_all_config_values`, return error to frontend

### Progress Updates
- Emit after each element read (not before)
- Calculate percentage: `(elements_read + elements_failed) / total_elements * 100`
- Use status transitions: Starting → ReadingNode → NodeComplete → Complete

### Cache Management
- **Clear cache** on node refresh to ensure fresh data
- **Keep cache** during navigation within Miller Columns
- **Invalidate specific value** on manual refresh

---

## Reference Implementation

See existing code:
- **Pattern**: `app/src-tauri/src/commands/discovery.rs::refresh_all_nodes` (batch operation with progress)
- **Pattern**: `lcc-rs/src/discovery.rs::read_cdi` (multi-datagram read with parsing)
- **Pattern**: `app/src/lib/components/MillerColumns/NodesColumn.svelte` (loading state UI)

---

## Next Steps After Implementation

1. Test with real LCC hardware (multiple nodes with varied config elements)
2. Validate SNIP priority cascade displays correct node names
3. Measure performance: 3-node network should complete in ≤15 seconds
4. Add manual "Refresh Value" button to DetailsPanel (future enhancement)
5. Consider caching strategy for large networks (100+ nodes)

---

## Troubleshooting

### Values not displaying
- Check browser console for Tauri command errors
- Verify cache key format matches: `${nodeId}:${elementPath.join('/')}`
- Ensure `configValues` Map is reactive in Svelte

### Progress not updating
- Verify event listener is set up in `onMount`
- Check `emit_progress: true` is passed to command
- Confirm Tauri event emission: `app.emit_all("config-read-progress", ...)`

### Incorrect node names in progress
- Verify SNIP data priority cascade implementation
- Check `get_node_display_name` helper function
- Ensure SNIP data was successfully read during node discovery

### Timeout errors
- Increase `timeout_ms` parameter (default 2000ms may be too short)
- Check network connectivity to LCC nodes
- Verify node is responding to Memory Configuration Protocol requests
