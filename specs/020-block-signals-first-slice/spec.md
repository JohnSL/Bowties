# Feature Specification: One-Direction Automatic Block Signals

**Feature Branch**: `020-block-signals-first-slice`  
**Created**: 2026-11-05  
**Status**: Draft  
**Input**: User description: "[One-Direction Automatic Block Signals first E2E slice proposal](block-signals-first-slice-proposal.md)"

## Clarifications

### Session 2026-11-05

- Q: Does changing eastward/westward reverse the operating sequence or change Signal-LCC programming? A: No. Blocks are entered in travel order; eastward/westward only orients the track diagram. Changing that selection preserves block order, signal identities, protected blocks, bindings, and hardware configuration. Reordering blocks is a separate operational edit.
- Q: Must Bowties approve the perceived yellow quality of an RG searchlight, or substitute flashing for Approach? A: No. Keep the two-driver RG display: red for Stop, red plus green for Approach, green for Clear. Color blending is the modeler's choice and is not an application or acceptance gate. RG uses fewer drivers than a dedicated R/Y/G display. Validate hardware/output compatibility, rule selection, actual output behavior, and published/received restrictions; do not require subjective color approval. No flashing substitute is needed.

### Session 2026-11-22

- Q: The S2R feedback release exposes more than this spec covers (movements in both directions, 2/3/4 aspects, turnouts, Restricting, far-end choices, approach lighting). Does this spec grow to match it? A: No. The preview release got no feedback, so the next goal is a working end-to-end release within this spec's original scope. Features beyond it are added afterward, one at a time, on top of the working path. The S2R pages stay. Choices outside this scope are shown disabled, with no "coming later" text, not removed, and the saved Line model must be able to hold them later without a migration.
- Q: Which guided steps does this scope use? A: The storyboard's five steps: Blocks, Aspects, Appearance, Wiring, Review. They replace the earlier Blocks, Signals, Display, Bindings, Confirm. In this scope, Aspects and Appearance show fixed values (three-indication policy; one RG searchlight head, always lit). Wiring and Review are where the modeler chooses outputs and confirms.
- Q: What do "save a draft" and "apply" mean in this spec? A: Bowties' existing lifecycle (FR-011, ADR-0001 2026-11-22 extension). Done stages the facility, its signal channels and the compiled Signal LCC configuration as unsaved configuration edits, exactly as hand edits are. "Save a draft" is Save; an offline Save stores those edits as Offline Changes. "Apply" is the bus write that Save performs when connected, or the Sync Panel writing the Offline Changes after reconnecting. There is no separate Apply action. "Read back" is the verification added in slice E4; until then, written values are never shown as verified.
- Q: Does the first release need the Review step? A: Not yet (decided 2026-11-22 during E1a). With one signal, Review would repeat what the unsaved configuration edits already show, and Discard undoes all of them. In E1a, Wiring is the last page: its Done runs the same compile check Review would, and blocking issues (an output in use, no free mast or Logic slot) are shown there. Review is shown disabled in the step list and arrives with slice E3, where it explains the added downstream logic. FR-010 applies from E3.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Operate a One-Block Line (Priority: P1)

A modeler names an Automatic Block Signals facility, chooses eastward or westward travel, and selects an existing detected block. Bowties creates one named signal at that block's entrance. The modeler assigns its red and green outputs on a compatible Signal-LCC, reviews the result, and saves the layout. Saving while connected writes the configuration to the board; saving offline keeps it as Offline Changes that the Sync Panel writes later. Occupied means Stop; clear means Clear, with no promise about track beyond this block.

**Why this priority**: A working occupancy-protected signal is useful on its own and establishes the normal creation and hardware lifecycle.

**Independent Test**: Complete the five-step guided flow (Blocks, Aspects, Appearance, Wiring, Review) for one block on one compatible node, apply and read back the configuration, change occupancy, reopen and rename the Line, then remove it. Verify the physical appearances and that shared block detection survives removal.

**Acceptance Scenarios**:

1. **Given** a named block with usable detection and a compatible node with free outputs, **when** the modeler completes Blocks, Aspects, Appearance, Wiring, and Review, saves a draft, and applies it, **then** the block's entrance signal shows Stop when occupied and Clear when clear, with the appropriate physical red and green appearances.
2. **Given** an applied one-block Line, **when** the modeler reads back and reopens it, **then** its name, direction, block, signal identity, signal name, and output assignments match the applied configuration.
3. **Given** an applied Line, **when** the modeler removes it after reviewing affected resources, **then** only its owned signal configuration is released; the shared block, detector, and unrelated node configuration remain.

