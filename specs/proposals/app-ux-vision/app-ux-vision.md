# Bowties Application UX Vision

**Status:** Draft vision — captures the target experience arc for the application.  
**Origin:** Brainstorm session, June 2026. Synthesizes behavior templates, hardware planner, and JMRI bridge proposals into a unified application experience.  
**Related:**  
- [Behavior Templates & Information Channels](./behavior-templates-proposal.md)  
- [Hardware Planner Wizard](./planner-proposal.md)  
- [JMRI Bridge](./jmri-bridge-proposal.md)  
- [Feasibility & Architecture Companion](./app-ux-vision-feasibility.md) — technical approach, existing infrastructure, and feasibility analysis

---

## Identity

Bowties is the tool that makes LCC accessible to people who are not engineers. It expands the LCC user base by eliminating the expertise barrier — users think in terms of railroad behavior ("I want ABS signaling on this passing siding"), not protocol details (event IDs, CDI fields, logic programming).

The current Bowties experience — connecting to nodes, reading CDI, editing fields, creating named event connections — is easier than JMRI but still demands deep technical knowledge. The vision described here transforms Bowties from "a friendlier CDI editor" into "the tool that turns hardware into a working railroad."

---

## Target Users

**Primary:** Model railroaders who want LCC automation but are not software engineers, electrical engineers, or PLC programmers. They understand their railroad (blocks, turnouts, signals, sidings) but not protocols, event addressing, or logic languages.

**Secondary:** Technically proficient users (engineers, JMRI power users) who find value in the higher-level abstractions for productivity, comprehension, and documentation — even though they *could* do it manually.

Both groups use both workspaces — every user wires hardware and configures behavior. The difference is depth: primary users work with channels and facilities in both workspaces and rarely need raw CDI fields or event IDs, while secondary users occasionally drill below the channel abstraction to raw bowtie connections or CDI fields for advanced tasks.

---

## The User Journey: Plan → Wire → Railroad → Operate

The application experience follows four activities that map to how a real layout project unfolds. These are presented in a natural first-time order, but in practice they are **concurrent, iterative, and sectional** — not a linear pipeline with gates.

A layout is built in sections over months or years. A user might plan one siding, wire it, apply a template, and have it running — then come back six months later to plan the next section. At any moment, multiple facilities coexist at different lifecycle stages: some active, some wired but awaiting templates, some still just planned. The workspace toggle is always available; nothing prevents switching between wiring and railroad activities at any time.

The planner works incrementally — it doesn't assume a blank slate. "I already have three blocks working; now I'm adding a siding" is a normal starting point. Plans grow as the layout evolves.

### Phase 1: Plan

**User mindset:** "I'm designing my layout. What LCC boards do I need? How many? What goes where?"

**What Bowties provides:**
- A guided planner that interviews the user about their layout — number of blocks, turnouts, sidings, desired signaling, detection method.
- A recommended bill of materials grounded in the user's answers, with rationale per board.
- A wiring outline at the connector level — which board's connector handles which track feature.
- The output persists as a **layout plan** — a set of prospective facilities with their hardware requirements. This plan becomes the scaffold for everything that follows.

**Key interaction:** Conversational wizard. Short questions, visual progress, immediate feedback. The user describes railroad intent; the system translates to hardware requirements.

**Lifecycle connection:** The plan creates facilities in a "planned" state and can also create **placeholder nodes** — profile-backed representations of boards the user intends to buy or has ordered but not yet installed. Placeholders are fully configurable: the user can assign daughter boards, name channels, and even apply templates before hardware arrives. As the user progresses through Wire and Railroad phases, the plan tracks progress: "Eagle Creek Siding: 2/3 boards assigned, 0/1 templates applied."

### Phase 2: Wire

**User mindset:** "I have boards in hand. I'm under the layout with a soldering iron. Which connector goes where? Which pins connect to which track feeders?"

**What Bowties provides:**
- A board-centric workspace where you select a node and configure its physical reality: daughter boards on connectors, pin modes, electrical parameters.
- Automatic creation of named information channels from hardware selection. Selecting a BOD-8 daughter board on Connector A immediately creates 8 occupancy channels with default names.
- Channel naming guided by the plan: "Your plan expects 'Block 7 Occupancy' — assign it to this pin?"
- Printable pin documentation showing every connector, pin assignment, and channel name — stickable to the underside of the layout.

**Key interaction:** Select a node → select a connector → choose daughter board or pin mode → name the resulting channels. The plan tracks progress and reminds the user what's still needed.

**Placeholder-to-connected reconciliation:** When a real board connects to the bus, the user maps it to an existing placeholder. This cannot be inferred automatically — a layout often has multiple nodes of the same type (e.g., three TowerLCC boards), so the user must explicitly say "this physical node is the one I called Tower-3." Once mapped, all pre-staged configuration (channels, daughter board selections, template assignments) syncs down to the real node.

**Lifecycle connection:** Channels move from "planned" (requirement in a facility) to "wired" (backed by physical hardware with a name). The intermediate state — configured on a placeholder but not yet connected to real hardware — is fully functional for planning purposes and is treated as "wired" in the facility lifecycle.

### Phase 3: Railroad

**User mindset:** "My hardware is installed. I want this collection of sensors and signals to behave like a real ABS signal block."

**What Bowties provides:**
- A facility-centric workspace where you see your railroad's functional units — sidings, signal blocks, CTC panels.
- A behavior template library browsable by intent: "What do I want this to do?" → pick a template → system shows what channels it needs.
- A guided apply workflow: the template declares channel requirements by type; the system shows matching channels from your layout; you map each requirement.
- Logic execution target selection: on-node (works without a computer) or JMRI LogixNG (works across protocols).
- The result is a named, live facility: "Eagle Creek Signal Block" — all channels, logic, and event wiring configured automatically.

