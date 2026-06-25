# Quickstart: Information Channels — Auto-Create & Inventory

**Feature**: 015-information-channels  
**Date**: 2026-06-24

## What are Information Channels?

An **information channel** is a named, typed representation of a single piece of layout-meaningful information — for example, "Block 7 Occupancy." Channels are layout-level abstractions: they describe *what* your layout knows, independent of the protocol details that carry the information.

In this first release, channels are automatically created from BOD-family daughter board selections and displayed in a new **Railroad tab** as an inventory. You can rename them to match your physical layout. No live state or behavior is involved yet — this is the foundation for future features.

## Creating Channels

Channels are created automatically when you select a BOD-family daughter board for a TowerLCC connector:

1. Open a layout and navigate to the **Config** tab.
2. Select a TowerLCC node.
3. In the connector configuration area, choose a BOD-family daughter board:
   - **BOD4** → creates 4 block-occupancy channels
   - **BOD4-CP** → creates 4 block-occupancy channels
   - **BOD-8-SM** → creates 8 block-occupancy channels
4. Channels are created instantly with default names like:
   - *"West Yard — Connector A — Input 1"*
   - *"West Yard — Connector A — Input 2"*
   - etc.

## Viewing Channels

Switch to the **Railroad** tab (rightmost tab in the tab bar) to see all channels across your layout:

- Channels are grouped by type (e.g., "Block Occupancy").
- Each group shows a count (e.g., "Block Occupancy (8)").
- Each channel displays its name, type, and the hardware reference (which node, connector, and input backs it).

If no channels exist yet, the Railroad tab shows guidance on how to create them.

## Renaming Channels

Default names are functional but generic. Rename channels to match your physical layout:

1. In the **Railroad** tab, click a channel's name.
2. Edit the name inline (e.g., change *"West Yard — Connector A — Input 3"* to *"Mainline Block 7 — Occupancy"*).
3. Press Enter to confirm.

Names persist across sessions — close and reopen the layout and your names are retained.

## Navigating to Hardware

Each channel shows its backing hardware reference. Click the reference to navigate directly to that node and connector in the Config tab.

## Changing Daughter Boards

If you change a connector's daughter board away from a BOD-family board:

1. The system warns that existing channels for that connector will be removed.
2. **Confirm** to proceed — channels are removed from the inventory.
3. **Cancel** to keep the existing daughter board and channels unchanged.

If you re-select a BOD board on the same connector later, fresh channels are created with new default names. Previous names are not restored.

## Persistence

Channel data is stored in `channels.yaml` in your layout folder alongside other layout files. It is saved when you save the layout (Ctrl+S) and loaded when you open the layout.

## Limitations (This Release)

- **No live state**: The Railroad tab shows channel inventory only — not whether blocks are occupied or clear.
- **No behavior**: Channels don't trigger actions or connect to other channels.
- **No facilities or templates**: The broader UX vision (facilities, wiring, logic) is future work.
- **BOD-family only**: Only BOD4, BOD4-CP, and BOD-8-SM daughter boards auto-create channels. Other daughter board types will gain channel support in future releases.
- **No retroactive creation**: Opening a layout saved before this feature shows an empty Railroad tab. Channels are not inferred from pre-existing daughter board selections.
