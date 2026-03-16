# Quickstart: Editable Bowties

**Feature Branch**: `009-editable-bowties`

## What This Feature Does

Editable Bowties lets you visually create and manage event connections between LCC nodes. Instead of manually copying event IDs between node configurations, you select a producer and consumer from a visual picker, and the app wires them together automatically. Connection names, tags, and organizational metadata are saved to a layout file you control.

## Prerequisites

- Bowties app is running and connected to an LCC network (TCP)
- At least one node has been discovered and its configuration read
- Features 004 (discovery), 006 (bowties view), 007 (editable config), and 008 (guided config) are functional

## Workflow 1: Create a Connection from the Bowties Tab

1. Click the **Bowties** tab
2. Click **+ New Connection**
3. In the dialog:
   - **Left panel (Producer)**: Browse the node tree, expand segments/groups, select an element with a free event slot
   - **Right panel (Consumer)**: Browse and select a consumer element
   - **Name** (optional): Type a descriptive name like "Yard Entry Signal"
4. Click **Create Connection**
5. A new bowtie card appears showing the producer on the left, consumer on the right
6. The card shows an unsaved indicator (dot) — click **Save** to write to nodes and persist

## Workflow 2: Create a Connection from the Config Tab

1. In the **Configuration** tab, navigate to an event slot (producer or consumer)
2. Right-click or use the context action **"Create Connection from Here"**
3. The New Connection dialog opens with one side pre-filled
4. Select the other side, name the connection, and create it
5. Save to write changes to nodes

## Workflow 3: Build Connections Incrementally (Intent-First)

1. Click **+ New Connection** without selecting any elements
2. Enter a name like "Main Turnout Control" and click Create
3. An empty bowtie card appears in a "planning" state
4. Later, click **+ Add producer** on the card and select an element
5. Click **+ Add consumer** and select an element
6. Save when ready

## Workflow 4: Work with Layout Files

1. When you create your first bowtie, you'll be prompted to save a layout file
2. Use **File → Save As** to choose a location and filename (e.g., `my-layout.bowties.yaml`)
3. On next app launch, the app offers to reopen your last layout file
4. Use **File → Open** to load a different layout
5. Layout files are YAML — you can view and edit them in any text editor

## Workflow 5: Handle Ambiguous Event Roles

1. When a node has no profile, its event slots appear as "ambiguous" (with a ? badge)
2. When you select an ambiguous element in the picker, the app asks: *"Is this a producer or consumer?"*
3. Your classification is saved in the layout file — you won't be asked again
4. To change a classification later, click the role badge on the bowtie card

## Key Concepts

- **Bowtie**: A named connection linking producers to consumers via a shared event ID
- **Layout file**: A YAML file storing bowtie names, tags, and role classifications
- **Unsaved indicator**: A dot on the bowtie card or toolbar means changes are pending
- **Save**: Writes event IDs to physical nodes AND saves metadata to the layout file
- **Discard**: Reverts ALL pending changes (both config and bowtie)

## Save/Discard Behavior

- Config edits (from Configuration tab) and bowtie edits share one Save/Discard lifecycle
- Save writes to nodes first, then saves the layout YAML
- If the YAML save fails, node writes are kept; you'll be prompted to retry or Save As
- Discard reverts everything — node slot values return to their original state, metadata changes are undone

## YAML Layout File Format

```yaml
schemaVersion: "1.0"
bowties:
  "05.01.01.01.FF.00.00.01":
    name: "Yard Entry Signal"
    tags: ["yard", "signals"]
  "05.01.01.01.FF.00.00.02":
    name: "Main Turnout Control"
    tags: ["mainline"]
roleClassifications:
  "05.02.01.02.03.00:Port I/O/Line #1/Event Produced":
    role: "Producer"
```

- Event IDs are in dotted hex format (e.g., `05.01.01.01.FF.00.00.01`)
- Role classifications are keyed by `{nodeId}:{elementPath}`
- You can edit this file in any text editor; changes take effect when reopened in the app

## Development: Running Tests

```bash
# Frontend tests
cd app
npm test

# Backend tests
cd app/src-tauri
cargo test

# All Rust tests including lcc-rs
cargo test --workspace
```

## Development: Key Files

| Area | Path | Purpose |
|------|------|---------|
| Layout persistence | `app/src-tauri/src/layout/` | YAML load/save, types |
| Bowtie commands | `app/src-tauri/src/commands/bowties.rs` | Extended catalog building |
| Bowtie API wrappers | `app/src/lib/api/bowties.ts` | Tauri IPC calls |
| Metadata store | `app/src/lib/stores/bowtieMetadata.svelte.ts` | Unsaved metadata tracking |
| Layout store | `app/src/lib/stores/layout.svelte.ts` | File path/state |
| New Connection dialog | `app/src/lib/components/Bowtie/NewConnectionDialog.svelte` | Dual picker UI |
| Element picker | `app/src/lib/components/Bowtie/ElementPicker.svelte` | Tree browser with search |
| Bowtie card | `app/src/lib/components/Bowtie/BowtieCard.svelte` | Extended: edit actions |
| Config leaf row | `app/src/lib/components/ElementCardDeck/TreeLeafRow.svelte` | Extended: context action |