---

### User Story 2 - Extend an Existing Line (Priority: P2)

A modeler adds a second detected block in travel order without rebuilding the original signal. The earlier signal now responds to the next signal's restriction; the new terminal signal still protects only its own block. The review explains changes to both signals before any hardware change.

**Why this priority**: Extending an already working Line proves that the initial signal is durable rather than throwaway scaffolding.

**Independent Test**: Append a second block to a one-block Line, apply on one compatible node, and vary both blocks' occupancy. Verify original identity and compatible output assignments, downstream restrictions, and physical indications.

**Acceptance Scenarios**:

1. **Given** an applied one-block Line, **when** a second detected block is appended, **then** the first signal retains its identity, name, protected block, and compatible outputs; the review shows the new downstream relationship and required resources.
2. **Given** a clear upstream block, **when** the terminal signal shows Stop, **then** the upstream signal shows Approach; **when** the terminal signal shows Clear, **then** the upstream signal shows Clear.
3. **Given** any signal's protected block becomes occupied, **when** its state is evaluated, **then** that signal shows Stop regardless of downstream state.
4. **Given** a compatible RG searchlight, **when** the Approach rule is selected, **then** Rule-to-Aspect activates both assigned red and green outputs and publishes Approach Track Speed; Bowties does not block application based on the modeler's perceived blended color.

---

### User Story 3 - Operate and Maintain a Cross-Node Line (Priority: P3)

A modeler creates and later edits a three-block Line whose signals are distributed across at least two compatible Signal-LCC nodes. The same ordered-block behavior works eastward and westward regardless of which node hosts a signal. A restriction at the last signal travels through the middle signal to the first.

**Why this priority**: A real Line may span nodes; same-node previews do not establish an operational multi-node feature.

**Independent Test**: For each direction, apply, read back, reopen, edit, and remove a three-block Line spanning two nodes. Vary occupancy to verify the restriction reaches the first signal, and inspect published restrictions, received restrictions, events, and physical outputs on both nodes.

**Acceptance Scenarios**:

1. **Given** a three-block Line on two compatible nodes with all blocks initially clear, **when** the last block becomes occupied, **then** its terminal signal shows Stop, the clear middle block's signal shows Approach, and the clear first block's signal shows Clear, including when a downstream relationship crosses nodes.
2. **Given** the same Line, **when** the last block clears and all blocks are clear, **then** all three signals show Clear; **when** the middle block becomes occupied, **then** its signal shows Stop and the clear first block's signal shows Approach.
3. **Given** an applied Line, **when** the modeler reorders blocks, rebinds outputs, or removes a nonfinal block, **then** the preview shows changed links, node resources, and releases; unaffected signal identities and bindings and shared blocks are preserved. Removing the last remaining block requires removal of the Line instead.
4. **Given** an applied Line, **when** the modeler changes only the eastward/westward selection, **then** the track diagram changes orientation while block order, signal identities, protected blocks, bindings, downstream links, resource claims, and Signal-LCC configuration remain unchanged; no hardware application is required.

---

### User Story 4 - Reject Invalid Work and Report Failures (Priority: P4)

A modeler can tell when detection, hardware, wiring, or capacity is insufficient. Bowties will not apply an invalid configuration, will distinguish unavailable observations from live ones, and will report the actual progress of an interrupted multi-node operation with a recovery path.

**Why this priority**: Accurate validation, status, and recovery are required for a usable configuration workflow.

**Independent Test**: Try missing detection, incompatible displays, claimed outputs, insufficient capacity, unresolved cross-node links, unavailable observations, and a failure after the first node is updated. Verify blocked application for invalid configurations, truthful observation and application status, and recovery.

**Acceptance Scenarios**:

1. **Given** a conflicting or unsupported binding, **when** the modeler reviews the Line, **then** application is blocked with an actionable explanation for each affected node; cancellation leaves committed hardware and resource claims unchanged.
2. **Given** a multi-node application fails after updating one node, **when** the modeler reviews the failure, **then** the updated and untouched nodes are identified separately, the Line is not reported fully applied, and a safe retry or recovery path is offered.

### Edge Cases