**Key interaction:** Browse templates by goal → select → map channels → choose logic target → apply. The template encodes the expert knowledge; the user just makes selections.

**Lifecycle connection:** Facilities move from "wired" (channels exist but no behavior) to "active" (behavior template applied, logic programmed, ready to operate).

### Phase 4: Operate (via JMRI)

**User mindset:** "My facilities are configured. I want to run trains, see signals on a panel, and fix things when they break."

**What Bowties provides:**
- Automatic JMRI bridge sync: channels and facilities are projected into JMRI objects (sensors, turnouts, signal masts) — no manual JMRI object creation.
- The user draws their Layout Editor panel in JMRI by selecting from dropdown lists of already-named, already-configured objects.
- Debugging tools in Bowties: open a facility, see the full signal chain, observe live channel states, identify where a problem occurs.

**What Bowties does NOT provide (deferred):**
- Panels, throttles, or runtime train control — JMRI owns operation.
- A native layout editor or track diagram — JMRI Layout Editor is the near-term tool.

**Key interaction:** Bowties is primarily a debugging/comprehension tool during operation. "My signal is red when it should be green" → open the facility → see which input is wrong → trace to the physical channel → identify the issue (wiring? logic? sensor failure?).

---

## Navigation Model: Two Workspaces

The application provides two workspaces that correspond to the two real-world activities users perform. These are not abstract taxonomies — they map to whether you're thinking about physical hardware or railroad behavior.

### Wiring Workspace

**Purpose:** The complete node management surface. Configure physical hardware (connectors, pins, daughter boards), manage board-level settings (throttle behavior, DCC parameters, detection thresholds, network config), and create/name information channels from hardware selections.

**When you're here:** Setting up new hardware. Assigning daughter boards. Naming pins. Adjusting board-level settings (DCC output mode, detection sensitivity, throttle behavior). Troubleshooting a dead sensor. Printing wiring documentation.

**Primary object:** Nodes. You pick a node (board) and work with everything about it.

