# Research: Information Channels — Auto-Create & Inventory

**Feature**: 015-information-channels  
**Date**: 2026-06-24  
**Status**: Complete — all NEEDS CLARIFICATION items resolved

## R1: Where should channel data live relative to the existing layout persistence model?

**Decision**: New `channels.yaml` file in the layout folder root.

**Rationale**: The layout directory already uses a one-file-per-concern pattern (`manifest.yaml`, `bowties.yaml`, `offline-changes.yaml`, `event-names.yaml`). Adding `channels.yaml` follows this established pattern exactly. The file is read/written through the same journaled save plan (ADR-0006) that protects all other layout files.

**Alternatives considered**:
- Embed channels inside `bowties.yaml` → rejected: violates one-file-per-concern; channels are conceptually independent of bowtie metadata.
- Store channels inside node snapshots → rejected: channels are layout-level abstractions that reference nodes, not owned by them. A channel survives across sessions even when its backing node is offline.

## R2: How to determine a BOD daughter board's pin count for channel auto-creation?

**Decision**: Derive from the `kind` field in the shared-daughterboards profile YAML plus explicit metadata.

**Rationale**: The existing daughter board profiles use a `kind` field (`"detection"`, `"mixed-io"`, etc.) and `lineOrdinals` arrays in validity rules to implicitly describe how many lines a board governs. However, `lineOrdinals` is a constraint mechanism, not a channel-count declaration. For channel auto-creation we need an explicit **channel-eligible line count**.

**Approach**: Add a `channelCount` metadata field to the shared-daughterboards YAML profile for each BOD variant. This is cleaner than deriving counts from constraint arrays:

| Board ID | `kind` | `channelCount` | Channel Type |
|----------|--------|-----------------|--------------|
| BOD4 | mixed-io | 4 | block-occupancy |
| BOD4-CP | mixed-io | 4 | block-occupancy |
| BOD-8-SM | detection | 8 | block-occupancy |

When `channelCount` is absent, no channels are auto-created (non-BOD boards like OI-IB-8, FOB-A).

**Alternatives considered**:
- Derive from `lineOrdinals` length → rejected: fragile; `lineOrdinals` absence means "all 8" which requires knowing the slot capacity; and some constraints apply to non-detection lines.
- Hard-code per board ID in frontend → rejected: violates single-source-of-truth; profile YAML is the authoritative source for daughter board capabilities.

## R3: Should channels be stored in the LayoutFile struct or in a separate file/struct?

**Decision**: Separate file (`channels.yaml`) with a new `ChannelsFile` struct, read and written alongside `LayoutFile` in the save plan.

**Rationale**: The `LayoutFile` struct owns bowtie metadata and role classifications — conceptually distinct from information channels. Adding a separate `ChannelsFile` struct:
- Keeps the LayoutFile struct focused (SOLID — single responsibility)
- Allows independent schema evolution
- Follows the existing pattern where `manifest.yaml`, `bowties.yaml`, and `offline-changes.yaml` each have their own top-level struct

**Implementation pattern**: Add `channels: ChannelsFile` field to `LayoutDirectoryWriteData` and `LayoutDirectoryReadData`. In `write_layout_capture`, serialize to `channels.yaml`. In `read_companion_contents`, read with existence-check fallback to `ChannelsFile::default()` (empty channels list).

## R4: How to hook channel auto-creation into the existing connector selection flow?

**Decision**: Extend `connectorSelectionOrchestrator.applyConnectorSelectionChange()` to call a channel-creation step after the existing three-step flow completes.

**Rationale**: The existing flow is: (1) update slot selection, (2) refresh node tree, (3) recompute compatibility. Channel auto-creation is a fourth step triggered by the same user action ("select a daughter board"). The orchestrator already owns this multi-step workflow and is the correct owner for the extension.

