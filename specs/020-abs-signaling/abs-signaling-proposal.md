# Proposal: ABS Signaling — Behavior Templates, Signal Aspect Channels, and Logic Compilation

**Created**: 2026-07-05
**Status**: Draft proposal — to be refined into a numbered spec
**Input**: Brainstorm session exploring how to extend the facility model (spec 018) to support Automatic Block Signaling. Grounded in the Tower LCC clinic handout, the app UX vision proposals, and RR-CirKits Tower LCC profile extractions.

---

## Problem

Spec 018 delivered the architectural foundation: channels, facilities, slots, and the Block Indicator behavior template. A user can scaffold a facility, bind a block-occupancy producer channel and a lamp-indicator consumer channel, and watch the LED track block occupancy in real time.

But Block Indicator is a trivial pass-through (`occupied → lit`). Real railroad behavior — ABS signaling — requires:

1. **Logic that evaluates conditions and determines signal aspects.** A signal's aspect depends on the next block's occupancy AND the downstream signal's aspect (the cascade). This logic must run on the Tower LCC node's conditional system, not just as event forwarding.
2. **A signal-aspect channel role** whose state vocabulary speaks in railroad terms (`stop`, `approach`, `clear`), not lamp terms (`lit`, `unlit`). Different hardware realizations (bicolor searchlight, tricolor searchlight, firmware-managed mast) all implement this role through different styles.
3. **A behavior template system** that can express ABS rules declaratively and compile them to Tower LCC conditional lines, including mast group structure, Track Circuit cascade publication, and aspect-to-event translation via the bound channel's style.

None of this exists today. The user who wants ABS signaling must manually configure each Tower LCC conditional line, understand mast group semantics, figure out Track Circuit allocation, and hand-wire event IDs — exactly the expertise barrier Bowties exists to eliminate.

## Scope

### In scope (first slice)

- **`signal-aspect` channel role** — parameterized state vocabulary. The first slice delivers 3-aspect (`stop`, `approach`, `clear`). The role is a single `signal-aspect` role whose aspects are declared per channel, not a family of fixed roles.
- **At least one signal-aspect style** — `2-led-bicolor-aspect` on Signal LCC Direct Lamp Control rows. Claims 2 rows (red LED, green LED). Provides an **aspect-to-event map**: `stop` → red on / green off; `approach` → red on / green on (simulates yellow); `clear` → red off / green on.
- **ABS 3-Aspect Signal behavior template** — YAML-defined. Inputs: `next_block` (block-occupancy), `downstream_signal` (signal-aspect, reads Stop for cascade). Output: `signal` (signal-aspect: stop/approach/clear). Rules: next block occupied → Stop; downstream signal at Stop → Approach; default → Clear.
- **Tower LCC Logic adapter** — compiles the template's abstract rules to Tower LCC conditional lines:
  - Mast group structure (Function = Group / Last)
  - Most-to-least restrictive evaluation order
  - `null => true` for default aspect
  - Track Circuit allocation for cascade publication (same-node only in first slice)
  - Aspect-to-event map expansion: abstract aspect → concrete lamp events based on bound style's pin count and event map
  - Conditional line allocation and capacity tracking
- **Facility apply workflow** — user creates a facility from the ABS template, maps channels to slots (next-block occupancy, downstream signal, signal head output), selects the logic target node, and Bowties compiles and writes the CDI.
- **Cascade wiring** — when two ABS signal facilities exist on the same node, Bowties connects them via a shared Track Circuit so the upstream signal reads the downstream signal's aspect.

### Deferred (noted but not scoped)

