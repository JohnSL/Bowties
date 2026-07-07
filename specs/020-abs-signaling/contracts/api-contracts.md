# API Contracts: ABS Signaling

**Feature Branch**: `020-abs-signaling` | **Date**: 2026-07-07

These contracts define the Tauri IPC boundary between frontend and backend for ABS signaling features.

## Existing Commands (Extended)

### `list_behavior_templates` (unchanged signature, new data)

Returns all registered behavior templates, now including the ABS 3-Aspect Signal template.

**Direction**: Frontend → Backend  
**Returns**: `BehaviorTemplate[]`

**New template in response**:
```json
{
  "templateId": "abs-3-aspect-signal",
  "displayName": "ABS 3-Aspect Signal",
  "slots": [
    {
      "label": "block",
      "displayLabel": "Block",
      "kind": "Producer",
      "requiredRole": "block-occupancy",
      "minChannels": 1,
      "maxChannels": 1
    },
    {
      "label": "downstream",
      "displayLabel": "Downstream Signal",
      "kind": "Producer",
      "requiredRole": "signal-aspect",
      "minChannels": 0,
      "maxChannels": 1
    },
    {
      "label": "signal_head",
      "displayLabel": "Signal Head",
      "kind": "Consumer",
      "requiredRole": "signal-aspect",
      "minChannels": 1,
      "maxChannels": 1
    }
  ],
  "rules": [
    {
      "condition": { "type": "InputState", "inputSlot": "block", "state": "occupied" },
      "aspect": "stop",
      "priority": 1
    },
    {
      "condition": { "type": "DownstreamAspect", "inputSlot": "downstream", "aspect": "stop" },
      "aspect": "approach",
      "priority": 2
    },
    {
      "condition": { "type": "Default" },
      "aspect": "clear",
      "priority": 3
    }
  ],
  "compilationTarget": "tower-lcc-conditional"
}
```

### `list_facilities` (unchanged)

Returns all facilities. No structural changes. ABS signal facilities use the same `Facility` shape.

### `save_layout_edits` (extended behavior)

When saving a facility whose template has a `compilationTarget`, the backend:
1. Compiles the template rules against the bound channels and target node
2. Produces CDI field writes as staged in-memory changes
3. Creates/updates the `LogicAllocationRecord`
4. Returns the changes along with other layout edits

**No signature change** — the compilation is triggered internally when processing facility deltas.

## New Commands

### `get_logic_capacity`

Query available logic resources on a Tower LCC node.

**Direction**: Frontend → Backend  
**Parameters**:
```typescript
interface GetLogicCapacityRequest {
  nodeKey: string;  // Target Tower LCC node
}
```

**Returns**:
```typescript
interface LogicCapacity {
  nodeKey: string;
  totalConditionalLines: number;      // Always 32 for Tower LCC
  usedConditionalLines: number;       // Lines claimed by any facility
  availableConditionalLines: number;  // 32 - used
  totalTrackCircuits: number;         // Always 8 for Tower LCC
  usedTrackCircuits: number;          // Circuits claimed by any facility
  availableTrackCircuits: number;     // 8 - used
  allocations: AllocationSummary[];   // Per-facility breakdown
}

interface AllocationSummary {
  facilityId: string;
  facilityName: string;
  conditionalLines: number[];  // 0-indexed line indices
  trackCircuits: number[];     // 1-indexed circuit numbers
}
```

**Errors**:
- `NodeNotFound` — node key doesn't match a known node
- `NotLogicCapable` — node is not a Tower LCC (no conditional lines)

### `suggest_logic_target`

Suggest the best Tower LCC node for a facility based on input channel proximity.

**Direction**: Frontend → Backend  
**Parameters**:
```typescript
interface SuggestLogicTargetRequest {
  facilityId: string;  // Facility being applied
}
```

**Returns**:
```typescript
interface LogicTargetSuggestion {
  suggestedNodeKey: string | null;   // Best candidate, or null if none
  reason: string;                    // Human-readable explanation
  candidates: LogicTargetCandidate[];
}

interface LogicTargetCandidate {
  nodeKey: string;
  nodeName: string;
  availableLines: number;
  availableCircuits: number;
  inputChannelCount: number;  // How many of the facility's inputs are on this node
}
```

### `delete_facility` (extended behavior)

When deleting a facility that has a `LogicAllocationRecord`:
1. Reset allocated conditional lines to Blocked/disabled state (CDI writes queued as staged changes)
2. Free Track Circuit allocations
3. Delete the `LogicAllocationRecord`
4. Warn if other facilities reference this one as a downstream signal

**Parameters** (unchanged):
```typescript
interface DeleteFacilityRequest {
  facilityId: string;
}
```

**Returns** (extended):
```typescript
interface DeleteFacilityResult {
  deleted: boolean;
  cascadeWarnings: CascadeWarning[];  // NEW: broken references
}

interface CascadeWarning {
  affectedFacilityId: string;
  affectedFacilityName: string;
  slotLabel: string;  // Which slot references the deleted facility's output
}
```

## Frontend Store Contracts

### effectiveLayoutStore (extended getters)

```typescript
// New getters on the effective layout facade
interface EffectiveLayoutExtensions {
  // Logic capacity for a node (delegates to backend cache or IPC)
  logicCapacity(nodeKey: string): LogicCapacity | undefined;
  
  // All Tower LCC nodes that support conditional lines
  logicCapableNodes(): NodeSummary[];
  
  // Eligible signal-aspect styles for binding
  signalAspectStyles(): SignalAspectStyle[];
}
```

### facilitiesStore (extended operations)

```typescript
// New operations on the facilities draft store
interface FacilitiesStoreExtensions {
  // Apply a compiled facility (triggers compilation + allocation)
  applyCompiledFacility(facilityId: string, targetNodeKey: string): Promise<void>;
}
```

## Channel Role Extensions

### New Role: `signal-aspect`

```typescript
// Added to ChannelRole type
type ChannelRole = 'block-occupancy' | 'lamp-indicator' | 'signal-aspect';

// Signal-aspect channel vocabulary
interface SignalAspectVocabulary {
  states: ['unknown', 'stop', 'approach', 'clear'];
}
```

### New Style: `2-led-bicolor-aspect`

```typescript
// Added to channelStyles registry
'2-led-bicolor-aspect': {
  pinCount: 2,
  pinLabels: ['Red', 'Green'],
  bindingKind: 'lampRow',  // Binds to Direct Lamp Control rows
  aspectMap: {
    stop:     [{ pin: 0, state: 'on' },  { pin: 1, state: 'off' }],
    approach: [{ pin: 0, state: 'on' },  { pin: 1, state: 'on' }],
    clear:    [{ pin: 0, state: 'off' }, { pin: 1, state: 'on' }],
    unknown:  [{ pin: 0, state: 'off' }, { pin: 1, state: 'off' }],
  },
  constraints: []
}
```
