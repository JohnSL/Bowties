# Proposal: JMRI Bridge — Bidirectional Sync via Jython Plugin

**Status:** Draft proposal — brainstorm capture for community feedback.  
**Origin:** Extension of behavior templates discussion, June 2026. Explores how Bowties-managed information channels and bowties can be automatically represented in JMRI without manual object creation.  
**Related:** [Behavior Templates & Information Channels](./behavior-templates-proposal.md)

---

## Problem

Users who run both Bowties and JMRI today must manually create JMRI objects (sensors, turnouts, signal masts, lights, reporters) for every event they configure in Bowties. This is tedious, error-prone, and creates a maintenance burden — changes made in one tool don't flow to the other.

The problem compounds with behavior templates: a single template application might create dozens of information channels, each requiring corresponding JMRI objects before panels can use them.

Additionally, real-world layouts are typically **multi-protocol** — LCC nodes for detection, DCC decoders for turnouts, LocoNet for legacy hardware. JMRI is often the only tool that sees the full picture across all protocols. Without integration, Bowties can only manage the LCC slice, leaving the user to manually coordinate everything else.

---

## Key Insight: JMRI Sees Everything; Bowties Understands Intent

JMRI has visibility into every protocol on the layout — every sensor, turnout, signal, and logic conditional regardless of whether it's LCC, DCC, LocoNet, or internal. Through JMRI, Bowties gains visibility into the **full layout** — not just the LCC portion.

Conversely, Bowties understands the **behavioral intent** — what channels mean, how they relate, what logic connects them, and what facilities they form. JMRI has the objects but not the semantic grouping.

Together: Bowties provides the intent and automation; JMRI provides the multi-protocol runtime and panel display.

---

## Key Insight: Bowties Already Has the Data

When Bowties manages a layout, it knows:
- Every bowtie (named event ID pair with producer/consumer roles)
- Every information channel (grouped meaning: occupancy, turnout position, signal aspect)
- Human-readable names for all of the above
- Which event IDs map to which states (active/inactive, thrown/closed, aspect names)

JMRI needs exactly this information to create its objects. The mapping is mechanical:

| Bowties Concept | JMRI Object | What JMRI Needs |
|---|---|---|
| Occupancy channel (occupied/clear) | Sensor | Two event IDs + user name |
| Turnout position channel (normal/diverging) | Turnout | Two event IDs + user name + feedback mode |
| Signal aspect channel (stop/approach/clear/...) | Signal Mast | Event ID per aspect + signal system + mast type |
| LED state channel (on/off) | Light | Two event IDs + user name |
| Button press channel (pressed/released) | Sensor | Two event IDs + user name |
| RFID/string value | Reporter | Event ID + user name |

In the reverse direction, JMRI has objects Bowties doesn't know about — DCC turnouts, LocoNet sensors, internal logic variables. Importing these into Bowties as channels gives the user a unified view of their entire layout.

---

## Multi-Protocol Visibility

Through JMRI, Bowties gains visibility into protocols it doesn't natively speak:

| Protocol | Command (out) | Feedback (in) | Bowties can see via JMRI |
|---|---|---|---|
| LCC | ✅ Event produced | ✅ Event consumed | ✅ (also sees directly) |
| DCC (basic) | ✅ Packet sent | ❌ No feedback | ✅ Command-only channels |
| DCC + RailCom | ✅ Packet sent | ✅ RailCom response | ✅ Bidirectional |
| LocoNet | ✅ Command sent | ✅ Reply received | ✅ Bidirectional |

This enables:
- **Channels backed by any protocol** — a "Block 7 Occupancy" channel can be backed by a LocoNet BDL168 sensor just as easily as an LCC BOD
- **Directionality awareness** — Bowties knows which channels are command-only (DCC) vs. bidirectional (LCC, LocoNet) and can warn when logic depends on feedback that doesn't exist
- **Gap analysis** — "This DCC turnout has no position feedback; add a microswitch sensor or switch to an LCC turnout for reliable signal logic"
- **Migration visibility** — "Upgrading this turnout from DCC to LCC would give you confirmed position feedback"