- **Cross-node cascade** — Track Transmitter/Receiver linking for signals on different Tower LCC nodes. First slice assumes all cascaded signals are on the same node.
- **Multi-head junction masts** — masts with multiple heads (mainline + diverging). First slice handles single-head signals on straight track. The template DSL's `mast` section and composite indication rule are designed but not implemented.
- **Additional styles** — `3-led-direct-aspect` (tricolor), `2-led-bicolor-flashing` (flashing yellow for approach), firmware Mast-driven style.
- **Additional signal systems** — 5-aspect, European main/distant separation, US railroad-specific systems (AAR, B&O, PRR, etc.). The parameterized `signal-aspect` role supports these; specific templates and styles are future work.
- **STL and LogixNG target adapters** — alternative compilation targets for Tower LCC+Q and JMRI.
- **Template library UI** — browsable template catalog. First slice uses a hardcoded template (like Block Indicator in spec 018) or a single bundled YAML file.
- **Template capture** — extracting a template from an existing working configuration.
- **Facility comprehension view** — the input→logic→output flow diagram for debugging.

## Key Concepts

### ABS Signal Logic Model

Standard ABS determines a signal's aspect from two things:

1. **The occupancy of the block immediately ahead.** If the next block is occupied, the signal shows Stop (red). This is the safety-critical rule.
2. **The aspect of the next signal downstream.** If the downstream signal shows Stop (because *its* next block is occupied), this signal shows Approach (yellow) to warn the train to prepare to stop. If the downstream signal shows anything more permissive, this signal shows Clear (green).

This cascade is what makes ABS work — each signal looks one block ahead for Stop, and reads the downstream signal's published aspect for Approach. The cascade propagates backward through the block system automatically.

On Tower LCC, this is implemented using **logic conditional groups** (mast groups) that evaluate most-to-least restrictive:

```
Line N   (Group):  if next_block occupied → send Stop events, exit group
Line N+1 (Group):  if downstream_signal == Stop → send Approach events, exit group
Line N+2 (Last):   null => true → send Clear events, exit group (default)
```

Each conditional line has 2 input variables and up to 4 output actions. The "exit group" behavior means the first matching rule fires and no subsequent rules in the group are evaluated.

### Signal-Aspect Channel Role

The `signal-aspect` role is parameterized — a single role whose state vocabulary is declared per channel, not a family of fixed roles. This matches how JMRI models its 48+ signal systems: aspects are semantic (what to communicate), not physical (which LEDs to light).

A 3-aspect channel has states: `unknown`, `stop`, `approach`, `clear`.
A 5-aspect channel would add: `restricting`, `medium-clear`.
A European distant signal might use: `expect-stop`, `expect-proceed`, `expect-slow`.

Facility slots bind by role and declare which aspects the template produces. Compatibility is checked at bind time: the bound channel's style must support all aspects the template produces.

### Style Aspect-to-Event Map

Each style implementing `signal-aspect` provides an **aspect-to-event map** — a declaration of how each abstract aspect translates to concrete hardware actions:

```yaml
style: 2-led-bicolor-aspect
implements: signal-aspect
pins: 2                         # claims 2 Direct Lamp Control rows
pin_labels: [red, green]
aspect_events:
  stop:
    - { pin: red, state: on }
    - { pin: green, state: off }
  approach:
    - { pin: red, state: on }
    - { pin: green, state: on }   # both on simulates yellow
  clear:
    - { pin: red, state: off }
    - { pin: green, state: on }
```

When the template compiler writes a conditional line for "show Stop," it reads the bound channel's style, looks up the `stop` entry in the aspect-to-event map, and writes the concrete event IDs (red lamp row on-event, green lamp row off-event) into the conditional's Action[0] and Action[1] slots. A tricolor style would write 3 actions per aspect (red/green/yellow); Tower LCC supports up to 4 actions per conditional line.

### Track Circuit Cascade

Each Tower LCC node has 8 internal Track Circuits — virtual code lines that carry speed/aspect information between logic conditionals. ABS cascade uses these to connect signals:

- A signal's conditional group writes its determined aspect to a Track Circuit (via Action Destination = Track Circuit N, Track Speed = Stop/Approach/Clear).
- The upstream signal's conditional reads that Track Circuit (via Variable Source = Track Circuit N, Track Speed = Stop) to decide its Approach condition.

