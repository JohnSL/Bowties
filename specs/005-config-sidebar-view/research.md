# Research: Configuration Tab — Sidebar and Element Card Deck

**Feature**: 005-config-sidebar-view  
**Phase**: 0 — Research  
**Date**: 2026-02-22  

---

## RQ-001: Card Body Rendering — New Tauri Command Required?

**Question**: Can the existing `get_column_items(nodeId, parentPath, depth)` API render the complete card body (all leaf fields within a CDI group, recursively expanded), or is a new Tauri command required?

**Research**:

`get_column_items` navigates one CDI level per call. For a card body, FR-011 requires "all CDI sub-groups rendered inline and fully expanded." A group with 2 sub-group levels and 3 peer sub-groups would require 3+ sequential round-trips, adding 200–400 ms of IPC overhead and violating SC-002 (<500 ms for all cards visible).

Memory address computation for field reads also requires the absolute address, which must be accumulated across the full CDI path from segment root. Providing this in a single server response is more reliable than client-side reconstruction from multiple partial responses.

Existing pattern in `cdi.rs` already provides dedicated commands for specific UI operations (`get_cdi_structure`, `get_element_details`, `expand_replicated_group`), confirming the convention of purpose-built commands.

**Decision**: Add a new Tauri command `get_card_elements(nodeId, groupPath)` in `app/src-tauri/src/commands/cdi.rs` that returns the full recursive element tree for a CDI group in one call.

**Rationale**: Single IPC round-trip; <500 ms performance goal (SC-002) met regardless of sub-group depth; consistent with existing command design pattern.

**Alternatives Considered**:
- *Reuse `get_column_items` iteratively from frontend*: Rejected — violates SC-002 for deep or wide group trees; complex client-side path and address reconstruction.
- *Embed full subtree in `get_column_items` response*: Rejected — changes existing API contract used by Miller Columns; breaks single-responsibility separation.

---

## RQ-002: User-Given Name Lookup for Card Header (FR-007)

**Question**: How does the card header display the "user-given name" for a CDI group instance (e.g., "Yard Button (Line 3)")?

**Research**:

The LCC CDI specification (S-9.7.4.1) allows hardware vendors to include a `<string>` element within each configurable group to let users label that instance. Common element names for this field across LCC node implementations: `"User Name"`, `"Name"`, `"Description"`. No standard CDI attribute marks it as "the user name field" — it is identified by convention.

The value is stored in the frontend config cache (`millerColumnsStore.configValues`) after a `read_all_config_values()` call (feature 004). The cache key format is `getCacheKey(nodeId, elementPath)` = `"nodeId:path/segments/joined"`.

**Decision**: Implement a pure `resolveCardTitle(groupInfo, nodeId, configValues)` function in `app/src/lib/utils/cardTitle.ts` that:

1. Searches the group's `CardElementTree.fields` for a `StringElement` whose `name` (case-insensitive) matches `["user name", "name", "description"]` — first match wins.
2. Looks up its value in `configValues` using `getCacheKey`.
3. Treats non-empty, non-null-byte (`\0`), non-whitespace-only strings as a valid user-given name.
4. Computes the final title per FR-007:
   - Replicated, named: `"Yard Button (Line 3)"`
   - Replicated, unnamed: `"Line 3 (unnamed)"`
   - Non-replicated, named: `"Yard Button (Port I/O)"`
   - Non-replicated, unnamed: CDI group name alone (no "(unnamed)" suffix for non-replicated per FR-007)

**Rationale**: Frontend-side logic avoids coupling backend to config-value cache state; pure function is fully unit-testable; matches the approach used in `NodesColumn.svelte` for SNIP-based display names.

**Alternatives Considered**:
- *Backend includes resolved `userGivenName` in `get_card_elements` response*: Rejected — backend has no access to config value cache (values live in the frontend Svelte store after feature 004).
- *Require CDI authors to use a `<userNameField>` attribute*: Rejected — CDI is authored by hardware vendors; we cannot mandate naming.

---

## RQ-003: Config Value Cache — Reuse or New Store?

**Question**: Should the card deck read config values from `millerColumnsStore.configValues` or a new store?

**Research**:

FR-017: "All previously implemented functionality for reading and caching configuration values (from feature 004) MUST continue to work unchanged under the new layout."

Feature 004 stores all config values in `millerColumnsStore.configValues` (`ConfigValueMap`). The cache is populated by `readAllConfigValues()` and updated incrementally by `readConfigValue()` (the [R] action). If the card deck creates a parallel cache, FR-017 is violated and memory usage doubles for large node configs.

