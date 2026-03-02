# Contract: Updated GroupConfigNode Type

**Feature**: 008-guided-configuration (Phase 2)
**Date**: 2026-03-01

This document defines the changes to `GroupConfigNode` (TypeScript frontend type) and the corresponding `GroupNode` (Rust backend type) to carry `RelevanceAnnotation` from the profile loader to the frontend rendering layer.

---

## Rust: Updated `GroupNode`

**File**: `app/src-tauri/src/node_tree.rs`

**Change**: Add one optional field `relevance_annotation`. All other fields are unchanged.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    pub name: String,
    pub description: Option<String>,
    pub instance: u32,
    pub instance_label: String,
    pub replication_of: String,
    pub replication_count: u32,
    pub path: Vec<String>,
    pub children: Vec<ConfigNode>,
    /// Profile-sourced relevance rule annotation.
    /// `None` when no matching profile, or when profile has no rule for this group.
    /// Serialized as `relevanceAnnotation: null` in JSON when absent.
    pub relevance_annotation: Option<RelevanceAnnotation>,  // NEW
}
```

The `RelevanceAnnotation` struct is defined in `profile/types.rs` and re-exported from `node_tree.rs` via `pub use`.

**Backward compatibility**: Adding a new `Option` field serialized via `serde(rename_all = "camelCase")` is backward-compatible. Old frontend code that does not reference `relevanceAnnotation` continues to work; the field is simply `null` for all groups pre-profile and for nodes without a matching profile.

---

## TypeScript: Updated `GroupConfigNode`

**File**: `app/src/lib/types/nodeTree.ts`

### New type: `RelevanceAnnotation`

```typescript
/**
 * Profile-sourced relevance rule annotation on a group node.
 * Present only when a profile declares a relevance rule that targets this group.
 */
export interface RelevanceAnnotation {
  /** Rule identifier from the profile (e.g., "R001"). */
  ruleId: string;

  /** Index-based tree path of the controlling leaf (e.g., ["seg:1", "elem:2#3", "elem:0"]). */
  controllingFieldPath: string[];

  /** Absolute memory address of the controlling field leaf. */
  controllingFieldAddress: number;

  /** Memory space of the controlling field. */
  controllingFieldSpace: number;

  /**
   * Integer values of the controlling field that render this group irrelevant.
   * If the controlling field's current value is in this list, the group is "not applicable".
   */
  irrelevantWhen: number[];

  /**
   * User-facing explanation text shown VERBATIM in the UI banner.
   * Never paraphrase or substitute this text.
   */
  explanation: string;
}
```

### Updated `GroupConfigNode`

```typescript
/**
 * A (possibly replicated) group of child nodes.
 * Mirrors Rust GroupNode.
 */
export interface GroupConfigNode {
  kind: 'group';
  name: string;
  description: string | null;
  /** 1-based replication instance number (1 when not replicated). */
  instance: number;
  /** Computed label, e.g., "Event 3" */
  instanceLabel: string;
  /** Original group name before replication — used for sibling grouping. */
  replicationOf: string;
  /** Total number of replications for this group template. */
  replicationCount: number;
  /** Index-based path, e.g., ["seg:0", "elem:2#3"] */
  path: string[];
  children: ConfigNode[];
  /**
   * Profile-sourced relevance annotation.
   * null when no profile matches the node, or when the profile has no rule for this group.
   */
  relevanceAnnotation: RelevanceAnnotation | null;  // NEW
}
```

---

## Frontend Component Changes

### `TreeGroupAccordion.svelte`

**File**: `app/src/lib/components/ElementCardDeck/TreeGroupAccordion.svelte`

**New props** (exported via `let { ... } = $props()`):
```typescript
// Existing props (unchanged)
let { group, nodeId, ... } = $props<{ group: GroupConfigNode; nodeId: string; ... }>();
// No new props — annotation is accessed directly from group.relevanceAnnotation
```

**New derived state**:
```typescript
import { pendingEditsStore, getPendingIntValue } from '$lib/stores/pendingEdits.svelte';
import { nodeTreeStore, findLeafByPath } from '$lib/stores/nodeTree.svelte';

const annotation = $derived(group.relevanceAnnotation);

/**
 * Current integer value of the controlling field.
 * Checks pending edit first (user-changed but unsaved), then committed tree value.
 * Returns null if the value is indeterminate (CDI not fully read, or no annotation).
 */
const controllingValue = $derived((): number | null => {
  if (!annotation) return null;
  const editKey = `${nodeId}:${annotation.controllingFieldSpace}:${annotation.controllingFieldAddress}`;
  const pending = $pendingEditsStore.get(editKey);
  if (pending?.pendingValue?.type === 'int') return pending.pendingValue.value;
  const tree = $nodeTreeStore.trees.get(nodeId);
  if (!tree) return null;
  const leaf = findLeafByPath(tree, annotation.controllingFieldPath);
  if (leaf?.value?.type === 'int') return leaf.value.value;
  return null;
});

/**
 * Whether this group is currently in an "irrelevant" state.
 * false when indeterminate (value unknown) — always safer to show.
 */
const isIrrelevant = $derived((): boolean => {
  if (!annotation || controllingValue === null) return false;
  return annotation.irrelevantWhen.includes(controllingValue);
});
```

**Visual behavior changes** (all gated on `isIrrelevant`):

1. **Standalone section mode** (when `siblings.length === 1` or `pillMode === false`):
   - When `isIrrelevant`: accordion is collapsed by default; muted explanation banner renders beneath the header.
   - User can still expand the accordion (FR-010).
   - Banner remains visible in expanded state.

2. **Pill mode** (when `siblings.length > 1`; replicated group shown as pills):
   - Individual pill items whose group has `isIrrelevant = true` are rendered with muted styling (opacity reduction + "not applicable" label).
   - Selecting a muted pill shows the explanation banner beneath the pill row.
   - Unaffected pills (whose group annotation has `isIrrelevant = false`) are unaffected.
   - Edge case: if ALL pills in the set are irrelevant → collapse the entire section with the banner (identical to standalone behavior).

3. **Transition** (FR-011): CSS transition of approximately 200ms on collapse/expand state and banner visibility.

### `TreeLeafRow.svelte`

No changes required for Phase 2. The "not applicable" treatment is at the group level (accordion/pill), not the individual leaf level.

---

## JSON Wire Format Example

`get_node_tree` response excerpt for a Tower-LCC Port I/O Line's Event#1 group (consumer events, with Output Function = 0):

```json
{
  "kind": "group",
  "name": "Event",
  "description": null,
  "instance": 1,
  "instanceLabel": "Event 1",
  "replicationOf": "Event",
  "replicationCount": 6,
  "path": ["seg:1", "elem:0#1", "elem:2"],
  "children": [ ... ],
  "relevanceAnnotation": {
    "ruleId": "R001",
    "controllingFieldPath": ["seg:1", "elem:0#1", "elem:0"],
    "controllingFieldAddress": 259,
    "controllingFieldSpace": 253,
    "irrelevantWhen": [0],
    "explanation": "Consumer events (Commands) that control line output state are irrelevant when no output function is configured. These events only take effect when an Output Function other than 'No Function' is selected."
  }
}
```

The `controllingFieldPath` and `controllingFieldAddress` both point to the "Output Function" leaf of the same Line instance. The frontend can use either to look up the current value.