**Same-node:** Signals on the same node share Track Circuits directly — zero latency, no network traffic.

**Cross-node (deferred):** Track Transmitter/Receiver linking via LCC events. Each node has 8 Transmitter and 8 Receiver circuits. The compiler would allocate a Transmitter on the source node and a Receiver on the destination node, copying the Link Address event ID.

Bowties tracks Track Circuit allocation per facility so deletion can reclaim them.

### Behavior Template Format

Templates are YAML files describing railroad behavior as condition → action rules. They do not reference CDI paths, logic line numbers, or target-specific concepts.

```yaml
id: abs-3-aspect
name: "ABS 3-Aspect Signal"
description: >
  Automatic Block Signaling with three aspects. Shows Stop when the
  next block is occupied, Approach when the downstream signal shows
  Stop, and Clear otherwise. Publishes its own aspect for upstream
  cascade.

inputs:
  - id: next_block
    role: block-occupancy
    label: "Next block ahead"
  - id: downstream_signal
    role: signal-aspect
    aspects: [stop]
    label: "Downstream signal aspect"
    optional: true              # end-of-line signals have no downstream
    source: facility-output     # binds to another facility's signal output

outputs:
  - id: signal
    role: signal-aspect
    aspects: [stop, approach, clear]
    label: "Protecting signal"

rules:
  - when: { next_block: occupied }
    then: { signal: stop }
  - when: { downstream_signal: stop }
    then: { signal: approach }
  - default:
    then: { signal: clear }

targets: [tower-lcc-logic]
```

### Tower LCC Logic Adapter

The adapter compiles the template into concrete CDI field writes:

**Input:** Template rules + mapped channels (with resolved event IDs) + target node

**Output:** A write plan — a list of CDI field writes for the conditional lines, plus Track Circuit allocation metadata

**Compilation steps:**

1. **Allocate conditional lines.** A 3-aspect signal needs 3 contiguous lines (one per rule + the default). Check that the target node has enough free lines.
2. **Allocate Track Circuit.** If the signal needs to publish its aspect for upstream cascade, allocate an unused Track Circuit (1–8) on the target node.
3. **Write mast group structure.** First line: Function = Group. Middle lines: Function = Group. Last line: Function = Last.
4. **Write variable inputs.** For each rule's condition:
   - Block occupancy: Variable Source = "Use V1 Events", set true/false = the channel's occupied/clear event IDs
   - Downstream signal aspect: Variable Source = Track Circuit N, Track Speed = Stop
5. **Write logic operation.** `V1 Only` for single-variable rules; `null => true` for the default.
6. **Write exit format.** When true = "Send then Exit Group"; when false = "Evaluate Next".
7. **Expand aspect-to-event map.** For each rule's `then` clause, look up the bound output channel's style aspect-to-event map. Write the concrete event IDs into Action[0..3].
8. **Write Track Circuit output.** For each action, also write Destination = Track Circuit N with the corresponding Track Speed code, so upstream signals can read this signal's aspect.

**Capacity constraints:**
- 32 conditional lines per node (a 3-aspect signal uses 3)
- 8 Track Circuits per node
- 4 actions per conditional line (bicolor = 2 actions, tricolor = 3)
- 2 variables per conditional line

## User Workflow

### Creating an ABS Signal Facility

1. **Prerequisites:** At least one `block-occupancy` channel exists (from a BOD daughter board selection, per spec 018). A Signal LCC node is connected with unclaimed Direct Lamp Control rows available.

2. **Add facility:** User clicks "+ Add facility" on the Railroad tab. Selects the "ABS 3-Aspect Signal" template. Provides a name (e.g., "Block 7 Signal").

