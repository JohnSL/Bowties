# Research: Panel Facility

Evidence comes from the current code (read 2026-11-25) and Specs 018 and 020. Line numbers are as of that date.

## R1. Compile lamps to configuration edits, not bowties

- **Decision**: A pure panel compiler in `bowties-core` produces configuration edits for each lamp row: Lamp Selection (direct), Lamp On, Lamp Off and Lamp Description. `facilityOrchestrator` stages them with `configEditor.applyEdit(..., { facilityId })`, as for Automatic Block Signals.
- **Rationale**: ABS established this route (ADR-0012 2026-11-22 extension "compiled facility intent"; ADR-0001 2026-11-22: facilities reach hardware through Save). Panel needs owner detection, releasing outside pins and resetting rows on removal. That support already lives in `bowties-core/src/signaling/` (`outputs.rs`, `facts.rs`, `slots.rs`). Block Indicator's `compose_bowtie_ops` (`facility_bowties/mod.rs` L161) only fills consumer event leaves for one fixed pair of slots. It doesn't set Lamp Selection or Lamp Description, and it can't release pins.
- **Alternatives**: Compose one bowtie pair per entry with `facility_bowties`. Rejected: it would need per-entry slots, and it would still need a second mechanism for description, selection, release and reset.

## R2. Lamps are channels; blocks are references

- **Decision**: Each wired entry owns one user-owned channel. Its role is `lamp-indicator`, its style `single-led-direct-lamp`, and its binding `lampRow { nodeKey, rowOrdinal }`. These channel types already exist (`channels.rs` L18-65; profile style at `Signal-LCC.profile.yaml` L102-123). The channels are attached to the facility's `lamps` slot. Entries refer to blocks by the same block reference ABS uses, `SelectedBlock.block_id`, and blocks are never attached to a slot.
- **Rationale**: This matches ADR-0013's 2026-11-22 extension: owned outputs are persisted channels, and slot bindings are the only source of membership. Lamps then show on the Channels panel, and pins used by lamps are visible to other facilities' owner checks. Block Indicator lamps are already `single-led-direct-lamp` channels, which is what makes migration (R6) a pure rename.
- **Alternatives**: Store the pin inline on the entry. Rejected: it duplicates owner checks and hides the lamps from the Channels panel, the same reason ABS rejected it (Spec 020 F3).

## R3. Claimed pins are derived

- **Decision**: A panel's claimed pins are the `lampRow` bindings of the channels in its `lamps` slot. `claimed_slots_from_facilities` (`slots.rs` L261-312) gains lamp rows from panels, so ABS and other panels see them as owned. `overlay_releases` then refuses to release them.
- **Rationale**: The code stores no claimed slots ("derived, never stored", `facilities.rs` L86-102; YAML test L1054). ADR-0012's extension wording says "recorded claimed slots". The code is right and the ADR wording should be corrected at feature finish.
- **Alternatives**: Store a claimed list on the record. Rejected: it can drift from the channels.

## R4. Wiring facts and the pin interaction come from ABS

- **Decision**:
  - The backend command `panel_wiring_facts` returns the boards that have Direct Lamp Control. For each board it lists the pins with labels from `signal_lcc_output_label` (`outputs.rs` L160-165, "#9 H3-G"). Each pin has an owner:
    - free;
    - this panel's entry;
    - a Bowties facility, attributed with `attribute_output_facilities`;
    - outside Bowties, with its Lamp Description.
  - The editor store's pin rule copies `blockSignalsEditor.seatLamp` (L312-324):
    - a free pin seats the entry's lamp;
    - the entry's own pin clears it (FR-016);
    - another entry's pin moves to this entry;
    - an outside pin goes through Release, as in `WiringStep.svelte` L328-368.
- **Rationale**: The modeler gets the same behavior in both Wiring steps, and the owner logic stays in one place.
- **To confirm in `/design`**:
  - The exact CDI element name for Lamp Description. It wasn't confirmed in the bundled CDI.
  - Whether the shared pin-grid rendering is extracted into a common component or kept separate. Extract it only if both callers really share it.

