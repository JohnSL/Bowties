# Data Model: Panel Facility

**Owner**: `bowties-core` owns the serialized values and pure transitions; the backend supplies board facts; the frontend facade derives status and live state.

## Facility (extended)

`Facility { facility_id, template_id, name, slot_bindings, block_signals?, panel? }`

| Field | Panel use |
|---|---|
| `template_id` | `"panel"` |
| `name` | Nonempty panel name |
| `slot_bindings["lamps"]` | Lamp channel IDs, the only source of lamp membership (ADR-0013) |
| `panel` | `PanelRecord`, skipped when absent; replaced wholesale by `SetPanelRecord` |

Template `PANEL`: `template_id "panel"`, display "Panel", one slot `lamps` (Consumer, role `lamp-indicator`, min 0, no max). No event mapping. It composes no bowties, like Automatic Block Signals.

## PanelRecord

| Field | Rule |
|---|---|
| `litWhen` | `occupied` (default) or `clear` |
| `entries` | Ordered `PanelEntry[]`; the order is the panel order (FR-010) |

### PanelEntry

| Field | Rule |
|---|---|
| `entryId` | Stable within the panel; survives reorder and re-pointing |
| `blockId` | Block reference, the same kind ABS uses (`SelectedBlock.block_id`); unique within the panel (FR-009) |
| `lampChannelId?` | A channel in this facility's `lamps` slot; at most one entry per channel |
| `litWhen?` | Override; absent means the panel setting |

Room for later row types: `entries` can gain a `kind` field defaulting to `block` without a migration.

## Lamp (existing channel type)

`InformationChannel { id, name, role: lamp-indicator, style: single-led-direct-lamp, ownership: user-owned, binding: lampRow { nodeKey, rowOrdinal } }`

- Its name defaults to the block name and becomes the Lamp Description (FR-020).
- Its identity is the Direct Lamp row (decision D1, ADR-0013 2026-11-25). The pin it drives is that row's effective Lamp Selection, read from configuration; no pin is stored.
- It is created on Done when an entry gains a lamp, rebound only when the lamp moves to another board (a pin change on the same board changes Lamp Selection only), and deleted when the entry is removed, cleared or the panel is removed.

## Claimed pin (derived)

`ClaimedSlot { node_key, kind: DirectLamp, index: row }` for every lamp channel in a panel's `lamps` slot, produced by the one claims function over facilities and channels (decision D2). The claimed pin is that row's effective Lamp Selection. Never stored. Edit and Remove reset only these rows.

## Derived status (FR-029)

```text
entries empty                                   → Empty
any entry: no lamp
        | block no longer detected
        | lamp's board not in layout            → Incomplete (count per reason)
otherwise                                       → Wired
```

Entry state for the card: `needsLamp`, `blockLost`, `lampBoardMissing`, or `wired`, with live lamp `lit | unlit | unknown`.

## Compiled effect per wired entry

For the lamp's row `Direct Lamp Control/Lamp[n]` on its node (kept if the lamp has one; otherwise the compiler allocates a free row, preferring n = pin):

| Leaf | Value |
|---|---|
| Lamp Selection | the chosen pin (1–16, "#k Hx-y"); 0 means "Used by Mast" / unused |
| Lamp On | occupied event if effective litWhen = occupied, else clear event |
| Lamp Off | the other event |
| Lamp Description | lamp channel name |

Rows the panel stops using are reset to the factory values the removal recipe defines (as ABS does, Spec 020 F4). Rows released from outside Bowties are part of the same staged batch (FR-024).

## Editor state (frontend, transient)

`{ facilityId?, name, step: blocks | wiring, litWhen, entries: [{ entryId, blockId, lamp?: { nodeKey, output, currentRow? }, litWhen? }], selectedEntryId?, boardChoice?, releases: [{ nodeKey, owner }], message? }` (releases use the existing `ReleaseRequest` shape; S4)

Pin click on the selected entry:

| Pin owner | Result |
|---|---|
| free | seat |
| this entry | clear (FR-016) |
| another entry on this panel | not choosable; shows that block; clear it there first (user decision 2026-11-26, same rule as ABS signals) |
| Bowties elsewhere | not choosable |
| outside Bowties | Release card; after release, seat and record the release |

Cancel discards the whole state (FR-003).

## Migration mapping (FR-030, FR-031)

| Block Indicator | Panel |
|---|---|
| facilities whose lamp channel binds to node N | one panel named after N's display name, entries in facility order |
| facility with a block, no lamp | entry in the single panel "Indicators without a lamp" |
| facility with no block | dropped |
| `slot_bindings["input"]` block channel | entry `blockId` |
| `slot_bindings["output"]` lamp channel | reused as entry lamp, renamed to the former facility name, moved to `lamps` |
| occupied → lit | `litWhen: occupied`, no overrides |

The result is idempotent: a layout without Block Indicators is unchanged, and there are no configuration edits.

## Versions

Manifest `LAYOUT_SCHEMA_VERSION` 6; facilities `SCHEMA_VERSION` "3.0"; channels unchanged at "2.0". Versions 4 and 5 open and migrate; newer ones are refused with the existing "newer Bowties" message.