**Sidebar:** Node tree (similar to today's Config sidebar). Nodes expand to show connectors, assigned daughter boards, a "Settings" entry for board-global configuration, and a "Raw CDI" drill-down. A virtual **"DCC Layout"** entry sits alongside the real nodes and groups every DCC accessory channel in the layout (see [Virtual Bindings: DCC Accessories](#virtual-bindings-dcc-accessories)).

**Main area — two presentation modes:**

1. **Guided view** (default, when a profile exists) — organized by purpose:
   - **Channels** — each pin/group that produces layout-wide information (occupancy, signal aspect, turnout position) is shown as a named channel. Per-channel, the view shows:
     - *Unmanaged settings* (shown by default) — per-pin configuration that the user freely adjusts without affecting the channel's role or style: active high/low, debounce time, current limit, common anode/cathode. These are the wiring decisions.
     - *Managed settings* (collapsed, expandable) — fields whose values are determined by the channel's style. Shown with a clear "managed by channel style" annotation. The style *constrains* these fields to compatible values — incompatible options are not offered. Expanding this section lets the user see what was set and why.
   - **Board-global settings** — configuration that isn't tied to any pin or channel: SNIP name, network config, Blue/Gold enable, DCC output mode, throttle behavior. Presented with profile-guided descriptions.
   - **Multi-pin channels** — channels whose style claims multiple physical pins (e.g., a 3-aspect signal style using R/G/Y outputs) group their constituent pins together. Each pin has its own unmanaged settings; managed settings span the group.

2. **Raw CDI** — full field-by-field view for power users who need direct access to any CDI field. The escape hatch, not the starting point. Bypasses all profile-level constraints.

**Key difference from today's Config tab:** The guided view organizes fields by what they *do* (channel-producing vs. board-global) and uses the profile's constraint system to prevent invalid combinations. Users see only compatible options for channel-managed fields. Raw CDI remains available for overriding anything.

**Boards without profiles** can still expose channels via user-authored channel styles — the user picks a channel role from the system catalog, picks a style shape that fits the board (or authors one), and binds the style's field roles to CDI paths on the board. This is technical but bounded; it serves DIY and firmware-author scenarios where a shipped profile doesn't yet exist. Raw CDI remains as the ultimate escape hatch for anything outside the channel model. Profiles eliminate the manual mapping step for popular boards. See [Channel Roles, Styles, and Bindings](#channel-roles-styles-and-bindings) for the underlying mechanism.

### Railroad Workspace

**Purpose:** Understand and manage railroad behavior. Facilities, channels, templates, logic. Debug problems. Apply behavior templates.

**When you're here:** Applying a template. Checking why a signal is misbehaving. Viewing the full signal chain for a siding. Planning what behavior to add next.

**Primary object:** Facilities. You see your railroad as functional units, not as boards.

**Sidebar:** Facility list with status indicators (planned / wired / active). Expandable to show constituent channels. A separate "Channels" section shows ungrouped channels not yet part of any facility.

**Main area:** Facility detail. Shows the full picture: input channels, logic, output channels, participating nodes, live status. For ungrouped channels: a list view with type, name, backing hardware, and "apply template" action.

**Relationship annotations:** When viewing a facility, channels show cross-references to other facilities that share them. "Block 7 Occupancy" might be consumed by both "Block 7 Signal" and "CTC Panel — East End." These links are navigable.

### Switching Between Workspaces

The workspaces are a top-level toggle (replacing today's Config/Bowties segmented control). Cross-references between workspaces are clickable: clicking a node reference in the Railroad workspace switches to that node in the Wiring workspace, and vice versa.

**Current UX mapping:**
- Today's **Config tab** → becomes the detail layer within the **Wiring workspace**
- Today's **Bowties tab** → becomes an advanced/detail view within the **Railroad workspace** (raw event connections below the channel/facility layer)

### Information Channels as Always-Visible Infrastructure

Channels are visible in both workspaces, viewed from different angles:

| Workspace | Channel presentation |
|---|---|
| **Wiring** | The profile-supplied label for the channel's binding — e.g., "Tower-3 — Connector A — Input 1", "Signal LCC #1 — Line 3", or "Signal LCC #1 — Mast 2" — shows what hardware backs the channel |
| **Railroad** | "Block 7 Occupancy, part of Eagle Creek Signal Block" — shows what the channel means |

A dedicated **Channels panel** (within the Railroad workspace) is the hardware-organised inventory: every channel in the layout grouped by node + subsystem + pin, showing its role, style, live state, and the slot/facility currently bound to it (or "unbound"). This is the surface for "is my hardware wired correctly and working" — connecting to the bus and triggering a real BOD input lights up the corresponding channel row immediately, no facility required. It also makes the slot's Select-channel picker obvious: the channels the user has been watching in the panel are exactly the candidates the slot shows, filtered to the slot's required role.

A second, **layout-organised** view — Channels-by-name, grouped by role and meaning rather than by hardware — lands later, alongside ref-counting + multi-slot binding for shared channels. It serves the planning-and-inventory questions ("I have occupancy on 8 blocks but signals on only 3") and the template apply workflow ("which channels should I map to this template requirement?") at the role/meaning level rather than at the hardware level.

### Channel Connectivity and Topology

Channels can be **connected** to express physical topology: "Block 7 is adjacent to Block 8", "this turnout protects this signal." Connectivity enables:
- **Auto-wiring in behavior templates:** a template can infer that "Block 8 Occupancy" is the next-block input for Block 7's signal logic, instead of asking the user to pick it manually.
- **Gap analysis:** "Block 7 has occupancy but no signal assigned" or "this turnout has no position feedback."

Topology can come from manual declaration in Bowties, import from JMRI's Layout Editor (where block connectivity is encoded in the panel drawing), or a future native track editor.

**Note:** Topology is a later-horizon feature. Until topology is available, template application requires the user to manually map each channel input (e.g., selecting which occupancy channel is the "next block ahead"). This is more explicit than the auto-wiring mockups suggest, but remains straightforward — the user picks from a filtered list of compatible channels.

### Channel Roles, Styles, and Bindings

Every channel is described by three orthogonal facts: its **role**, its **style**, and its **binding**.

- The **role** is what the channel does in the layout — its state vocabulary plus the slot-binding contract. Examples: `block-occupancy` (states `unknown` / `occupied` / `clear`), `lamp-indicator` (states `unknown` / `lit` / `unlit`), `signal-aspect-3-color` (states `unknown` / `stop` / `approach` / `clear`). Facility slots bind **by role** — any channel of the slot's required role can fill it, regardless of which style implements it. State vocabularies name real-world intent (`occupied` / `clear`, `lit` / `unlit`), never electrical or boolean abstractions (`true` / `false`, `on` / `off`).
- The **style** is the specific hardware shape that realises a role on a specific subsystem — pins claimed, event-leaf mapping, CDI-field constraints. Examples: `bod-block-detector-input` (1 input pin on a BOD daughter board, realises `block-occupancy`); `single-led-direct-lamp` (1 Direct Lamp Control row, realises `lamp-indicator`); `3-led-direct-aspect` and `2-led-bicolor-aspect` (both realise `signal-aspect-3-color` on different pin counts and wiring shapes); a future Mast-driven style would realise the same role through a single firmware mast resource that hides per-LED management entirely. A role may be realised by one style or by several; the channel layer is permanently insulated from the difference.
- The **binding** is the concrete target a channel's style is wired to. Most bindings are **physical**: a specific pin on a real node, a specific Logic block, or (in the broader vision) a pin on a placeholder node. A small number of styles use **virtual bindings** — addresses in a protocol namespace that no Bowties-managed node owns, but that gateway hardware translates to wire-level packets (see [Virtual Bindings: DCC Accessories](#virtual-bindings-dcc-accessories)). Every channel always has a binding — there are no logical, unbound channels. Planning before hardware arrives uses placeholder nodes whose pins/Logic-blocks back channels exactly the way real-node pins do (see [Placeholder Nodes](#placeholder-nodes-pre-arrival-configuration)).

> *Code-mapping note (for implementers only).* The OO analogues are: role ≈ interface (state-vocabulary contract), style ≈ implementation class, channel ≈ instance. User-facing language is `role` and `style`; `interface` and `implementation class` should not surface in the UI or in product-facing docs except as this single anchor.

#### Directionality

A channel is always either a **producer** (generates information) or a **consumer** (acts on information), never both. When physical hardware does both — a turnout motor that drives position and reports it — those are modeled as two separate channels that may share the same device. This maps directly to the facility comprehension view: producer channels appear as inputs (left), consumer channels as outputs (right).

A role determines which events are meaningful and which are implementation detail. For example, a two-button role cares about "pressed" events; "released" events are filtered out at the channel level.

The following are representative roles, not an exhaustive list. Profiles declare which roles each board can host and which styles realise them.

**Producer roles** — generate information about the layout:

| Role | Pins (per style) | Information produced |
|---|---|---|
| **Block Occupancy** | 1 | Occupied / clear for a detection block |
| **Turnout Position Feedback** | 1–2 | Normal / diverging position report |
| **Two-Button Command** | 2 (one per direction) | Pressed events only — commands intent (e.g., "throw turnout left"). Released events are filtered |
| **Single Button** | 1 | Pressed event — a momentary command or toggle |
| **Current Sensor** | 1 | Analog or threshold-based current reading |

**Consumer roles** — act on information to drive physical outputs:

| Role | Pins (per style) | Information consumed |
|---|---|---|
| **Signal Aspect** | 2–3 (R/G/Y outputs, depending on style) | Aspect value → drives the corresponding LED combination |
| **Turnout Motor Command** | 1–2 | Normal/diverging command → drives servo or stall motor |
| **Lamp Indicator** | 1 | Lit/unlit state → drives a single lamp (LED, incandescent, anything) |
| **Turnout Position Indicator** | 2 (lamps) | Normal/diverging state → drives a pair of lamps to show position on a fascia panel |
| **DCC Accessory Command** | 0 (virtual binding to a DCC address) | Normal/reverse (or on/off) command → emitted as the well-known LCC events for that DCC accessory address; a gateway converts to a DCC accessory packet on the track |

**Key property:** Multiple channels of the same role can feed the same facility input. Two button-pair channels on opposite sides of a layout module — physically independent, on different nodes — can both serve as "turnout command" inputs to the same turnout facility. The facility is what unifies them logically; the channels remain physically grounded in their respective pin groups.

**Style polymorphism is hidden from the role.** A single role can be realised by different styles on different boards. A `signal-aspect-3-color` channel might use a Mast-driven style on Signal LCC (a single firmware-level mast that internally drives the LEDs by aspect rules), a `3-led-direct-aspect` style on a board exposing raw LED drivers, or a `2-led-bicolor-aspect` style on a board doing 2-LED color mixing. The facility slot doesn't know which — it sees only "a `signal-aspect-3-color` channel is bound here." For compound entities like a signal aspect, the user picks the style (e.g., `3-led-direct-aspect` vs `2-led-bicolor-aspect`) at planning time; the hardware-requirements report can then aggregate ("you need 3 more LED outputs") by inspecting bindings to placeholder-node pins.

#### Channel ownership

Two ownership flavours coexist:

- **Hardware-owned channels** are auto-created when a hardware-configuration choice fixes the role of pins — selecting a BOD-family daughter board on a connector says "these pins are block-occupancy detectors, full stop, they can't do anything else while this board is selected." Bowties creates one channel per configured pin with the role and style implied by the choice, with an auto-generated default name. The user can rename these channels for layout meaning; rename does **not** change ownership. When the hardware-config choice is cleared or changed, all of its hardware-owned channels disappear, including renamed ones. Any facility slot bound to a deleted channel becomes empty.
- **User-owned channels** are created on demand by user action (the facility slot's **Add channel** action picks the style and binding target as part of channel creation) and persist until the user removes them. In this slice, removing a user-owned channel from its only slot deletes it. Future scope adds ref-counting and delete-on-zero for channels shared across multiple slots, which is where a layout-organised Channels-by-name surface earns its keep alongside the hardware-organised Channels panel that ships first.

#### Three creation paths

A channel comes into existence through one of three paths. All three produce the same record shape and feed the same downstream machinery (display, constraint enforcement, event resolution).

| Path | Who authors what | When this applies |
|---|---|---|
| **Profile-pre-instantiated** (hardware-owned) | Profile ships fully-bound channels for fixed-function hardware; daughter-board selection materialises them with default names | BOD-4 in Tower-LCC Connector A produces 4 `block-occupancy` channels with style `bod-block-detector-input` at known field paths |
| **Slot-template** (user-owned) | Profile declares "from this slot you can create a channel with one of these styles"; user picks the style and binding at hardware-setup time as part of slot's Add channel | Direct Lamp Control rows on Tower-LCC (each row → `lamp-indicator` via `single-led-direct-lamp`); Signal LCC I/O lines that can become occupancy / button / lamp channels |
| **User-mapped** (user-owned, DIY) | User authors a channel style on an unprofiled board — picks the role, selects (or authors) a style shape from the system catalog, and binds its field roles to CDI fields by hand | DIY boards or firmware where a shipped profile doesn't exist |

The user-mapped path is what makes the channel model viable on boards without profiles. It does *not* introduce new roles — those stay in a closed system catalog. The user authors only the field signature for one specific channel style on one specific board.

#### Virtual Bindings: DCC Accessories

Not every channel is backed by a pin on an LCC board. **DCC accessories** — turnouts and stationary decoders controlled by DCC packets on the track bus — are addressed by a DCC accessory address (1–2044), not by a node and pin. Bowties supports them as first-class consumer channels through a **virtual binding** model that reuses the rest of the channel machinery unchanged.

**The mechanism.** The OpenLCB standard defines a well-known event range for DCC accessories: each DCC accessory address has a deterministic pair of LCC event IDs (one for each command state). Gateway hardware — JMRI's LCC↔DCC bridge, the TCS CS-105, and the TCS LT-50 — listens for those events on the LCC bus and emits the corresponding DCC accessory packet on the track. The events are the contract; the gateway is the wire-level translator.

**What the user sees.** In the Wiring workspace, DCC accessory channels live under a virtual **"DCC Layout"** node in the node tree — a single grouping for every DCC-addressed output in the layout, not tied to any physical LCC board. Adding one prompts for:

- **DCC address** (the only user-editable identity field)
- **Channel name** (e.g., "Yard Ladder T-15")
- **Style** (`dcc-accessory-turnout` or `dcc-accessory-signal`, when more than one applies)

The channel's **LCC event IDs are computed from the DCC address per the OpenLCB DCC accessory event allocation and shown as read-only values** in the channel detail view. The user never picks event IDs by hand for a DCC accessory channel — changing the DCC address re-derives them, and any event bowties wired to the channel re-resolve to the new IDs.

**How facilities and templates consume them.** A DCC accessory channel exposes a standard consumer role (`turnout-command`, `signal-aspect-2-color`, etc., depending on its style). Facility slots and behavior templates bind by role and never need to know that the binding is virtual — a CTC panel template applied to a DCC-controlled turnout produces the same bowties as it would for an LCC-controlled turnout, except that the consumer side of each bowtie uses the computed DCC-accessory event IDs and there is no consumer-side CDI write (no node to write to).

**What's missing on purpose.** Basic DCC accessories have no feedback. A DCC accessory channel is consumer-only; if the user wants position feedback they wire a separate producer channel (LCC microswitch, current sensor, or — once supported — a DCC + RailCom feedback channel) and the facility consumes both. The vision's directionality rule (one role per direction) handles this naturally.

**Why this fits.** Virtual bindings don't change the channel record shape or the facility slot model — only the binding payload differs (a DCC address instead of a node-scoped resource id like a pin or a Logic block). Style polymorphism already insulates facility slots from binding shape, so a `turnout-command` slot accepts a stall-motor channel, a servo channel, and a DCC accessory channel interchangeably. The gateway hardware is configured separately (in JMRI or via the gateway's own setup tool); Bowties does not own its configuration.

---

## Entry Points

Users arrive at Bowties from different starting positions. All paths converge on the same result: a layout with named channels organized into facilities.

### Start from Scratch (Primary)

New user, no hardware yet or hardware just purchased. Enters the Plan phase:
1. Layout Picker → "New Layout" → optional Planner Wizard
2. Planner creates prospective facilities and placeholder nodes from hardware recommendations
3. User pre-configures placeholders: daughter boards, channel names, templates — all before hardware arrives
4. User acquires boards → connects to bus → maps each physical node to its placeholder
5. Pre-staged configuration syncs to real hardware → facilities become active

### Start from Existing LCC Hardware

User has boards already installed and possibly partially configured (manually or via JMRI). Enters the Wiring phase:
1. Layout Picker → "New Layout" → connect to bus → discovery
2. Bowties reads existing configuration, identifies configured events
3. System suggests channels based on discovered configuration
4. User names/confirms channels → can organize into facilities or apply templates

### Import from JMRI (Transitional)

User has an existing JMRI setup with sensors, turnouts, and signal masts. May include Layout Editor topology:
1. Connect JMRI bridge → Bowties reads all JMRI objects
2. User adopts JMRI objects as channels (bulk or selective)
3. Channels carry protocol identity (LCC, LocoNet, DCC) and directionality
4. Optional: if Layout Editor panel exists, topology suggests facility groupings
5. User confirms/adjusts facility organization

This path serves existing LCC users migrating to Bowties. Expected to be a small subset of long-term users but important for early adoption.

### Future: Import from Track Planning Software

Users who designed their layout in XTrackCAD, AnyRail, SCARM, or similar tools have track geometry and turnout placement already defined. Importing this data could pre-populate the plan with track topology.

**This is out of scope for this document.** Acknowledged as a potential future entry point. In the near term, users can import track plans into JMRI and use the JMRI import path, or start from scratch in the Planner.

---

## Key Interaction Designs

### Template Apply Flow

The highest-stakes UX moment — where expert knowledge meets user intent.

**Structure:** Side panel workflow. The template details appear in a panel alongside the existing layout view so the user can see their channels and hardware while making mapping decisions.

**Steps:**

1. **Browse & Select** — Template library, filterable by category (signaling, detection, turnout control, panel). Each template shows: name, one-sentence purpose, required channel roles, required logic capacity.

2. **Requirements Check** — System shows each channel requirement with role-matched candidates from the layout. Color-coded: green (exact match available), yellow (compatible match), red (no match — not yet available).

3. **Channel Mapping** — For each requirement, user picks from candidates. If a matching channel doesn't exist yet, the user can:
   - **Create inline:** system knows the needed role → shows compatible pins across connected nodes → user picks → channel created without leaving the template flow.
   - **Leave unmapped:** mark the requirement as "pending" — the facility is created in an incomplete state. The user can return later to map this requirement after setting up the hardware.

4. **Logic Target** — User selects where decision logic runs: on-node (which node, which logic lines/STL slot) or JMRI LogixNG. System shows available capacity per node.

5. **Naming** — Template provides name patterns with substitution variables. User fills in location name (e.g., "Eagle Creek") and all generated artifacts get consistent names.

6. **Review & Apply** — Summary of what will be created: channels, bowties, CDI writes, logic programs, facility grouping. Unmapped requirements are clearly flagged as incomplete. "Apply" commits everything that's ready; incomplete requirements remain as pending slots.

7. **Result** — Facility appears in Railroad workspace with its lifecycle status reflecting completeness. Fully mapped = "Ready" or "Live". Partially mapped = "Setup incomplete (2/3 channels mapped)."

**Partial application is a first-class state.** A facility with unmapped requirements is valid — it represents "I know what behavior I want here, I just haven't finished the wiring yet." This supports the real workflow where you might decide on behavior before all hardware is installed.

**Channel remapping:** After a facility is created, any channel mapping can be changed. Open the facility → click a channel slot → pick a different channel of the same type (or remove the mapping). This supports: swapping a channel to a different pin/board, connecting a previously-unmapped requirement, or correcting a mistake. Remapping updates the underlying CDI writes and event wiring.

**Escape hatches:** At any step during initial apply, the user can go back. The system never blocks — it explains what's missing and offers paths to resolve (inline creation, leave pending, or navigate to Wiring workspace).

### Facility Comprehension View

The primary debugging and understanding surface.

**Layout:** A flow diagram or structured view showing:
- **Inputs** (left): channels feeding into the facility (occupancy sensors, position feedback, button presses)
- **Logic** (center): what decisions are being made, where they execute (on-node or JMRI), current evaluation state
- **Outputs** (right): channels the facility drives (signal aspects, turnout commands, lamp-indicator states)

**Live state:** Each channel shows its current state when connected (occupied/clear, red/yellow/green). State flows visually through the diagram so users can see "occupancy says clear, but signal shows red — the logic must be wrong" without understanding event IDs.

**Annotations:** Each element links to its physical backing (click an input to jump to its pin in the Wiring workspace) and to other facilities that share the channel.

**Facility status:** Simple badge showing lifecycle state:
- **Setup incomplete** — not all channel requirements are satisfied (e.g., "3/4 channels mapped"). Logic is not deployed to the target until all required inputs are mapped — the facility structure exists but the on-node or LogixNG logic is not programmed.
- **Ready** — fully configured, all channels mapped, logic programmed on the target
- **Live** — connected to bus and operational (when online)

### Planner Wizard

**Structure:** Full-page guided wizard (replaces main content area while active). Short, conversational steps. Visual progress indicator.

**Key design principles:**
- Every question uses railroad vocabulary, not LCC vocabulary. "How many track blocks do you want to detect trains in?" not "How many occupancy event pairs do you need?"
- Minimal questions — each one meaningfully changes the recommendation. No 50-question survey.
- Visual illustrations where possible — show a simple track diagram with the feature being asked about.
- Output is concrete and actionable: board names, quantities, and a brief rationale per recommendation.
- Output persists as the layout plan — user can revisit and modify as their thinking evolves.

### Wiring Workspace: Hardware Setup Flow

Channel creation happens differently depending on the board's hardware model. The profile declares which pattern applies.

**Tier 1 — Daughter board determines all channels (e.g., BOD-8 on TowerLCC):**
1. User selects a daughter board for a connector
2. System creates N channels of a single type (e.g., 8× occupancy)
3. Default names are sequential: "Connector A - Input 1" through "Input 8"
4. User renames to match track: "Block 5", "Block 6", "Block 7"...
5. If a plan exists, system suggests: "Your plan expects 'Eagle Creek East Approach' — assign to Input 1?"

**Tier 2 — Per-pin choice of role + style (e.g., Signal LCC, generic I/O):**

Channel creation supports multiple entry points — all converge on the same action (applying a role + style to a pin via the board profile):

- **Pin-first (bottom-up):** User clicks an unconfigured pin → profile shows which roles and styles this pin can host → user picks one → channel created. Best for: "I just wired a signal head to these pins."
- **Role-first (top-down):** User clicks "Add channel" on the node → picks a role from the profile's supported roles → system shows which styles realise that role on this board and which pins each can claim → user picks → channel created. Best for: "I need a signal output, where can I put it?"
- **Plan-driven (guided):** A plan requirement says "needs `signal-aspect-3-color` channel" → user clicks "fulfill" → system shows compatible pins (filtered by available styles on connected hardware) → user picks → channel created and auto-linked to the plan. Best for: users following Plan → Wire.

All three paths produce the same result: a named channel with role + style fixed and the style's constraint contract active.

**Multi-pin channels:** When a style claims multiple pins (e.g., a `3-led-direct-aspect` style for a 3-aspect signal using R/G/Y outputs), the profile declares the pin group size. After the user selects the first pin, the system asks the user to confirm which additional pins form the group. The resulting channel spans all constituent pins as a unit.

**Steps common to all entry points:**
1. Role and style are selected (determines managed-field constraints)
2. Pin(s) are assigned (the binding)
3. Style's constraint contract activates on the bound pins' managed fields
4. User adjusts unmanaged settings (polarity, debounce, etc.) as needed
5. User names the channel
6. Channel appears in both workspaces immediately

**Tier 3 — General-purpose hardware that behavior templates configure into channels with appropriate styles at apply time (e.g., TowerLCC logic lines):**
1. These don't materialise channels through manual pin setup
2. Behavior templates claim the underlying hardware capacity at apply time and create the channels with the right role and style as part of the apply
3. Visible in the Wiring workspace's Channels panel as "claimed by [facility name]" with a link to the facility

**Channel roles, styles, and board profiles are separate concepts:**

- A **channel role** (e.g., `signal-aspect-3-color`, `block-occupancy`, `lamp-indicator`) is an abstract definition: what states does it have, what information does it carry, what templates can consume it. Roles are board-independent — `signal-aspect-3-color` means the same thing regardless of whether it's on a Signal LCC or a TowerLCC with signal driver.

- A **channel style** is the specific hardware-shape realisation of a role on a specific subsystem — pins claimed, event-leaf mapping, CDI-field constraints. Multiple styles can realise the same role on different boards (or even on the same board with different wiring).

- A **board profile** declares which roles a board can host and which styles realise them, and maps each style to specific CDI fields. It says: "to create a `signal-aspect-3-color` channel on Signal LCC pin 3 via the `3-led-direct-aspect` style, set field X to value A, field Y to value B, and constrain field Z to options {C, D}." Different boards achieve the same role through different styles; different styles use different CDI field combinations.

This separation means roles can be defined once and reused across all boards that support them. Styles are the bridge — they say "this board can produce this role through this hardware shape, and here's how."

**Style as a constraint contract:** When a role + style is applied to a pin (through Tier 1 or Tier 2 selection), the style's constraint rules activate. Managed fields are locked to compatible values — the guided view only offers options that are valid for that style. Unmanaged fields (polarity, debounce, current limit) remain freely editable because they don't affect the channel's role or style identity.

A style's constraints fall into two natural tiers, surfaced differently in the editor:

| Layer | Example for `single-led-direct-lamp` on a Direct Lamp Control row | Editor presentation |
|---|---|---|
| **Shape / mode** — the CDI field that determines "what this hardware is right now" | `Lamp Selection = Direct Command` (locked when this style is active) | Primary managed field; presented first |
| **Leaf rules** — values under the established shape | `Output Function = Steady Active Hi` (locked); brightness, fade = unmanaged | Secondary managed fields + unmanaged fields below |

The shape constraint matters because it determines which other CDI fields are even relevant under the bound subtree. The relevance-rule machinery the profile system already uses for daughter-board selection extends naturally to this case.

This uses the same constraint mechanism that profiles already provide for relevance rules: one field's value narrows the valid options for other fields. The role + style is simply another controlling value in that system.

**Channel detail view zones.** The Wiring workspace's per-channel detail view is a profile-curated lens onto the bound subtree's CDI fields. Three zones, top to bottom:

1. **Identity.** Name, role, style, binding label.
2. **Unmanaged settings.** Freely-editable fields under the style's shape, presented as ordinary inputs.
3. **Managed settings.** Collapsed by default; expandable for inspection. Shows which fields the active style locked, with the rationale and a pointer to Raw CDI for override.

For multi-pin styles (e.g., a 3-LED-direct aspect style claiming three lamp rows) the detail view adds a per-component sub-row between identity and unmanaged settings. Each constituent has its own unmanaged fields scoped to that constituent (per-LED brightness, fade, effect). Channel-level unmanaged fields (anything that applies to the channel as a whole, e.g., a lamp-fade group setting) sit above the per-component section.

**Drift detection for external changes only:** Within Bowties' guided view, the constraint system prevents invalid states — users cannot select an incompatible value. Drift warnings appear only when an external tool (JMRI, raw CDI edit, another configuration utility) writes a value that violates the channel style's constraints. The warning identifies which values diverged and offers a one-action repair: "Restore to compatible settings for `3-led-direct-aspect`." This is always actionable and never the user's fault within Bowties' guided interface.

**Pin documentation:** Accessible from the Wiring workspace. Shows a formatted, printable view of a single node's complete pin/connector/channel assignments with facility membership. Designed to be physically printed and attached near the board.

---

## Placeholder Nodes: Pre-Arrival Configuration

Placeholder nodes are the bridge between planning and physical hardware. They exist today (implemented as read-only explorations of a board's profile), but in the vision they become **fully writable** — the primary way users pre-configure boards before they arrive.

**What a placeholder provides:**
- A full CDI-aware representation of a specific board model. The minimum requirement is the actual CDI XML (bundled from a reference read of that board model). The profile adds guided-configuration capabilities on top of the CDI, but does not supplant it — the CDI is the ground truth for what fields exist.
- Daughter board selection, pin mode configuration, and channel naming — identical to a connected node.
- Template application and facility membership — channels on placeholders are real layout channels.
- The same guided-configuration experience (configuration modes, relevance rules, field descriptions) as a connected node, when a profile exists. Without a profile, the placeholder still provides Raw CDI editing against the bundled CDI XML.

**What a placeholder does NOT do:**
- Write to the LCC bus (there's no physical hardware to receive writes).
- Show live state (no sensor readings, no signal feedback).

**Placeholder as board exploration:** Because a placeholder is a fully interactive representation of a board's configuration surface, it also serves as the way to explore what a board can do *before purchasing it*. A user curious about a TurnoutBoss adds a temporary placeholder, explores its configuration modes (Left/Right), sees what channels it would create, and deletes the placeholder if they decide against it. No separate "Profile Explorer" feature is needed — placeholders serve that purpose naturally.

**Reconciliation with real hardware:** When multiple nodes of the same model connect to the bus, the system cannot automatically determine which physical node corresponds to which placeholder. The user must explicitly map each: "This node (ID 05.01.01.01.3A.00) is Tower-3." Once mapped, all pre-staged configuration is synced down to the real node.

**Placeholder pins as the planning seam for channels.** Because every channel in the model always has a binding ([Channel Roles, Styles, and Bindings](#channel-roles-styles-and-bindings)), there are no logical, unbound channels to use as planning stand-ins. Placeholder nodes are the planning mechanism: a channel created against a placeholder is bound to one of the placeholder's pins (or Logic-blocks) and is indistinguishable in role/style/binding shape from a channel bound to a real node. When the placeholder is promoted to a real node, the channel's binding migrates to the real node's pin. This is what enables the "buy 3 more LED outputs for this aspect style" aggregate: the layout already has channels bound to placeholder-node pins, and the placeholder declares what hardware those pins represent.

---

## The Bowties Concept: Evolution

The "bowtie" — a named connection that ties producers to consumers through a central knot — is the defining pattern of the application. With channels and facilities, this pattern doesn't disappear; it repeats at a higher abstraction level.

**The bowtie pattern operates at two levels:**

- **Event bowtie (wire level):** One or more producer events → named connection → one or more consumer events. This is today's bowtie: a single named event pair connecting things that produce it to things that consume it. It remains the implementation mechanism for all event wiring.

- **Facility bowtie (behavior level):** Input channels → behavior logic → output channels. A facility has the same shape: multiple inputs converge through a behavior definition (the knot) and fan out to outputs. The facility comprehension view literally draws this bowtie shape (inputs → logic → outputs).

Each facility bowtie is *implemented by* multiple event bowties underneath. The layers nest: a user sees "Eagle Creek Signal Block" (facility bowtie), which is implemented by event bowties connecting occupancy sensors to logic inputs and logic outputs to signal drivers.

**User interaction shifts upward:** Primary users interact with facility bowties — they map channels, apply templates, and debug signal chains at the behavior level. Event bowties are the implementation substrate: always present, always correct, but not the starting point for understanding.

**Visibility:** The raw event-bowtie view (today's Bowties tab) becomes an advanced detail view accessible within the Railroad workspace. Power users and debuggers drill into it when they need to see actual event IDs and producer/consumer relationships below the channel abstraction. It's not removed — it's repositioned as a drill-down for the wire level.

**The app name "Bowties" is strengthened by this evolution.** The name doesn't describe a single feature that got superseded — it describes the fundamental architectural pattern that repeats at every level. Tying things together through a named central knot is what the app does, whether you're connecting events or connecting channels through behavior logic.

---

## JMRI Integration Experience

### Connection Setup

Simple, one-time configuration: "Connect to JMRI" → localhost + port → connected. Visual indicator in toolbar showing JMRI bridge status (disconnected / connected / syncing).

### Automatic Projection

When the bridge is connected, channel and facility changes in Bowties automatically sync to JMRI objects. The user never manually creates sensors, turnouts, or signal masts in JMRI — they appear from Bowties work. This eliminates one of the most tedious steps in current LCC+JMRI workflows.

### JMRI as the Operations Layer

The vision explicitly defers panels, throttles, and runtime control to JMRI. Bowties owns configuration and comprehension; JMRI owns operation and display. The bridge keeps them in sync.

**The user's workflow for a working layout:**
1. Configure everything in Bowties (Plan → Wire → Railroad)
2. JMRI objects appear automatically via bridge
3. Draw Layout Editor panel in JMRI (pick from dropdown lists of already-configured objects)
4. Click "Auto-discover Signal Mast Logic" in JMRI
5. Run trains

Steps 3–5 are pure JMRI. Bowties made steps 1–2 accessible to non-engineers. Step 3 became trivial because all objects are already named and configured.

### Debugging Across the Boundary

When something goes wrong during operation, the user returns to Bowties for comprehension:
- Open the misbehaving facility
- See live channel states (fed from the bus, or from JMRI via bridge for non-LCC channels)
- Identify which input is unexpected or which logic isn't producing the right output
- If a channel is backed by JMRI (non-LCC protocol), Bowties shows its current state as reported by the bridge

### DCC Accessory Gateways

DCC accessory channels (see [Virtual Bindings: DCC Accessories](#virtual-bindings-dcc-accessories)) reach the track through a gateway that converts the well-known LCC events into DCC accessory packets. Three gateways are recognized:

- **JMRI** — when the JMRI bridge is connected, JMRI's own LCC↔DCC translation can serve as the gateway. No extra setup beyond the bridge itself.
- **TCS CS-105** (command station) and **TCS LT-50** (LCC-to-DCC translator) — listen on the LCC bus and emit DCC accessory packets directly, with no computer required.

Bowties does not own the gateway's configuration. From Bowties' point of view the gateway is implicit: emit the right LCC event on the bus and the gateway does its job. The Wiring workspace surfaces gateway status as a layout-level indicator (which gateway is present, whether it's reachable) so the user has somewhere to look when a DCC accessory channel isn't taking effect on the track.

---

## Visual Design Direction

The vision introduces richer UX patterns where they deliver understanding, while keeping the rest of the application consistent with its current utilitarian style.

**Where to be richer:**
- Facility comprehension view: signal-chain flow diagram with live state
- Template apply flow: visual mapping between requirements and candidates
- Planner wizard: illustrated questions, visual layout concepts
- Plan progress: visual lifecycle indicators on facilities

**Where to stay utilitarian:**
- Wiring workspace: tree + detail panel (evolution of today's Config)
- Channel lists and property editors
- Connection management
- Toolbar and navigation chrome

**Progressive disclosure:** The application defaults to showing the most meaningful abstraction layer (facilities, channels) and allows drilling into detail (CDI fields, raw bowties, event IDs) on demand. Non-engineers rarely drill down; power users drill down when needed.

---

## Implementation Horizons

The vision is the north star. Implementation proceeds in phases that each deliver standalone value.

### Now (Behavior Templates spec — 015)

- Information channel abstraction and auto-creation from hardware selection
- Channel naming and persistence
- Behavior template format and library
- Template apply workflow
- Facility grouping and basic facility view
- Wiring workspace as evolution of current Config tab
- Railroad workspace as new facility-centric view

### Next

- JMRI bridge (sync channels → JMRI objects)
- Facility comprehension view with live state
- Plan progress tracking (lifecycle badges on facilities)
- Pin documentation generation

### Later

- Hardware planner wizard
- Template capture from working configurations
- LogixNG as logic execution target (via bridge)
- Channel connectivity / topology declarations
- JMRI Layout Editor topology import (via bridge)
- Layout-wide health and completeness analysis

### Future (out of scope for this document)

- Native Bowties layout editor / track diagram
- Track planning software import (XTrackCAD, AnyRail, etc.)
- Community template sharing platform
- CTC panel rendering in Bowties

---

## Resolved Design Questions

1. **Channel-to-facility relationship when no template is used** — Facilities are template-first: every facility originates from applying a behavior template. For power users with existing configurations, a future workflow will allow attaching a template to already-configured elements, mapping existing CDI settings into a facility structure. This path prioritizes the primary user (non-technical, starting fresh with templates) while keeping the advanced path possible — it just requires more manual mapping.

2. **Slot empty-state hint vocabulary is role-based, not hardware-named** — When a facility slot's Select and Add actions are both unavailable (no compatible channel exists and no hardware can create one), the hint text the slot surfaces describes the missing **role** (e.g., "no `lamp-indicator` channels available"), not a specific node family or board model. Naming specific hardware ("connect a Signal LCC") looks helpful in early slices with one or two hardware types but stops scaling the moment a layout supports many node families: there's no general way to enumerate "which of your ten boards could provide this role." Role-based vocabulary scales with the role catalog (small, slow-growing) rather than with the hardware catalog (large, growing per board). Per-style or per-board suggestions are a future UX exploration once usage data clarifies what actually helps users get unstuck.
