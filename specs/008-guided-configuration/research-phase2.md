# Research: Profile Schema, Event Roles, and Conditional Relevance

**Feature**: 008-guided-configuration (Phase 2)
**Date**: 2026-03-01

---

## 1. YAML Parsing Crate in Rust

### Decision
Use `serde_yaml_ng = "0.10"`.

### Rationale
- `serde_yaml 0.9` (dtolnay) was **officially deprecated and archived in mid-2023**. It still compiles as of Rust 1.75 but receives no security patches or bug fixes.
- `serde_yaml_ng` is a transparent fork starting from the last dtolnay commit, preserving full git history. It is the current community-recommended successor (~2M crates.io downloads, actively maintained by Antoine Catton, as of early 2026).
- API is drop-in compatible with `serde_yaml 0.9`: same `from_str` / `to_string` calls, same `serde::Deserialize` derive.
- `serde_yml` (another fork) was flagged by the community for supply-chain risk patterns (large "Initial commit" with no attribution history).

### Addition to Cargo.toml
```toml
serde_yaml_ng = "0.10"
```

### Alternatives Considered
- **`serde_yaml 0.9`**: Still compiles but unmaintained; not appropriate for new code.
- **`serde_yml`**: Supply-chain risk; community consensus is to avoid it.
- **`marked-yaml`**: Provides source-location tracking in error messages but requires a low-level AST API — significant boilerplate. Only necessary if precise YAML error reporting with line numbers is a hard requirement (it is not).
- **`yaml-rust2` + manual serde**: Substantial boilerplate with no advantage over `serde_yaml_ng`.

---

## 2. Profile Path Addressing: Name-Based vs Index-Based

### Decision
The `.profile.yaml` format uses **name-based CDI paths** with `#N` ordinal suffix for same-named siblings (e.g., `Port I/O/Line/Event#1`). The Rust profile loader **resolves** these name-based paths to the existing **index-based tree paths** (`seg:N/elem:M/elem:K#I`) at profile load time, producing a static mapping stored in `ProfileStore`. This mapping is used at tree annotation time without further CDI traversal.

### Rationale

**Why name-based paths in the profile file?**
- Profile files are **authored by humans** (hardware manufacturers, community contributors) who know CDI groups by their display names, not by their positional indices in the CDI XML.
- CDI element names are stable across firmware versions; positional indices can shift when a firmware update inserts or reorders elements.
- The spec mandates name-based paths (FR-015) with `#N` ordinal suffix notation for same-named siblings.

**Why index-based paths in the runtime tree?**
- The existing codebase (`node_tree.rs`) already uses index-based paths (`seg:N/elem:M`) throughout: `merge_event_roles`, `classify_leaf_roles_from_protocol`, and `pendingEditsStore` all key by path strings in this format.
- The frontend (`TreeLeafRow`, `TreeGroupAccordion`) uses these paths for identity and dirty tracking.
- Changing the tree path format would be a large breaking change with no benefit.

**Resolution strategy in `resolver.rs`**:

Walk the CDI simultaneously by name and by index. For each step in a name-based profile path:
1. Find all elements at the current level whose `<name>` matches the path component (ignoring `#N` suffix for now).
2. Among same-named elements, pick the N-th one (1-based ordinal) according to the `#N` suffix (or the first if no suffix).
3. Record the resolved index for that step.
4. Continue recursively.

The resolver produces `HashMap<ProfilePathKey, ResolvedIndexPath>`. The full resolution runs once at profile load time (triggered by the first `get_node_tree` call for that node type).

### Alternatives Considered
- **Name-based paths in the tree**: Would require changing ~15 existing functions in `node_tree.rs`, `commands/cdi.rs`, and `commands/bowties.rs`, plus all TypeScript types. Not feasible.
- **Index-based paths in the profile file**: Profile authors would need to inspect CDI XML raw structure; brittle to firmware updates. Explicitly rejected by FR-015.
- **Lazy resolution at query time** (per-tree-build): Slower; resolution result would be discarded and re-computed every call. The mapping is deterministic per CDI (same profile + same CDI = same mapping) so caching is safe.