3. **Map inputs:**
   - **Next block occupancy:** User selects from existing `block-occupancy` channels (e.g., "Block 8 Occupancy").
   - **Downstream signal:** User selects from existing ABS signal facility outputs (e.g., "Block 8 Signal → signal output"), or selects "None" for end-of-line signals.

4. **Map output:**
   - **Signal head:** User clicks "Add channel" on the output slot. Picks a style (e.g., `2-led-bicolor-aspect`). Picks two unclaimed Direct Lamp Control rows on a Signal LCC node. Channel is created and bound to the slot.

5. **Select logic target:** User selects which Tower LCC node runs the logic (must have sufficient free conditional lines and Track Circuits). Bowties shows available capacity.

6. **Apply:** Bowties compiles the template:
   - Writes conditional lines to the target node's CDI
   - Allocates a Track Circuit for cascade publication
   - Creates the underlying bowties for the signal-aspect events
   - Facility transitions to Wired/Live

### Building an ABS Cascade

For a straight mainline with blocks 5–8, the user creates 4 ABS signal facilities:
- **Block 8 Signal** — downstream_signal = None (end of line). Publishes to Track Circuit 1.
- **Block 7 Signal** — downstream_signal = Block 8 Signal. Reads Track Circuit 1 for Approach. Publishes to Track Circuit 2.
- **Block 6 Signal** — downstream_signal = Block 7 Signal. Reads Track Circuit 2. Publishes to Track Circuit 3.
- **Block 5 Signal** — downstream_signal = Block 6 Signal. Reads Track Circuit 3. Publishes to Track Circuit 4.

Each facility is created independently. The cascade wiring (Track Circuit connections) is managed by the compiler based on which facility's output is bound as another facility's `downstream_signal` input.

## Data Model Changes

### New channel role: `signal-aspect`

```rust
// Channel role becomes an enum with parameterized variants
pub enum ChannelRole {
    BlockOccupancy,
    LampIndicator,
    SignalAspect { aspects: Vec<SignalAspectValue> },
}

pub enum SignalAspectValue {
    Stop,
    Approach,
    Clear,
    // Future: Restricting, MediumClear, etc.
}
```

### New channel style: `2-led-bicolor-aspect`

Declares:
- Role: `signal-aspect` with aspects `[stop, approach, clear]`
- Pins: 2 (red LED row, green LED row)
- Aspect-to-event map (see above)
- Constraint contract: Lamp Selection = Direct Command, Output Function = Steady Active Hi

### Behavior template storage

Templates are loaded from YAML files bundled with the app. The template registry extends the existing hardcoded `BehaviorTemplate` with YAML-loaded variants.

### Facility extensions

Facilities gain:
- **Logic target reference** — which node and which conditional lines are allocated
- **Track Circuit allocation** — which Track Circuits are used, per node
- **Template parameters** — resolved values for parameterized inputs (e.g., which Track Circuit carries the downstream signal)

### Logic allocation tracking

A new persistence record tracks what Bowties has allocated on each node:

```yaml
# logic-allocations.yaml (per layout)
allocations:
  - node_key: "0201570002D9"
    conditional_lines:
      - lines: [1, 2, 3]
        facility_id: "uuid-block-7-signal"
      - lines: [4, 5, 6]
        facility_id: "uuid-block-8-signal"
    track_circuits:
      - circuit: 1
        facility_id: "uuid-block-8-signal"
        purpose: "aspect-cascade-output"
      - circuit: 2
        facility_id: "uuid-block-7-signal"
        purpose: "aspect-cascade-output"
```

## Architecture Boundaries

### Where logic lives