- A one-block Line or final signal has no inferred downstream signal; its Clear indicates only that its own block is clear.
- Changing eastward/westward does not reverse the entered block order or move signals to different block entrances; changing the operating sequence requires an explicit block-order edit.
- Prepending a block leaves the former terminal signal terminal; appending converts it into an internal signal without replacing it.
- A known downstream Approach results in upstream Clear for this three-indication policy when the upstream block is clear: the first signal provides no separate advance-warning indication.
- A signal's own occupied block takes precedence over any downstream indication.
- During startup, disconnection, or loss of fresh observation, the UI marks unavailable observations as unavailable rather than live; it does not claim to control how Signal-LCC behaves when an input stops reporting.
- Duplicate or externally claimed outputs, insufficient mast/logic/receiver/driver capacity, incompatible hardware, malformed existing configuration, and unresolved downstream addresses block application without silent repair.
- An edit that moves or removes a signal must account for references on every affected node; uncertain ownership or external references block destructive application.
- A failed partial application must distinguish actually applied changes from planned ones across nodes.

## Requirements *(mandatory)*

The behavior of supported Signal-LCC hardware is a constraint on this feature, not something Bowties can redefine. Verify hardware-dependent requirements and acceptance scenarios against the actual nodes before treating them as achievable. If supported configuration cannot produce a specified behavior, document the discrepancy and revise the affected requirement, scenario, or supported scope before accepting the feature; do not claim that Bowties supplies behavior the hardware does not provide or silently count a mismatch as passing.

### Functional Requirements

- **FR-001**: Modelers MUST be able to create one named Automatic Block Signals for one selected travel direction (eastward or westward) from one or more ordered, existing named blocks with usable detection. Each block MUST have exactly one entrance signal in that Line.
- **FR-002**: The guided flow MUST follow Blocks, Aspects, Appearance, Wiring, Review and distinguish observed occupancy from configuration preview; unsupported choices MUST be absent or clearly unavailable. Choices that the S2R feedback release exposes but this scope does not support (movements in both directions, 2- or 4-aspect, turnouts, Restricting, a far end treated as Stop, approach lighting, other signal types) MUST be shown unavailable and MUST NOT reach a saved or applied Line.
- **FR-003**: Each signal MUST have an editable name, a stable identity, a protected-block reference, and its own node and red/green output assignments. Modelers MUST NOT have to enter internal event identifiers, logic tables, or downstream receiver numbers.
- **FR-004**: The initial Line MUST support only one-direction, uncoordinated automatic block signaling with Speed/Essential Stop, Approach, Clear behavior where applicable. A single or terminal signal MUST use only Stop and Clear; no opposing movement protection or authority beyond the terminal block is implied.
- **FR-005**: An occupied protected block MUST result in Stop regardless of downstream restriction. For a clear protected block, an internal signal MUST show Approach when the next signal publishes Stop, and Clear when the next signal publishes Approach or Clear. A terminal signal with a clear block MUST show Clear. The published Stop, Approach, and Clear speeds and the receiving signal's comparisons MUST produce this mapping on the supported hardware before application is allowed.
- **FR-006**: Signal relationships MUST follow the explicitly entered block order, which is travel order, independent of node placement and eastward/westward diagram orientation. The next signal MUST be the signal protecting the next block in that list. Changing only eastward/westward MUST NOT reverse the list or alter signal identities, protected blocks, bindings, downstream links, resource claims, or Signal-LCC configuration, and MUST NOT require hardware application. For each internal signal, its receiving downstream restriction MUST be associated with that signal's node even if the next signal is on a different node.
- **FR-007**: The initial display MUST support a two-driver red/green searchlight arrangement: red for Stop, red plus green for Approach, and green for Clear. Rule-to-Aspect MUST map selected rules to these outputs independently of their Stop, Approach, or Clear Track Speed; conditionals select rules rather than directly sequencing lamp outputs. The perceived blended color is the modeler's choice and MUST NOT be an application or acceptance gate. Technical hardware/output compatibility and correct output activation remain required. No dedicated yellow driver or flashing substitute is required in this slice.
- **FR-008**: The modeler MUST assign the red and green physical outputs separately for every signal on a compatible Signal-LCC. Existing block detection MUST be reused rather than re-entered during signal setup.
- **FR-009**: Before application, Bowties MUST validate per-node compatibility, available mast/logic/receiver/driver resources, existing configuration integrity, output ownership, display and input compatibility, and resolvable cross-node downstream references. Failures MUST identify the affected signal and node and MUST NOT silently overwrite or repair existing configuration.
- **FR-010**: The Review step MUST show each signal's block, behavior, physical outputs, expected indications, affected existing configuration, resource claims and conflicts, and all proposed cross-node changes and releases. Application MUST be blocked until required checks pass.
- **FR-011**: Saving a draft, discarding it, applying to hardware, reading back, reconnecting, and recovering from partial application MUST follow the existing Bowties lifecycle. Cancelling or failing validation MUST NOT change committed hardware or create lasting resource claims.
- **FR-012**: Applied Line records MUST preserve Line identity, name, direction, ordered block membership, stable signal identities and names, protected-block references, per-signal bindings, and ownership of generated resources on each affected node. Reopening MUST reflect actual saved or read-back state, not example defaults.
- **FR-013**: Modelers MUST be able to edit Line name, eastward/westward diagram orientation, block order and membership, signal names, node assignments, and output assignments. Diagram-orientation edits MUST be saved as Line metadata without changing hardware configuration. Operational edits MUST retain unaffected signal identities and compatible bindings, recompute affected links and ownership, and show changes before application.
- **FR-014**: Modelers MUST be able to remove a Line after reviewing affected references. Removal MUST release only owned signals and resources; shared blocks, detectors, and unrelated configuration MUST remain. Unknown ownership or unresolved external references MUST block destructive application.
- **FR-015**: Application across nodes MUST report per-node progress and failures truthfully; partial success MUST NOT be presented as complete, and the modeler MUST have a safe retry or recovery path without assuming atomic changes to separate physical nodes.
- **FR-016**: Offline or unavailable observations MUST be marked unavailable rather than presented as current live state. Previewed aspects MUST be identified as expectations, not observations.
- **FR-017**: An equivalent Line entered through the guided flow or a headless scenario MUST produce the same validated proposal and use the same draft and apply process; neither route may bypass validation checks.