---

## 3. Profile Transport to Frontend: Embedded in GroupNode vs Sidecar

### Decision
Profile data is **embedded directly in the serialized `GroupNode`** returned by `get_node_tree`. A new optional `relevanceAnnotation` field is added to `GroupNode` (Rust) and `GroupConfigNode` (TypeScript). Event role data is already on `LeafNode.event_role` and requires no additional field.

### Rationale
- The frontend already consumes `NodeConfigTree` as a single tree structure and accesses all data via the tree. Adding annotations to nodes keeps all rendering information co-located.
- FR-002 requires the profile to be "fully applied on first render — no asynchronous update". Embedding in the tree ensures the frontend never receives a partial tree.
- `get_node_tree` is already the command the frontend calls; adding a field avoids a new Tauri command.
- The `relevanceAnnotation` field is `Option<RelevanceAnnotation>` / `null` — fully backward-compatible. Nodes without a profile annotation produce `null`, which the frontend already handles via the `if (annotation)` pattern used for other optional fields.

### Alternatives Considered
- **Separate Tauri command for profile data**: Requires two round-trips before render and coordination logic in the frontend. Rejected by FR-002's synchronous constraint.
- **Svelte store for profile data alongside the tree**: Would require a separate broadcast event + store subscription; risks the "flash on load" pattern explicitly called out as unacceptable in the spec clarifications.
- **Attach rules to `LeafNode` instead**: Relevance rules fire on a group, not individual leaves — grouping at the `GroupNode` level matches the CDI structure and the UI structure (`TreeGroupAccordion`).

---

## 4. Relevance Rule Reactivity in Svelte 5

### Decision
`TreeGroupAccordion.svelte` derives the controlling field's current value reactively using `$derived` from `nodeTreeStore`. When the value changes (via `pendingEditsStore` or `node-tree-updated` refresh), the derived computation re-runs and the accordion's visual state updates within Svelte's next animation frame.

### Rationale

**How the controlling field value is accessed**:
The `RelevanceAnnotation` embedded in the `GroupConfigNode` contains `controllingFieldPath: string[]` — the full index-based tree path of the controlling leaf (e.g., `["seg:1", "elem:2#3", "elem:0"]`). The current value of that leaf is available from two places:

1. **`pendingEditsStore`** — if the user has made a pending (unsaved) edit to the field, use the `pendingValue` from the store. This is keyed by `"${nodeId}:${space}:${address}"`.
2. **`nodeTreeStore`** — the committed tree value from the last CDI read, accessible by walking the tree to the leaf at `controllingFieldPath`.

Precedence: pending edit value > committed tree value > null (indeterminate → show all sections).

**Svelte 5 reactivity pattern**:
```svelte
// In TreeGroupAccordion.svelte
const controllingValue = $derived(() => {
  if (!annotation?.controllingFieldPath) return null;
  // Check pending edit first
  const editKey = `${nodeId}:${annotation.controllingSpace}:${annotation.controllingAddress}`;
  const pending = $pendingEditsStore.get(editKey);
  if (pending?.pendingValue?.type === 'int') return pending.pendingValue.value;
  // Fall back to committed tree value
  return findLeafValue(treeStore.trees.get(nodeId), annotation.controllingFieldPath);
});

const isIrrelevant = $derived(() => {
  if (!annotation?.irrelevantWhen || controllingValue === null) return false;
  return annotation.irrelevantWhen.includes(controllingValue);
});
```

This is a pure derivation — no explicit subscription management, no manual invalidation. Svelte 5's fine-grained reactive graph re-evaluates `isIrrelevant` whenever either the pending edits map changes or the node tree is refreshed.

### Alternatives Considered
- **Tauri event (`node-tree-updated`) triggers re-fetch**: The tree is already refreshed via this event; the `$derived` above will automatically react. No additional wiring needed.
- **Polling the controlling field value**: Latency and unnecessary CPU use. Svelte's reactive graph handles this correctly.
- **One-time evaluation on CDI load**: Explicitly rejected by the spec (Assumption 5 correction; FR-011). Configuration write mode is active; users can change controlling fields.

