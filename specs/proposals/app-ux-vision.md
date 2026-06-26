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

**Sidebar:** Node tree (similar to today's Config sidebar). Nodes expand to show connectors, assigned daughter boards, a "Settings" entry for board-global configuration, and a "Raw CDI" drill-down.

**Main area — two presentation modes:**

1. **Guided view** (default, when a profile exists) — organized by purpose:
   - **Channels** — each pin/group that produces layout-wide information (occupancy, signal aspect, turnout position) is shown as a named channel. Per-channel, the view shows:
     - *Unmanaged settings* (shown by default) — per-pin configuration that the user freely adjusts without affecting channel type: active high/low, debounce time, current limit, common anode/cathode. These are the wiring decisions.
     - *Managed settings* (collapsed, expandable) — fields whose values are determined by the channel type. Shown with a clear "managed by channel type" annotation. The channel type *constrains* these fields to compatible values — incompatible options are not offered. Expanding this section lets the user see what was set and why.
   - **Board-global settings** — configuration that isn't tied to any pin or channel: SNIP name, network config, Blue/Gold enable, DCC output mode, throttle behavior. Presented with profile-guided descriptions.
   - **Multi-pin channels** — channels that span multiple physical pins (e.g., a 3-aspect signal using R/G/Y outputs) group their constituent pins together. Each pin has its own unmanaged settings; managed settings span the group.

2. **Raw CDI** — full field-by-field view for power users who need direct access to any CDI field. The escape hatch, not the starting point. Bypasses all profile-level constraints.

**Key difference from today's Config tab:** The guided view organizes fields by what they *do* (channel-producing vs. board-global) and uses the profile's constraint system to prevent invalid combinations. Users see only compatible options for channel-managed fields. Raw CDI remains available for overriding anything.

**Boards without profiles** fall back to the raw CDI view — the same experience as today's Config tab. Profiles are what enable the guided view.

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
| **Wiring** | "Pin 3 on Connector A of Tower-3" — shows what hardware backs the channel |
| **Railroad** | "Block 7 Occupancy, part of Eagle Creek Signal Block" — shows what the channel means |

A dedicated **Channels view** (within the Railroad workspace) provides a flat or type-grouped list of all channels across the layout. This serves as the inventory: what information does my layout produce and consume? Useful for planning ("I have occupancy on 8 blocks but signals on only 3") and for the template apply workflow ("which channels should I map to this template requirement?").

### Channel Connectivity and Topology

Channels can be **connected** to express physical topology: "Block 7 is adjacent to Block 8", "this turnout protects this signal." Connectivity enables:
- **Auto-wiring in behavior templates:** a template can infer that "Block 8 Occupancy" is the next-block input for Block 7's signal logic, instead of asking the user to pick it manually.
- **Gap analysis:** "Block 7 has occupancy but no signal assigned" or "this turnout has no position feedback."

Topology can come from manual declaration in Bowties, import from JMRI's Layout Editor (where block connectivity is encoded in the panel drawing), or a future native track editor.

**Note:** Topology is a later-horizon feature. Until topology is available, template application requires the user to manually map each channel input (e.g., selecting which occupancy channel is the "next block ahead"). This is more explicit than the auto-wiring mockups suggest, but remains straightforward — the user picks from a filtered list of compatible channels.

### Channel Types and Directionality

A channel is always either a **producer** (generates information) or a **consumer** (acts on information), never both. When physical hardware does both — a turnout motor that drives position and reports it — those are modeled as two separate channels that may share the same device. This maps directly to the facility comprehension view: producer channels appear as inputs (left), consumer channels as outputs (right).

Channel types define the information semantics, not just pin assignments. A channel type determines which events are meaningful and which are implementation detail. For example, a two-button channel cares about "pressed" events; "released" events are filtered out at the channel level.

The following are representative types, not an exhaustive list. Profiles declare which types a board supports.

**Producer channels** — generate information about the layout:

| Type | Pins | Information produced |
|---|---|---|
| **Block Occupancy** | 1 | Occupied / clear for a detection block |
| **Turnout Position Feedback** | 1–2 | Normal / diverging position report |
| **Two-Button Command** | 2 (one per direction) | Pressed events only — commands intent (e.g., "throw turnout left"). Released events are filtered |
| **Single Button** | 1 | Pressed event — a momentary command or toggle |
| **Current Sensor** | 1 | Analog or threshold-based current reading |

**Consumer channels** — act on information to drive physical outputs:

| Type | Pins | Information consumed |
|---|---|---|
| **Signal Aspect** | 2–3 (R/G/Y outputs) | Aspect value → drives the corresponding LED combination |
| **Turnout Motor Command** | 1–2 | Normal/diverging command → drives servo or stall motor |
| **LED Indicator** | 1 | On/off or color state → drives a single LED |
| **Turnout Position Indicator** | 2 (LEDs) | Normal/diverging state → drives a pair of LEDs to show position on a fascia panel |

**Key property:** Multiple channels of the same type can feed the same facility input. Two button-pair channels on opposite sides of a layout module — physically independent, on different nodes — can both serve as "turnout command" inputs to the same turnout facility. The facility is what unifies them logically; the channels remain physically grounded in their respective pin groups.

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

1. **Browse & Select** — Template library, filterable by category (signaling, detection, turnout control, panel). Each template shows: name, one-sentence purpose, required channel types, required logic capacity.

2. **Requirements Check** — System shows each channel requirement with type-matched candidates from the layout. Color-coded: green (exact match available), yellow (compatible match), red (no match — not yet available).

3. **Channel Mapping** — For each requirement, user picks from candidates. If a matching channel doesn't exist yet, the user can:
   - **Create inline:** system knows the needed type → shows compatible pins across connected nodes → user picks → channel created without leaving the template flow.
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
- **Outputs** (right): channels the facility drives (signal aspects, turnout commands, LED states)

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

**Tier 2 — Per-pin mode determines channel type (e.g., Signal LCC, generic I/O):**

Channel creation supports multiple entry points — all converge on the same action (applying a channel type to a pin via the board profile):

- **Pin-first (bottom-up):** User clicks an unconfigured pin → profile shows which channel types this pin can become → user picks one → channel created. Best for: "I just wired a signal head to these pins."
- **Type-first (top-down):** User clicks "Add channel" on the node → picks a channel type from the profile's supported types → system shows which pins can host it → user picks → channel created. Best for: "I need a signal output, where can I put it?"
- **Plan-driven (guided):** A plan requirement says "needs 3-aspect signal output" → user clicks "fulfill" → system shows compatible pins → user picks → channel created and auto-linked to the plan. Best for: users following Plan → Wire.

All three paths produce the same result: a named channel with its type constraints active.

**Multi-pin channels:** When a channel type requires multiple pins (e.g., a 3-aspect signal using R/G/Y outputs), the profile declares the pin group size. After the user selects the first pin, the system asks the user to confirm which additional pins form the group. The resulting channel spans all constituent pins as a unit.

**Steps common to all entry points:**
1. Channel type is selected (determines managed field constraints)
2. Pin(s) are assigned
3. Board profile activates constraints on managed fields
4. User adjusts unmanaged settings (polarity, debounce, etc.) as needed
5. User names the channel
6. Channel appears in both workspaces immediately

**Tier 3 — General-purpose resources configured by behavior templates (e.g., TowerLCC logic lines):**
1. These don't create channels through manual pin setup
2. Behavior templates claim and configure them at apply time
3. Visible in the Wiring workspace as "claimed by [facility name]" with a link to the facility

**Channel types and board profiles are separate concepts:**

- A **channel type** (e.g., "3-aspect signal output", "block occupancy", "button input") is an abstract definition: what states does it have, what information does it carry, what templates can consume it. Channel types are board-independent — "3-aspect signal output" means the same thing regardless of whether it's on a Signal LCC or a TowerLCC with signal driver.

- A **board profile** maps channel types to specific CDI fields on a specific board model. It declares: "to create a 3-aspect signal output on Signal LCC pin 3, set field X to value A, field Y to value B, and constrain field Z to options {C, D}." Different boards achieve the same channel type through different CDI field combinations.

This separation means channel types can be defined once and reused across all boards that support them. Board profiles are the bridge — they say "this board can produce these channel types, and here's how."

**Channel type as a constraint contract:** When a channel type is applied to a pin (through Tier 1 or Tier 2 selection), the board profile's constraint rules activate. Managed fields are locked to compatible values — the guided view only offers options that are valid for that channel type. Unmanaged fields (polarity, debounce, current limit) remain freely editable because they don't affect the channel's type identity.

This uses the same constraint mechanism that profiles already provide for relevance rules: one field's value narrows the valid options for other fields. Channel type is simply another controlling value in that system.

**Drift detection for external changes only:** Within Bowties' guided view, the constraint system prevents invalid states — users cannot select an incompatible value. Drift warnings appear only when an external tool (JMRI, raw CDI edit, another configuration utility) writes a value that violates the channel type's constraints. The warning identifies which values diverged and offers a one-action repair: "Restore to compatible settings for 3-aspect signal output." This is always actionable and never the user's fault within Bowties' guided interface.

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