### Key Entities

- **Automatic Block Signals**: Named, stable facility for one-direction operation, with references to existing detected blocks explicitly ordered in travel order, an eastward/westward selection for diagram orientation only, and ownership of its signals and generated resources.
- **Block**: Existing named, detected stretch of track; shared with other features and not owned or deleted by the Line.
- **Signal**: Stable, named entrance signal protecting one member block; has a host node, two physical output bindings, a restriction it publishes, and optionally a next signal in travel order.
- **Node and output binding**: A compatible Signal-LCC and its assigned red/green outputs, with availability and ownership checked against other uses.
- **Line change**: Reviewable proposed creation, edit, or removal, including per-node claims, releases, application progress, and read-back status.

### Assumptions

- Existing named blocks already have usable detection; adding detectors or placeholder blocks is outside this feature.
- The entered block list is authoritative travel order. Eastward/westward is diagram-orientation metadata, not an instruction to reverse that order or reprogram hardware.
- One-direction only does not protect opposing trains. Two-direction, APB, Plant connections, turnouts, 2- and 4-aspect policies, Restricting, approach lighting, signal-type customization, alternative displays, and Tower fallback are deferred. Each is added after the end-to-end path works, as its own increment. The S2R preview of several of these is kept as unavailable UI, not as accepted behavior.
- An RG Approach is the specified red-plus-green output combination, not a guarantee of a particular yellow hue. The modeler chooses the color-quality/resource tradeoff; Bowties does not judge it.
- The three-indication policy has no distinct advance-warning indication: a clear block ahead of a signal displaying Approach results in Clear at the preceding signal. This is an intentional refinement of the attached proposal's more conservative Approach-propagation suggestion; actual speed comparisons must be validated on supported hardware before release.
- Signal-LCC determines physical behavior when occupancy or downstream updates stop; this feature does not require the hardware to detect stale inputs or change its indication on input loss.
- Physical changes on separate nodes are not atomic; recovery must preserve and expose their actual state.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A modeler can create, review, save, apply, read back, reopen, edit, and remove a working one-block Line without entering an event identifier or a hardware logic table.
- **SC-002**: In 100% of tested occupied/clear one-block cases, the physical signal displays Stop/Clear respectively; the terminal signal never claims protection beyond its block.
- **SC-003**: In 100% of tested two- and three-block occupancy/restriction combinations for both directions, selected rules, physical output combinations (red, red plus green, green), and observed published and received restrictions match the specified Stop/Approach/Clear behavior, including a relationship spanning two nodes. Perceived yellow quality is not a pass/fail criterion.
- **SC-004**: A three-block Line on at least two compatible nodes can be applied, read back, reopened, edited, and removed without losing unaffected signal identities or shared block detection.
- **SC-005**: In 100% of tested unavailable-observation, conflict, and partial-application cases, the UI never labels an unavailable observation as live or a partial application as complete, and invalid configurations are not applied.
- **SC-006**: In a guided usability check, at least 4 of 5 modelers unfamiliar with Signal-LCC logic can identify the protected block, expected indication, and required output wiring for each signal from the review without consulting raw node configuration.
