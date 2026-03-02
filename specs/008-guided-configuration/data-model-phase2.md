# Data Model: Structure Profile Schema and Tree Extensions

**Feature**: 008-guided-configuration (Phase 2)
**Date**: 2026-03-01

This document defines all new and modified data structures for Phase 2: the `.profile.yaml` deserialization schema (Rust), the tree extensions that carry profile data to the frontend, and the path resolution types.

---

## 1. StructureProfile — Rust Deserialization Target

Deserialised from a `.profile.yaml` file using `serde_yaml_ng`. This is the in-memory representation of one profile file.

```rust
// profile/types.rs

use serde::{Deserialize, Serialize};

/// Root of a `.profile.yaml` file.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureProfile {
    /// Schema version string. Currently must be "1.0".
    pub schema_version: String,

    /// Node type identification (manufacturer + model).
    pub node_type: ProfileNodeType,

    /// Optional firmware version range. Advisory only — does not gate profile application.
    #[serde(default)]
    pub firmware_version_range: Option<FirmwareVersionRange>,

    /// Event role declarations for CDI groups containing eventid leaves.
    #[serde(default)]
    pub event_roles: Vec<EventRoleDecl>,

    /// Conditional relevance rules.
    #[serde(default)]
    pub relevance_rules: Vec<RelevanceRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileNodeType {
    pub manufacturer: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareVersionRange {
    pub min: Option<String>,
    pub max: Option<String>,
}

/// Declares the event role for all eventid leaves within a named CDI group.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRoleDecl {
    /// Name-based CDI path using '/' separators and '#N' ordinal suffix for same-named
    /// siblings (1-based). E.g., "Port I/O/Line/Event#1".
    pub group_path: String,

    /// Declared role for all eventid leaves in this group.
    pub role: ProfileEventRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ProfileEventRole {
    Producer,
    Consumer,
}

impl From<ProfileEventRole> for lcc_rs::cdi::EventRole {
    fn from(r: ProfileEventRole) -> Self {
        match r {
            ProfileEventRole::Producer => lcc_rs::cdi::EventRole::Producer,
            ProfileEventRole::Consumer => lcc_rs::cdi::EventRole::Consumer,
        }
    }
}

/// Conditional relevance rule.
///
/// When the `allOf` conditions are satisfied (V1: only single-condition rules are
/// evaluated; multi-condition rules are skipped with a log warning), the
/// `affected_group_path` section is considered irrelevant.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceRule {
    /// Unique identifier within this profile (e.g., "R001").
    pub id: String,

    /// CDI group path of the section rendered irrelevant when the condition fires.
    pub affected_group_path: String,

    /// Conditions that must ALL be true (V1: only single-entry lists are evaluated).
    pub all_of: Vec<RelevanceCondition>,

    /// User-facing explanation text shown verbatim in the UI banner.
    pub explanation: String,
}

/// One condition within a relevance rule's allOf list.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceCondition {
    /// CDI name of the controlling field, sibling within the same replicated group
    /// instance as the affected group. E.g., "Output Function".
    pub field: String,

    /// Integer enum values of the controlling field that render the section irrelevant.
    pub irrelevant_when: Vec<i64>,
}
```

---

## 2. ProfileStore — In-Memory Cache

One `ProfileStore` per Tauri `AppState`. Loaded lazily on first `get_node_tree` call for a given manufacturer+model.

```rust
// profile/mod.rs

use std::collections::HashMap;
use tokio::sync::RwLock;

/// Key: "{manufacturer}::{model}" (normalized, lowercase, trimmed)
pub type ProfileKey = String;

pub fn make_profile_key(manufacturer: &str, model: &str) -> ProfileKey {
    format!("{}::{}", manufacturer.trim().to_lowercase(), model.trim().to_lowercase())
}

/// Cache of loaded structure profiles.
/// `None` entry means "profile was looked up but not found" (avoids re-scanning).
pub type ProfileCache = Arc<RwLock<HashMap<ProfileKey, Option<StructureProfile>>>>;
```

Added to `AppState`:
```rust
pub profiles: ProfileCache,
```

---

## 3. RelevanceAnnotation — Tree Extension

Added to `GroupNode` in `node_tree.rs`. Carries all information the frontend needs to evaluate and display relevance state, including the pre-resolved controlling field address so the frontend can look up the pending edit without tree traversal.

```rust
// node_tree.rs

/// Relevance rule annotation attached to a GroupNode.
/// Present only when a profile declares a relevance rule for this group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceAnnotation {
    /// Unique rule identifier from the profile (e.g., "R001").
    pub rule_id: String,

    /// Index-based path of the controlling leaf within the same tree.
    /// Frontend uses this to find the leaf and read its current value.
    pub controlling_field_path: Vec<String>,

    /// Memory address of the controlling field leaf.
    /// Combined with `controlling_space`, forms the pendingEditsStore key.
    pub controlling_field_address: u32,

    /// Memory space of the controlling field.
    pub controlling_field_space: u8,

    /// Integer enum values of the controlling field that make this section irrelevant.
    pub irrelevant_when: Vec<i64>,

    /// User-facing explanation rendered verbatim in the UI banner.
    pub explanation: String,
}
```

Updated `GroupNode` struct:
```rust
pub struct GroupNode {
    // ... existing fields unchanged ...
    pub name: String,
    pub description: Option<String>,
    pub instance: u32,
    pub instance_label: String,
    pub replication_of: String,
    pub replication_count: u32,
    pub path: Vec<String>,
    pub children: Vec<ConfigNode>,
    /// Profile-sourced relevance rule annotation. None when no profile rule targets this group.
    pub relevance_annotation: Option<RelevanceAnnotation>,  // NEW
}
```

