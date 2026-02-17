# Bowties - Product Vision

*This document represents the North Star for Bowties. It describes the aspirational goals and user experience we're building toward, not necessarily what's currently implemented. See [docs/technical/architecture.md](../technical/architecture.md) for current implementation status.*

## Overview

Bowties is a desktop application built with Tauri and Rust that makes LCC (OpenLCB) layout control accessible to hobbyists by transforming complex producer/consumer events into simple visual workflows. Users can understand their existing layout configuration at a glance and configure connections through intuitive drag-and-drop interactions — all without needing to understand Event IDs, MTIs, or CDI XML internals.

**Target Users:** Model railroad hobbyists with minimal technical background who need simple guided workflows.

**Core Value Proposition:** 
- Understand what's connected (bowtie diagrams showing "when this → do that")
- Configure elements before physical wiring
- Create event links visually by dragging elements onto bowties
- Diagnose issues with real-time event monitoring
- Single-element configuration (no scrolling through large node dialogs)

## Terminology

**Physical Connections** = Hardware wiring (button wired to Line 3 screw terminal)

**Event Links** = Logical relationships (when Button A pressed → Light B turns on)

**Elements** = CDI-defined configuration points (Line 1, Output 5, Timer 1, Event 1-6 slots)

**Devices** = User-configured physical equipment with friendly names (Front Door Button, Tower Red LED)

**Bowties** = Visual representation of event relationships showing producers on left, consumers on right, connected by shared Event ID

**Nodes** = Physical or virtual devices on the OpenLCB network

**Segments** = Major sections in CDI structure (Port I/O, Conditionals, Track Receiver)

**Groups** = Collections within segments (Lines 1-16, Timers 1-4)

**CDI (Configuration Description Information)** = XML document describing node's configuration structure (fixed per firmware version)

**Configuration Values** = Runtime data (Event IDs, names, parameters) stored in node memory

## Three-View Architecture

The application is organized around three primary views, each serving a distinct purpose in the configuration workflow:

### View 1: Configuration

**Purpose:** Setting up individual elements, configuring parameters

**UI Pattern:** Miller Columns (Mac Finder style)

**Focus:** Deep drilling into node structure before creating event links

**Layout:**

```
┌──────────┬──────────┬──────────┬──────────┬──────────────────┐
│ NODES    │ SEGMENTS │ GROUPS   │ ELEMENTS │ CONFIGURATION    │
│          │          │          │          │                  │
│ Tower    │ Port I/O │ Line     │ Line 3   │ Function: Button │
│ Ctrl ▶   │   ▶      │   ▶      │   ▶      │ Active: Low      │
│          │ Condition│ Line 1   │          │ Debounce: 50ms   │
│ East     │ Track RX │ Line 2   │          │ ───────────────  │
│ Panel    │ Track TX │ Line 3   │          │ Events           │
│          │          │ ...      │          │ Event On:        │
│          │          │ Line 16  │          │ 05.02.01.02...   │
│          │          │          │          │ [Copy]           │
│          │          │          │          │                  │
│          │          │          │          │ Event Off:       │
│          │          │          │          │ 05.02.01.02...   │
│          │          │          │          │ [Copy]           │
│          │          │          │          │                  │
│          │          │          │          │ User Name:       │
│          │          │          │          │ "Front Door"     │
│          │          │          │          │                  │
│          │          │          │          │ [Revert] [Apply] │
└──────────┴──────────┴──────────┴──────────┴──────────────────┘
           ◄────────── Horizontal scroll ──────────►
```

**Column 1: Nodes (200px)**
- Hierarchical list of all discovered nodes
- Badge shows configured/total elements: "East Panel 3/16"
- Auto-refreshes when nodes added/removed
- Selection populates Column 2

**Column 2: Segments (180px)**
- Shows CDI segments for selected node
- Examples: Port I/O, Conditionals, Track Receiver, Track Transmitter
- Retrieved from cached/fetched CDI XML
- Selection populates Column 3

**Column 3: Groups (180px)**
- Shows groups within selected segment
- Examples: Lines (1-16), Timers (1-4), Outputs (1-8)
- May show replicated groups
- Selection populates Column 4

**Column 4: Elements (180px)**
- Individual configuration elements
- Status indicators:
  - ✓ Green: Fully configured
  - ⚠️ Yellow: Configured but has issues/conflicts
  - ○ Gray: Unconfigured/disabled
  - 🔧 Wrench: Unsaved changes
- For consumers with multiple event slots, shows sub-items:
  - Line 5
    - Event 1 (configured)
    - Event 2 (empty)
    - Event 3 (configured)
- Selection populates Column 5

**Column 5: Configuration Panel (flexible, 300-400px)**
- Shows editable configuration for selected element
- Fields vary by element type:
  - Function/device type
  - Type-specific parameters (debounce, polarity, delays)
  - Event ID fields with [Copy] [Paste] buttons
  - User-editable name/description
  - Location tags
- Modified fields marked with indicator (blue dot, asterisk)
- Validation messages inline
- [Revert] discards changes, [Apply] writes to node
- For consumers: List of Event 1-6 slots
  - Status indicator per slot (● configured, ○ empty)
  - Click slot to edit in-place

**Benefits of Miller Columns:**
- Shows context (where you are in 4-5 level hierarchy)
- Shows siblings at each level (all 16 lines visible)
- Efficient navigation (click to drill, backtrack visible)
- Better use of horizontal space
- Familiar pattern (Mac Finder, mobile breadcrumbs)

### View 2: Event Bowties

**Purpose:** Visualize and create "when this → do that" event relationships

**UI Pattern:** Canvas with bowtie diagrams

**Focus:** Visual representation of event logic

**Layout:**

