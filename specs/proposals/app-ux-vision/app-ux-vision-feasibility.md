# Bowties UX Vision — Feasibility & Architecture Companion

**Status:** Draft — captures architectural thinking behind the UX vision.  
**Companion to:** [App UX Vision](./app-ux-vision.md)  
**Origin:** Feasibility analysis session, June 2026. Responds to external review feedback and documents the technical approach that makes the vision achievable.

---

## Template System Architecture

### Overview

Behavior templates are the mechanism that transforms channels (raw hardware I/O) into working railroad behavior (signal logic, interlocking, automation). The vision describes the user experience; this section describes how it works underneath.

The architecture has three layers:

1. **Templates (YAML)** — Declarative behavior descriptions: inputs, outputs, and condition → action rules. Human-readable, AI-authorable, target-agnostic.
2. **Target adapters (Rust)** — Per-provider modules that expose primitives for a specific logic execution environment. Each adapter knows how to express boolean logic in its native mechanism.
3. **Apply engine (Rust)** — Orchestrates template application: validates channel mappings, invokes the selected target adapter, and produces a write plan (CDI field writes for on-node targets, or API calls for LogixNG).

### Template Format

A template describes railroad behavior as condition → action rules over named channel states. It does not reference CDI paths, logic line numbers, or target-specific concepts.

```yaml
id: abs-3-aspect
name: "ABS 3-Aspect Signaling"
description: >
  Automatic Block Signaling with three aspects. Protects a single block
  by displaying Stop when the next block is occupied, Approach when the
  downstream signal shows Stop, and Clear otherwise. Each signal also
  publishes its own aspect so that the next upstream signal can read it
  for cascade.

inputs:
  - id: next_block
    role: block-occupancy
    label: "Next block ahead"
  - id: downstream_signal
    role: signal-aspect
    aspects: [stop]               # only needs to read "stop" from downstream
    label: "Downstream signal aspect"
    source: facility-output       # binds to another facility's signal output

outputs:
  - id: signal
    role: signal-aspect
    aspects: [stop, approach, clear]
    label: "Protecting signal"

rules:
  # Evaluated most-to-least restrictive (mast group order).
  # First matching rule fires and exits the group.
  - when: { next_block: occupied }
    then: { signal: stop }
  - when: { downstream_signal: stop }
    then: { signal: approach }
  - default:
    then: { signal: clear }

targets: [tower-lcc-logic, stl, logixng]
```

Templates are human-readable and human-verifiable: "when next block is occupied, signal shows stop; when downstream signal shows stop, signal shows approach" is auditable by anyone who understands ABS. They are also straightforward for AI to generate or update when adding new signaling patterns.

**Key ABS design insight.** Real ABS signaling determines a signal's aspect from two things: (1) the occupancy of the block immediately ahead, and (2) the **aspect of the next signal downstream** (not just the next block's occupancy). This cascade is what makes ABS work — each signal looks one block ahead for Stop, and reads the downstream signal's aspect for Approach. The template DSL models this explicitly: `downstream_signal` is a `signal-aspect` input that binds to another facility's output, forming the cascade chain.

**Aspect-to-event compilation.** The template's rules produce abstract aspects (`stop`, `approach`, `clear`). The bound output channel's style provides an **aspect-to-event map** that translates each aspect into concrete hardware actions at compile time. For example, a `2-led-bicolor-aspect` style maps `approach` to "red on + green on" (simulating yellow); a `3-led-direct-aspect` style maps it to "yellow on." The template never knows about lamps — the style is the hardware adapter. The number of action slots the compiler needs per conditional line depends on the style's pin count (2 actions for bicolor, 3 for tricolor; Tower LCC supports 4 actions per line).

The DSL design will require further iteration to handle the full range of common signaling patterns (APB with directional authority, CTC interlocking, timed sequences). The goal is not to handle every possible scenario — it is to make the majority of cases expressible in the DSL, with uncommon cases handled through direct configuration.

**Signal system diversity.** JMRI supports 48+ signal systems ranging from 6 aspects (basic) to 100+ aspects (French SNCF). The `signal-aspect` role is parameterized — it is a single role whose state vocabulary is declared per channel and per template slot, not a family of fixed roles. US systems emphasize finely-graduated speed commands within a single mast; European systems emphasize mast separation (main/distant/shunting) with simpler per-mast vocabularies. The template DSL handles both: a US 5-aspect template declares `aspects: [stop, restricting, approach, medium-clear, clear]`; a European distant-signal template declares `aspects: [expect-stop, expect-proceed, expect-slow]`. Style compatibility is checked at bind time: the bound channel's style must support all aspects the template produces.

### Target Adapters

Each target adapter is a Rust module that knows how to express boolean condition → action logic in a specific execution environment. The target set is small and stable:

| Target | Environment | Characteristics |
|---|---|---|
| **Tower-LCC Logic** | RR-CirKits Tower-LCC logic conditionals | 32 lines, 2 variables per line, 4 actions per line, boolean operations (AND, OR, XOR, etc.) |
| **STL** | RR-CirKits Tower-LCC+Q STL programs | Procedural, more expressive per line, larger capacity |
| **LogixNG** | JMRI LogixNG (via bridge) | Expression trees, unlimited capacity, requires computer running, works across all protocols |

An adapter provides:
- **Capacity query** — How many logic slots are free? What are the per-slot constraints?
- **Feasibility check** — Can this template's rule set fit on this target?
- **Instantiation** — Rules + mapped channels → write plan (CDI field writes or API calls)

The adapter handles target-specific concerns internally. For example, the Tower-LCC Logic adapter manages:
- **Allocation** — finding free logic lines (lines with no wired producer events on their variables)
- **Chaining** — rules with more than 2 conditions are split across multiple logic lines, with intermediate action events feeding the next line's variables
- **Track Circuit linking** — inter-signal aspect cascading, transparently handling same-node vs cross-node scenarios (see below)
- **Defragmentation** — optionally shifting existing grouped logic lines to consolidate free space when allocation would otherwise fail

#### Track Circuit Management (Tower-LCC Logic Adapter)

ABS signaling requires signals to be aware of each other's aspects. Each signal publishes its own aspect for the upstream signal to read. On Tower-LCC, this is implemented via **Track Circuits** — virtual code lines that carry speed/aspect information between logic conditionals.

**Architecture:** Each Tower-LCC node has 8 Track Receiver circuits (consumer) and 8 Track Transmitter circuits (producer). Each carries 8 speed codes: Stop, Restricting, Slow, Medium, Limited, Approach, Approach-Medium, Clear. Conditionals read Track Circuits via the Variable Source field (`Track Circuit 1–8`) and write to them via Action Destination fields (`Track Circuit 1–8`).

**Same-node vs cross-node:** When two signals are on the same Tower-LCC node, they share Track Circuits directly — a conditional writes `Destination = Track Circuit 3, Speed = Stop`, and another conditional on the same node reads `Source = Track Circuit 3, Speed = Stop`. No network traffic, zero latency. When signals are on different nodes, the Track Transmitter on the source node publishes a Link Address (producer event ID) carrying all 8 speed codes, and the Track Receiver on the destination node subscribes by pasting that Link Address (consumer event ID).

**The template compiler handles this transparently.** The template DSL expresses `downstream_signal` as a signal-aspect input. At apply time, the compiler determines whether the source and destination signals are on the same node:

| Scenario | Compiler action | Resources consumed |
|---|---|---|
| **Same node** | Allocate a shared Track Circuit (1–8) on the node; source signal's conditional writes to it, destination signal's conditional reads from it | 1 Track Circuit (shared, no transmitter/receiver) |
| **Different nodes** | Allocate a Track Transmitter circuit on the source node; allocate a Track Receiver circuit on the destination node; copy the Transmitter's Link Address into the Receiver | 1 Transmitter on source + 1 Receiver on destination |

**Capacity limits:**
- 8 Track Circuits per node (shared for same-node signals)
- 8 Track Transmitter circuits per node (for outbound cross-node links)
- 8 Track Receiver circuits per node (for inbound cross-node links)
- One-to-many is supported: multiple receivers can subscribe to the same transmitter's Link Address
- Many-to-one is not supported: each receiver listens to exactly one transmitter

**Capacity surfacing:** Bowties reports Track Circuit / Transmitter / Receiver availability at feasibility-check time: "Node Tower-3 has 2/8 Track Transmitter circuits remaining." When a template apply would exceed a node's capacity, the adapter reports the constraint and offers alternatives (choose a different node, use LogixNG).

**Allocation tracking:** Bowties tracks which Track Circuits, Transmitters, and Receivers are allocated per facility, so deletion can reclaim them. This allocation metadata is stored alongside the facility record, not in the channel model — Track Circuits are an implementation detail of the Tower-LCC Logic adapter, not a user-visible concept.

### Template Application Creates a Facility

Applying a template produces a **facility** — a named, live functional unit in the Railroad workspace. The facility is the container that ties together:
- The input channels the user mapped during application
- The logic rules instantiated on the target
- The output channels driven by the logic
- The target and channel/logic-line allocation (e.g., "Tower-3, Logic Lines 5–7")

A facility created by template application knows its template origin, which enables:
- Re-application if the template is updated
- Comprehension view (the bowtie diagram showing inputs → logic → outputs)
- Debugging ("which rule is producing this aspect?")

Facilities are template-first: every facility originates from a template application. For advanced scenarios where a user has already configured logic directly (e.g., an experienced user with a pre-existing layout), a future workflow will allow attaching a template to existing configured elements — mapping what's already on the node into a facility structure. This path does not need to be as streamlined as starting fresh with a template; it serves technically proficient users who are willing to do more manual mapping.