---

## 4. TypeScript Extensions

Added to `app/src/lib/types/nodeTree.ts`:

```typescript
// New type
export interface RelevanceAnnotation {
  ruleId: string;
  controllingFieldPath: string[];
  controllingFieldAddress: number;
  controllingFieldSpace: number;
  irrelevantWhen: number[];
  explanation: string;
}

// Modified GroupConfigNode — one new optional field
export interface GroupConfigNode {
  kind: 'group';
  name: string;
  description: string | null;
  instance: number;
  instanceLabel: string;
  replicationOf: string;
  replicationCount: number;
  path: string[];
  children: ConfigNode[];
  relevanceAnnotation: RelevanceAnnotation | null;  // NEW
}
```

The `pendingEditsStore` key format `"${nodeId}:${space}:${address}"` already supports looking up edits by the controlling field address — no store changes needed.

---

## 5. Profile Path Resolution

The `resolver.rs` module converts a name-based profile group path (e.g., `"Port I/O/Line/Event#1"`) to the index-based tree path format (`["seg:N", "elem:M#I"]`). Resolution runs once per profile per CDI parse, storing the result in `ProfileStore`.

```rust
// profile/resolver.rs

/// Maps a profile group path string to a resolved index-based path.
/// Key: profile group path (e.g., "Port I/O/Line/Event#1")
/// Value: tree index path prefix (e.g., ["seg:1", "elem:2"])
pub type ProfilePathMap = HashMap<String, Vec<String>>;

/// Resolve all paths declared in a profile against the parsed CDI.
///
/// Returns a map suitable for fast lookup during tree annotation.
/// Paths that do not resolve produce a log warning and are excluded.
pub fn resolve_profile_paths(
    profile: &StructureProfile,
    cdi: &lcc_rs::cdi::Cdi,
) -> ProfilePathMap {
    let mut map = HashMap::new();
    
    for decl in &profile.event_roles {
        match resolve_one_path(&decl.group_path, cdi) {
            Ok(path) => { map.insert(decl.group_path.clone(), path); }
            Err(e) => eprintln!("[profile] Could not resolve path '{}': {}", decl.group_path, e),
        }
    }

    for rule in &profile.relevance_rules {
        match resolve_one_path(&rule.affected_group_path, cdi) {
            Ok(path) => { map.insert(rule.affected_group_path.clone(), path); }
            Err(e) => eprintln!("[profile] Could not resolve path '{}': {}", rule.affected_group_path, e),
        }
    }

    map
}

/// Parse a name-based profile path and walk the CDI by name+ordinal to produce
/// an index-based path prefix.
///
/// Path format: "Segment Name/Group Name[/#N]/..."
/// '#N' suffix (1-based) selects among same-named sibling groups.
/// Groups with unique names within their parent require no suffix.
fn resolve_one_path(
    profile_path: &str,
    cdi: &lcc_rs::cdi::Cdi,
) -> Result<Vec<String>, String> {
    // Implementation: walk CDI by name+ordinal, emit "seg:N" / "elem:M" steps
    // Full implementation in tasks.
    todo!()
}
```

**Path component parsing rules**:
- `Event#1` → base name `Event`, ordinal 1 (first among same-named siblings)
- `Event#2` → base name `Event`, ordinal 2
- `Line` → base name `Line`, ordinal 1 (no suffix = first, unique if only one)
- Ordinal is 1-based; N=1 is implicit when no `#N` suffix is present

---

## 6. Profile Application Flow in `get_node_tree`

The call sequence when `get_node_tree` is invoked for a node with a matching profile:

```
get_node_tree(node_id)
  → get_cdi_from_cache(node_id)           // CDI parse cache hit
  → build_node_config_tree(node_id, &cdi)  // or cache hit from AppState.node_trees
  → load_profile(manufacturer, model, &app_handle, &state.profiles)
     → check user data dir: {app_data}/profiles/{Manufacturer}_{Model}.profile.yaml
     → check bundled resources: profiles/{Manufacturer}_{Model}.profile.yaml
     → parse YAML → StructureProfile
     → resolve_profile_paths(&profile, &cdi) → ProfilePathMap
  → annotate_tree(&mut tree, &profile, &path_map)
     → for each EventRoleDecl → override matching leaf event_roles
     → for each RelevanceRule (single-condition only) →
          resolve controlling field path →
          add RelevanceAnnotation to matching GroupNode
  → return tree (with relevance_annotations and overridden event_roles)
```

---

## 7. Tower-LCC Profile: Key Entries

The bundled Tower-LCC profile must include at minimum:

| Entry | Type | Affected Group | Controlling Field | Irrelevant When |
|-------|------|----------------|-------------------|-----------------| 
| Consumer events role | EventRole | `Port I/O/Line/Event#1` | — | — |
| Producer events role | EventRole | `Port I/O/Line/Event#2` | — | — |
| Consumer events irrelevant | RelevanceRule | `Port I/O/Line/Event#1` | `Output Function` | `[0]` (No Function) |
| Producer events irrelevant | RelevanceRule | `Port I/O/Line/Event#2` | `Input Function` | `[0]` (Disabled) |
| Delay group irrelevant | RelevanceRule | `Port I/O/Line/Delay` | `Output Function` | `[0]` (No Function) |

All other Tower-LCC event groups (Conditionals, Track Receiver, Track Transmitter, Node Power Monitor) require event role declarations. See `profiles/tower-lcc/event-roles.json` (Phase 1 extraction output) for the complete list.