**Note on DCC accessory channels.** DCC accessories controlled through the OpenLCB well-known DCC accessory event allocation are first-class consumer channels in Bowties with a virtual binding (see the vision document's [Virtual Bindings: DCC Accessories](./app-ux-vision.md#virtual-bindings-dcc-accessories) section). They do not require JMRI: any compliant gateway (JMRI itself, TCS CS-105, TCS LT-50) listens on the LCC bus and emits the DCC packet. The JMRI bridge is only one of several gateways; Bowties is gateway-agnostic and the channel-level event wiring is the same in all cases.

---

## Architecture: The Bridge Is a Projection Layer

The JMRI bridge is **not** a core part of the template system or channel model. It is a downstream consumer — a projection of Bowties' information model into JMRI's object model.

```
Templates → create Information Channels → LCC Bus Sync (writes CDI to nodes)
                                        → JMRI Bridge (syncs to JMRI objects)
                                        → (future) Track Diagram (renders visually)
```

Templates produce channels. The bridge translates channels into JMRI objects. A template never says "create JMRI sensor MS..." — it says "create an occupancy channel." This keeps templates tool-agnostic.

The bridge also works without templates: any bowtie or channel in Bowties can be synced to JMRI, regardless of how it was created.

---

## Implementation: Jython Script with HTTP Server

The bridge is a Jython script (`bowties-jmri-bridge.py`) that runs inside JMRI, started via Preferences → Startup → Run Script. It launches a lightweight HTTP server on localhost that Bowties connects to.

### Why Jython (vs. Java JAR or stock JSON API)

| Concern | Jython script | Java SPI JAR | Stock JSON API |
|---|---|---|---|
| Development/iteration speed | **Fast** — edit and reload | Slow — compile, package, restart | N/A (can't extend) |
| Distribution | **Trivial** — single .py file | Simple — one .jar | N/A |
| Signal mast full configuration | **✅** Full internal API access | ✅ | ❌ Partial |
| Event name store access | **✅** Direct `OlcbEventNameStore` | ✅ | ❌ Not exposed |
| Bulk operations | **✅** | ✅ | ❌ One at a time |
| Trigger save | **✅** `ConfigureManager.storeUser()` | ✅ | ❌ Not exposed |
| Resilience to JMRI updates | **Best** — no compiled bytecode | Good | N/A |
| WebSocket / real-time push | ❌ (polling) | ✅ | ✅ |
| JMRI Preferences UI | ❌ | ✅ | N/A |

The Jython approach provides the fastest path to a working integration with no JMRI source modifications. If real-time push or deeper UI integration becomes necessary, a Java SPI JAR can be developed later using the same HTTP API contract — Bowties doesn't need to change.

### Bridge Endpoints

The script listens on `http://127.0.0.1:9521` (port configurable via script constant).

```
GET  /status
  → Bridge version, JMRI version, connection prefixes (with protocol types),
    available signal systems

GET  /sync
  → Full snapshot: all JMRI sensors, turnouts, signal masts, lights,
    reporters, and event names across all protocols. Each object includes
    its connection prefix (protocol identity). Pre-structured for Bowties.

POST /sync
  → Accept a Bowties sync payload: create/update objects in batch.
    Idempotent — existing objects matched by system name are updated.

POST /save
  → Trigger JMRI's storeUser() to persist configuration to disk.

GET  /signalSystems
  → List available signal systems with their mast types and aspect definitions.
    Enables Bowties to present signal system choices to the user.

GET  /eventNames
  → Full OlcbEventNameStore dump (event ID → name mappings)

POST /eventNames
  → Bulk update event name mappings

GET  /topology
  → Layout Editor panel topology: block connectivity graph, turnout connections,
    signal mast placements with protecting relationships. Only from Layout Editor
    panels (the sole panel type with real topology).

POST /logixng
  → Create or update LogixNG conditionals from Bowties logic definitions.
    Used when a behavior template targets LogixNG as its execution platform.

GET  /logixng
  → Current LogixNG conditionals managed by Bowties (for sync/conflict detection).
```

### GET /sync Response Shape

```json
{
  "version": "1.0",
  "connections": [
    {"prefix": "M", "protocol": "openlcb", "description": "OpenLCB via USB"},
    {"prefix": "L", "protocol": "loconet", "description": "LocoNet"},
    {"prefix": "D", "protocol": "dcc", "description": "DCC++", "directionality": "command-only"}
  ],
  "sensors": [
    {
      "systemName": "MS01.02.03.04.05.06.07.08;09.0A.0B.0C.0D.0E.0F.10",
      "userName": "Eagle Creek - Occupied",
      "connectionPrefix": "M",
      "protocol": "openlcb",
      "activeEventId": "01.02.03.04.05.06.07.08",
      "inactiveEventId": "09.0A.0B.0C.0D.0E.0F.10",
      "state": "active",
      "inverted": false,
      "comment": "BOD-8 Connector A Pin 1",
      "properties": {
        "bowties.managed": "true",
        "bowties.channelId": "ch-001"
      }
    },
    {
      "systemName": "LS44",
      "userName": "Yard Entrance - Occupied",
      "connectionPrefix": "L",
      "protocol": "loconet",
      "state": "inactive",
      "inverted": false,
      "properties": {}
    }
  ],
  "turnouts": [
    {
      "systemName": "MT21.22.23.24.25.26.27.28;29.2A.2B.2C.2D.2E.2F.30",
      "userName": "Eagle Creek - East Turnout",
      "thrownEventId": "21.22.23.24.25.26.27.28",
      "closedEventId": "29.2A.2B.2C.2D.2E.2F.30",
      "state": "closed",
      "feedbackMode": "MONITORING",
      "inverted": false,
      "properties": {
        "bowties.managed": "true",
        "bowties.channelId": "ch-002"
      }
    }
  ],
  "signalMasts": [
    {
      "systemName": "IF$olm:basic:one-searchlight($1)",
      "userName": "Eagle Creek - East Signal",
      "signalSystem": "basic",
      "mastType": "one-searchlight",
      "aspects": {
        "Stop": "31.32.33.34.35.36.37.38",
        "Approach": "39.3A.3B.3C.3D.3E.3F.40",
        "Clear": "41.42.43.44.45.46.47.48"
      },
      "litEventId": "51.52.53.54.55.56.57.58",
      "notLitEventId": "59.5A.5B.5C.5D.5E.5F.60",
      "heldEventId": "61.62.63.64.65.66.67.68",
      "notHeldEventId": "69.6A.6B.6C.6D.6E.6F.70",
      "currentAspect": "Clear",
      "properties": {
        "bowties.managed": "true",
        "bowties.channelId": "ch-003"
      }
    }
  ],
  "lights": [],
  "reporters": [],
  "eventNames": {
    "01.02.03.04.05.06.07.08": "Eagle Creek - Occupied",
    "09.0A.0B.0C.0D.0E.0F.10": "Eagle Creek - Clear"
  }
}
```

### POST /sync Request Shape

```json
{
  "version": "1.0",
  "sensors": [
    {
      "systemName": "MS01.02.03.04.05.06.07.08;09.0A.0B.0C.0D.0E.0F.10",
      "userName": "Eagle Creek - Occupied",
      "comment": "BOD-8 Connector A Pin 1",
      "inverted": false,
      "properties": {
        "bowties.managed": "true",
        "bowties.channelId": "ch-001"
      }
    }
  ],
  "turnouts": [],
  "signalMasts": [
    {
      "systemName": "IF$olm:basic:one-searchlight($1)",
      "userName": "Eagle Creek - East Signal",
      "signalSystem": "basic",
      "mastType": "one-searchlight",
      "aspects": {
        "Stop": "31.32.33.34.35.36.37.38",
        "Approach": "39.3A.3B.3C.3D.3E.3F.40",
        "Clear": "41.42.43.44.45.46.47.48"
      },
      "litEventId": "51.52.53.54.55.56.57.58",
      "notLitEventId": "59.5A.5B.5C.5D.5E.5F.60",
      "heldEventId": "61.62.63.64.65.66.67.68",
      "notHeldEventId": "69.6A.6B.6C.6D.6E.6F.70",
      "properties": {
        "bowties.managed": "true",
        "bowties.channelId": "ch-003"
      }
    }
  ],
  "lights": [],
  "reporters": [],
  "eventNames": {
    "01.02.03.04.05.06.07.08": "Eagle Creek - Occupied",
    "09.0A.0B.0C.0D.0E.0F.10": "Eagle Creek - Clear"
  }
}
```

### POST /sync Response

```json
{
  "created": ["MS01.02.03.04.05.06.07.08;09.0A.0B.0C.0D.0E.0F.10"],
  "updated": ["IF$olm:basic:one-searchlight($1)"],
  "unchanged": [],
  "errors": []
}
```

---

## Sync Model: Snapshot-Based Bidirectional Merge

Both Bowties and JMRI can be edited by the user. The bridge supports bidirectional changes using a three-way merge based on stored snapshots.

### Sync Trigger

- **On connection:** When Bowties connects to the bridge (user clicks "Connect to JMRI" or auto-connect on layout open), an initial sync runs.
- **On demand:** User can trigger sync manually at any time (e.g., after making changes in either tool).

### Three-Way Merge Logic

Bowties stores a **base snapshot** (per synced object) representing the last-known-synced state:

```
Base (last sync)  →  JMRI current (via GET /sync)
                  →  Bowties current (local channel data)
```

| Base → JMRI | Base → Bowties | Action |
|---|---|---|
| Unchanged | Unchanged | No action |
| Changed | Unchanged | Accept JMRI change → update Bowties |
| Unchanged | Changed | Push Bowties change → update JMRI |
| Changed | Changed (same value) | No action (convergent edit) |
| Changed | Changed (different) | **Conflict** — present to user |

### Conflict Resolution

Conflicts are expected to be rare (user edited the same field in both tools between syncs). When they occur, Bowties presents them:

```
Conflict: "Eagle Creek - East Turnout"
  Field: userName
  JMRI value:    "EC East Turnout"  (changed in JMRI)
  Bowties value: "Eagle Creek East" (changed in Bowties)
  
  [Use JMRI]  [Use Bowties]  [Skip]
```

### Object Scope Marker

Objects synced by Bowties carry a `bowties.managed` property (stored in JMRI's generic bean properties, persisted in panel XML):

- **`bowties.managed = true`** — Part of the sync set. Bowties tracks changes bidirectionally.
- **No marker** — JMRI-native object. Bowties can read it (for channel discovery/import) but doesn't push changes unless the user explicitly adopts it into a channel.

This prevents Bowties from accidentally overwriting unrelated JMRI objects.

### New Object Discovery

On sync, Bowties can discover JMRI objects it doesn't know about:
- Untagged OpenLCB sensors/turnouts/masts → "JMRI has LCC objects not tracked by Bowties"
- Non-OpenLCB objects (DCC turnouts, LocoNet sensors) → "JMRI has objects on other protocols that could be imported as channels"
- User can **adopt** them: Bowties creates a corresponding channel, links by system name, marks as managed
- Each adopted channel carries its protocol identity and directionality (LCC = bidirectional, basic DCC = command-only, etc.)
- This enables the onboarding path: existing JMRI layout → import into Bowties → facilities/channels auto-discovered across all protocols

### Deleted Object Handling

- If a managed object disappears from JMRI (user deleted it), Bowties detects "was in snapshot, now missing" → offers to recreate or unlink
- If a channel is deleted in Bowties, next sync can optionally remove the JMRI object (with confirmation)

---

## Signal System Handling

### Per-Mast Scope

JMRI assigns signal systems **per mast** — a single layout can mix signal systems freely. The system name encodes both signal system and mast type: `IF$olm:<signalSystem>:<mastType>($N)`.

### Where Signal System Metadata Lives

In the Bowties model, signal system and mast type are properties of a **signal aspect channel**:

```
Information Channel: "Eagle Creek - East Signal"
  Type: signal-aspect
  States: [Stop, Approach, Clear]
  Signal System: basic              ← needed for JMRI bridge
  Mast Type: one-searchlight        ← needed for JMRI bridge
```

This metadata can come from:
1. **Behavior template** — template specifies or parameterizes signal system/mast type
2. **Hardware template** — signal driver hardware implies physical head type
3. **User selection** — during channel creation or template application
4. **Import from JMRI** — parsed from existing mast system name

### Signal System Discovery

The bridge exposes `GET /signalSystems` so Bowties can present valid choices:

```json
{
  "systems": [
    {
      "name": "basic",
      "aspects": ["Clear", "Approach", "Stop"],
      "mastTypes": ["one-searchlight", "one-low", "two-searchlight", "two-low"]
    },
    {
      "name": "AAR-1946",
      "aspects": ["Clear", "Approach Medium", "Approach", "Stop"],
      "mastTypes": ["SL-1-high", "SL-2-high", "SL-3-high"]
    }
  ]
}
```

---

## Workflows

### Initial Setup

1. User installs `bowties-jmri-bridge.py` in JMRI:
   - Preferences → Startup → Add → Run Script → select file
   - JMRI restarts; bridge starts automatically on port 9521
2. In Bowties, user configures JMRI connection (host: localhost, port: 9521)
3. On first connect, Bowties reads all existing JMRI objects (all protocols)
4. User can adopt existing objects as channels (onboarding existing layout)
5. Adopted channels carry their protocol identity and directionality

### Template Apply → JMRI Sync

1. User applies behavior template in Bowties → channels created
2. User chooses logic execution target (on-node or LogixNG)
3. Bowties connects to bridge (if not already connected)
4. Sync runs: new channels → POST to bridge → JMRI objects created
5. If LogixNG target: logic conditionals pushed via POST /logixng
6. User sees new sensors/turnouts/masts appear in JMRI's tables
7. User assigns them to Layout Editor panel elements (dropdown selections)
8. User clicks "Auto-discover Signal Mast Logic" in JMRI — signal logic auto-configured from panel topology
9. User saves in JMRI when ready (or Bowties triggers save via POST /save)

### Editing in JMRI → Bowties Sync

1. User renames a sensor in JMRI's Sensor Table (e.g., for panel label clarity)
2. Next sync: Bowties GETs current state, detects userName changed vs. snapshot
3. Since Bowties didn't change it: auto-accepts JMRI's new name
4. Channel name updated in Bowties, snapshot updated

### Editing in Bowties → JMRI Sync

1. User renames a channel in Bowties
2. Next sync: Bowties detects local change vs. snapshot, JMRI unchanged
3. POSTs update to bridge → JMRI sensor userName updated
4. Snapshot updated

### Import Existing JMRI Layout

1. User has existing JMRI with sensors/turnouts/masts (any protocol — OpenLCB, LocoNet, DCC, etc.)
2. Installs bridge, connects Bowties
3. Bowties shows: "Found 24 sensors, 8 turnouts, 6 signal masts not tracked by Bowties" (grouped by protocol)
4. User selects which to adopt → Bowties creates channels, matches event IDs or system names to existing bowties
5. Each adopted channel carries protocol identity and directionality metadata
6. Optionally: user groups adopted channels into facilities
7. If Layout Editor panel exists: Bowties reads topology via GET /topology → suggests facility groupings based on block adjacency

---

## Security Considerations

- Bridge listens on **127.0.0.1 only** (not 0.0.0.0) — localhost access only
- No authentication (matches JMRI's own JSON server pattern)
- If network exposure is needed in future, add a shared-secret header check
- Bridge validates all incoming JSON (reject malformed payloads, oversized requests)
- No arbitrary code execution from POST payloads — only predefined operations on known object types

---

## Distribution

The bridge script ships as part of Bowties:
- Located in Bowties installation (e.g., `tools/jmri/bowties-jmri-bridge.py`)
- User copies to their JMRI scripts directory (or references it directly from Bowties install path)
- Bowties UI could offer "Install JMRI Bridge" button that copies the file and shows setup instructions
- Version compatibility checked on connect (bridge reports its version in GET /status)

---

## Future Evolution

### LogixNG as Logic Execution Target

JMRI's LogixNG provides conditional logic that works across **all protocols** — it can read a LocoNet sensor and command a DCC turnout in the same conditional. This makes it essential for mixed-protocol layouts where on-node logic (LCC-only) is insufficient.

When a behavior template's logic is targeted at LogixNG (instead of on-node), the bridge creates LogixNG conditionals:
- Template logic rules → compiled to LogixNG expression trees
- Input channels → mapped to JMRI sensor/turnout expressions
- Output channels → mapped to JMRI signal mast/turnout actions
- Pushed to JMRI via the bridge as structured LogixNG definitions

LogixNG operates transparently on JMRI objects. Setting an `OlcbTurnout` via LogixNG automatically produces the corresponding LCC event on the bus. Reading an `OlcbSensor` in a LogixNG expression automatically reacts to LCC events. This means LogixNG-hosted logic participates fully in the LCC world without special handling.

**When to use each execution target:**

| Target | Best for | Requires |
|---|---|---|
| On-node (TowerLCC logic lines) | Pure LCC; must work without computer; lowest latency; safety-critical interlocking | TowerLCC or similar with logic capability |
| On-node (STL program) | Complex on-node logic; still computer-independent | TowerLCC+Q |
| JMRI LogixNG | Mixed-protocol; inputs/outputs span multiple protocols; computer always running | JMRI with bridge |

Users can mix targets within a single facility — safety-critical interlocking on-node, convenience automation in LogixNG.

### Layout Editor Topology Integration

JMRI's Layout Editor encodes **physical track topology** — which blocks connect to which, where turnouts join paths, and where signals protect entrances. This topology is exactly what behavior templates need to auto-wire channel relationships.

The bridge provides topology access via:

```
GET  /topology
  → Block connectivity graph from Layout Editor panels.
    Which blocks are adjacent, which turnouts join them,
    which signal masts are placed where.
```

This enables:
- **Auto-wiring during template apply:** "Block 8 is adjacent to Block 7" → automatically assign Block 8 occupancy as the "next-block" input for Block 7's signal logic
- **Gap analysis across the layout:** "Block 7 has occupancy but no signal protecting its entrance"
- **Facility-to-geography mapping:** Facilities correspond to physical track sections
- **Future Bowties layout editor:** Import JMRI topology → render in Bowties with modern UX → maintain as single source of truth

**Layout Editor is the focus** — it's the only JMRI panel type with real track topology and automatic signal mast logic discovery. Other panel types (Panel Editor, Switchboard Editor, Control Panel Editor) display the same beans but don't add topology knowledge.

The Layout Editor workflow with the bridge:
1. Bowties creates beans (sensors, turnouts, signal masts) via template or manually
2. Beans appear in JMRI's tables — fully configured with event IDs, names, and aspect mappings
3. User draws track in JMRI's Layout Editor (or Bowties imports an existing panel)
4. User assigns beans to track elements — just picking from dropdown lists of **already named, already configured** objects
5. User clicks JMRI's "Auto-discover Signal Mast Logic" — JMRI traverses the panel topology and automatically configures which blocks/turnouts affect each signal
6. Panel is fully functional

The value of the bridge here: steps 1-2 eliminate the hardest manual work (creating objects with correct event IDs and aspect mappings). Steps 3-6 remain lightweight UI operations — dropdown selections and one auto-discovery click.

### Layout-Wide Health and Completeness

With visibility into all JMRI objects plus Bowties' semantic understanding, the system can perform layout-wide analysis:
- "These 3 blocks have occupancy detection but no signals assigned"
- "This turnout is commanded but has no position feedback — signal logic using it is unreliable"
- "This signal head has no logic driving it — it's dark"
- "These two sensors produce the same event — conflict"
- "Tower-3 has 4 logic lines remaining; Tower-5 is full"

This extends naturally from the channel model: every channel has a type, a directionality, a backing implementation, and connections to other channels. Missing connections and capability gaps become visible.

---

## Relationship to Behavior Templates

The JMRI bridge is architecturally downstream of the template system:

| Layer | Responsibility | Knows about JMRI? |
|---|---|---|
| Behavior template | Defines channel requirements and logic | **No** |
| Template apply engine | Creates channels, compiles logic to target | **Yes** (if LogixNG is the target) |
| Information channel | Stores states, metadata, directionality | **No** (protocol-agnostic) |
| **JMRI Bridge** | **Projects channels into JMRI objects; provides multi-protocol visibility and LogixNG execution** | **Yes** |

Templates never reference JMRI directly. They produce channels and logic rules. When the execution target is on-node, the apply engine writes CDI configuration with no JMRI involvement. When the target is LogixNG, the apply engine uses the bridge to push logic to JMRI.

This separation means:
- Templates work without JMRI (users who don't use JMRI are unaffected)
- The bridge works without templates (manually created bowties/channels sync too)
- Both can evolve independently
- Multi-protocol channels (via JMRI import) participate in the same template system as LCC channels

---

## Open Questions

1. **Port selection** — Fixed port 9521, or configurable? If configurable, where does the user set it in Bowties? In the script?

2. **Multiple JMRI connections** — If JMRI has multiple OpenLCB connections (different prefixes), should the bridge filter by prefix? Or sync all? (For non-OpenLCB objects, the connection prefix identifies the protocol.)

3. **Signal mast instance numbering** — When creating a new `OlcbSignalMast`, the instance number (`$N`) must be unique. Bridge needs to find the next available number per signal-system+mast-type combination.

4. **JMRI Blocks** — Should the bridge also create JMRI Block objects (for track diagram coloring)? Blocks reference a sensor for occupancy detection. Creating Block + assigning its sensor would further reduce the user's panel-wiring work.

5. **LogixNG expression format** — What's the exact data structure for pushing LogixNG conditionals via the bridge? LogixNG has its own XML schema. Does the bridge push raw LogixNG XML, or a simpler abstract format that the bridge compiles?

6. **Reconnection** — If JMRI restarts while Bowties is running, how does Bowties detect and reconnect? Polling /status? mDNS announcement?

7. **Concurrent access** — If Bowties and a user are both modifying JMRI objects simultaneously, the bridge's thread-safety model needs to handle this. JMRI's managers are thread-safe, but the snapshot comparison window could race.

8. **Event name ownership** — If both Bowties and JMRI users manually add event names, the event name sync could conflict. Same snapshot-merge approach, or simpler "union merge" (never delete names)?

9. **Non-OpenLCB channel creation** — When Bowties creates a channel backed by a non-LCC protocol (e.g., DCC turnout), the bridge creates the JMRI object with the appropriate system name format for that protocol. Does Bowties need to know protocol-specific addressing (DCC address, LocoNet slot), or does it always work through JMRI system names?

10. **Layout Editor topology scope** — If the user has multiple Layout Editor panels, does the bridge merge their topology? Or does it present per-panel views?

11. **Java SPI JAR upgrade path** — If the Jython script hits limitations (WebSocket, Preferences UI, performance), the bridge API contract should remain stable when re-implemented as a Java JAR. What aspects of the API need versioning to support this transition?
