# Research: Editable Bowties

**Feature Branch**: `009-editable-bowties`  
**Date**: 2026-03-15

## R-001: YAML Layout File Schema Design

**Decision**: Use a flat YAML document with event ID (dotted hex) as top-level keys for bowtie metadata, and a separate `roleClassifications` section keyed by `{nodeId}:{elementPath}`.

**Rationale**: The spec requires event IDs as stable keys (FR-020), human-readable format (FR-025), and dotted hex notation (Clarification). The existing codebase uses `serde_yaml_ng` 0.10 with `#[serde(rename_all = "camelCase")]` patterns (profile types). Event IDs are already stored as dotted hex strings in `BowtieCard.event_id_hex`. A flat keyed structure is natural for YAML, readable, and mergeable.

**Alternatives Considered**:
- **JSON**: Rejected — spec explicitly requires YAML (FR-019) for consistency with existing profile files and human readability.
- **SQLite**: Rejected — over-engineered for metadata storage; not human-editable outside the app.
- **Nested structure** (bowties as array): Rejected — dotted hex as map keys provides O(1) lookup and aligns with spec requirement FR-020.

**Schema**:
```yaml
schemaVersion: "1.0"
bowties:
  "05.01.01.01.FF.00.00.01":
    name: "Yard Entry Signal"
    tags: ["yard", "signals"]
    state: "active"  # active | incomplete | planning
  "05.01.01.01.FF.00.00.02":
    name: "Main Turnout Control"
    tags: ["mainline"]
    state: "active"
roleClassifications:
  "05.02.01.02.03.00:Port I/O/Line #1/Event Produced":
    role: "Producer"
  "05.02.01.02.03.00:Port I/O/Line #1/Event Consumed":
    role: "Consumer"
```

---

## R-002: Tauri File Dialog Integration

**Decision**: Add `tauri-plugin-dialog` v2 to `Cargo.toml` and `tauri.conf.json` for native OS file dialogs (Open/Save As).

**Rationale**: The spec requires native OS file dialogs (FR-021, FR-022). Tauri v2 does not include dialog support in core — it requires the `tauri-plugin-dialog` plugin. The project already uses `tauri-plugin-opener` as a precedent for Tauri plugins.

**Alternatives Considered**:
- **Custom dialog in Svelte**: Rejected — would not feel native; spec says "native OS file dialogs."
- **Browser `<input type="file">`**: Not available in Tauri desktop context for Save As.

**Implementation**: 
- Backend: Add `tauri-plugin-dialog = "2"` to `app/src-tauri/Cargo.toml`
- Frontend: Import from `@tauri-apps/plugin-dialog` — `open()` for file open, `save()` for save-as
- Register plugin in `lib.rs` builder: `.plugin(tauri_plugin_dialog::init())`

---

## R-003: Bidirectional Sync Architecture

**Decision**: Event-driven architecture where config edits trigger bowtie catalog recalculation, and bowtie edits create pending config edits through the existing `pendingEditsStore`.

**Rationale**: The existing architecture already has the right primitives:
1. **Config → Bowtie**: When event ID values change in `pendingEditsStore`, the bowtie catalog view should reactively recompute which event IDs are shared. This is a derived computation from the config value cache + pending edits overlay.
2. **Bowtie → Config**: When a bowtie edit adds/removes an element, the system creates entries in `pendingEditsStore` (same store used by Config tab), so the unified save/discard lifecycle (FR-018f) works naturally.

Both directions flow through `pendingEditsStore` as the single source of truth for unsaved changes, with the live bowtie catalog being a derived view.

**Alternatives Considered**:
- **Separate bowtie edits store**: Rejected as primary — would create two parallel save/discard lifecycles. However, bowtie-only metadata (names, tags) does need a separate tracking mechanism since it doesn't correspond to node memory writes.
- **Direct node writes on bowtie edit**: Rejected — violates the unsaved-change tracking requirement (FR-018a). All changes must be pending until explicit Save.

**Sync Flow**:
```
Config Tab Edit → pendingEditsStore.setEdit() → derived bowtie preview recomputes
Bowtie Tab Edit → pendingEditsStore.setEdit() for event slot writes
                → bowtieMetadataStore.setEdit() for names/tags (YAML-only)
Save → write all pendingEdits to nodes → save bowtieMetadata to YAML
Discard → clearAll() on both stores
```

---

## R-004: Unified Save/Discard Lifecycle

**Decision**: Extend `pendingEditsStore` to track bowtie-originated config edits alongside manual config edits. Add a parallel `bowtieMetadataStore` for YAML-only changes (names, tags, role classifications). Both stores share the same Save/Discard buttons.

**Rationale**: FR-018f requires "Pending bowtie changes and pending config changes MUST share the same save/discard lifecycle." The existing `pendingEditsStore` already handles config writes with a proven state machine (dirty → writing → clean/error). Bowtie edits that change event slot values are functionally identical to config edits — they just originate from a different UI. Metadata-only changes (names, tags) are purely local and don't need node writes, only YAML file save.

**Alternatives Considered**:
- **Single monolithic store**: Rejected — conflates node memory writes with local file writes. The write protocols are different (Memory Config protocol vs. file I/O).
- **Fully separate lifecycles**: Rejected — violates FR-018f.

