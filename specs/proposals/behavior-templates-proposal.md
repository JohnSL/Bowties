# Proposal: Behavior Templates & Information Channels

**Status:** Draft proposal — brainstorm capture for community feedback.  
**Origin:** LayoutCommandControl groups.io thread "Node templates?" (June 2026) — discussion about reusable configuration templates for LCC nodes. Full brainstorm with AI assistant, June 21–22, 2026.

---

## Problem

Configuring LCC nodes for common railroad behaviors (ABS signal blocks, passing sidings, CTC-controlled turnouts) currently requires deep knowledge of event IDs, CDI field settings, and cross-node wiring. Users must either:

1. Manually configure each field and event from scratch, or
2. Clone a JMRI backup file and hand-edit node IDs — a fragile approach that only works for single-node scenarios and breaks cross-node event relationships.

Existing "clone" tools (e.g., `lcc-cdi-clone`) operate on individual backup files with no visibility into the broader layout topology. As noted in the community discussion, simple search-and-replace of event IDs severs or corrupts cross-node links as soon as more than one device is involved.

---

## Key Insight: Bowties Sees the Full Topology

Unlike standalone tools, Bowties has access to:

- Every node on the bus and their CDI structure
- Every configured event and who produces/consumes it
- Named events (bowties) with human-readable purpose
- Which nodes share events — the cross-node topology is already resolved
- What's unallocated — which I/O lines and event ranges are free
- Profile metadata — event roles, daughter board constraints, relevance rules

This transforms a search/reconciliation problem into a lookup. Bowties can apply templates **correctly** because it sees both sides of every connection.

---

## Core Concepts

### Information Channels

An **information channel** represents a single piece of meaningful information (e.g., "block occupancy", "turnout position") independent of protocol or wiring. Each channel:

- Has a **type** (from a well-known set or user-defined)
- Has **states** (e.g., occupied/clear, normal/diverging, red/yellow/green)
- Has a **backing implementation** — LCC event ID pairs (bowties), DCC addresses via JMRI, LocoNet messages, or other protocols
- Has **directionality** — full bidirectional (LCC, LocoNet), command-only (basic DCC), or mixed (DCC command + separate LCC feedback sensor)
- Reduces cognitive load: users think "occupancy" once; the system handles protocol-specific details

Channels are **protocol-agnostic at the abstraction level.** A "Block 7 Occupancy" channel works the same way in templates and facilities regardless of whether it's backed by an LCC BOD, a LocoNet BDL168, or any other detector. The protocol and directionality are implementation properties, not identity.

**Examples of well-known channel types:**
- Block Occupancy (binary: occupied / clear)
- Turnout Position (binary: normal / diverging)
- Turnout Command (binary: throw normal / throw diverging)
- Signal Aspect (enum: red / yellow / green / dark / flashing variants)
- Button Press (binary: pressed / released)
- LED State (binary: on / off)
- Route Request / Route Active

Each well-known type defines: state count, state names, default naming pattern, and compatible pin/slot types.

User-defined types handle anything custom or board-specific.

### Facilities

A **facility** is a named, live instance of applied behavior — the result of applying one or more templates to specific hardware. Examples: "Eagle Creek Siding", "Mainline Block 7", "CTC Panel — East End."

Facilities provide:
- A higher-level grouping than bowties for understanding layout configuration
- Traceability: which information channels were created by which facility
- Navigation: see all the resources, channels, and nodes involved in a functional unit
- Future: association with physical locations on a track diagram

### Two Types of Template

There are two distinct template types that address different concerns:

#### Hardware Templates — "What's physically attached?"

A **hardware template** declares the physical configuration of a board's connectors and pins. It sets daughter board selections, pin modes, and electrical parameters. Applying a hardware template triggers automatic creation of typed information channels.

A hardware template is **board-specific** (bound to a model + firmware version). It defines:
1. **Metadata** — name, description, target board model/firmware
2. **Connector/pin assignments** — which daughter boards or pin modes to set
3. **Configuration values** — CDI field settings for the hardware configuration
4. **Channel auto-creation rules** — what information channels result from this hardware choice

