# Finish Proxy Migration Plan

> **Status: Historical — implemented.** The Node Proxy migration is complete. Retain as implementation history.

## Status Quo

The Node Proxy actor architecture is in place and working. Every Tauri command
now reads from the proxy first and writes to both the proxy and the legacy
`AppState` fields (bridge pattern). The `CDI_PARSE_CACHE` lazy_static has been
fully removed.

Five legacy fields remain on `AppState`:

| Field | Type | Call sites | Notes |
|---|---|---|---|
| `nodes` | `Vec<DiscoveredNode>` | 11 | All writes are bridge-only; reads in `bowties.rs` and one fallback in `cdi.rs` |
| `config_value_cache` | `HashMap<String, HashMap<String, [u8; 8]>>` | 6 | Consumed by bowtie catalog builder; written as bridge alongside proxy |
| `node_trees` | `HashMap<String, NodeConfigTree>` | 21 | Heaviest — modify/write/discard flows all read+write this |
| `config_read_cancel` | `AtomicBool` | 3 | Session-global cancel flag, not per-node |
| `bowties_catalog` | `Option<BowtieCatalog>` | 4 | Session-global, not per-node |

## Decision: What Stays on AppState

Two fields are session-global and do not belong on per-node proxies:

- **`config_read_cancel`** — a single flag that aborts the active
  `read_all_config_values` loop across all nodes. Keep as-is.
- **`bowties_catalog`** — cross-node catalog built after all config reads
  complete. Keep as-is.

Everything else (`nodes`, `config_value_cache`, `node_trees`) migrates fully to
the proxy, and the bridge writes are deleted.

## Steps

### Step 1 — Remove `state.nodes` bridge writes (discovery.rs)

Delete the bridge calls in `discovery.rs` that write to the legacy `state.nodes`
Vec alongside proxy operations. There are 7 sites:

| Function | Line | Bridge call | Action |
|---|---|---|---|
| `register_node()` | ~103 | `state.add_node(...)` | Delete |
| `query_snip_single()` | ~135 | `state.update_node(...)` (SNIP sync) | Delete |
| `query_snip_batch()` | ~188 | `state.update_node(...)` (SNIP sync) | Delete |
| `verify_node_status()` | ~236 | `state.update_node(...)` (connected) | Delete |
| `verify_node_status()` | ~247 | `state.update_node(...)` (not-responding) | Delete |
| `query_pip_single()` | ~388 | `state.update_node(...)` (PIP sync) | Delete |
| `query_pip_batch()` | ~432 | `state.update_node(...)` (PIP sync) | Delete |

`verify_node_status()` currently only writes to `state.nodes`. It should push
the status through the proxy instead: call `proxy.update_connection_status()`.

After this step, nothing in `discovery.rs` touches `state.nodes`.

**Compile check.**

### Step 2 — Remove `state.nodes` bridge writes (cdi.rs)

Delete the 2 bridge `update_node` calls in `cdi.rs`:

| Function | Line | Bridge call | Action |
|---|---|---|---|
| `download_cdi()` | ~234 | `state.update_node(...)` (CDI data) | Delete — proxy.set_cdi_data() is already called |
| `get_cdi_xml()` | ~326 | `state.update_node(...)` (CDI from file cache) | Delete — proxy.set_cdi_data() is already called |

Also delete the state.nodes fallback read in `get_cdi_xml()` (~296) that reads
SNIP data. It should read from the proxy snapshot instead (already done on the
primary path; this is the secondary fallback when proxy CDI lookup fails but we
still need SNIP for file-cache path). Replace with
`proxy.get_snapshot().snip_data`.

**Compile check.**

### Step 3 — Replace `state.nodes` reads in bowties.rs

Two call sites read `state.nodes` to get node snapshots:

| Function | Line | What it does | Replacement |
|---|---|---|---|
| `query_event_roles()` | ~839 | Iterates nodes for alias+node_id | `state.node_registry.get_all_snapshots()` |
| `build_bowtie_catalog_command()` | ~1091 | Passes node list to `build_bowtie_catalog()` | `state.node_registry.get_all_snapshots()` |

After this, `state.nodes` has zero readers and zero writers outside of `state.rs`
itself. Remove the field, its RwLock, all helper methods (`add_node`,
`update_node`, `get_nodes`, `set_nodes`, `clear_nodes`), and the `refresh_all_nodes`
retain call (already superseded by `registry.remove()`).

**Compile check.**

### Step 4 — Replace `state.config_value_cache` with proxy reads

The config_value_cache stores `[u8; 8]` EventId bytes keyed by
`node_id → element_path`. The proxy already stores the same data via
`merge_config_values()` / `get_config_values()`.

**4a.** In `read_all_config_values()` (~2110): delete the
`state.config_value_cache.write()` block. The proxy already receives these
values via `proxy.merge_config_values()` a few lines above.

