# Feature Specification: ABS Signaling

**Feature Branch**: `020-abs-signaling`  
**Created**: 2026-07-05  
**Status**: Draft  
**Input**: Proposal document — ABS Signaling: Behavior Templates, Signal Aspect Channels, and Logic Compilation

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create an ABS 3-Aspect Signal Facility (Priority: P1)

A model railroader wants to set up a signal that automatically shows Stop (red) when the next block is occupied, Approach (yellow) when the downstream signal shows Stop, and Clear (green) otherwise. They select the "ABS 3-Aspect Signal" behavior template, map the required inputs (block occupancy channel and optional downstream signal), map the signal head output to a pair of LED rows on a Signal LCC node, choose a Tower LCC logic target node, and apply. Bowties compiles the abstract signal rules into Tower LCC conditional lines and writes the configuration to the target node's CDI.

**Why this priority**: This is the core value proposition — eliminating manual Tower LCC conditional configuration for the most common signaling use case. Without this, no other ABS functionality is meaningful.

**Independent Test**: Can be fully tested by creating a single standalone signal facility (no downstream cascade), verifying the compiled conditional lines produce Stop and Clear aspects in response to block occupancy changes.

**Acceptance Scenarios**:

1. **Given** at least one block-occupancy channel exists and a Signal LCC node has unclaimed Direct Lamp Control rows, **When** the user creates an ABS 3-Aspect Signal facility with no downstream signal, **Then** the facility compiles to conditional lines that show Stop when the block is occupied and Clear otherwise, and the signal head LEDs are driven accordingly.
2. **Given** a valid ABS signal facility configuration, **When** the user selects a Tower LCC logic target node, **Then** Bowties shows the node's available conditional line and Track Circuit capacity before applying.
3. **Given** a valid ABS signal facility configuration, **When** the user applies the facility, **Then** the compiled conditional lines are written to the target node's CDI, the allocated resources (conditional lines, Track Circuits) are tracked, and the compiled CDI changes are staged in memory as normal config edits (no signal-specific lifecycle states beyond what spec 018 defines).

---

### User Story 2 - Build an ABS Signal Cascade (Priority: P2)

A model railroader with a straight mainline of 3–4 blocks wants all signals to cascade: each signal reads the downstream signal's aspect so that occupying one block causes Stop on that signal and Approach on the one behind it, propagating backward through the block system. The user creates multiple ABS signal facilities, binding each signal's downstream input to the previous signal's output. Bowties automatically manages the Track Circuit wiring between them.

**Why this priority**: Cascade is the defining characteristic of ABS — without it, signals are isolated and far less useful. This is the second most important story because it builds directly on Story 1 and delivers the full ABS experience.

**Independent Test**: Can be tested by creating 3 ABS signal facilities chained via downstream-signal bindings on a single Tower LCC node, then verifying that occupying a block causes the correct Stop/Approach/Clear cascade across all three signals.

**Acceptance Scenarios**:

1. **Given** two ABS signal facilities on the same Tower LCC node, **When** the user binds the upstream signal's downstream input to the downstream signal's output, **Then** Bowties allocates a Track Circuit for the cascade and compiles the upstream signal's Approach rule to read from that Track Circuit.
2. **Given** a cascade of 3 signals, **When** the middle block becomes occupied, **Then** the protecting signal shows Stop, the signal behind it shows Approach, and the signal two blocks back shows Clear.
3. **Given** an end-of-line signal with no downstream signal, **When** the facility is applied, **Then** the Approach rule is omitted and the signal shows only Stop or Clear.

---

### User Story 3 - Select Signal Head Style (Priority: P3)

A model railroader needs to tell Bowties how their physical signal head is wired. They select a style (e.g., "2-LED bicolor") when creating the signal-aspect output channel. The style declares how many LED rows it claims and how each abstract aspect (Stop, Approach, Clear) maps to concrete LED on/off states. Bowties uses this mapping when compiling conditional line actions.

**Why this priority**: Style selection bridges the gap between abstract signal aspects and physical hardware. It must exist for Story 1 to produce correct output events, but the first slice only needs one style.

**Independent Test**: Can be tested by creating a signal with the 2-LED bicolor style and verifying the compiled conditional actions use the correct event IDs for each aspect (red on/green off for Stop, both on for Approach, red off/green on for Clear).

**Acceptance Scenarios**:

1. **Given** the user is creating a signal-aspect output channel, **When** they select the "2-LED bicolor" style, **Then** the channel claims 2 Direct Lamp Control rows and the aspect-to-event map is configured for red/green LED combinations.
2. **Given** a signal facility with a bicolor style, **When** the template compiler expands the Stop aspect, **Then** the compiled conditional line's actions contain the red LED on-event and green LED off-event from the bound rows.

---

### User Story 4 - Delete a Signal Facility and Reclaim Resources (Priority: P4)

A model railroader wants to remove a signal facility they no longer need. When deleted, Bowties resets all allocated conditional lines on the Tower LCC node to disabled/default state and frees the Track Circuit allocation so those resources are available for reuse.

