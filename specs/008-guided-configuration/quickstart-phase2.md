# Quickstart: Implementing Profile Schema, Event Roles, and Conditional Relevance

**Feature**: 008-guided-configuration (Phase 2)
**Date**: 2026-03-01

This guide provides a sequenced implementation walkthrough for Phase 2. Follow the steps and check off each one before proceeding to the next. All paths are relative to the repository root unless noted.

---

## Prerequisites

- Phase 1 extraction complete: `profiles/tower-lcc/event-roles.json` and `profiles/tower-lcc/relevance-rules.json` exist and are validated.
- Rust toolchain: stable 1.75+. Verify with `cargo --version`.
- Working `cargo build` and `cargo test` on the feature branch.
- SvelteKit dev server compiles without errors: `npm run dev` from `app/`.

---

## Step 1 — Add `serde_yaml_ng` to Cargo.toml

In `app/src-tauri/Cargo.toml`:

```toml
[dependencies]
serde_yaml_ng = "0.10"   # YAML parsing for .profile.yaml files
```

Verify: `cargo check` passes.

---

## Step 2 — Create the `profile/` Module

Create the module directory and four files:

```
app/src-tauri/src/profile/
├── mod.rs
├── types.rs
├── loader.rs
└── resolver.rs
```

### 2a. `types.rs`

Copy the full struct definitions from [data-model-phase2.md](../data-model-phase2.md) section 1. Key structs: `StructureProfile`, `ProfileNodeType`, `FirmwareVersionRange`, `EventRoleDecl`, `ProfileEventRole`, `RelevanceRule`, `RelevanceCondition`.

Add the `impl From<ProfileEventRole> for lcc_rs::cdi::EventRole` conversion.

### 2b. `loader.rs`

```rust
use tauri::{path::BaseDirectory, Manager};
use super::types::StructureProfile;
use super::{ProfileCache, ProfileKey, make_profile_key};

pub async fn load_profile(
    manufacturer: &str,
    model: &str,
    app_handle: &tauri::AppHandle,
    cache: &ProfileCache,
) -> Option<StructureProfile> {
    let key = make_profile_key(manufacturer, model);

    // Fast path: cache hit
    {
        let cache_r = cache.read().await;
        if let Some(cached) = cache_r.get(&key) {
            return cached.clone();
        }
    }

    // Build expected filename: replace invalid chars with '_'
    let sanitize = |s: &str| s.chars()
        .map(|c| if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect::<String>();
    let filename = format!("{}.profile.yaml", sanitize(&format!("{}_{}", manufacturer, model)));

    // Check user data dir first (FR-005: takes precedence)
    let result = try_load_from_user_data(app_handle, &filename).await
        .or_else(|| try_load_from_resources(app_handle, &filename));

    // Write to cache (including None sentinel)
    cache.write().await.insert(key, result.clone());
    result
}

async fn try_load_from_user_data(
    handle: &tauri::AppHandle, filename: &str,
) -> Option<StructureProfile> {
    let path = handle.path().app_data_dir().ok()?.join("profiles").join(filename);
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    parse_profile_yaml(&content, path.to_string_lossy().as_ref())
}

fn try_load_from_resources(
    handle: &tauri::AppHandle, filename: &str,
) -> Option<StructureProfile> {
    let rel = format!("profiles/{}", filename);
    let path = handle.path().resolve(&rel, BaseDirectory::Resource).ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    parse_profile_yaml(&content, path.to_string_lossy().as_ref())
}

fn parse_profile_yaml(content: &str, source: &str) -> Option<StructureProfile> {
    match serde_yaml_ng::from_str::<StructureProfile>(content) {
        Ok(p) => {
            if p.schema_version != "1.0" {
                eprintln!("[profile] Unknown schemaVersion '{}' in {}, applying anyway",
                    p.schema_version, source);
            }
            Some(p)
        }
        Err(e) => {
            eprintln!("[profile] Failed to parse {}: {}", source, e);
            None
        }
    }
}
```

### 2c. `resolver.rs`

Implement `resolve_profile_paths(profile, cdi) -> ProfilePathMap`. Key logic:
1. Split the profile path on `/` to get name components.
2. Strip `#N` suffix from each component to get base name + ordinal (default ordinal = 1).
3. At each level, find the N-th element (`<group>`, `<segment>`) whose `name` matches the base name.
4. Record the index-based step (`seg:N` for segments, `elem:M` for groups).

Tests for resolver (before implementation, TDD):
```rust
#[test]
fn resolves_simple_segment() { ... }
#[test]
fn resolves_ordinal_suffix() { ... }
#[test]
fn resolves_replicated_group_template() { ... }
```

### 2d. `mod.rs`

Export `load_profile`, `annotate_tree`, `AnnotationReport`, `ProfileCache`, `ProfileKey`, `make_profile_key`. Re-export `types::*`.

Implement `annotate_tree` per the [backend-profile-module.md](contracts/backend-profile-module.md) contract.

### 2e. Wire into `lib.rs`

```rust
// app/src-tauri/src/lib.rs
mod profile;
```

Add `profiles: Arc::new(RwLock::new(HashMap::new()))` to `AppState::new()` and `pub profiles: ProfileCache` to `AppState`.

Verify: `cargo check --all-targets` passes.

---

## Step 3 — Update `get_node_tree` in `commands/cdi.rs`

After the existing cache check + tree build and before returning the tree:

```rust
// Get SNIP data for this node
let snip = {
    let nodes = state.nodes.read().await;
    nodes.iter()
        .find(|n| n.node_id.to_hex_string() == node_id)
        .and_then(|n| n.snip_data.clone())
};

if let Some(snip) = snip {
    if let Some(profile) = crate::profile::load_profile(
        &snip.manufacturer, &snip.model, &app_handle, &state.profiles
    ).await {
        let report = crate::profile::annotate_tree(&mut tree, &profile, &cdi);
        if !report.warnings.is_empty() {
            eprintln!("[profile] {} annotation warnings for node {}", report.warnings.len(), node_id);
        }
    }
}
```

**Also add profile roles to bowtie catalog** (in `read_all_config_values` after tree annotation, where `build_bowtie_catalog` is called): pass profile-resolved event roles as the new `profile_group_roles` parameter.

---

## Step 4 — Add `relevance_annotation` to `GroupNode`

In `app/src-tauri/src/node_tree.rs`:

```rust
pub struct GroupNode {
    // ... existing fields ...
    pub relevance_annotation: Option<RelevanceAnnotation>,
}
```

Initialize to `None` in `build_node_config_tree`. The `annotate_tree` call in `get_node_tree` populates it for groups with matching rules.

Verify: `cargo test` passes (existing unit tests for `GroupNode` should pass because the field is optional with `#[serde(skip_serializing_if = "Option::is_none")]`, or handle the default).

---

## Step 5 — Update TypeScript Types

In `app/src/lib/types/nodeTree.ts`:

1. Add the `RelevanceAnnotation` interface (from [group-node-updated.md](contracts/group-node-updated.md)).
2. Add `relevanceAnnotation: RelevanceAnnotation | null` to `GroupConfigNode`.

Verify: `npm run check` passes (TypeScript strict mode).

---

## Step 6 — Update `TreeGroupAccordion.svelte`

Add the relevance derived state and visual behavior per [group-node-updated.md](contracts/group-node-updated.md). Key implementation notes:

1. **Helper `findLeafByPath`** in `nodeTree.svelte.ts` — walk the tree by path array, return the `LeafConfigNode` or `null`.
2. **Pending edit lookup** — uses existing `pendingEditsStore`. The `editKey` format is `"${nodeId}:${space}:${address}"` which already exists in the store.
3. **Explanation banner** — new `ExplanationBanner.svelte` sub-component (or inline snippet). Style: muted background (opacity 60%), small italic text, visible in both collapsed and expanded states.
4. **200ms transition** — use `transition:slide={{ duration: 200 }}` on the collapsible content block (Svelte built-in transition).

---

## Step 7 — Author and Bundle Tower-LCC Profile

### 7a. Use `profile-7-assemble` skill

Run the `profile-7-assemble` Copilot skill with:
- `profiles/tower-lcc/event-roles.json` (Phase 1 output)
- `profiles/tower-lcc/relevance-rules.json` (Phase 1 output)
- `temp/Tower LCC CDI.xml` (CDI XML for path notation conversion)

The skill produces `RR-CirKits_Tower-LCC.profile.yaml`.

### 7b. Manual verification

Check every path in the produced YAML against the CDI XML:
- Does the `#N` ordinal correctly identify the intended group?
- Do `irrelevantWhen` values match the CDI `<map>` entries for each controlling field?
- Are `explanation` strings complete, user-friendly sentences?

### 7c. Place profile in resources

Copy the verified file to `app/src-tauri/profiles/RR-CirKits_Tower-LCC.profile.yaml`.

Add the resources declaration to `app/src-tauri/tauri.conf.json` (Step 2 prerequisite if not done).

---

## Step 8 — Run All Tests

```powershell
# Backend
cargo test --manifest-path app/src-tauri/Cargo.toml

# Frontend
cd app; npm test
```

Expected:
- `tower_lcc_profile_parses_without_warnings` integration test passes.
- `annotate_tree_*` unit tests pass.
- `resolve_profile_paths_*` unit tests pass.
- Zero regressions in existing tests.

---

## Step 9 — Manual Acceptance Testing

1. Connect a Tower-LCC node. Open the config view.  
   **Check**: All Port I/O event groups show PRODUCER or CONSUMER badge (SC-001).

2. Set Output Function on Line 1 to "No Function" (value 0).  
   **Check**: Consumer event items in the Event picker are muted. Selecting one shows the explanation banner. Producer events are unaffected. (SC-002, US-2 AC 1–4)

3. Change Output Function to any non-zero value.  
   **Check**: Consumer events become active; banner disappears. (FR-011)

4. Connect a non-Tower-LCC node.  
   **Check**: No profile-related UI appears. (SC-004)

5. Place a malformed YAML file in the user data profiles directory.  
   **Check**: App loads normally; log contains a warning about the malformed file; no crash. (SC-005)

6. Open the Bowties tab with a Tower-LCC node.  
   **Check**: Zero Tower-LCC entries in "Unknown role" section. (SC-008)

---

## CDI Template Generator (Phase 2A Tooling)

A Python script to scaffold an empty `.profile.yaml` from any CDI XML:

```
scripts/cdi-template-generator/generate-profile-template.py <cdi-xml-path> [--output <output-path>]
```

The script:
1. Parses the CDI XML.
2. Walks all groups, recording name-based paths with `#N` ordinals for same-named siblings.
3. Emits a `.profile.yaml` skeleton with `eventRoles` entries for all groups containing `<eventid>` elements (role left as a placeholder comment), and empty `relevanceRules: []`.

See `scripts/cdi-template-generator/README.md` for usage.