Hardware templates answer: "What can these connectors/pins do?"

**Example:** "TowerLCC — BOD-8 on Connector A + Signal Driver on Connector B" → applies daughter board settings and creates 8 occupancy channels + signal output channels with default names.

Note: in many cases, hardware setup is simply a manual step (selecting a daughter board in the configuration UI). Hardware templates are an optional convenience for common configurations, not a required step.

#### Behavior Templates — "What should these channels accomplish?"

A **behavior template** captures a reusable behavior pattern — how information channels cooperate to achieve a functional goal. It operates on information channels that **already exist** (created by hardware setup, hardware templates, or prior behavior templates). A behavior template is independent of specific nodes or physical layout.

A behavior template defines:
1. **Metadata** — name, description, behavior category
2. **Channel requirements** — typed information channels it needs (e.g., "1× block-occupancy input", "1× signal-aspect output")
3. **Logic requirements** — decision-making capacity needed, expressed abstractly (e.g., "signal aspect logic for 3-block ABS")
4. **Logic programming** — the decision rules connecting input channels to output channels, expressed in an execution-target-agnostic form that can be compiled to on-node logic (TowerLCC logic lines, STL programs) or JMRI LogixNG conditionals
5. **Name templates** — patterns with substitution variables (e.g., `"${location} - Occupied"`)
6. **Board-specific bindings** (optional) — for Tier 3 general-purpose pins where the template also needs to set CDI values to achieve the behavior

Behavior templates answer: "How do these pieces cooperate to achieve a goal?"

A single behavior template can support multiple board types via board-specific bindings. Since hardware is already set up when the behavior template is applied, the template uses the appropriate bindings for the boards that are present.

**Example:** "ABS Signal Block" — requires 1× block-occupancy channel (input), 1× next-block-occupancy channel (input), 1× signal-aspect channel (output), plus logic capacity to determine signal aspect from occupancy. The logic can execute on-node (TowerLCC logic lines or STL program) or in JMRI (LogixNG conditional). The template encodes the actual signal engineering rules once; the apply engine targets the user's chosen execution platform.

#### The Separation

Hardware configuration and behavior composition are separate concerns:

| Layer | What it does | Creates | Answers |
|-------|-------------|---------|---------|
| **Hardware setup** (manual or template) | Declares physical reality | Typed information channels | "What can this pin do?" |
| **Behavior template** | Composes channels into functional patterns via logic | Facilities, intra-facility wiring, logic programs | "What should these channels accomplish?" |

A behavior template never says "you need a BOD-8." It says "I need a block-occupancy channel." How that channel came to exist — daughter board auto-creation, per-pin selection, or manual setup — is not the behavior template's concern.

If a behavior template's requirements aren't met (e.g., no block-occupancy channel exists yet), the system guides the user to set up hardware first.

### The Abstraction Hierarchy

```
Facility (behavior instance — "Eagle Creek Siding")
  ├── Information Channel (meaning — "Eagle Creek Occupancy")
  │    ├── Backing: LCC event pair (bowtie per state)
  │    │    └── Slot (one pin/field on one node participating in that event)
  │    ├── Backing: JMRI object (sensor/turnout/mast of any protocol)
  │    └── Properties: directionality, signal system, connectivity
  └── Logic (decision rules — "if next block occupied → show yellow")
       ├── On-node: Logic block / STL program on a specific node
       └── JMRI: LogixNG conditional (for mixed-protocol or computer-hosted logic)
```

Users think and work at the facility and channel levels. Logic, bowties, slots, and protocol details are the implementation layer — still essential but not the primary interaction surface for common tasks.

### Channel Connectivity and Topology

Information channels can be **connected** to express physical topology:
- "Block 7 Occupancy" connects-to "Block 8 Occupancy" (adjacent blocks)
- "East Turnout Position" protects "Block 7 Signal Aspect" (signal protection)
- "Block 7 Occupancy" feeds "Block 7 Signal Logic" (input relationship)

