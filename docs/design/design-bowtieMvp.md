# Design: Bowties MVP — Connection-First LCC Configuration

**Executive Summary:** A desktop tool that separates two distinct workflows behind two tabs — **Configuration** (inspect and edit a node's physical elements) and **Bowties** (create, view, and manage producer-consumer connections). Event IDs are entirely hidden from users; the app handles slot assignment automatically. The Bowties tab is the primary creative workspace; it starts empty and grows as the user builds their layout logic intentionally. This is an MVP: functional, clean, and dramatically simpler than JMRI, designed to be extended later.

---

## Core Abstraction

Event IDs are fully abstracted. Here is what the user knows vs. what the app handles:

| User sees | App handles invisibly |
|---|---|
| "Button 1 on East Panel" | Which producer event slot holds the ID |
| "Signal 4 on Tower Ctrl" | Which consumer event slot to write to |
| A named connection ("Yard to Signal") | What Event ID value is used |
| "+ Add producer" / "+ Add consumer" | Finding the first free slot |
| Edit / Remove on a card | Reading current slot value, writing new or clearing |

The only time an Event ID surfaces is as a small, secondary detail in the element picker — not as something the user needs to understand or act on.

---

## Connection Flow — Step by Step

### Adding a first consumer to a producer with no connections yet

1. All config values have been read — every node's event slots are known. The app identifies which event IDs are "live" (non-default, written by the user) vs. "default" (factory values, effectively free).
2. The Bowties tab starts empty.
3. The user clicks **+ New Connection**.
4. The New Connection dialog opens with two panels: Producer (left) and Consumer (right).
5. The user selects an element on each side and optionally names the connection.
6. The app inspects the consumer element's event slots, finds the first free one, and writes the producer's Event ID to that slot on the physical node.
7. The dialog closes and a bowtie card appears on the canvas. Two writes have happened silently.

### Adding a second producer to an existing bowtie

1. The bowtie already has one producer (Button 1) and one consumer (Signal 4), sharing a specific Event ID.
2. The user clicks **+ Add producer** on the bowtie card.
3. A picker opens, filtered to elements that have at least one free producer event slot.
4. The user selects "West Panel → Button 2 Event On."
5. The app finds Button 2's first free event slot and writes the bowtie's **existing** Event ID to it.
6. The bowtie now shows two producers and one consumer.

**Key principle:** The Event ID "belongs to" the bowtie. When adding to an existing bowtie, all newcomers adopt the existing ID. When creating a brand new connection, the producer's current Event ID is used as the bowtie's identity.

---

## Experience Narratives

### Narrative 1: Building a first connection
The Bowties tab is empty: a large centered message reads "No connections yet — click **+ New Connection** to link a producer to a consumer." Emma clicks **+ New Connection**. The New Connection dialog opens. She expands "East Panel" on the producer side and sees "Yard Button (Line 3)" — she clicks it. A preview card appears at the bottom: node name, element name, CDI breadcrumb, key config values. She selects "Signal 4 (Line 6)" on the consumer side. She types "Yard to Signal 4" as the connection name. She clicks **Create**. The dialog closes and a bowtie card appears on the canvas. The Event ID is now set on both nodes in their first available slots, invisibly.

### Narrative 2: Editing a connection
Emma double-clicks the producer card on a bowtie. The Edit Element dialog opens showing the same config fields as the Configuration tab. She changes the description and clicks Save. The change is written to the node and the bowtie card updates its display name.

### Narrative 3: Removing a producer
A button is no longer needed for a bowtie. Emma right-clicks the producer card and selects "Remove from connection." Because this is the only producer in the connection, the confirmation reads "Removing the only producer will delete this connection entirely. Continue?" She confirms. The slot on the node is written back to a default/empty Event ID, and the bowtie card is removed from the canvas — a single unconnected consumer has no meaning in the Bowties view and is not shown. If there had been a second producer, the bowtie would remain with the other producer still connected.

### Narrative 4: Tracing from config to bowtie
In the Configuration tab, Lin is looking at "Line 5" on the Tower Controller. She sees the event slot shows "Used in: Yard to Signal 4" as a link. She clicks it. The view switches to the Bowties tab, scrolls to, and highlights that bowtie card.

---

## Bowties Tab — Detailed Design

### Initial empty state

```
┌──────────────────────────────────────────────────────────┐
│  [+ New Connection]   🔍 Filter...   [Group by: Tag ▾]  │
├──────────────────────────────────────────────────────────┤
│                                                          │
│              (illustration: two unconnected nodes)       │
│         "No connections yet."                            │
│         "Click + New Connection to get started."        │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### Populated canvas

Each connection is a bowtie card. Cards are stacked vertically in a scrollable list (not a freeform canvas — a list is more predictable and scannable for an MVP):

```
┌─────────────────────────────────────────────────────────┐
│  Yard to Signal 4                              [•••]    │
│  ─────────────────────────────────────────────────────  │
│  PRODUCERS                      CONSUMERS               │
│  ┌──────────────────────┐       ┌──────────────────────┐│
│  │ 🔘 Yard Button       │       │ 💡 Signal 4           ││
│  │  East Panel          │       │  Tower Controller    ││
│  │  Line 3 · Input      │  ══●══│  Line 6 · Output     ││
│  └──────────────────────┘       └──────────────────────┘│
│  [+ Add producer]               [+ Add consumer]        │
└─────────────────────────────────────────────────────────┘
```

- **[•••]** menu on each bowtie card: Rename, Add tag, Delete connection
- Clicking a producer or consumer card: selects it (blue ring) + shows **[Edit] [Remove]** inline action buttons
- **[+ Add producer]** and **[+ Add consumer]**: opens the element picker scoped to the appropriate role, pre-filtered to elements with at least one free slot; if the candidate element has no free slots the add is blocked
- A bowtie is shown as **Incomplete** (yellow banner) only when it has 2 or more elements on one side but zero on the other — this state can only be reached by removing the last element from one side of an existing multi-element connection. A single unconnected producer or consumer is not shown in the Bowties view.

### Grouping and filtering

Groups are user-defined tags (free-form text). Tags are assigned per connection from the [•••] menu. Initially all connections appear in a flat list. When grouped:

```
  📁 Staging Yard  (12 connections)  [▼]
     ┌──────────────────────────────────────────┐
     │  Route Yard 11 to Main                   │
     │  [Yard Button] ──●── [Signal 4, Motor 2] │
     └──────────────────────────────────────────┘
     ... 11 more

  📁 Main Line  (8 connections)  [▶ collapsed]
  📁 Untagged  (34 connections)  [▶ collapsed]
```

- **Tags** (user-defined, e.g. "Staging Yard," "Main Line") are the primary grouping unit
- **Filter bar** searches by connection name, node name, or element name
- Large layouts (100s of bowties) are managed primarily through tagging and filtering

---

## New Connection Dialog

The most important UX surface in the app. Two-panel layout:

```
┌──────────────── New Connection ────────────────────────── ┐
│  Connection name: [_______________________________]       │
│                                                           │
│  PRODUCER (causes the action)  CONSUMER (what happens)   │
│  ──────────────────────────────────────────────────────  │
│  🔍 Search...          │  🔍 Search...                   │
│                        │                                 │
│  ▾ East Panel          │  ▾ Tower Controller             │
│    ▸ Identification    │    ▸ Identification             │
│    ▸ Port I/O          │    ▾ Port I/O                   │
│      Line 1            │      Line 1                     │
│      ● Yard Button  ✓  │      Line 2                     │
│      Line 4            │      ● Signal 4  ← selected     │
│    ▸ Settings          │      Line 7                     │
│  ▾ West Panel          │    ▸ Settings                   │
│    ...                 │  ▾ East Panel                   │
│                        │    ...                          │
│  ──────────────────────────────────────────────────────  │
│  SELECTED PRODUCER:            SELECTED CONSUMER:         │
│  ┌──────────────────────┐      ┌──────────────────────┐  │
│  │ Yard Button          │      │ Signal 4             │  │
│  │  East Panel          │      │  Tower Controller    │  │
│  │  Port I/O › Line 3   │      │  Port I/O › Line 6   │  │
│  │  Input · Active Low  │      │  Output · Active Hi  │  │
│  │  Debounce: 3         │      │  6 event slots avail │  │
│  └──────────────────────┘      └──────────────────────┘  │
│                                                           │
│                     [Cancel]   [Create Connection]        │
└───────────────────────────────────────────────────────────┘
```

### Key design decisions

- Both sides use the same tree: node → segment → element. This mirrors the Configuration tab's sidebar structure.
- Elements with **no free slots** are shown grayed out with a "No free slots" label — selectable to view info, but Create remains disabled.
- The **selection preview card** shows the element's CDI breadcrumb path, key config values (type, function, current values), and available slot count — context without leaving the dialog.
- Connection name is optional. If left blank, the connection is saved with no name and displayed untitled until the user names it.
- **[Create Connection]** is disabled until both sides have a selection.
- The element picker is also used for **+ Add producer** and **+ Add consumer** on existing bowties, scoped to one side with the existing side shown as a read-only summary.

---

## Configuration Tab — Option B

Sidebar (node → segment) + element card deck in the main area:

```
┌──────────────────────────────────────────────────────────┐
│  Configuration tab                                       │
├───────────────────┬──────────────────────────────────────┤
│ ▾ East Panel      │  Port I/O                            │
│   Identification  │  ──────────────────────────────────  │
│   User Info       │  ┌─ Yard Button (Line 3) ────── [▼] ┐│
│ ▶ Port I/O ◀      │  │  Name: [Yard Button  ] [R] [W]   ││
│   Settings        │  │  Input Function: [Active Lo ▾]   ││
│                   │  │  ▾ Advanced Settings              ││
│ ▾ Tower Ctrl      │  │    Output Function: [Pulse Hi ▾] ││
│   Identification  │  │    ▾ Delay                        ││
│   User Info       │  │      Interval 1 · Interval 2     ││
│   Port I/O        │  │      Delay Time: [0  ] [R] [W]   ││
│   Settings        │  │      Units: [Milliseconds ▾]      ││
│                   │  │  ▾ Event Slots                    ││
│                   │  │    Slot 1: 05.02.01.02.03.00      ││
│                   │  │    ↳ Used in: Yard to Signal 4    ││
│                   │  │    Slots 2–6: (free)              ││
│                   │  └──────────────────────────────────┘│
│                   │  ┌─ Line 4 (unnamed) ─────────── [▶]┐│
│                   │  └──────────────────────────────────┘│
│                   │  ┌─ Line 5 (unnamed) ─────────── [▶]┐│
└───────────────────┴──────────────────────────────────────┘
```

### Key decisions

- Sidebar shows: node names (collapsible), segment names under each node. Only segments are navigation items — no deeper drilling in the sidebar.
- Named elements show their user-given name in the accordion header (e.g. "Yard Button (Line 3)"). Unnamed show "Line 4 (unnamed)."
- Event slots in the config view show the raw Event ID value as a secondary detail, plus "Used in: *connection name*" as a link to the Bowties tab.
- CDI description text (the long "Amount of time to wait…" paragraphs) appears as collapsed gray helper text, revealed by a small "?" icon or subtle expander. Not hidden — just not prominent.
- **[R]** = Refresh from node, **[W]** = Write to node. Small, secondary actions.
- **Advanced Settings** is a disclosure group — hidden by default. Basic users never encounter debounce, output function, delay unless they choose to.

---

## Information Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Toolbar: [Discover] [Refresh]  [Traffic Monitor ↗]  ●  │
├────────────┬─────────────────────────────────────────────┤
│            │  [Configuration]        [Bowties]           │
│ (sidebar   ├─────────────────────────────────────────────┤
│  within    │                                             │
│  config    │    Element card deck  (Config tab)          │
│  tab only) │    — or —                                   │
│            │    Bowtie card list   (Bowties tab)         │
│            │                                             │
└────────────┴─────────────────────────────────────────────┘
```

- Traffic Monitor remains a separate window launched from the toolbar — unchanged.
- The sidebar (node → segment navigation) is only present in the Configuration tab.
- The Bowties tab has its own toolbar row: [+ New Connection], filter, group-by.

---

## Interaction Patterns

- **Bowtie card [+ Add consumer/producer]** → opens element picker scoped to one side, with the existing side shown as a read-only summary
- **Click producer/consumer card in bowtie** → select (blue ring) + [Edit] [Remove] action bar appears inline
- **Double-click producer/consumer card** → opens Edit Element dialog (same fields as Configuration tab for that element)
- **Right-click producer/consumer card** → context menu: Edit, Remove from connection, View in Configuration tab
- **"Used in: connection name" link** in Config tab event slot → switches to Bowties tab, scrolls to and highlights that bowtie card
- **[+ New Connection] toolbar button** → opens New Connection dialog
- **Bowtie card [•••]** → Rename, Add/edit tag, Delete connection (with confirmation)
- **Delete connection** → clears Event ID slots on all participating nodes (writes default/empty ID back), removes bowtie card; all participating nodes must be online — if any node is offline the operation is blocked with an error and no writes are made. Deferred/queued writes (**Save All** pattern) are a candidate for a future iteration.

---

## States to Design

| State | Behavior |
|---|---|
| Bowties tab, empty | Centered illustration + "No connections yet" + prominent **+ New Connection** |
| Bowties tab, filtered to zero results | "No connections match your filter" + [Clear filter] |
| Incomplete bowtie (2+ on one side, 0 on other) | Yellow "Incomplete" banner; [+ Add producer] or [+ Add consumer] call-to-action prominent; reached only by removing the last element from one side of a multi-element connection |
| Write attempted, node offline | Operation blocked; error "Node X is offline — cannot write"; no partial writes made |
| Write in progress | Field shows subtle spinner; [W] disabled; other fields remain interactive |
| Write failed | Field background soft red; inline "Write failed — node timeout"; [Retry] link |
| Write confirmed | Field briefly flashes soft green; "Saved" fades out after 1.5s |
| Element in picker — no free slots | Grayed out, "No free slots" tooltip; Create button disabled with explanation |
| Element in picker — unnamed | Shows CDI path as label (e.g. "Port I/O › Line 4") with italic "(unnamed)" suffix |

---

## Data Persistence

- Connection names and tags are stored locally on disk (JSON file alongside app data).
- The file path is platform-appropriate (Tauri's `appDataDir`).
- Node event slot values remain the ground truth — local storage is display metadata only.
- Sharing / multi-machine sync is out of scope for this MVP.

---

## Resolved Decisions

1. **Incomplete bowtie visibility:** Only shown when 2 or more elements exist on one side with zero on the other. A single unconnected producer or consumer is never shown in the Bowties view — the Bowties tab is only about connections.
2. **Write when node is offline:** All participating nodes must be online for any write operation. If any node is offline, the operation is blocked entirely with an error; no partial writes are made. Deferred writes (Save All) are a future consideration.
3. **Bowtie name default:** No default name is generated. The connection name field starts blank; the user names it explicitly or leaves it untitled.
4. **Multi-producer, no free slot:** Adding a producer or consumer to an existing bowtie is blocked if the selected element has no free event slots. The picker grays out elements with no free slots and the add action cannot proceed.
5. **Segment naming:** CDI segment names ("Port I/O," "Identification") are displayed as-is. User renaming is out of scope for this MVP.