### Multi-Head Masts and Composite Indication

At junctions, a single signal mast often carries multiple heads — one per route (e.g., mainline head + diverging head). Each head is an independent `signal-aspect` channel with its own style and hardware binding, but the mast introduces two interactions that go beyond per-head independence:

1. **Route interlocking between heads.** Each head's rules depend on the turnout position — the mainline head shows Stop when the route is set to diverging, and vice versa. A multi-head template encodes these per-head rule sets within a single facility.

2. **Composite mast indication for upstream cascade.** The upstream signal needs to know the mast's composite state, not individual head states. For example, an upstream signal shows Approach only when *all* heads on the downstream mast show Stop (meaning no route is available). If any head shows a permissive aspect, the upstream signal shows Clear. On Tower-LCC, this is handled by publishing all heads' aspects to the same Track Circuit — the circuit naturally carries the most permissive indication across the mast group.

**Template DSL extension for masts:**

```yaml
# Multi-head mast template (junction signal)
outputs:
  - id: mainline_head
    role: signal-aspect
    aspects: [stop, approach, clear]
  - id: diverging_head
    role: signal-aspect
    aspects: [stop, approach, clear]

rules:
  # Mainline head rules (route-aware)
  - when: { turnout_position: diverging }
    then: { mainline_head: stop }
  - when: { mainline_block: occupied }
    then: { mainline_head: stop }
  - when: { downstream_mainline: stop }
    then: { mainline_head: approach }
  - default:
    then: { mainline_head: clear }

  # Diverging head rules (route-aware)
  - when: { turnout_position: normal }
    then: { diverging_head: stop }
  # ... (parallel structure)

mast:
  heads: [mainline_head, diverging_head]
  composite_rule: most-permissive
```

The `mast` section tells the Tower-LCC adapter to place all heads' conditional groups within the same mast group and publish to the same Track Circuit. The `composite_rule: most-permissive` means the upstream signal's cascade input reads the least restrictive aspect any head is currently showing.

**Scope.** Multi-head masts are not required for the first ABS slice (single-head signals on straight track). They become necessary when junction signals are implemented. The template DSL's `mast` section and the adapter's mast-group compilation are designed as an extension that does not change the single-head model — a single-head template is simply a mast with one head.

### Capacity and Overflow

When a template's rules exceed a target's capacity, the adapter reports this at feasibility-check time — before any writes occur. The user sees:
- Which target(s) can accommodate the template
- Why a target can't (e.g., "requires 5 logic lines, only 3 available on Tower-3")
- Options: choose a different node with more capacity, choose a different target (LogixNG has no capacity limit), or reduce the template's scope

For the Tower-LCC Logic adapter specifically, capacity management includes:
- Detecting free lines by checking whether any other node produces events that a logic line's variables consume — if no producer exists for a line's consumer events, that line is unused
- Optionally defragmenting — shifting existing grouped logic lines to consolidate free space
- Chaining rules that exceed the 2-variable-per-line limit across multiple lines

**Defrag safety and v1 policy.** Defragmenting active logic lines is safe only within Bowties' ownership boundary — the lines Bowties itself allocated and tracks. Within that region Bowties is the only writer, so shifting lines is an internal bookkeeping operation that happens to flush to the board. The risk that defrag could perturb another facility's working logic only arises if Bowties shifts lines it doesn't own (lines configured by hand, by another tool, or by an earlier non-Bowties workflow). v1 takes the simplest correct stance: do not defragment. When a template apply would otherwise succeed but for fragmentation, the apply fails with "not enough contiguous capacity — choose a different node or use LogixNG." This is more conservative than necessary within Bowties' own ownership region, but it is the safest behavior for a tool whose value depends on non-engineers being able to trust that an apply will not perturb other facilities. Scoped defrag — defragmenting only Bowties-owned lines and refusing if non-Bowties lines would need to move — is a later-horizon enhancement, not a v1 commitment.

### Scoping: Common Cases Easy, Uncommon Possible

The template DSL is designed to cover the **majority** of railroad signaling and automation patterns — ABS, APB, simple interlocking, detection-based automation. These are well-established, standardized patterns with bounded complexity.

For **uncommon cases** that exceed the DSL's expressiveness (complex custom interlocking, non-standard logic, experimental configurations):
- Users configure logic directly using the guided CDI editor or raw CDI view
- A future workflow will allow attaching a template to existing configured elements, bringing directly-configured logic into the facility model for comprehension and debugging
- The template DSL is designed to grow over time — patterns that start as uncommon can be formalized into templates as demand emerges

This means the system does not need to solve every possible logic scenario in the DSL. It needs to make the common path effortless while keeping the advanced path accessible.

---

## Channel Model: Data & Persistence