**Decision**: Reuse `millerColumnsStore.configValues` as the source of truth. Create a separate `configSidebarStore` (`configSidebar.ts`) only for sidebar navigation state (node expansion, selected segment, card deck loading). `ElementCard` and `FieldRow` derive their displayed values from `millerColumnsStore.configValues`.

**Rationale**: FR-017 compliance guaranteed; [R] action naturally calls `readConfigValue()` which already updates `millerColumnsStore.configValues`; no data synchronisation overhead.

**Alternatives Considered**:
- *New `configSidebarStore` with its own `configValues`*: Rejected — duplicates potentially large cache; violates FR-017; breaks [R] action update flow.
- *Merge everything into one replacement store*: Rejected — would unnecessarily break feature 004 dependencies in other components.

---

## RQ-004: Context Menu Actions (FR-016)

**Question**: What context menu actions exist in the current Miller Columns node list that must be preserved in the new sidebar?

**Research**:

From `app/src/lib/components/MillerColumns/NodesColumn.svelte` (lines 144–173):

| Action | Handler | Behavior |
|--------|---------|----------|
| View CDI XML | `openCdiViewer(nodeId, false)` | Opens `CdiXmlViewer` modal with cached CDI XML |
| Download CDI from Node | `openCdiViewer(nodeId, true)` | Downloads fresh CDI from node via `download_cdi` command, then opens viewer |

`CdiXmlViewer` (`app/src/lib/components/CdiXmlViewer.svelte`) is already a self-contained reusable component.

**Decision**: `NodeEntry.svelte` replicates the same right-click context menu (two actions) and includes `CdiXmlViewer` using the same import pattern as `NodesColumn.svelte`. No new backend commands or viewer logic required.

**Rationale**: Direct reuse of existing components satisfies FR-016 with minimal code.

---

## RQ-005: Sidebar Reset on Node Refresh (FR-018)

**Question**: How does the new sidebar detect and respond to a node refresh operation?

**Research**:

Node refresh (Discover Nodes / Refresh Nodes) is triggered from `+layout.svelte`. The resulting node list is published to `nodeInfoStore`. Currently `NodesColumn.svelte` calls `millerColumnsStore.reset()` in its `refresh()` method, which is invoked from `MillerColumnsNav.svelte`.

Since `MillerColumnsNav` is being replaced, the config tab's `+page.svelte` will:
1. Subscribe to `nodeInfoStore`
2. On node list change, call `configSidebarStore.reset()`

**Decision**: `configSidebarStore.reset()` clears: `expandedNodeIds`, `selectedSegment`, `cardDeck`, `nodeLoadingStates`, `nodeErrors`. The config page `+page.svelte` subscribes to `nodeInfoStore` and calls `reset()` reactively. This mirrors the existing refresh pattern in `MillerColumnsNav.svelte`.

---

## RQ-006: Offline Node Indicator (Edge Case)

**Question**: How is an "offline" node indicated? What data is available?

**Research**:

`DiscoveredNode.connection_status` (from `app/src/lib/api/tauri.ts`) can be `'NotResponding'`. This is set by the backend when a node fails to respond to verification.

**Decision**: `NodeEntry.svelte` shows a visual offline indicator (colour change and icon) when `connectionStatus === 'NotResponding'`. Segments remain listed (per spec edge case). Live field reads show an error state; cached values are still displayed.

---

## Summary of Decisions

| ID | Decision | Files Affected |
|----|----------|---------------|
| RQ-001 | New `get_card_elements` Tauri command | `app/src-tauri/src/commands/cdi.rs` (+~120 lines) |
| RQ-002 | `resolveCardTitle()` utility in `cardTitle.ts` | `app/src/lib/utils/cardTitle.ts` (new, ~60 lines) |
| RQ-003 | Reuse `millerColumnsStore.configValues`; new `configSidebarStore` for navigation state | `app/src/lib/stores/configSidebar.ts` (new) |
| RQ-004 | Replicate context menu in `NodeEntry.svelte`; reuse `CdiXmlViewer` | `app/src/lib/components/ConfigSidebar/NodeEntry.svelte` |
| RQ-005 | `configSidebarStore.reset()` on `nodeInfoStore` change | `app/src/routes/config/+page.svelte` |
| RQ-006 | `NotResponding` → offline indicator in `NodeEntry.svelte` | `app/src/lib/components/ConfigSidebar/NodeEntry.svelte` |
