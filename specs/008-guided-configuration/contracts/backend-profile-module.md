# Contract: Backend Profile Module API

**Feature**: 008-guided-configuration (Phase 2)
**Date**: 2026-03-01

This document defines the public Rust API surface of the new `profile/` module in `app/src-tauri/src/`.

---

## Module Layout

```
app/src-tauri/src/profile/
├── mod.rs       — Public API surface exported to the rest of the crate
├── types.rs     — StructureProfile, EventRoleDecl, RelevanceRule, etc.
├── loader.rs    — YAML file discovery and parsing
└── resolver.rs  — Name-based path → index-based path resolution
```

---

## Public API (`profile/mod.rs`)

### `load_profile`

```rust
pub async fn load_profile(
    manufacturer: &str,
    model: &str,
    cdi: &lcc_rs::cdi::Cdi,
    app_handle: &tauri::AppHandle,
    cache: &ProfileCache,
) -> Option<StructureProfile>
```

**Behaviour**:
1. Compute `ProfileKey = make_profile_key(manufacturer, model)`.
2. Check `cache` (read lock): if present (including `None` sentinel), return cached result.
3. Discover `.profile.yaml` files:
   a. Check `{app_data_dir}/profiles/{Manufacturer}_{Model}.profile.yaml` (user-placed, takes precedence).
   b. Check `{resource_dir}/profiles/{Manufacturer}_{Model}.profile.yaml` (bundled built-in).
   c. File naming: spaces in manufacturer/model are preserved in the file name (e.g., `RR-CirKits_Tower-LCC.profile.yaml`). Characters invalid in filenames are percent-encoded.
4. If no file found: store `None` in cache under this key; return `None`.
5. If file found: parse with `serde_yaml_ng::from_str::<StructureProfile>`. If parse fails: log warning with file path and error detail, store `None`, return `None` (FR-006).
6. If parse succeeds: validate `schemaVersion == "1.0"` (log warning if unknown version but continue); store `Some(profile)` in cache; return `Some(profile)`.

**Error handling**: all errors are non-fatal. The function never returns `Err`; structural errors produce `None` with a `eprintln!` warning.

---

### `annotate_tree`

```rust
pub fn annotate_tree(
    tree: &mut NodeConfigTree,
    profile: &StructureProfile,
    cdi: &lcc_rs::cdi::Cdi,
) -> AnnotationReport
```

**Behaviour**:
1. Call `resolver::resolve_profile_paths(profile, cdi)` → `ProfilePathMap`.
2. For each `EventRoleDecl` in `profile.event_roles`:
   - Resolve `group_path` via `path_map`. If not found: log warning, skip.
   - Walk the tree to find all `GroupNode`s whose `path` prefix matches the resolved path.
   - For every `LeafNode` with `element_type == LeafType::EventId` within those groups (including all replicated instances): set `leaf.event_role = Some(decl.role.into())`.
3. For each `RelevanceRule` in `profile.relevance_rules`:
   - If `rule.all_of.len() != 1`: log warning `"[profile] Rule {} skipped: multi-condition allOf (V1 evaluates only single-field rules)"`, skip (FR-009a).
   - Resolve `affected_group_path` via `path_map`. If not found: log warning (FR-012), skip.
   - Within the resolved group, find the sibling leaf named `rule.all_of[0].field`. If not found: log warning, skip.
   - Build `RelevanceAnnotation` with the resolved controlling field path + address + space, `irrelevant_when` values, and `explanation`.
   - Set `group.relevance_annotation = Some(annotation)` on all matching `GroupNode`s.
4. Return `AnnotationReport { event_roles_applied, rules_applied, warnings }`.

**Notes**:
- This function is purely synchronous and does no I/O.
- It is called by `get_node_tree` after profile loading.
- Event roles applied by `annotate_tree` take precedence over any roles previously set by `merge_event_roles` (protocol-exchange roles). Call `annotate_tree` AFTER `merge_event_roles`.

---

### `AnnotationReport`

```rust
pub struct AnnotationReport {
    pub event_roles_applied: usize,
    pub rules_applied: usize,
    pub warnings: Vec<String>,
}
```