This connectivity enables:
- **Auto-wiring in templates:** When topology is known, the template can infer relationships instead of asking the user to pick each one manually. "Block 8 Occupancy is the 'next block' input for Block 7's signal logic" is automatic if the system knows they're adjacent.
- **Gap analysis:** "Block 7 has occupancy detection but no signal assigned" or "This turnout has no position feedback — signal logic using it is unreliable."
- **Directionality warnings:** "This turnout is DCC (command-only) — signal logic that reads its position depends on a separate feedback sensor. None exists."

Topology can come from:
- **Manual specification** in Bowties (user declares adjacency)
- **Import from JMRI's Layout Editor** (where block connectivity is already encoded in the panel drawing)
- **Future Bowties layout editor** (draws topology natively with channels integrated)

---

## Design Principles

### Templates Capture Behavior, Not Node State

A behavior template is NOT "clone this node." It's "here's how to implement a passing siding" — independent of which physical nodes host the behavior. The same template can be deployed across one node or split across many, depending on physical layout. It doesn't care what hardware created the channels — only that channels of the right type exist.

### Templates Claim Resources; Nodes Provide Them

A node is a shared resource pool. Multiple templates can compose onto the same node without conflict, each claiming different resources. A single TowerLCC might host parts of three different facilities, each using different lines/connectors.

### Information Channels Exist Independently

Information channels are shared infrastructure. A facility connects to them; it doesn't exclusively own them. "Block 6 Occupied" can be:
- Produced by the Block 6 facility (detection)
- Consumed by the Block 6 facility (its own signal logic)
- Consumed by the Block 5 facility (approach aspect calculation)
- Consumed by the CTC Panel facility (panel indicator)

Multiple producers are also valid: a turnout command channel can have producers from a local push button, a CTC panel, and route automation logic — all different facilities.

### Templates Compose

Behavior templates compose at different scales:

- **Single-concern** — "ABS Signal Block" — one behavior, creates one facility
- **Multi-concern** — "Passing Siding with ABS" — combines detection + turnout + signaling into one facility
- **Cross-facility** — "CTC Panel Overlay for 3 Blocks" — creates a new facility that connects to existing channels produced by other facilities