---

## 5. Tauri Resource Bundling for Built-in Profiles

### Decision
Bundle built-in profiles as Tauri resources in `app/src-tauri/profiles/` and declare them in `tauri.conf.json` under `bundle.resources`. Read at runtime using `app_handle.path().resolve("profiles/...", BaseDirectory::Resource)`.

### Rationale
- Tauri 2.x `bundle.resources` is the standard mechanism for shipping static data files alongside the binary. Files are placed next to the executable on all platforms (Windows: `.\resources\`, macOS: `MyApp.app/Contents/Resources/`, Linux: next to binary).
- `BaseDirectory::Resource` is the stable Tauri 2.x API for accessing these files at runtime from Rust.
- Alternative of embedding YAML as `include_str!()` would work but prevents profile updates without recompilation; resource files are accessible by path and could in principle be updated without a full release.

### Tauri config addition
```json
{
  "bundle": {
    "resources": {
      "profiles/": "profiles/"
    }
  }
}
```

### User-placed profiles
Accessed at `app_handle.path().app_data_dir().join("profiles")`. This resolves to:
- Windows: `%APPDATA%\{bundle-id}\profiles\`
- macOS: `~/Library/Application Support/{bundle-id}/profiles/`
- Linux: `~/.local/share/{bundle-id}/profiles/`

User-placed files take precedence over built-in (FR-005): the loader checks user data dir first, then built-in resources.

---

## 6. Profile-7-Assemble: Path Notation Conversion

### Decision
The `profile-7-assemble` skill converts CDI paths from **extraction output notation** (index-range brackets, e.g., `Port I/O/Line/Event[0-5]`) to **profile file notation** (`#N` ordinal suffix, e.g., `Port I/O/Line/Event#1`). The skill reads `event-roles.json` and `relevance-rules.json` (Phase 1 extraction outputs) and produces a `.profile.yaml` ready for validation.

### Rationale
- Phase 1 extraction outputs use index-range notation because the extraction prompts analyze CDI document order; this is more explicit for LLM reasoning.
- Phase 2 profile files use `#N` ordinal because it's more compact and maps directly to "the 1st/2nd etc. group with this name" for human authors.
- The conversion is purely mechanical: `Event[0-5]` is the first (index 0 through 5) `<group name="Event">` in document order → `Event#1`; `Event[6-11]` is the second → `Event#2`.

### Conversion rule
For a CDI path like `Segment/Group/Name[low-high]`:
1. Strip the index range to get the base name: `Name`.
2. Determine which ordinal this group is among same-named siblings at the same level in the CDI by looking up the CDI XML.
3. If ordinal > 1, append `#N`; if ordinal = 1 and no same-named siblings, no suffix needed.

---

## 7. Bowtie Catalog Integration Point for Profile Roles

### Decision
Profile-declared event roles are applied in `build_bowtie_catalog` via a new parameter: an optional `profile_roles: Option<&HashMap<String, lcc_rs::EventRole>>` keyed by node_id+group_path. In the same-node ambiguity resolution block (where `both` is non-empty), if a profile declares the role for the slot's group, that role is used instead of the CDI heuristic.

### Rationale
- `build_bowtie_catalog` is a pure function already accepting `event_roles` and `config_value_cache`; adding a `profile_roles` parameter keeps it pure and testable.
- The existing ambiguity resolution block (Tier 0) checks the slot's `heuristic_role` (from CDI text heuristics). Profile roles override this before the Tier 0 check.
- FR-016 and FR-017 require profile resolution only for nodes with a matching profile; the `Option<&HashMap>` naturally handles no-profile nodes.
- Alternative of resolving ambiguity in `get_node_tree` and then re-propagating to bowtie builder is more complex (requires storing resolved roles in `AppState` and passing to bowtie builder separately). Passing directly is simpler.