**Why this priority**: Resource cleanup is essential for iterative configuration. Users need confidence that deleting a facility fully reclaims capacity and leaves no stale event IDs in the CDI.

**Independent Test**: Can be tested by creating a signal facility, applying it, then deleting it and verifying that conditional lines are reset and Track Circuit capacity is restored.

**Acceptance Scenarios**:

1. **Given** an applied ABS signal facility using conditional lines 1–3 and Track Circuit 1, **When** the user deletes the facility, **Then** lines 1–3 are set to disabled with all variable and action fields cleared, and Track Circuit 1 is freed.
2. **Given** a deleted facility that was part of a cascade, **When** another facility's downstream input referenced it, **Then** Bowties warns the user about broken cascade references before confirming deletion.

---

### User Story 5 - View Logic Target Capacity (Priority: P5)

Before applying a signal facility, the user wants to see how many conditional lines and Track Circuits are available on a prospective logic target node. Bowties displays current allocation and remaining capacity, and suggests the best target node based on input channel proximity.

**Why this priority**: Capacity visibility prevents users from over-allocating a node and supports informed node selection. It is not blocking for basic functionality but is important for multi-facility layouts.

**Independent Test**: Can be tested by querying a node's capacity display before and after applying a facility, verifying counts update correctly.

**Acceptance Scenarios**:

1. **Given** a Tower LCC node with 32 conditional lines and 8 Track Circuits, **When** the user opens the logic target selection for a new facility, **Then** the display shows how many lines and circuits are already allocated and how many remain.
2. **Given** multiple candidate nodes, **When** the user opens target selection, **Then** Bowties suggests the node hosting the most input channels as the default, with an option to override.

### Edge Cases

- What happens when the target node has no remaining conditional lines? Bowties prevents apply and displays an error identifying the capacity constraint.
- What happens when all 8 Track Circuits on a node are allocated? Bowties prevents cascade wiring to that node and suggests using a different node or removing an existing facility.
- What happens when a user deletes a downstream signal that other facilities reference? Bowties warns about broken cascade references, lists the affected upstream facilities, and requires confirmation before proceeding.
- What happens when a signal-aspect style requires more actions per aspect than the hardware supports (e.g., 5 actions when Tower LCC supports 4)? Bowties rejects the style at bind time with a clear error message.
- What happens when an end-of-line signal has no downstream signal? The Approach rule is omitted; the signal evaluates only Stop and Clear.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide a `signal-aspect` channel role with parameterized aspect vocabulary. The first delivery supports 3-aspect signals with states: `unknown`, `stop`, `approach`, `clear`. The signal-aspect channel is created inline during the facility apply workflow (template-driven), consistent with existing block-detection lamp facilities.
- **FR-002**: System MUST provide at least one signal-aspect style (`2-led-bicolor-aspect`) that declares pin count, pin labels, and an aspect-to-event map translating each abstract aspect to concrete LED on/off states.
- **FR-003**: System MUST provide an "ABS 3-Aspect Signal" behavior template that declares inputs (block occupancy, optional downstream signal aspect), outputs (signal aspect), and condition-action rules: occupied → Stop, downstream Stop → Approach, default → Clear.
- **FR-004**: System MUST compile behavior template rules into Tower LCC conditional lines, including mast group structure (Group/Last), most-to-least restrictive evaluation order, variable inputs, logic operations, exit format, and aspect-to-event map expansion.
- **FR-005**: System MUST allocate and track contiguous conditional lines on the target Tower LCC node per facility, enforcing the 32-line-per-node limit. Lines must be adjacent indices because Tower LCC uses fall-through evaluation for mast groups.
- **FR-006**: System MUST allocate and track Track Circuits (1–8) per node for cascade publication, enforcing the 8-circuit-per-node limit.
- **FR-007**: System MUST automatically wire cascade connections via Track Circuits when one facility's signal output is bound as another facility's downstream-signal input, provided both are on the same node.
- **FR-008**: System MUST allow users to select a logic target node and display available capacity (conditional lines and Track Circuits) before applying.
- **FR-009**: System MUST suggest a default logic target node based on proximity to input channels, with an override option.
- **FR-010**: System MUST support end-of-line signals where the downstream-signal input is omitted, producing only Stop and Clear rules.
- **FR-011**: System MUST reset allocated conditional lines to disabled/default state and free Track Circuit allocations when a facility is deleted.
- **FR-012**: System MUST warn the user when deleting a facility that is referenced as a downstream signal by other facilities.
- **FR-013**: System MUST validate at bind time that a style's aspect-to-event map supports all aspects the behavior template produces.
- **FR-014**: System MUST validate at bind time that a style's action count per aspect does not exceed the hardware limit (4 actions per Tower LCC conditional line).
- **FR-015**: System MUST persist logic allocations (conditional lines, Track Circuits) per node per facility so that resources survive app restart and are available for capacity reporting and cleanup.

### Key Entities