| Concern | Layer | Location |
|---|---|---|
| Template YAML parsing and validation | `bowties-core` | `bowties-core/src/behavior_templates/` |
| Template compilation to write plans | `bowties-core` | `bowties-core/src/logic_adapters/tower_lcc.rs` |
| Style aspect-to-event maps | `bowties-core` | `bowties-core/src/layout/styles/` |
| Logic allocation tracking | `bowties-core` | `bowties-core/src/layout/logic_allocations.rs` |
| CDI write execution | `app/src-tauri` | Existing `set_modified_value` pipeline |
| Facility apply orchestration | `app/src/lib/orchestration` | `facilityApplyOrchestrator` |
| Template selection UI | `app/src/lib/components` | Railroad tab facility creation flow |

### What stays in bowties-core vs app

The template compiler, style registry, and logic adapter belong in `bowties-core` because they encode domain logic (signal engineering rules, Tower LCC conditional semantics) that is independent of the Bowties UI. The app layer orchestrates the user workflow (channel selection, target node picking) and invokes `bowties-core` for compilation.

Protocol-specific behavior (Tower LCC conditional structure, Track Circuit semantics) stays in `bowties-core`'s logic adapter, not scattered across app code.

## Design Decisions (Resolved)

1. **Signal-specific YAML templates, not a general template system.** The YAML template format described here is purpose-built for signal facilities — it assumes mast group semantics, Track Circuit cascade, and aspect-to-event compilation. Other facility types (turnout control, CTC panels, approach lighting) may need different template structures or different compilation strategies. Rather than prematurely generalizing a universal template DSL, this feature delivers a signal-template loader that works well for the signaling domain. The Block Indicator template remains hardcoded; it predates this system and does not need logic compilation.

2. **Logic target selection: suggest with override.** When the user creates an ABS signal facility, Bowties suggests the best logic target node (e.g., the node that hosts the most input channels, minimizing cross-node traffic) but allows the user to select a different node. The suggestion is presented as a pre-selected default with a "Change" affordance, not as a mandatory choice.

3. **Cascade wiring is automatic.** When the user binds another facility's signal output as the `downstream_signal` input, the Track Circuit connection is made automatically by the compiler. Logic block allocation, Track Circuit assignment, and conditional line numbering are implementation details — the user expresses intent ("this signal reads that signal's aspect"), and the compiler handles the rest. Bowties tracks ownership of all allocated resources (conditional lines, Track Circuits) per facility for cleanup and capacity reporting.

4. **End-of-line signals show Stop or Clear only.** When `downstream_signal` is omitted (end of line), the Approach rule is skipped. The signal shows Stop when the next block is occupied, Clear otherwise. This is correct for a terminal signal — there is no downstream signal to cascade from, so Approach has no meaning.

5. **Facility deletion clears all allocated resources.** When an ABS signal facility is deleted, Bowties: (a) sets allocated conditional lines to Function = Blocked (disabled), clearing their variable events, logic operation, and action events; (b) frees the Track Circuit allocation; (c) removes the underlying bowties. All CDI fields on the allocated lines are reset to default values, not just disabled — this ensures the lines are fully available for reuse and no stale event IDs linger.

## Relationship to Existing Work

- **Spec 018** — provides the channel/facility/slot architecture this feature extends
- **App UX vision** — the north-star for the template apply workflow, facility comprehension view, and the broader Plan → Wire → Railroad → Operate journey
- **Behavior templates proposal** — the original channel/role/style/template concept design
- **Feasibility companion** — template system architecture (three layers), target adapter design, Track Circuit management, multi-head mast extension point
- **Tower LCC profile extractions** — field-level CDI structure for conditionals, recipes for signal logic

## Future Extensions (designed but not scoped)

- **Cross-node cascade** via Track Transmitter/Receiver linking
- **Multi-head junction masts** with composite indication (`most-permissive` rule) — see feasibility companion
- **Additional styles** (tricolor, flashing, firmware mast)
- **Additional signal systems** (5-aspect, European, railroad-specific)
- **STL and LogixNG compilation targets**
- **Template library UI** with category browsing and search
- **Block Indicator migration** — the existing Block Indicator template could be reimplemented as a degenerate case of the template system (no logic target, direct event forwarding)