**Implementation**:
- `pendingEditsStore` handles: event ID value changes (from either Config or Bowtie tabs)
- `bowtieMetadataStore` handles: name changes, tag changes, bowtie creation/deletion, role classifications
- `SaveControls` reads both stores' dirty state for the global indicator
- Save action: (1) write pending config edits to nodes, (2) write bowtie metadata to YAML
- Discard action: clear both stores

---

## R-005: Event ID Selection Rules

**Decision**: Implement the three-tier event ID selection logic from FR-002 in the frontend New Connection dialog, with backend validation before write.

**Rationale**: The selection rules are:
1. If one side is already connected → use its event ID, write to other side
2. If both connected to different bowties → prompt user to choose
3. If both unconnected → use producer's current event ID, write to consumer

This logic is UI-driven (requires user prompts in case 2) and should live in the frontend dialog component. The backend validates that the selected event ID and target slot are consistent before executing the write.

**Alternatives Considered**:
- **Backend-only logic**: Rejected — case 2 requires user interaction (choice prompt), which must happen in the UI.
- **Always generate new event IDs**: Rejected — LCC event IDs are node-assigned; the system reuses existing slot values.

---

## R-006: Element Picker Design

**Decision**: Reuse `NodeConfigTree` data with filtering for the element picker in the New Connection dialog. Present as a collapsible tree (nodes → segments → groups → event slots) with role badges and search.

**Rationale**: The `NodeConfigTree` structure already contains the full hierarchy, event slot addresses, and event role annotations. The `collectEventIdLeaves()` helper from `nodeTree.ts` can enumerate all event slots. Profile-annotated roles and heuristic roles are already on each leaf node's `eventRole` field. The picker needs to filter by: (1) correct role side, (2) has unconnected slots, (3) search query match.

**Alternatives Considered**:
- **Flat list of all event slots**: Rejected — loses context (which node, which group/segment).
- **Separate data fetch for picker**: Rejected — `NodeConfigTree` already has all needed data cached in `nodeTreeStore`.

---

## R-007: Rollback Strategy for Multi-Node Writes

**Decision**: Sequential writes with best-effort rollback. Store pre-write values, attempt restore on failure, surface detailed error state if rollback fails.

**Rationale**: FR-029a requires sequential writes with rollback attempt. The existing `write_config_value` command returns `WriteResponse` with success/failure per write. The rollback strategy:
1. Before writing, snapshot original values from `pendingEditsStore.originalValue`
2. Write node A → success → write node B → failure
3. Attempt rollback: write original value back to node A
4. If rollback fails: surface error with full details (which node/address succeeded, which failed)

**Alternatives Considered**:
- **Two-phase commit**: Rejected — LCC Memory Configuration protocol doesn't support prepare/commit semantics.
- **No rollback**: Rejected — spec explicitly requires rollback attempt (FR-029a).

---

## R-008: Recent Layout File Tracking

**Decision**: Store the most recently opened layout file path in `app_data_dir/recent-layout.json`. Simple single-path tracking, not a full recent-files list.

**Rationale**: FR-027 requires "The app MUST remember the most recently opened layout file path and offer to reopen it on startup." The existing `connection.rs` pattern stores JSON in `app_data_dir`. A simple JSON file with the last opened path is sufficient for v1. The atomic write pattern (write temp → rename) from connection prefs should be reused.

**Alternatives Considered**:
- **Full recent-files list**: Over-engineered for v1 — spec only requires remembering the most recent.
- **OS-level recent files API**: Would vary by platform; simple JSON is portable.

---

## R-009: Ambiguous Role Classification Persistence

**Decision**: Store user role classifications in the layout YAML file under a `roleClassifications` section, keyed by `"{nodeId}:{elementPath}"` with value `{ role: "Producer" | "Consumer" }`.

**Rationale**: FR-015c requires classifications to persist "in the layout file, keyed by node ID and element path." The layout file is the natural location since classifications are per-layout (different layouts might classify the same element differently for different use cases). The key format `{nodeId}:{path}` matches the existing `profile_group_roles` key format used in `build_bowtie_catalog`.

**Alternatives Considered**:
- **Store in profile YAML**: Rejected — profiles are per-node-type, not per-layout; user classifications are instance-specific.
- **Separate classifications file**: Rejected — spec says "in the layout file."

---

## R-010: Bowtie State Model

**Decision**: Three bowtie states: `active` (has producers and consumers), `incomplete` (has elements but missing one side), `planning` (no elements attached, name only).

**Rationale**: The spec defines these states across multiple stories:
- US4: "planning" state for intent-first bowties with no elements
- FR-010: "incomplete" state when one side has zero elements
- Normal: "active" when both sides have at least one element

State transitions:
- `planning` → `active` (both sides populated)
- `planning` → `incomplete` (one side populated)
- `active` → `incomplete` (remove last element from one side)
- `incomplete` → `active` (add element to empty side)
- `active`/`incomplete` → `planning` (remove all elements, user chooses to keep)
- Any → deleted (user deletes)

**Alternatives Considered**:
- **Two states** (active/draft): Rejected — doesn't distinguish between "no elements" and "missing one side."
- **Boolean flags**: Rejected — cleaner as an enum with clear semantics.
