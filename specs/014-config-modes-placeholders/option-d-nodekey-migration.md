# Option D: NodeKey-Only In Memory — Session Instructions

**Branch:** `014-config-modes-placeholders`
**Created:** 2026-06-01
**Context:** The NodeKey sum type migration (ADR-0010, regression-fix-plan Steps 2–6) landed
the type and migrated the registry + event router + frontend tree/roster/info stores, but
stopped short of migrating the bowtie catalog pipeline. The result is a format split:
backend `EventSlotEntry.node_id` is dotted (via `to_hex_string()`), frontend tree store keys
are canonical (via `toCanonicalNodeKey`). Composite slot keys `${nodeId}:${path}` never match,
producing phantom duplicate bowtie entries.

## Design rule

- **In memory (Rust & TS):** every node identity is `NodeKey` (or its serialized canonical form).
  `to_hex_string()` is display-only — never used as a key, comparison operand, or composite-key component.
- **On disk (layout YAML):** dotted hex is fine for human readability. Parse to `NodeKey` at
  the read boundary; serialize from `NodeKey` at the write boundary.
- **IPC wire format:** `NodeKey` serializes as canonical 12-hex for live, `placeholder:<uuid>` for placeholders.

## Regression test (write FIRST)

Before any migration code, add this test to `bowties.svelte.test.ts` (or a new
`bowties.slotkey.test.ts`):

```
Test: "catalog entries and tree entries use the same slot-key form"

Given:
  - A BowtieCatalog from the backend containing an EventSlotEntry with
    node_id = "05.02.01.02.02.00" (dotted, as the backend currently emits)
    and element_path = ["seg:1", "elem:1", "elem:2"]
  - A nodeTreeStore tree keyed by the canonical form "050201020200"
    containing a leaf at the same CDI path with a matching eventId value

When: buildEffectiveBowtiePreview() runs

Then:
  - The preview contains exactly ONE entry for that slot (not two)
  - The entry's node_name resolves to the SNIP name (not the raw dotted id)
```

This test should FAIL before the fix and PASS after.

---

## Phase 1 — Backend core types (compiler-driven)

### 1A. `state.rs` — Migrate `NodeRoles` and `EventSlotEntry`

**File:** `app/src-tauri/src/state.rs`

| Field | Current | Target |
|-------|---------|--------|
| `NodeRoles.producers` | `HashSet<String>` | `HashSet<NodeKey>` |
| `NodeRoles.consumers` | `HashSet<String>` | `HashSet<NodeKey>` |
| `EventSlotEntry.node_id` | `String` (dotted) | `node_key: NodeKey` |
| `OfflineBowtieData.config_values` | `HashMap<String, HashMap<String, [u8; 8]>>` | `HashMap<NodeKey, HashMap<String, [u8; 8]>>` |
| `OfflineBowtieData.profile_roles` | `HashMap<String, EventRole>` (key = `"nodeId:path"`) | Keep as `HashMap<String, EventRole>` BUT construct keys using `NodeKey::to_string()` (canonical), not `to_hex_string()` |
| `OfflineBowtieData.cdi_xml` | `HashMap<String, String>` | `HashMap<NodeKey, String>` |
| `ActiveLayoutContext.layout_node_keys` | `Vec<String>` | `Vec<NodeKey>` |

**Serde note:** `EventSlotEntry` is serialized over IPC. Renaming `node_id` to `node_key`
changes the JSON field name. The frontend `EventSlotEntry` interface must match. If you
want backward compatibility, use `#[serde(rename = "node_id")]` on the Rust side — but
since IPC payloads are ephemeral (not persisted), a clean rename is preferred.

After changing these types, `cargo check` will surface every call site that needs updating.
Follow the compiler errors.

### 1B. `commands/bowties.rs` — Follow compiler errors

Key patterns to replace:

| Pattern | Current | Target |
|---------|---------|--------|
| `node.node_id.to_hex_string()` as map key | `slot_map.insert(node.node_id.to_hex_string(), slots)` | `slot_map.insert(NodeKey::from(node.node_id), slots)` |
| `config_value_cache: &HashMap<String, ...>` | String-keyed parameter | `&HashMap<NodeKey, ...>` |
| `.find(\|n\| n.node_id.to_hex_string() == *node_id)` (8 occurrences) | String comparison | `.find(\|n\| NodeKey::from(n.node_id) == *node_key)` |
| `format!("{}:{}", node_id, path_key)` composite keys (~6 occurrences) | Dotted composite | `format!("{}:{}", node_key, path_key)` — `NodeKey::Display` emits canonical |
| `entry.node_id: String` construction | Dotted string | `entry.node_key: NodeKey` |
| `alias_to_node_id: HashMap<u16, String>` | Dotted value | `HashMap<u16, NodeKey>` |
| `node_display_name` helper | Takes `&str` node_id | Takes `&NodeKey` or keep taking `&DiscoveredNode` |

**Node name resolution:** The `node_display_name(n)` helper at line ~89 takes a
`&DiscoveredNode`. The fallback `node_id.clone()` (when no SNIP name) should become
`node_key.to_string()` (canonical form). This is a display string, but it's the fallback —
the user will see `050201020200` instead of `05.02.01.02.02.00`. If you want the dotted
form as the display fallback, use `node_id.to_hex_string()` but store `NodeKey` in the
struct. The key point is: the struct field is `NodeKey`; the display derivation is separate.

### 1C. `commands/layout_capture.rs` — Follow compiler errors

| Pattern | Current | Target |
|---------|---------|--------|
| `let dotted_id = snapshot.node_id...to_hex_string()` → used as key | Dotted key | `let nk = NodeKey::from(*snapshot.node_id.as_ref().unwrap()); ... offline_data.config_values.insert(nk, ...)` |
| `offline_data.profile_roles.insert(key.clone(), role)` on layout load | Key is saved dotted-form `"05.02...:seg:1/..."` | Parse the node-id prefix to `NodeKey`, reconstruct as `format!("{}:{}", nk, path_portion)` |
| `offline_data.cdi_xml.insert(dotted_id, xml)` | Dotted key | `NodeKey` key |

**Layout file read boundary:** When loading `bowties.yaml` role classifications, the saved
keys are dotted (`05.02.01.02.02.00:seg:1/elem:1/elem:2`). At the read boundary, split on
the first `:`, parse the node-id portion via `NodeKey::parse()`, and reconstruct the composite
key using canonical form. This is the "dotted-only-on-disk, canonical-in-memory" boundary.

### 1D. `commands/cdi.rs` — Follow compiler errors

IPC request structs (`GetNodeTreeRequest`, `ReadCdiRequest`, etc.) have `node_id: String`.
These are already parsed to `NodeKey` at the command boundary (Step 4b of the original plan
landed this). Verify no new string-keyed paths were added since then.

Composite keys in `read_all_config_values` (`format!("{}:{}", node_id, path)`) — update to
use canonical form from `NodeKey`.

### 1E. Other backend files

- `node_tree.rs`: `NodeConfigTree.nodeId` is `String` sent to frontend. Change to `node_key: String`
  containing the canonical form (or change to `NodeKey` with serde). The frontend already
  reads `tree.nodeId` from the IPC payload — update the TS type to match.
- `commands/connector_profiles.rs`: `node_id: String` in IPC structs — change to `NodeKey`.
- `commands/sync_panel.rs`: `node_id: Option<String>` — change to `Option<NodeKey>`.
- `layout/node_snapshot.rs`: `canonical_node_filename(node_id: &str)` — change to accept `&NodeKey`.

### 1F. Run `cargo check` — iterate until clean

The compiler is the migration tool. Every error it surfaces is a site that needs `NodeKey`.
Do not suppress with `.to_string()` shortcuts — each site should hold a `NodeKey` value,
not a `String` derived from one.

### 1G. Run `cargo test` — iterate until green

Existing tests may construct `EventSlotEntry` with string node_ids. Update them to use
`NodeKey::from_node_id(...)` or `NodeKey::parse(...)`.

---

## Phase 2 — Frontend alignment

### 2A. `api/tauri.ts` — Update `EventSlotEntry` interface

```typescript
export interface EventSlotEntry {
  node_key: string;        // canonical 12-hex or placeholder:<uuid>
  // ... (rename from node_id)
}
```

Or if you keep `node_id` with `#[serde(rename)]` on the Rust side:
```typescript
export interface EventSlotEntry {
  node_id: string;         // NOW canonical 12-hex (not dotted)
  // ...
}
```