```
┌─────────────────────────────────────────────────────────────┐
│ Event Bowties                        [Filter ▼] [+ Manual]  │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Connected Bowties (3)                                       │
│                                                              │
│  ┌──────────────┐                    ┌──────────────┐       │
│  │ East Panel   │                    │ Tower Ctrl   │       │
│  │ Line 3       │──── When pressed ──▶│ Line 5       │       │
│  │ Front Door   │     05.02.01...    │ Red Light    │       │
│  └──────────────┘                    │ Event 3      │       │
│                                      └──────────────┘       │
│                                                              │
│  23 unconnected elements  [Show ▼]                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Default State:** Connected bowties only
**Expandable:** Unconnected elements tree (grouped by Node → Segment)

**Bowtie Diagram Elements:**

Each bowtie shows:
- **Left side:** Producer boxes (green border)
  - Node name, Element path, User-friendly name
- **Center:** Connection with Event ID and label ("When pressed", "When active")
- **Right side:** Consumer boxes (red border)
  - Node name, Element path, User-friendly name

**Interaction Patterns:**

1. **Drag to Create Link:** Drag element from unconnected tree onto bowtie
2. **Hover:** Tooltip with summary
3. **Click:** Details panel or jump to configuration
4. **Filter:** Connected/Unconnected/All, by node, by segment

### View 3: Event Monitor

**Purpose:** Real-time diagnostics, seeing live events, troubleshooting

**UI Pattern:** Scrolling log with expandable event details

**Focus:** Testing physical connections and understanding event flow

**Event Display:**

```
10:23:45  🟢 Event: 05.02.01.02.00.00.00.03    [Copy] [▼]
          Possible producers (1):
          • East Panel / Line 3 / Event On / Front Door
          Consumed by (2):
          • Tower Ctrl / Line 5 / Event 3 → turned ON
          • Hall Light / Event 1 → turned ON
```

**Features:**
- Live event stream with producer/consumer correlation
- Pause/resume, clear, export
- Filter by node, element, or event type
- Click to jump to configuration
- Event deduplication for rapid repeats

**Use Cases:**
1. Test physical wiring
2. Diagnose misconfiguration
3. Identify event sources
4. Verify linkage

### View Switching

- Tab bar at top: [Configuration] [Event Bowties] [Monitor]
- Deep linking between views:
  - Click element in Bowtie → Switches to Config, pre-drilled to that element
  - Click "View Bowties" in Config → Switches to Bowties, highlights element's connections
- Keyboard shortcuts: Ctrl+1, Ctrl+2, Ctrl+3

## Design Principles

### UX-First Approach

User experience is the PRIMARY metric. Every feature must make working with LCC nodes easier than command-line tools or low-level protocol libraries.

- Interactive UI preferred over complex command-line arguments
- Clear, human-readable output (not just raw protocol dumps)
- Error messages must be actionable and explain what went wrong
- Common workflows should require minimal steps
- Help text and documentation must be beginner-friendly

### Event Management Excellence

Event discovery, inspection, and modification are core competencies.

- List all events produced/consumed by a node
- Modify event configurations safely
- Event IDs displayed in human-readable format (dotted hex)
- Event operations include validation and confirmation
- Changes reversible or provide "dry-run" modes

## Resolved Design Decisions

✅ **CDI Caching:** Per-node-type with on-disk cache  
✅ **Configuration Values:** Fetch Event IDs at startup, others on-demand  
✅ **Navigation Pattern:** Miller Columns (not tree view)  
✅ **Consumer Multi-Event UI:** List with specific slot selection  
✅ **Event ID Management:** Never auto-generate, copy/paste or drag-to-bowtie only  
✅ **Duplicate Event IDs:** Not flagged as error (by design in LCC)  
✅ **Configuration Saving:** Manual [Apply] required  
✅ **Element Naming:** Use CDI names + user-configurable descriptions  
✅ **Event Monitor History:** Session-only  
✅ **Unconnected Elements Grouping:** By Node → Segment  
✅ **Initial Bowties View:** Connected only with unconnected count  

## Open Design Questions

**UX Questions:**

1. **Miller Columns Width:** Should columns be fixed-width or resizable? Auto-collapse left columns when drilling deep (>5 levels)?

2. **Bowtie Canvas Layout:** Auto-layout algorithm for initial placement? Allow manual drag positioning? Save positions per project?

3. **Consumer Event Slot Display:** When showing "Line 5 / Event 3" in bowtie diagram, clarify "Event 3" annotation clearly visible or risk confusion?

4. **Configuration Panel Sections:** Group parameters by category (Hardware, Events, Metadata)? Use tabs or accordions?

5. **Event Monitor Auto-Clear:** Should monitor auto-clear after X events or file size to prevent memory issues? Warn user before clearing?

## Success Metrics

**Usability Goals:**
- New user can understand existing configuration in <5 minutes
- Creating an event link takes <30 seconds (vs. 5+ minutes in JMRI)
- Configuration discovery <3 seconds for typical network (10-20 nodes)
- Zero Event ID knowledge required for basic usage

**Performance Goals:**
- Node discovery: <1 second
- CDI cache hit: <100ms
- Configuration value retrieval: <500ms per element
- Event monitoring: <50ms latency from physical event to display
- Support networks up to 100 nodes, 2000 elements

**Quality Goals:**
- 100% protocol compliance with OpenLCB standard
- No data corruption (configuration writes are safe)
- Graceful handling of network errors
- Clear error messages (avoid technical jargon)

---

*For current implementation status, see [docs/technical/architecture.md](../technical/architecture.md)*  
*For user workflows, see [docs/design/workflows.md](workflows.md)*  
*For feature roadmap, see [docs/project/roadmap.md](../project/roadmap.md)*