## R5. "Lit when"

- **Decision**: The effective "lit when" is the entry's override, or the panel setting otherwise. For Occupied, Lamp On gets the block's occupied event and Lamp Off its clear event; Clear swaps them. Both are always set (FR-019).
- **Rationale**: This mirrors Block Indicator's mapping (occupied → lit, `behavior_templates.rs` L88-97), and the `single-led-direct-lamp` style mapping puts lit on consumer leaf 0 and unlit on leaf 1.

## R6. Migration at load

- **Decision**: `migrate_block_indicators(&mut FacilitiesDocument, &ChannelsDocument, &layout node names) -> MigrationSummary` is a pure function in core. `io.rs` calls it after load, next to `normalize_facility_channel_refs` (L319-324). It works as follows:
  - Group `block-indicator` facilities by the `nodeKey` of their lamp channel's binding, in facility order.
  - Make one panel per board, named after the board's display name.
  - Make one panel, "Indicators without a lamp", for facilities with a block but no lamp.
  - Drop facilities with no block.
  - Reuse each lamp channel, renamed to the former facility name, and keep it in the `lamps` slot.
  - Set `litWhen` to Occupied; Block Indicator only did occupied → lit.
  - Reuse the facility ID of the first indicator in each group as the panel's ID, and generate IDs for the rest.
- **The modeler is told and the layout is marked changed**: the open result carries the summary. The route shows the notice and marks the layout changed through the facade. The existing repair only cleans up on the next save and doesn't mark the layout dirty, so this explicit step is needed for FR-032. Verify the facade call in build.
- **No board writes**: the lamps already hold the block's event IDs. The Lamp Description on the board stays as it is until the panel is next edited (FR-031).
- **Alternatives**:
  - Convert in the frontend after load. Rejected: the backend owns layout file data (ADR-0002, 0005).
  - Keep Block Indicator as a legacy type. Rejected by clarification.

## R7. Layout version

- **Decision**: Set `LAYOUT_SCHEMA_VERSION` to 6 and the facilities document `SCHEMA_VERSION` to "3.0". `validate()` keeps accepting 4 up to the current version.
- **Rationale**: ADR-0005's 2026-11-22 extension says to bump when older builds would mishandle data. serde ignores unknown fields, so 0.4.x would load a panel as a facility with an unknown template and drop its record on save. 0.4.x refuses manifest 6.

## R8. Status and live state are derived in the frontend facade

- **Decision**:
  - `effectiveLayoutStore.facilityStatus` handles panels:
    - Empty with no entries;
    - Incomplete if any entry has no lamp, its block is no longer detected, or its lamp's board is missing;
    - Wired otherwise.
  - Lamp state comes from `deriveChannelState` (`utils/channelState.ts` L44-66) with the block's event IDs, mapped by the effective "lit when". Neither event seen gives unknown.
- **Rationale**: ADR-0004 says status is derived, and it is already derived today (`effectiveLayoutStore` L304-314). The observation source is the existing `eventStateStore`.

## R9. Retire Block Indicator

- **Decision**: Replace `BLOCK_INDICATOR` in `REGISTRY` with `PANEL`. The string `"block-indicator"` stays only as the migration's input. Remove Block Indicator composition and the slot-picker paths once they have no callers. Rewrite the tests that depend on them.
- **Rationale**: The clarification rules out a legacy type. YAGNI.

## R10. Channel usage includes records

- **Decision**: `channelUsageMap` adds:
  - "{panel} / block" for each panel entry's block occupancy channel;
  - "{ABS facility} / block" for each ABS block, if it isn't shown already;
  - lamp channels through the existing slot path.
- **Rationale**: FR-028. The usage map is slot-based today, so blocks that are only referenced from records are invisible. Check during build whether ABS blocks already appear.

## Glossary

The glossary says a channel binds to at most one facility slot, but the code doesn't enforce that (`channels.rs` has no back-reference). It is false for blocks referenced by panels and ABS. Update it at feature finish, and add a **Panel** term.