At each channel reference, the behavior template can:
- **Connect to** an existing channel (user picks from what's in the layout)
- **Create** a new channel (user provides naming variables)
- **Create or connect** — system asks which, enabling incremental build-up

### Hardware First, Then Behavior

The natural workflow is: set up hardware (creating typed channels), then apply behavior templates (composing those channels into functional patterns). This matches physical reality — you install boards before you program them.

However, a user can also start from intent: browse the behavior template library to see "what's possible," then work backward to determine what hardware they need. The template's channel requirements tell them what hardware capabilities are necessary. This supports the "planning" use case without conflicting with the "hardware first" execution order.

When a behavior template needs to set Tier 3 CDI values (logic line configuration, general-purpose pin modes), it can include bindings for multiple board types — so the same behavior template works whether the user has a TowerLCC or a TowerLCC+Q.

### Logic Blocks as Template Resources

Some nodes provide internal logic capabilities that enable complex behavior without external software:

- **TowerLCC** — Logic blocks (fixed-function logic elements configured via CDI)
- **TowerLCC+Q** — Programmable logic via STL (Structured Text Language, a PLC-based language)
- Other boards may offer simpler conditional logic (e.g., "if input A, produce event B")

These logic blocks are the critical piece that transforms raw I/O and information channels into real railroad behavior. Without them, you can detect that a block is occupied and drive a signal lamp — but you can't implement "show yellow when the next block is occupied and this block is clear" or "lock turnout when route is set and block is occupied."

#### Why Logic Matters for Templates

For real ABS, CTC, interlocking, and approach lighting, the behavior template must include logic programming as part of what it applies. This is where behavior templates deliver the most value: the logic is where cognitive load is highest today, and where expert knowledge is most concentrated.

A behavior template for "ABS 3-Aspect Signal Block" needs to:
1. Connect to input channels (this block occupancy, next block occupancy)
2. Connect to output channels (signal aspect)
3. **Program the logic** that determines signal state from the inputs

Without #3, the template is just wiring — the user still has to figure out the hard part. With #3, the template encodes the actual railroad engineering.

#### Logic as a Typed Resource

Logic blocks fit into the resource model:

| Resource type | Example | How templates use it |
|------|------|------|
| I/O pin | Connector A pin 3 | Produces/consumes events |
| Logic block (fixed) | TowerLCC Logic Line 5 | Evaluates conditions, triggers outputs |
| Logic program (STL) | TowerLCC+Q STL slot | Executes programmed behavior |
| JMRI LogixNG conditional | LogixNG conditional tree | Evaluates conditions, controls JMRI objects |

A behavior template's requirements would include logic resources:
- "Requires: signal logic capacity (2× logic lines, or 1× STL slot, or LogixNG conditional)"

At apply time, the user chooses the **execution target**:
- **On-node (TowerLCC logic lines):** "Use logic lines 3-4 on Tower-3" — works without a computer running, lowest latency
- **On-node (STL program):** "Use STL slot on Tower-3" — more complex logic, still computer-independent
- **JMRI LogixNG:** "Run in JMRI" — required for mixed-protocol layouts (logic that reads LocoNet sensors and commands DCC turnouts), requires computer running

The choice of execution target depends on the user's hardware, protocol mix, and reliability requirements. Templates encode the logic once; compilation to the target platform is handled by the apply engine.

#### Logic Programs in Templates

A behavior template carries logic in an **abstract decision-rule form**:
- Input channels (conditions to evaluate)
- Output channels (actions to take)
- Decision rules (if/then/else logic connecting inputs to outputs)

The apply engine **compiles** this to the chosen execution target:
- **TowerLCC logic lines:** CDI field values that configure fixed-function logic elements
- **TowerLCC+Q STL:** Program text with event IDs substituted at apply time
- **JMRI LogixNG:** Conditional expression tree in LogixNG's structure, pushed via the JMRI bridge

This means a template author writes the signal engineering rules once; every user who applies the template gets a working program customized to their specific channels and target platform — without ever reading the STL language reference or understanding LogixNG's expression syntax.

For boards with on-node logic, the template can also carry **board-specific bindings** — pre-compiled logic configurations for specific firmware versions when the abstract compilation isn't sufficient.

#### Taming Cognitive Load

Today: "Read the STL manual, understand PLC logic, figure out which events map to which variables, write and debug the program." Or: "Open JMRI LogixNG, figure out the expression tree, manually connect sensors and signal masts, hope you got the aspect rules right."

With templates: "Apply 'ABS 3-Aspect Signal Block', select your occupancy channels and signal heads, pick whether logic runs on-node or in JMRI, done."

The expert's knowledge (signal engineering + platform-specific programming) gets captured once and reused many times. This is where the payoff is highest — not in simple I/O wiring, but in the logic layer that makes complex behaviors work. It applies equally whether the user has an all-LCC layout or a mixed DCC/LocoNet/LCC layout where logic must run in JMRI.

---

## Hardware Integration

Hardware setup (whether manual or via hardware template) is the foundation that creates information channels. Behavior templates operate on top of this foundation.

### Three Tiers of Pin Configuration

| Tier | Hardware example | Channel creation | User effort |
|------|-----------------|-----------------|-------------|
| **Fully determined** | BOD-8 daughter board, standalone BOD node | Auto-create on hardware selection | Name them |
| **Small choice set** | Per-pin mode selector (button / LED / combo) | Present short picker, then auto-create | Pick mode + name |
| **General purpose** | TowerLCC logic lines, wide I/O matrices | Behavior templates configure these | Apply behavior template or configure manually |

Tier 1 and 2 are handled at hardware setup time — channels appear automatically once the physical configuration is declared. Tier 3 is where behavior templates provide the most value, encoding expert knowledge about which combination of settings achieves a specific behavior.

### Constraint Granularity

The granularity of hardware constraint varies by board:

- **Board-level** (BOD-8): selecting the daughter board determines all pins at once
- **Per-pin** (SignalLCC): each pin independently selects its mode/type

Profiles already encode this distinction via CDI structure and relevance rules.

### Auto-Creation from Hardware Selection

When a daughter board is assigned (manually or via hardware template), e.g., BOD-8 to Connector A:
- All pins have known, fixed function → immediately create information channels with default names
- User's next step is just renaming: "which blocks are these?"
- This bootstraps the information model from the hardware choice alone
- These channels are now available for behavior templates to connect to

If the board type is changed:
- Bowties warns about affected channels and facilities
- User confirms or cancels
- Affected facilities flagged as incomplete if confirmed

### Profile Integration

Profiles encode which pin configurations produce which channel types. The daughter board selection field in the CDI, combined with profile relevance rules, tells Bowties:
- What channel types each pin can support
- Whether constraint is board-level (Tier 1) or per-pin (Tier 2)
- What CDI field values correspond to each channel type

### The Typical Setup Sequence

```
1. Hardware setup (manual or via hardware template)
   → Select daughter boards, pin modes
   → Information channels auto-created and typed
   → User names them ("Block 7 Occupancy", "Block 8 Occupancy"...)

2. Behavior template applied
   → "I want ABS signaling for these blocks"
   → Picks from existing channels: "which occupancy?", "which signal heads?"
   → Assigns logic resources: "use logic lines 3-4 on Tower-3"
   → Programs the logic blocks / STL with the signal engineering rules
   → Creates a facility grouping
```

The two layers are independent: hardware setup can happen without ever applying a behavior template (for users who prefer manual configuration), and behavior templates can be applied at any later time once the necessary channels exist.

---

## Workflows

### Apply a Hardware Template (or Manual Hardware Setup)

1. Select a node and connector/pin
2. Choose daughter board or pin mode (manually, or by picking a hardware template)
3. System auto-creates typed information channels with default names
4. User renames channels to match their layout ("Block 7 Occupancy", "East Signal Head")
5. Channels are now available for behavior templates

### Apply a Behavior Template

1. Browse template library → pick by intent/behavior category
2. Template shows what it needs (typed channel requirements)
3. For each required channel:
   - Pick from existing layout channels that match the type, OR
   - Create a new one (guided to set up hardware first if needed)
4. User provides substitution variables for naming (e.g., `location = "Eagle Creek"`)
5. User selects target node(s) for logic resources — assigns logic lines or STL program slots
6. Bowties writes configuration: sets CDI values, programs logic blocks/STL, creates information channels and bowties, applies facility grouping
7. Optionally: validates result — "here's the event flow, does this look right?"

### Capture a Behavior Template from a Working Layout

1. User identifies the behavior scope — manually select channels/bowties, or use a grouping to identify them
2. Bowties reads the current config for all involved channels and resources
3. Classifies each channel:
   - Channels within the selection whose connections are all internal → intra-template (auto-wired at apply time)
   - Channels that connect to things outside the selection → become parameters (requirements)
4. User reviews and names parameters, defines name-template variables
5. Non-event fields: user classifies as fixed, parameterized (user picks at apply time), or defaulted
6. Edits available: remove a slot, mark optional, adjust defaults
7. Save as template file (part of installation library or user-custom)

### Incremental Build-Up

Templates support layered application:
1. Set up hardware → occupancy channels exist on 10 blocks
2. Apply "ABS Signal Block" behavior template to each → finds existing occupancy channels, adds signaling
3. Later: apply "CTC Panel" behavior template → connects to existing signal and occupancy channels

Each step adds without disturbing what's already there. Multiple behavior templates can compose onto the same nodes without conflict, each claiming different resources.

---

## Template Distribution

- **Shipped templates** — curated, tested, bundled with Bowties installation (both hardware and behavior templates)
- **User-created templates** — captured from working layouts, stored locally
- **Shared templates** — exported as files for sharing with other users (future community library potential)

Hardware templates are board-specific by nature. Behavior templates declare channel requirements by type and are hardware-agnostic in their core logic, with optional board-specific bindings for Tier 3 configurations.

---

## Relationship to Existing Bowties Features

| Feature | Relationship |
|---------|-------------|
| **Bowties (named events)** | Information channels group bowties into higher-level meaning |
| **Profiles** | Provide the data for pin type classification, relevance rules, and channel type derivation |
| **Event tags** | Could represent facility membership or logical grouping |
| **Daughter board selection** | Triggers auto-creation of information channels (Tier 1 and 2) |
| **Logic blocks / STL** | Behavior templates program these to implement decision rules |
| **Backup/Restore** | Existing config read/write infrastructure supports template apply |
| **Sync** | Full layout topology enables correct cross-node wiring |
| **JMRI Bridge** | Channels created by templates are automatically projected into JMRI objects (sensors, turnouts, signal masts) via a separate bridge layer — see [JMRI Bridge proposal](./jmri-bridge-proposal.md) |

---

## Side Benefits

### Pin Documentation

The same data that supports templates enables generating per-board wiring documentation:

```
Tower-3 (TowerLCC v4.2) — 
━━━━━━━━━━━━━━━━━━━━━━━━
Connector A (BOD-8):
  Pin 1: Eagle Creek - East approach    [facility: Eagle Creek Siding]
  Pin 2: Mainline Block 7 - detection   [facility: Mainline Block 7]
  Pin 3-8: (available)

Connector B (Signal Driver):
  Pin 1: Eagle Creek - East signal Red  [facility: Eagle Creek Siding]
  Pin 2: Eagle Creek - East signal Grn  [facility: Eagle Creek Siding]
  Pin 3-4: (available)

Logic Lines:
  Lines 1-2: Eagle Creek signal logic   [facility: Eagle Creek Siding]
  Lines 3-4: Mainline Block 7 signals   [facility: Mainline Block 7]
  Lines 5-16: (available)
```

Printable, stickable-to-the-underside-of-the-layout documentation that comes for free from template application data.

### Conflict and Capacity Detection

- "Line 3 is already used by Mainline Block 7" (conflict)
- "Tower-3 has 4 logic lines remaining" (capacity planning)
- Resource allocation tracked per-facility

### Validation After Apply

Because Bowties sees the full topology, it can render the result in context: "This template connected sensor X to signal Y via event Z; here's the flow diagram."

---

## Open Questions

1. **Naming for "facility"** — Alternatives considered: circuit, module, assembly, installation, unit, apparatus, subsystem. "Facility" has railroad precedent ("signal facility", "interlocking facility") but final naming TBD.

2. **Template format** — File format for templates (YAML? JSON? Custom?). Must be human-readable for sharing and version control.

3. **Multi-board template UX** — When a template supports multiple board types, how exactly does the user specify hardware assignment? Sequential wizard? Visual drag-and-drop?

4. **Versioning** — Template captured from firmware v4.0 applied to v5.0 node. Compatibility detection and migration strategy.

5. **Partial overlap** — Two templates both want to create a "turnout command" channel for the same turnout. Detect and merge, or error?

6. **STL portability** — STL programs on TowerLCC+Q may differ between firmware versions. How do templates handle logic program compatibility across versions?

7. **Scope of initial implementation** — Which tier/level to implement first for maximum early value with manageable complexity.

---

## Relationship to Original Thread

The community discussion started from node cloning and concluded it was too limited for multi-node scenarios. This proposal addresses those concerns by:

- Operating with full topology knowledge (not single-file blind editing)
- Working at the behavior level (not node clone level)
- Supporting multi-node deployment of a single template
- Using typed information channels instead of raw event ID manipulation
- Letting hardware constraints drive automatic channel creation

The "average layout owner" workflow that Jim Betz described — specify human-friendly names, never touch event IDs, get a working configuration — becomes achievable because Bowties already has the foundational infrastructure (profiles, sync, event topology) to make it reliable.