- **Signal-Aspect Channel**: A channel whose role is `signal-aspect`, parameterized with a set of aspects (e.g., stop/approach/clear). Bound to a physical signal head via a style.
- **Signal-Aspect Style**: A declaration of how an abstract signal-aspect channel maps to physical hardware. Defines pin count, pin labels, and an aspect-to-event map. Example: `2-led-bicolor-aspect` claims 2 Direct Lamp Control rows.
- **Behavior Template**: A YAML-defined set of condition-action rules expressing railroad behavior. Declares inputs (with channel roles), outputs (with channel roles and produced aspects), and ordered rules. Target-independent.
- **Logic Adapter**: A target-specific compiler that translates a behavior template's abstract rules into concrete CDI field writes. The Tower LCC Logic Adapter produces mast group structure, variable inputs, aspect-to-event expansions, and Track Circuit allocations.
- **Logic Allocation Record**: A persistent record of what conditional lines and Track Circuits Bowties has allocated on each node, keyed by facility ID. Used for capacity tracking, cleanup, and conflict detection.
- **Mast Group**: A Tower LCC CDI concept where consecutive conditional lines are grouped by a Group/Last flag. The node evaluates lines within a mast group most-restrictive-first with first-match-wins semantics. A 3-aspect ABS signal compiles to one mast group of 3 conditional lines (Stop, Approach, Clear).
- **Track Circuit**: An internal communication channel on a Tower LCC node (8 per node) that carries aspect/speed information between conditional groups. Used for same-node signal cascade.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can create and apply a standalone ABS 3-Aspect Signal facility (no cascade) in under 3 minutes, starting from an existing block-occupancy channel and unclaimed signal head rows.
- **SC-002**: A user can build a 4-signal ABS cascade on a single Tower LCC node in under 10 minutes, with all cascade wiring handled automatically.
- **SC-003**: 100% of compiled conditional lines produce the correct signal aspect when tested against block occupancy and downstream signal state inputs.
- **SC-004**: Deleting a signal facility fully reclaims all allocated resources — a subsequent facility can reuse the same conditional lines and Track Circuits without manual intervention.
- **SC-005**: Users never need to manually configure Tower LCC conditional lines, Track Circuits, or mast group structure for ABS signaling — Bowties handles all compilation and CDI writes.
- **SC-006**: The system prevents resource over-allocation — users cannot apply a facility when the target node lacks sufficient conditional lines or Track Circuits, and receive a clear explanation of the constraint.

## Scope Boundaries

### In Scope (First Slice)

- `signal-aspect` channel role with 3-aspect vocabulary (stop, approach, clear)
- `2-led-bicolor-aspect` style with aspect-to-event map
- ABS 3-Aspect Signal behavior template (YAML-defined)
- Tower LCC Logic Adapter (template compilation to conditional lines)
- Same-node Track Circuit cascade wiring
- Logic allocation tracking and persistence
- Facility apply workflow with logic target selection
- Facility deletion with resource reclamation
- Capacity display and target node suggestion

### Out of Scope (Deferred)

- Cross-node cascade via Track Transmitter/Receiver linking
- Multi-head junction masts with composite indication rules
- Additional styles (tricolor, flashing, firmware mast-driven)
- Additional signal systems (5-aspect, European, railroad-specific)
- STL and LogixNG compilation targets
- Template library UI with browsable catalog
- Template capture from existing configurations
- Facility comprehension view (input → logic → output diagram)

## Assumptions

- The channel/facility/slot architecture from spec 018 is complete and available as the foundation for this work.
- At least one Tower LCC node profile with conditional line CDI structure is available for compilation targeting.
- Signal LCC nodes with Direct Lamp Control rows are available and their CDI structure is profiled.
- Block-occupancy channels can be created independently of this feature (via existing daughter board selection from spec 018).
- A 3-aspect ABS signal requires 3 contiguous conditional lines on the target node (Tower LCC uses fall-through evaluation, so mast group lines must be adjacent indices).
- Tower LCC conditional lines support up to 4 output actions each, which is sufficient for bicolor (2 actions) and tricolor (3 actions) styles.
- Track Circuits (8 per node) are sufficient for typical single-node signaling scenarios (e.g., 4 signals = 4 Track Circuits).
- The YAML behavior template format is purpose-built for signaling; other facility types (turnout control, CTC panels) may need different template structures in the future.

## Clarifications

### Session 2026-07-07

- Q: What lifecycle states should a signal facility pass through? → A: Reuse existing facility lifecycle from spec 018; compilation is an internal step during apply, not a user-visible state. Compiled config changes appear as staged in-memory changes like any other edits.
- Q: What happens if a CDI write fails mid-apply? → A: Compilation produces staged in-memory changes (always succeeds); CDI writes use the existing sync/write flow with its connectivity error handling. No signal-specific error recovery needed.
- Q: How is the signal-aspect channel created? → A: Inline as part of the facility apply workflow — the behavior template drives channel creation, same as existing block-detection lamp facilities. Users do not create signal-aspect channels independently.
- Q: What is a "mast group" in this context? → A: A Tower LCC CDI concept where consecutive conditional lines are grouped by a Group/Last flag and evaluated most-restrictive-first with first-match-wins semantics.
- Q: Must conditional lines for a mast group be contiguous indices? → A: Yes — Tower LCC uses fall-through evaluation, so mast group lines must be physically adjacent indices.
