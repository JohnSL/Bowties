# Quickstart: Configuration Tab — Sidebar and Element Card Deck

**Feature**: 005-config-sidebar-view  
**Target audience**: End users (model railroad hobbyists)  
**Date**: 2026-02-22  

---

## What Changed

The Configuration tab has been redesigned with a cleaner, more practical layout:

- **Before**: A side-scrolling drill-down (Miller Columns) — you clicked deeper and deeper to reach any field.
- **After**: Two panels — a sidebar on the left for navigation, and a card deck on the right that shows everything at once.

---

## Getting Started

### Open the Configuration Tab

Click **Configuration** in the navigation bar at the top of the screen.

**If no nodes are discovered yet**, the sidebar shows:

> *"No nodes discovered — use Discover Nodes to scan the network"*

Click **Discover Nodes** in the toolbar, wait a moment, then return to the Configuration tab.

---

### Navigate in 3 Clicks

The new layout is designed so you can reach any configuration field in 3 clicks or fewer:

```
Click 1 → Expand a node in the sidebar
Click 2 → Select a segment (e.g., "Port I/O")
Click 3 → Expand an element card to see its fields
```

#### Click 1 — Expand a Node

Click any node name in the left sidebar. The node expands to reveal its configuration sections (called **segments**).

> You can expand multiple nodes at the same time. The sidebar remembers which nodes you've opened as you switch between segments.

#### Click 2 — Select a Segment

Click a segment name under the expanded node (for example, **"Port I/O"** or **"Identification"**).

The main panel fills with cards — one card for each configurable item in that segment.

#### Click 3 — Open an Element Card

Click any card header to expand it and see its configuration fields. Naming works like this:

| Card Header | Meaning |
|-------------|---------|
| `Yard Button (Line 3)` | Named output: "Yard Button" is your label; "Line 3" is its CDI position |
| `Line 3 (unnamed)` | Output #3 — you haven't given it a custom name yet |
| `Port I/O` | A non-replicated group (only one of these) |

The values shown are the **last cached values** from your most recent "Read All Config Values" operation.

---

## Reading and Refreshing Values

### Refresh a Single Field

If a field looks stale or you want to confirm the current live value:

1. Expand the relevant card
2. Click **[R]** next to the field you want to refresh

The app re-reads that specific value from the node and updates the display immediately.

### Reading All Values

To populate the cache for a whole node, use **Read All Config Values** from the node toolbar (same as in the previous version).

---

## Event IDs

Event slots display their raw event ID in dotted-hex format:

```
01.02.03.04.05.06.07.08
```

If an event slot hasn't been assigned a value, it shows **"(free)"** instead of a raw default ID.

---

## Field Descriptions

Most fields have a built-in description from the node's CDI. These are hidden by default to keep the display clean.

To read the description for a field, click the **[?]** icon next to the field label. Click again to hide it.

---

## Context Menu (Right-Click on a Node)

Right-click any node name in the sidebar for these options:

| Option | What it does |
|--------|-------------|
| **View CDI XML** | Opens the full raw CDI XML for the node in a viewer |
| **Download CDI from Node** | Downloads a fresh copy of the CDI from the node (useful after firmware updates) |

---

## Edge Cases and What to Expect

| Situation | What you'll see |
|-----------|----------------|
| Node has an offline indicator | Node is unreachable; cached values still shown; live [R] reads will fail with an error |
| Card shows "(no configurable fields)" | This group contains no editable fields in this segment |
| Field shows no value / dash | Run "Read All Config Values" first to populate the cache |
| Two nodes share the same name | Both appear in the sidebar; manufacturer/model shown below the name to tell them apart |
| CDI not yet loaded for a node | Clicking the node shows a loading spinner; if CDI can't be loaded, an error replaces the segment list |
| Selected segment is cleared after Refresh Nodes | This is expected — a node refresh clears all sidebar state and resets to the initial view |

---

## Known Limitations (This Version)

- **Read-only**: You can view current configuration values but not change them yet. Field editing is planned for the next feature.
- **No event editing**: Event slot assignment from this tab is not available yet.
- **Sidebar width is fixed**: The sidebar cannot be resized in this version.
- **No cross-tab navigation**: Clicking an event ID does not yet navigate to the Bowties tab (planned for a future feature).