Returned by `annotate_tree`. Warnings are also `eprintln!`-ed immediately; this struct allows the caller to log a summary or include it in telemetry.

---

## `ProfileCache` (in `AppState`)

```rust
// In state.rs
pub profiles: Arc<RwLock<HashMap<ProfileKey, Option<StructureProfile>>>>,
```

Initialized as empty `HashMap` on `AppState::new()`. The `Option<StructureProfile>` value allows caching a "not found" result (avoids repeated file I/O on every `get_node_tree` call for nodes without profiles).

---

## File Naming Convention

Profile file names follow `{Manufacturer}_{Model}.profile.yaml` where:
- Spaces are preserved.
- Characters invalid in Windows/macOS/Linux file names (`\ / : * ? " < > |`) are replaced with `_`.
- Example: `RR-CirKits_Tower-LCC.profile.yaml`

---

## Integration in `get_node_tree`

```rust
#[tauri::command]
pub async fn get_node_tree(
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
    node_id: String,
) -> Result<NodeConfigTree, String> {
    // ... existing cache check and build ...

    // NEW: apply profile if available
    let snip = /* get snip from node cache */;
    if let Some((manufacturer, model)) = snip.as_ref().map(|s| (&s.manufacturer, &s.model)) {
        if let Some(profile) = crate::profile::load_profile(
            manufacturer, model, &cdi, &app_handle, &state.profiles
        ).await {
            let report = crate::profile::annotate_tree(&mut tree, &profile, &cdi);
            eprintln!("[profile] {} — {} event roles, {} rules applied, {} warnings",
                node_id, report.event_roles_applied, report.rules_applied, report.warnings.len());
        }
    }

    // Cache and return
    let mut trees = state.node_trees.write().await;
    trees.insert(node_id.clone(), tree.clone());
    Ok(tree)
}
```

---

## Tauri Config Addition

In `app/src-tauri/tauri.conf.json`, add the profiles directory to `bundle.resources`:

```json
{
  "bundle": {
    "resources": {
      "profiles/": "profiles/"
    }
  }
}
```

Built-in profiles live in `app/src-tauri/profiles/` relative to the manifest file.

---

## `build_bowtie_catalog` Addition (FR-016)

```rust
pub fn build_bowtie_catalog(
    nodes: &[lcc_rs::DiscoveredNode],
    event_roles: &HashMap<[u8; 8], NodeRoles>,
    config_value_cache: &HashMap<String, HashMap<String, [u8; 8]>>,
    // NEW optional parameter — profile-declared group roles, keyed by "node_id:path_key"
    profile_group_roles: Option<&HashMap<String, lcc_rs::EventRole>>,
) -> BowtieCatalog
```

In the same-node ambiguity resolution block: before checking `heuristic_role`, check if `profile_group_roles` contains an entry for `"{node_id}:{slot.element_path.join("/")}"`. If found, use that role and place the slot in `producers` or `consumers` (not `ambiguous_entries`).

---

## Tests Required

| Test | Type | Location |
|------|------|----------|
| `load_profile_parses_valid_yaml` | Unit | `profile/loader.rs` |
| `load_profile_returns_none_for_invalid_yaml` | Unit | `profile/loader.rs` |
| `load_profile_returns_none_for_missing_file` | Unit | `profile/loader.rs` |
| `annotate_tree_applies_event_roles` | Unit | `profile/mod.rs` |
| `annotate_tree_skips_multi_condition_rules` | Unit | `profile/mod.rs` |
| `annotate_tree_skips_unknown_path` | Unit | `profile/mod.rs` |
| `resolve_profile_paths_basic` | Unit | `profile/resolver.rs` |
| `resolve_profile_paths_ordinal_suffix` | Unit | `profile/resolver.rs` |
| `resolve_profile_paths_roundtrip` | Property | `profile/resolver.rs` |
| `build_bowtie_catalog_uses_profile_roles` | Unit | `commands/bowties.rs` |
| `tower_lcc_profile_parses_without_warnings` | Integration | `tests/profile_integration.rs` |