Either way, the value is now canonical. Every frontend consumer that reads `entry.node_id`
gets canonical form and no longer needs to call `toCanonicalNodeKey`.

### 2B. `stores/bowties.svelte.ts` — Add `canonicalSlotKey` helper

```typescript
function canonicalSlotKey(nodeId: string, elementPath: string[]): string {
  return `${toCanonicalNodeKey(nodeId)}:${elementPath.join('/')}`;
}
```

Replace every inline `\`${entry.node_id}:${entry.element_path.join('/')}\`` with
`canonicalSlotKey(entry.node_id, entry.element_path)`. After Phase 1 lands, the
`toCanonicalNodeKey` call in the helper becomes a no-op (input is already canonical),
but the helper makes the pattern greppable and protects against future regressions.

**Sites to update in bowties.svelte.ts:**
- `nodeSlotMap` getter (~line 101)
- `effectiveNodeSlotMap` getter (~line 131)
- `getRoleForSlot` (~line 175)
- `buildEffectiveBowtiePreview` `existingKeys` set (~line 300)
- `buildEffectiveBowtiePreview` `newEntryKeys` set (~line 325)
- `buildTreeEntriesIndex` slot key and `entry.node_id` (~line 531)

### 2C. Other frontend stores

- `bowtieMetadata.svelte.ts`: `getRoleClassification(slotKey)` — callers must pass
  canonical slot keys. Verify the metadata store's internal map keys are canonical.
- `effectiveLayoutStore.svelte.ts`: `usedInMap` and `nodeSlotMap` derivations — verify
  they use the same canonical form.
- Verify `nodeRoster.svelte.ts` keys (should already be canonical via `liveKeyFromBytes`).

### 2D. Components

- `ElementPicker.svelte`: verify `entry.node_id` lookups into `nodeTreeStore` use
  canonical form. After Phase 1, `entry.node_id` (now `entry.node_key`) is canonical, so
  `nodeTreeStore.getTree(entry.node_key)` should work — `getTree` already accepts
  `NodeKeyInput` and canonicalizes internally.
- `BowtieCatalogPanel.svelte`, `BowtieCard.svelte`, `ElementEntry.svelte`: verify any
  `entry.node_id` references are updated if the field was renamed.

### 2E. Run `npm test -- --run` — iterate until green

---

## Phase 3 — Cleanup and verification

### 3A. Grep for remaining `to_hex_string()` used as keys

```bash
grep -rn "to_hex_string()" app/src-tauri/src/ | grep -v "// display" | grep -v "test"
```

Every remaining `to_hex_string()` call should be in a display/logging context, not as a
map key or comparison operand.

### 3B. Grep for remaining un-canonicalized slot-key construction

```bash
grep -rn '`\${.*node_id' app/src/lib/ | grep -v 'toCanonicalNodeKey\|canonicalSlotKey'
```

Should return zero results (excluding test files).

### 3C. Manual smoke test

1. New layout → connect → read config for one node → verify bowties show correctly (no duplicates)
2. Save → close → reopen → verify bowties still show correctly with node names
3. Verify the saved `bowties.yaml` has dotted-form keys (on-disk format unchanged)
4. Verify role classifications work (classify an unknown role → save → reopen → still classified)

### 3D. Post-work enrichment

- Update `aiwiki/owners.md` to note the completed migration.
- Update `aiwiki/architecture-health.md` to close the "stringly-typed NodeKey" risk.
- Extend ADR-0010 with a dated section noting the migration completion.
- Check `specs/backlog.md` for related items.

---

## Slicing strategy

This is a compiler-driven migration — the type changes in Phase 1A propagate mechanically.
The recommended approach:

1. **Slice 1 (AFK):** Phase 1A + 1B + 1F + 1G — core Rust types + bowties.rs + compile + test
2. **Slice 2 (AFK):** Phase 1C + 1D + 1E + recompile — remaining backend files
3. **Slice 3 (AFK):** Phase 2A + 2B + 2C + 2D + 2E — frontend alignment + tests
4. **Slice 4 (HITL):** Phase 3 — grep audit, manual smoke test, enrichment

Each slice lands green (compiles + tests pass) before the next starts. The app may not run
correctly between Slice 1 and Slice 3 because the IPC contract changes; that's expected.