The UX vision describes channels in terms of role, style, binding, and ownership ([Channel Roles, Styles, and Bindings](./app-ux-vision.md#channel-roles-styles-and-bindings)). This section captures how that model is persisted and how the constraint contract is implemented.

### Persisted Shape

`channels.yaml` records only what's needed to identify and re-bind a channel. Each channel is `{ id, name, role, style, binding, owner }`:

```yaml
schemaVersion: '2.0'
channels:
  # Hardware-owned: auto-created when the BOD-4 daughter board was selected on Tower-3 Connector A.
  - id: 2b8dc48f-a9b0-45d6-b394-39f11d55de2c
    name: "Eagle Creek — East Approach"
    role: block-occupancy
    style: bod-block-detector-input
    owner: hardware-config
    binding:
      kind: node-pin
      nodeKey: 0201570002D9
      pin: ca-input-1

  # User-owned: created via a facility slot's "Add channel" action,
  # binding picked as part of channel creation.
  - id: 9f1ad22e-7d40-4d44-bb88-6c1c8b2f9e10
    name: "Eagle Creek — East Lamp"
    role: lamp-indicator
    style: single-led-direct-lamp
    owner: user
    binding:
      kind: node-pin
      nodeKey: 0201570002D9
      pin: lamp-row-3

  # On-node firmware resource (not a physical pin): a Tower-LCC Logic block
  # used as the binding target for a derived-signal channel.
  - id: 4f2c1e88-3a55-44c7-b1d9-7e0a2b6e1c44
    name: "Westbound Approach Lit"
    role: lamp-indicator
    style: tower-lcc-logic-output
    owner: user
    binding:
      kind: node-logic-block
      nodeKey: 0201570002D9
      block: logic-line-7

  # Virtual binding: DCC accessory channel. Not on any node; the address is the binding.
  - id: 6cd0e0b1-8a3a-4d2e-9d4d-2a3b8a0d7e21
    name: "Yard Ladder T-15"
    role: turnout-command
    style: dcc-accessory-turnout
    owner: user
    binding:
      kind: dcc-accessory
      address: 15
```

That's the entire shape. No copies of CDI values, no field-binding details, no override state, no constraint cache. The CDI tree is the truth; the style catalog (system + profile) is the binding contract; the channel is the identity.

Every binding carries a `kind` discriminator plus the fields that `kind` requires. Three families are anticipated:

| Family | Example `kind` values | Required fields | Notes |
|---|---|---|---|
| **On-node physical** | `node-pin` | `nodeKey`, `pin` | Pins on real or placeholder nodes — detector inputs, lamp rows, signal outputs. |
| **On-node firmware** | `node-logic-block`, `node-stl-slot`, `node-mast`, … | `nodeKey` + a resource id (`block`, `slot`, `mast`, …) | Resources allocated inside a node's firmware that are not a pin: a Logic line, an STL program slot, a firmware-managed signal mast, etc. The exact set is open and grows as profiles declare new resource kinds. |
| **Virtual** | `dcc-accessory` | `address` (no `nodeKey`) | Addresses in a protocol namespace that no Bowties-managed node owns; a gateway translates to wire-level packets. |

Styles declare which `kind` they expect. `bod-block-detector-input` rejects anything but `node-pin`; `tower-lcc-logic-output` rejects anything but `node-logic-block`; `dcc-accessory-turnout` rejects anything but `dcc-accessory`. The validation happens at channel-creation time, so an invalid pairing is impossible to persist. The schema deliberately does not enumerate the set of on-node firmware `kind`s — new ones can be added by a profile without a schema bump.

The `owner` discriminator drives lifecycle: `hardware-config` channels disappear when the underlying hardware-configuration choice is cleared or changed (any facility slot bound to one becomes empty); `user` channels persist until the user removes them (which in this first slice means removal from their only slot — future scope adds ref-counting + delete-on-zero across multiple slots).

### Constraint Contract

The constraint rules live on the **style**, not on the channel and not on a cross-product of role × hardware-kind. A style declares which CDI fields it manages, with two natural tiers:

| Layer | What it pins | Editor presentation |
|---|---|---|
| **Shape / mode** — the CDI field that determines what the bound hardware is right now | Fixed to a specific value when the style is active (e.g., `Pin Function = Output`, `Lamp Selection = Direct Command`) | Primary managed field; presented first |
| **Leaf rules** — values under the established shape | Locked or restricted to a subset (e.g., `Output Function = Steady Active Hi`); brightness, fade, polarity = unmanaged | Secondary managed fields + unmanaged fields below |

The shape constraint matters because it determines which other CDI fields are even relevant under the bound subtree. The relevance-rule machinery the profile system already uses for daughter-board selection extends naturally to this case.

A style's constraints are in force for the entire life of the channel — from the moment it is created (hardware-owned: daughter-board selection; user-owned: Add channel completes) until the moment it is destroyed. The only path to override a managed field is **Raw CDI** (the existing escape hatch). There is no per-channel override flag in `channels.yaml`: drift detection — already in the vision — flags managed fields that are out of range and offers a one-click repair. A per-channel override would add stored state, sync complexity, and a third constraint mechanism without enabling anything Raw CDI doesn't already enable.

### Resolution and Display

- `resolve_channel_event_ids` becomes a style lookup: find the channel's style → read its producer/consumer event-leaf mapping → return event IDs against the bound resource (pin, Logic block, STL slot, mast, …). No connector/input arithmetic at the call site.
- For virtual-binding styles (DCC accessory), `resolve_channel_event_ids` instead computes the event IDs deterministically from the binding's address per the OpenLCB DCC accessory event allocation. There is no node CDI to read; the address is the input and the event pair is the output. Bowties displays these event IDs in the channel detail view as read-only.
- Event bowties wired to a virtual-binding channel use those computed IDs on the consumer side and have **no consumer-side CDI write** to emit — the gateway listens on the bus instead. The bowtie writer treats this as a normal "no-op" for that endpoint.
- Channel display labels come from the profile's binding label (`Tower-3 — Connector A — Input 1`, `Signal LCC #1 — Mast 2`, `Tower-3 — Logic Line 7`). The shape is profile-supplied per binding `kind`, not built from slot slugs at render time. Virtual bindings supply their own label shape (`DCC Layout — Accessory #15`).
- The Channels panel (hardware-organised, ships in the first slice) groups channels by node + subsystem + resource and shows role, style, live state, and the slot/facility binding. A later layout-organised view (Channels-by-name) lands with ref-counting + multi-slot binding.

### Implementation Surface

| Area | Change |
|---|---|
| **Channel record** | `{ id, name, role, style, owner, binding }`. `owner` is `hardware-config` or `user`. `binding` is always non-null and carries a `kind` discriminator — every channel is tied to a specific resource, whether a physical pin, an on-node firmware resource (Logic block, STL slot, mast, …), or a virtual address. |
| **Profile schema (`.profile.yaml`)** | Declares which roles a board can host, which styles realise them on which subsystems, and for each style: the binding `kind` it expects, the constraint contract (managed-field rules), the event-leaf mapping, and the binding-label shape. New on-node `kind`s can be introduced by a profile without a schema bump. |
| **`channels.yaml` schema** | `schemaVersion: '2.0'`, the shape shown above. |
| **Constraint engine** | Given a node's channels, compute the set of active style rules and feed them to ConfigEditor's relevance-rule machinery. Existing relevance rules handle the actual UI filtering. |
| **Hardware-owned creation** | When a hardware-configuration choice fixes the role of pins, create one channel per pin with the implied role + style + binding, default-named. Clearing or changing the choice deletes those channels. |
| **User-owned creation** | The facility slot's Add channel action picks the style (when more than one realises the slot's role on the available hardware) and the binding target, and creates the channel already bound. |

---

## JMRI Bridge Sync Philosophy

### Strategy: Single Master First

The v1 target user is a non-technical model railroader who uses Bowties as their sole configuration tool. For this user, there is no second master — Bowties creates channels and facilities, the JMRI bridge projects them into JMRI objects, and JMRI serves as the downstream display and operation platform.

The two-master synchronization problem (what happens when both tools can write?) is a real concern for the secondary user — technically proficient people with existing JMRI layouts. That path is a deliberate later evolution, not a v1 constraint. The existing three-way sync model (shipping today for offline-edit → bus-connect reconciliation) provides the mechanism when that time comes.

### The Bridge Creates Atoms; JMRI Connects Them

The JMRI bridge's primary value is eliminating the tedious manual creation of JMRI objects. Today, a user must know to create a sensor with the right system name format, get the event IDs right, set up signal masts with the correct signal system and aspect mappings, then create Blocks and assign sensors to them. The bridge automates all of this.

JMRI objects fall into two tiers:

**Tier 1 — Bowties creates these** (event-backed, channel-mapped):

| JMRI Bean | Created From | System Name Identity | Event IDs Mutable? |
|---|---|---|---|
| **Sensor** | Occupancy, button, or current channel | Event IDs embedded in system name | No — delete + recreate |
| **Turnout** | Turnout position/command channel(s) | Event IDs embedded in system name | No — delete + recreate |
| **Signal Mast** | Signal aspect channel | Type + ordinal (no event IDs) | Yes — aspect events updated via setters |
| **Light** | `lamp-indicator` channel | Event IDs embedded in system name | No — delete + recreate |
| **Block** | Auto-created alongside occupancy Sensor | Separate system name | N/A — references Sensor by name |

**Tier 2 — JMRI connects these** (topology, logic — JMRI-owned):

| JMRI Structure | What It Does | Who Owns It |
|---|---|---|
| **Path + BeanSetting** | Block adjacency + required turnout positions | JMRI (Layout Editor drawing) |
| **LayoutBlock** | 1:1 wrapper around Block; runtime adjacency | JMRI (Layout Editor) |
| **Signal Mast Logic** | Auto-discovered signal rules (source → destination) | JMRI (auto-discover from topology) |
| **Section** | Ordered block sequence for dispatch/CTC | JMRI (manual or auto) |
| **LogixNG** | Conditional logic expressions | JMRI or Bowties (when template targets LogixNG) |

Bowties creates tier 1 objects. JMRI connects them via tier 2. The bridge reads tier 2 (via `GET /topology`) for comprehension and gap analysis but does not write it — with the exception of LogixNG when a template targets it as the logic execution platform.

### Channel → JMRI Bean Mapping

Each Bowties channel maps to a JMRI bean based on the channel's role:

| Bowties Channel Role | Direction | JMRI Bean | Notes |
|---|---|---|---|
| Block Occupancy | Producer | Sensor + Block | Bridge auto-creates Block and assigns Sensor |
| Turnout Position Feedback | Producer | Turnout (feedback side) | Combined into one Turnout bean with command |
| Turnout Command | Consumer | Turnout (command side) | Same Turnout bean — JMRI merges both directions |
| Signal Aspect | Consumer | Signal Mast | One event per aspect; events are mutable properties |
| Lamp Indicator | Consumer | Light | Lit/unlit event pair |
| Button Press | Producer | Sensor | Pressed/released event pair |
| Current Sensor | Producer | Sensor | Active/inactive event pair |

The turnout case is notable: Bowties may model command and feedback as separate channels, but the bridge merges them into a single JMRI Turnout bean. This is a bridge-layer concern, not a channel-model concern — the internal representation can be resolved independently of the sync architecture.

### Facility → JMRI Structure

Facilities do not map to a single JMRI object. A facility spans multiple beans plus logic:

| Bowties Facility | JMRI Beans Created | Logic Target | JMRI Topology Equivalent |
|---|---|---|---|
| ABS Signal Block | Sensors (occupancy) + Block + Signal Mast | On-node or LogixNG | SignalMastLogic (auto-discovered from panel) |
| Turnout Control | Sensor (button) + Turnout | On-node or LogixNG | Turnout + triggering logic |
| Yard Lighting | Sensor (button) + Lights | On-node or LogixNG | LogixNG conditional |

JMRI has no general "facility" concept. The closest equivalents are Signal Mast Logic (for signal facilities) and Sections (for dispatch). Bowties' facility model is richer — it groups channels by behavioral intent, which JMRI doesn't represent.

### Sync Surface Area

For tier 1 beans (the only ones Bowties writes), the mutable properties are limited:

| Property | Sensor/Turnout/Light | Signal Mast |
|---|---|---|
| System name | Immutable (contains event IDs) | Immutable (type + ordinal) |
| User name | Mutable — sync target | Mutable — sync target |
| Comment | Mutable — sync target | Mutable — sync target |
| Event IDs | Immutable (in system name) | Mutable (per-aspect setters) |
| Inverted flag | Mutable — rarely changed | N/A |
| Feedback mode (turnout) | Mutable — rarely changed | N/A |

**What could collide between Bowties and JMRI:**

- **User name changes** — if renamed in both tools between syncs. Rare in the Bowties-first flow.
- **Signal mast aspect events** — if re-mapped in JMRI. Very rare.
- **Deletion** — if a user deletes a Bowties-managed bean in JMRI. Detectable via the `bowties.managed` property.

**What cannot collide:**

- Event IDs on sensors/turnouts/lights — immutable in the system name, and the system name IS the identity. If Bowties and JMRI both have a sensor with the same system name, they're referencing the same event IDs by definition.
- Tier 2 structures (paths, topology, signal mast logic) — JMRI-owned, Bowties reads only.

### Prior Art: Offline Sync Model

Bowties already ships a three-way sync model for offline changes. The same conflict classification applies to JMRI sync:

| Base → JMRI | Base → Bowties | Action |
|---|---|---|
| Unchanged | Unchanged | No action |
| Changed | Unchanged | Accept JMRI change |
| Unchanged | Changed | Push Bowties change |
| Changed | Changed (same) | No action (convergent) |
| Changed | Changed (different) | Conflict — present to user |

The existing sync panel UI handles conflict presentation and resolution. The JMRI bridge would use the same pattern with the same UI treatment.

### Structural Changes: Delete + Recreate

When an event ID changes in Bowties (e.g., channel rewired to different hardware), the corresponding JMRI sensor or turnout must be deleted and recreated because the event IDs are baked into the system name.

This has a downstream impact: any JMRI panel references to the old system name break. The bridge can detect this and warn the user ("changing this event ID will require recreating the JMRI sensor; panel references will need updating").

In practice, event ID changes are rare in the Bowties-first flow — event IDs are assigned at channel creation and don't change unless the user deliberately rewires hardware. Signal masts are exempt from this concern entirely, since their event IDs are mutable properties.

---

## Placeholder Reconciliation

Placeholder nodes let users pre-stage configuration for boards they don't yet own. The user creates a placeholder, configures it (daughter boards, channel names, possibly facility membership), and then promotes it to a real node when the physical hardware connects. The question this raises is what happens to whatever configuration is already on the physical board at promotion time.

### Promote With Overwrite Confirmation

Promoting a placeholder to a real node uses the same UX pattern as overwriting an existing file or installing software over a prior install: the user is asked to confirm a replacement, shown what will be replaced, and the action is committed atomically. That is the entire purpose of a placeholder — let a user configure before they own the hardware, then promote that configuration onto the real board when it arrives.

When the user maps a physical node to a placeholder, Bowties shows a confirmation prompt listing what will be written: channels, daughter board assignments, named events, and any facility-driven logic. If the physical board already has meaningful configuration on the affected CDI fields, that configuration is listed as what will be replaced. The user confirms, and Bowties writes.

That is the entire model. There is deliberately no per-field merge, no "keep the board's value here, the placeholder's value there" picker, no diff-and-pick UI. Those would add real implementation complexity — per-field provenance tracking, merge UI, conflict semantics — in exchange for very limited value. A user who pre-staged configuration on a placeholder has already decided what the board should look like. Users who want to start from existing board state instead use a separate workflow entirely (connect the board, let Bowties discover and adopt its configuration); that path does not involve a placeholder.

Keeping promotion as a single overwrite-with-confirmation step is the design choice, not a limitation. It matches the user's mental model ("the placeholder is what I want; promote it") and resists the complexity that would otherwise accumulate around two-master reconciliation and field-level conflict resolution.

---

## Adopting Existing Configurations

The vision describes a separate entry point for users who already have configured LCC hardware — migrated from LccPro, or set up directly in JMRI. This audience is distinct from the primary persona: they are existing LCC users adopting Bowties as a more comprehensible front-end, not new users being introduced to LCC. Adoption is a different goal from market expansion.

The mechanism for this path is **manual mapping**: the user defines channels and facilities in Bowties, then explicitly binds each channel's style to existing CDI fields and event assignments on the already-configured boards. Bowties doesn't try to infer the mapping from board state — the user knows what their layout does, and the bound concepts produce the same comprehension view and debugging surface that a from-scratch layout would.

This path is intentionally less streamlined than the placeholder + template flow. That flow is optimized for users who want LCC to be easy; the adoption flow is for users who already speak LCC and want comprehension and tooling on top of what they have. They can tolerate explicit mapping steps because they understand what is being mapped. v1's commitment to this audience is that the manual mapping path exists and works — not that it matches the placeholder path's polish.

---

## Feasibility Assessment

### Development Context

Bowties is built by one experienced architect using AI-assisted development with a structured workflow (TDD-first, architecture enforcement via ADRs and placement rules, multi-session build tracking). The AI workflow includes profile extraction skills, subagent delegation, and enrichment gates that maintain architecture consistency automatically.

Implementation velocity and architecture quality are not constraints. The profile authoring pipeline is itself AI-assisted — adding a new board profile is a ~2-hour guided session, not weeks of manual specification. The template DSL is designed to be AI-authorable. The JMRI bridge is well-specified enough for AI-assisted implementation.

The primary remaining risk is **UX validation with the target audience** — will non-technical users actually find the experience accessible? This is only answerable by putting it in front of people.

### Existing Infrastructure

The vision builds on substantial existing infrastructure, not greenfield speculation:

| Capability | Status |
|---|---|
| Profile system (v2 schema, loader, resolver, annotation) | **Shipping** — 5 bundled profiles |
| AI profile extraction pipeline (8 skills, PDF → profile) | **Proven** — Tower-LCC and TurnoutBoss authored this way |
| CDI signature-based firmware variant detection | **Shipping** — Tower-LCC legacy vs. rev-C7 |
| Configuration Modes with variant overlays | **Shipping** — connector slots, Left/Right pairing |
| Connector selection + constraint evaluation | **Shipping** — auto-stages compatible field values |
| Offline change model + three-way sync | **Shipping** — full conflict classification and resolution |
| Placeholder boards (pre-stage config without hardware) | **Early** — read-only view ships; editable placeholders and full testing are pending |
| Channel data model + persistence | **In-flight** — architecture proven on branch |

### Confidence Table: Bowties as Single Master (Primary User)

Non-technical users who use Bowties as their sole configuration tool. JMRI is a downstream display/operation platform. No external writes to reconcile.

| Capability | Confidence | Key Factor |
|---|---|---|
| Workspace toggle + guided wiring view | ~92% | Evolutionary over shipping code |
| Channel abstraction + auto-creation | ~88% | Architecture proven; bounded types for v1 |
| Profile system across v1 boards | ~82% | AI extraction pipeline is the authoring tool; adding boards is cheap |
| Template DSL + ABS on Tower-LCC Logic | ~85% | Clear architecture (YAML DSL + Rust adapter); Tower-LCC logic was purpose-built for signal logic |
| Facility model (template-first) | ~85% | Simplified by template-first decision; straightforward data model |
| JMRI bridge (tier 1 bean creation) | ~75% | Well-specified API; small sync surface; no conflict resolution needed |
| Template → LogixNG (via bridge) | ~60% | Two unbuilt systems in series; both well-documented |
| Facility comprehension + live state | ~75% | Standard UI work with clear data model |
| Planner wizard | ~65% | UX-heavy (needs user validation), implementation straightforward |

### Confidence Table: Bidirectional with JMRI (Advanced User)

Technically proficient users with existing JMRI layouts who edit in both tools. Requires conflict detection and resolution.

| Capability | Confidence | Key Factor |
|---|---|---|
| Tier 1 bean sync (sensors, turnouts, masts, lights) | ~65% | Three-way merge model proven for offline sync; collision surface is small (user names, comments) |
| Conflict detection + resolution UI | ~70% | Pattern exists in offline sync panel; same approach applies |
| Signal mast event ID updates (mutable) | ~75% | JMRI setters exist; no delete+recreate needed |
| Sensor/turnout event ID changes (immutable) | ~55% | Requires delete+recreate; panel references break; needs user warning flow |
| LogixNG ownership (Bowties-created conditionals) | ~45% | Needs clear ownership marker; user could edit in JMRI's LogixNG editor |
| JMRI topology import (read-only) | ~70% | Read-only from Bowties side; no write conflicts; well-specified GET endpoint |
| Full bidirectional with concurrent editing | ~40% | Race conditions, snapshot window, thread safety in JMRI managers |

### Comparison to External Review

An external review estimated 30–40% confidence for "template-driven working signal logic across multiple board families." That assessment assumed:
- The profile system was net-new (it ships today with 5 bundled profiles)
- The template→logic path required a general-purpose compiler (it's a parameterized write plan for purpose-built hardware)
- JMRI sync required solving a two-master conflict problem (it's single-master-first with a small sync surface)
- A solo developer constraint implied both skill and velocity limitations

With the actual context — existing infrastructure, decided architecture, AI-assisted implementation, and explicit v1 scoping to common patterns on Tower-LCC — confidence for the "Now" horizon (working ABS signal logic via templates) is **~85%**.

### Remaining Risks

| Risk | Mitigation |
|---|---|
| DSL expressiveness limits for patterns beyond ABS | Explicit deferral; DSL grows iteratively. APB and interlocking are v2 patterns. |
| UX accessibility for non-technical users | Only validated by real user testing. Prototype early, iterate. |
| JMRI bridge + LogixNG as two unbuilt systems in series | Decoupled: bridge works without LogixNG; LogixNG works without templates (manual channel push). |
| Board count scaling beyond v1 | AI extraction pipeline keeps per-board cost low (~2 hours). Market is dominated by RR-CirKits; coverage is good with few profiles. |

### Conclusion: Value vs. Risk by Audience

The two target audiences have inverted risk profiles:

**Primary user (non-technical, Bowties as single master):**

| Dimension | Assessment |
|---|---|
| **Value** | High — market expansion. Makes LCC accessible to people who cannot use it today. This is the product's reason to exist. |
| **Engineering risk** | Low (~85% confidence). Infrastructure exists, architecture is decided, implementation is bounded. |
| **UX risk** | The real unknown. Will channels, facilities, and templates make sense to non-engineers? Only testable with real users. |

**Advanced user (technically proficient, bidirectional with JMRI):**

| Dimension | Assessment |
|---|---|
| **Value** | Moderate — productivity gain. Eliminates tedious manual work, adds comprehension and debugging. But these users *can* already do it manually; this is convenience, not enablement. |
| **Engineering risk** | Higher (~40–70% depending on sync seam). Ownership boundaries, concurrent editing, delete+recreate flows. |
| **UX risk** | Low. These users are technical and tolerant of rough edges. |

**Strategic implication:** Invest v1 in the primary user, where the risk is UX-testable rather than engineering-blocking. The advanced user's sync complexity is solvable once the foundation is solid — and that audience tolerates interim limitations (read-only JMRI visibility, manual object creation in the meantime).

This sequencing means:
- v1 delivers the highest-value outcome (market expansion) with the lowest engineering risk
- The UX risk is testable early — put the template apply workflow in front of non-technical users and iterate
- Advanced user features build on the same infrastructure (channels, facilities, bridge) without requiring the foundation to change
- The hard sync problems (bidirectional editing, LogixNG ownership) are deferred to a point where the product has users and feedback, not solved speculatively