**Approach**:
1. After step 3 (compatibility), check if the newly selected daughter board has `channelCount > 0` in its profile metadata.
2. If yes, auto-create channels via `channelsStore.createChannels(...)`.
3. If the previous selection had channels and the new selection is different, show confirmation dialog before removing old channels.
4. The orchestrator coordinates the dialog + store mutation; the store owns the state.

**Alternatives considered**:
- Create a separate `channelAutoCreationOrchestrator` → rejected: over-engineering; the trigger is already owned by `connectorSelectionOrchestrator`. A separate orchestrator would need to subscribe to the same event and coordinate with the same dialog flow.
- React to store changes in a Svelte `$effect` → rejected: violates the "orchestrators own multi-step async workflows" boundary; effects are for rendering, not workflow sequencing.

## R5: How should the Railroad tab integrate with the existing tab architecture?

**Decision**: Add `'railroad'` to the existing tab discriminated union in `+page.svelte`, following the identical pattern used for `'config'` and `'bowties'` tabs.

**Rationale**: Tabs are simple local state in the route component — a string union with conditional rendering. No tab framework or registry exists. Adding a third option is minimal, consistent, and doesn't require architectural changes.

**Implementation**:
- Extend type: `'config' | 'bowties' | 'railroad'`
- Add keyboard navigation: ArrowRight from bowties → railroad; ArrowLeft from railroad → bowties
- Add toolbar button with same CSS class pattern (`toolbar-seg`, `toolbar-btn-active`)
- Add `{:else if activeTab === 'railroad'}` conditional block
- Railroad tab is last (rightmost) per spec

## R6: How should channel removal on daughter board change work with confirmation?

**Decision**: Use the existing `@tauri-apps/plugin-dialog` confirm dialog (already a dependency) to show a warning before removing channels.

**Rationale**: The spec requires a confirmation warning (FR-009, Story 4). The app already has `@tauri-apps/plugin-dialog` for native OS dialogs. Using the same dialog mechanism keeps UX consistent.

**Flow**:
1. User selects a different daughter board (or "none") for a slot that has existing channels.
2. Before applying the selection, the orchestrator checks `channelsStore` for channels referencing that node + slot.
3. If channels exist, show confirm dialog: "Changing the daughter board will remove N channels. Continue?"
4. If confirmed: apply selection + remove channels.
5. If cancelled: revert the selection in the UI (no backend mutation).

## R7: What happens to `channels.yaml` when opening a layout saved before this feature?

**Decision**: `channels.yaml` is absent → `ChannelsFile::default()` (empty channels map). No migration, no retroactive inference.

**Rationale**: The spec explicitly states "channels are not retroactively inferred from existing daughter board selections." The `#[serde(default)]` pattern used throughout the layout persistence layer handles this naturally. When the user saves the layout after any edit, `channels.yaml` will be written (even if empty), matching the behavior of other companion files.

## R8: Should the `LayoutEditDelta` enum support channel operations?

**Decision**: Yes — add `CreateChannel`, `RenameChannel`, and `DeleteChannel` delta variants.

**Rationale**: The existing delta pattern (`LayoutEditDelta`) is the established mechanism for all layout mutations. It provides:
- Atomic application via `apply_layout_deltas()`
- Offline change journaling (though channels are not written to nodes, the layout mutation pathway should be consistent)
- A single code path for both online and offline state changes

**New variants**:
```rust
CreateChannel { channel_id: String, name: String, channel_type: String, hardware_ref: HardwareReference },
RenameChannel { channel_id: String, new_name: String },
DeleteChannel { channel_id: String },
```

## R9: Channel ID format and generation

**Decision**: UUID v4, generated on the frontend at creation time.

**Rationale**: Per spec, channel IDs use UUID v4 — globally unique without coordination, consistent with existing ID patterns (e.g., `connectionId` in layout manifest, placeholder node UUIDs). Frontend generation avoids an IPC round-trip for ID allocation. The UUID is passed to the backend via the `CreateChannel` delta.