**4b.** In `read_all_config_values()` last-node block (~2187): replace
`state.config_value_cache.read().await.clone()` with a gather from all proxies:

```rust
let config_cache_snap: HashMap<String, HashMap<String, [u8; 8]>> = {
    let handles = state.node_registry.get_all_handles().await;
    let mut map = HashMap::new();
    for h in &handles {
        if let Ok(vals) = h.get_config_values().await {
            if !vals.is_empty() {
                map.insert(h.node_id.to_hex_string(), vals);
            }
        }
    }
    map
};
```

**4c.** In `set_modified_value()` (~2871): replace the
`state.config_value_cache.write()` block with a `proxy.merge_config_values()`
call using the single updated entry.

**4d.** In `discard_modified_values()` (single-node ~2924 and all-nodes ~2960):
delete the `state.config_value_cache.write()` blocks that rebuild the cache from
tree leaves. Instead, rebuild and push to the proxy via:

```rust
let rebuilt: HashMap<String, [u8; 8]> = collect_event_id_leaves(tree)
    .filter_map(|l| l.value.map(|v| (l.path.join("/"), v)))
    .collect();
proxy.set_config_values(rebuilt).await;
```

**4e.** In `build_bowtie_catalog_command()` (~1092): replace
`state.config_value_cache.read().await.clone()` with the same proxy-gather
pattern from 4b.

After this, remove `config_value_cache` field from `AppState`.

**Compile check.**

### Step 5 — Move `node_trees` operations to proxy

This is the largest step (21 call sites). The strategy is: each tree mutation
happens through `proxy.update_config_tree()` or `proxy.set_config_tree()`,
reads happen through `proxy.get_config_tree()`.

**Group A — get_node_tree():** Already migrated to check proxy first. Delete the
remaining `state.node_trees` bridge fallback read (~1477) and bridge write
(~1516).

**Group B — read_all_config_values() tree operations:** The function builds the
tree, merges config bytes, applies event roles, and applies profiles — all
currently via `state.node_trees`. Rewrite to:

1. Get or build the tree via `proxy.get_config_tree()` (fall back to
   `build_node_config_tree` if None).
2. `merge_config_values(tree, &raw_data_by_address)` locally.
3. Store back via `proxy.set_config_tree(tree)`.
4. In the last-node block, iterate all proxies (not all trees) for event role
   merge and profile application, using `proxy.update_config_tree()`.
5. Collect `profile_group_roles` by iterating proxy trees.

**Group C — set_modified_value():** Read tree from proxy, apply
`set_modified_value()`, write back to proxy. Remove the `state.node_trees`
read/write.

**Group D — discard_modified_values():** Same pattern — pull tree from proxy,
discard, push back.

**Group E — write_modified_values():** Pull trees from proxies, collect modified
leaves, update write states in-flight, commit values, push trees back.

**Group F — has_modified_values():** Iterate all proxy handles, pull tree, check
for modifications.

**Group G — build_bowtie_catalog_command():** Replace
`state.node_trees.read()` with proxy-gathered trees.

After this, remove `node_trees` field from `AppState`.

**Compile check.**

### Step 6 — Remove dead code from state.rs

Delete from `AppState`:
- `nodes` field, its initialization, and all helper methods
- `config_value_cache` field and its initialization
- `node_trees` field and its initialization
- The `disconnect()` code that clears these (registry.shutdown_all() handles it)
- Unused imports (`DiscoveredNode` if no longer referenced, etc.)

Keep: `connection`, `transport_handle`, `node_registry`, `event_router`,
`our_alias`, `config_read_cancel`, `bowties_catalog`, `profiles`, `diag_stats`,
`bowties_log`.

**Compile check.**

### Step 7 — Audit and final cleanup

- `cargo clippy` — fix any warnings
- Run `cargo test` on lcc-rs (328 tests)
- Full `cargo build` on the Tauri app
- Manual smoke test: connect, discover, read config, modify, write, discard
- Search for any remaining references to removed fields (should be zero)

## Risk Notes

1. **node_trees is the riskiest step** — 21 call sites, many with interleaved
   read-modify-write patterns. The proxy's `update_config_tree(FnOnce)` helps
   but complex multi-tree operations (last-node profile application across all
   nodes) need careful sequencing.

2. **config_value_cache gather latency** — Replacing a single HashMap read with
   N proxy round-trips (one per node) adds latency. For 10 nodes this is ~10
   message pairs through mpsc channels — sub-millisecond on a local machine.
   Not a concern.

3. **Ordering** — Steps 1-3 (remove `state.nodes`) are independent of Step 4
   (remove `config_value_cache`) and Step 5 (remove `node_trees`). Steps 4 and
   5 are also independent of each other. Step 6 depends on all of 1-5.

## Suggested Execution Order

Steps 1 → 2 → 3 → compile → 4 → compile → 5 → compile → 6 → 7

Steps 4 and 5 can be swapped. Each compile check should produce zero errors
before proceeding.
