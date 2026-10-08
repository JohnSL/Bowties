# Guided Block Signal Setup — Feature Proposal

Status: **proposed input for `speckit.specify`**.

## Purpose

Define the next Bowties feature around the first choice in the signaling
storyboard: **a signal that protects a block of track**.

The feature should deliver the smallest useful signaling workflow first, then
deepen that same workflow without introducing a different facility type or a
parallel configuration path:

1. create a working two-aspect block signal using only protected-block
   occupancy;
2. extend it to a three-aspect block signal using downstream signal state.

Both are lasting user-facing capabilities. The two-aspect case is not temporary
implementation scaffolding that disappears when three-aspect support arrives.

This proposal is intentionally about user behavior and feature boundaries. The
subsequent specification and design sessions should determine detailed
requirements, architecture, and task slices.

## Product Context

Bowties is intended to let a model railroader describe signaling in railroad
terms while Bowties owns Event IDs, CDI fields, resource selection, and node
configuration.

The current signaling storyboard begins by asking:

> What does this signal protect?

This feature supports one answer:

> A block of track — one direction of travel, protecting the block ahead of
> this signal.

The other storyboard choices are outside this feature:

- a turnout or crossing;
- a single-track section;
- a dispatcher-controlled point.

Signal-LCC is the initial hardware target because its built-in per-mast
capabilities can implement these block-signal behaviors. The user is not asked
to select the hardware strategy or configure its internal tables.

## User Problem

Today, a modeler can configure the required Signal-LCC behavior manually, but
doing so requires understanding CDI structure, Event IDs, Rule-to-Aspect
configuration, and physical output assignments.

Bowties should instead guide the modeler through recognizable railroad choices:

- which block the signal protects;
- which indications the signal should display;
- what the physical signal looks like;
- where its lamps are connected;
- and, for three-aspect operation, which signal lies ahead.

Bowties should translate those choices into a validated, reviewable
configuration and apply it without exposing Signal-LCC implementation details.

## Proposed Capability

Add guided creation, review, application, editing, and removal of a **Signal
Mast** that protects a plain block of track in one direction.

The same Signal Mast workflow supports two indication sets.

### Two-aspect block signal

The signal uses the occupancy of its protected block:

| Protected block | Local indication |
|---|---|
| Occupied | Stop |
| Clear | Clear |

This is a real supported configuration for layouts that use two-aspect
signaling.

### Three-aspect block signal

The signal uses protected-block occupancy and the state communicated by the
next mast in advance:

| Protected block | Downstream state | Local indication |
|---|---|---|
| Occupied | Any | Stop |
| Clear | Restrictive | Approach |
| Clear | Permissive | Clear |

The downstream input represents the next mast's signaling state, not merely the
occupancy of another block. The specification and design work must determine
how Bowties binds and communicates that state using Signal-LCC's built-in
capabilities.

## Intended Guided Flow

The feature follows the practical-first signaling storyboard.

### 1. Purpose

The modeler chooses **A block of track**.

Unsupported purpose choices may remain visible as future capabilities, but the
product must not imply that they work in this feature.

### 2. Protected shape

The modeler identifies the plain block and direction protected by the signal.
Turnouts, crossings, bidirectional single-track coordination, and dispatcher
control are not part of this feature.

### 3. Indications

The modeler can choose:

- **two aspects:** Clear and Stop;
- **three aspects:** Clear, Approach, and Stop.

The terminology and placement of the two-aspect choice must fit coherently with
the storyboard's signaling-family and vocabulary controls. Adding three-aspect
support must not remove or invalidate existing two-aspect Signal Masts.

### 4. Display

Bowties offers only physical display styles capable of showing the selected
indications.

The first supported physical forms should be grounded in the proven
Signal-LCC/color-light configuration:

- a compatible red/green display for two-aspect operation;
- a one-head red/yellow/green color-light display for three-aspect operation.

The indication set and physical display remain separate concepts. Selecting
three aspects does not make a particular three-lamp display the permanent
domain representation.

### 5. Bindings

The modeler binds:

- the protected block's occupancy source;
- the physical lamp outputs;
- and, for three-aspect operation, the next mast in advance.

The guided flow does not expose Event IDs, CDI paths, conditional rows, Track
Circuit numbers, or raw Signal-LCC enum values.

### 6. Confirm and apply

Before applying changes, Bowties presents a railroad-language summary of:

- what the signal protects;
- the indications it can display;
- what conditions produce those indications;
- its physical output bindings;
- and any validation problem that prevents safe application.

Applying the Signal Mast uses Bowties' normal review, draft, save, and node
configuration behavior. Cancellation or validation failure must not leave a
partially owned Signal Mast.

## User-Visible Increments

The feature should be implementable as thin, end-to-end vertical increments.
The exact slice plan belongs to later design work, but the intended behavioral
progression is:

### Increment 1 — Two-aspect block signal

A modeler can create, apply, operate, edit, and remove a Clear/Stop signal that
responds to the occupancy of one protected block.

The increment is complete only when a user can exercise the full guided flow
and observe the physical or simulated signal changing between Clear and Stop.

### Increment 2 — Three-aspect block signal

A modeler can extend the same workflow to Clear/Approach/Stop by selecting the
next mast in advance.

This increment explicitly includes upgrading an already-created and tested
two-aspect Signal Mast. The user edits that mast, changes its indication set to
Clear/Approach/Stop, and binds the next mast in advance. Bowties updates the
existing Signal Mast rather than requiring the user to create a replacement.
The protected-block binding and compatible physical output assignments are
preserved where they remain valid; any required changes or conflicts are shown
for review before application.

The increment is complete only when the user can observe:

- Stop whenever the protected block is occupied;
- Approach when the protected block is clear and the downstream state is
  restrictive;
- Clear when the protected block is clear and the downstream state is
  permissive.

The user can continue to edit the Signal Mast after the upgrade, and existing
two-aspect Signal Masts that have not been upgraded continue to work.

## Required Product Behavior

The specification produced from this proposal should cover at least these
behavioral requirements:

1. The user can create a Signal Mast whose purpose is to protect a plain block
   in one direction.
2. The user can select a lasting two-aspect Clear/Stop configuration.
3. The user can select a three-aspect Clear/Approach/Stop configuration once
   downstream cascade support is available.
4. Bowties shows only display styles compatible with the selected indications.
5. Bowties validates required node capability, configuration capacity, input
   bindings, and output bindings before application.
6. Validation problems are explicit and actionable; Bowties does not silently
   repair or overwrite unknown configuration.
7. Generated changes are reviewable and require explicit application.
8. Applying the Signal Mast configures the node through Bowties' established
   configuration workflow rather than a separate developer-only write path.
9. The user can edit an existing Signal Mast without recreating it.
10. The user can remove a Signal Mast without leaving configuration or
    resources that Bowties owns untracked.
11. Bowties does not overwrite pre-existing node configuration unless the user
    is shown the conflict and explicitly accepts a supported resolution.
12. Existing two-aspect Signal Masts remain valid after three-aspect support is
    introduced.
13. The user is not asked to choose between Signal-LCC, Tower LCC+Q, or another
    implementation target.
14. The default guided experience exposes no Event IDs, CDI paths, Track
    Circuit numbers, conditional-table rows, or generated logic.
15. A user can create and test a two-aspect Signal Mast, then edit that same
    Signal Mast to use three aspects and add its next-mast binding.
16. When upgrading an existing two-aspect Signal Mast, Bowties preserves valid
    protected-block and physical-output bindings, identifies any incompatible
    or newly required bindings, and shows proposed configuration changes
    before application.
17. Upgrading one Signal Mast does not silently create a duplicate or leave
    resources from its previous configuration untracked.

## Acceptance Evidence

The feature should ultimately be demonstrated with behavior, not merely by
confirming that configuration writes were sent.

### Two-aspect evidence

- The generated configuration can be reviewed before application.
- Applied configuration can be read back from the target node.
- Publishing or simulating protected-block occupancy produces Stop.
- Clearing the protected block produces Clear.
- Editing and removing the Signal Mast behave predictably.

### Three-aspect evidence

- The generated configuration can be reviewed before application.
- Applied configuration can be read back from the target node.
- An already-created and tested two-aspect Signal Mast can be edited in place
  to add Approach and a next-mast binding.
- The upgrade preserves valid existing bindings and explicitly presents
  required changes for review.
- Protected-block occupancy always produces Stop.
- With the protected block clear, a restrictive downstream state produces
  Approach.
- With the protected block clear, a permissive downstream state produces
  Clear.
- Existing two-aspect Signal Masts remain operational.

Hardware-in-the-loop evidence is desirable for the initial Signal-LCC target;
deterministic automated tests should cover the same behavior without requiring
hardware for every test run.

## Out of Scope

This feature does not include:

- turnout or crossing protection;
- multi-route or multi-signal Signal Plants;
- single-track APB coordination;
- dispatcher-controlled points or CTC;
- Tower LCC+Q fallback;
- user-authored signaling logic;
- a public behavior-template or scenario-file format;
- arbitrary CDI editing through the signaling wizard;
- reverse engineering arbitrary existing Signal-LCC conditionals into a Signal
  Mast;
- the full rulebook catalog;
- position-light, semaphore, or custom display authoring;
- a standalone developer-facing node-application tool.

A developer scenario runner may be proposed later if repeated hardware
experimentation with Track Circuits or Rules becomes a demonstrated bottleneck.
It is not a prerequisite for this feature.

## Relationship To Prior Work

### Practical-first storyboard

This feature is the first narrow implementation of the storyboard's **A block
of track** path. It does not attempt to implement every choice shown in the
storyboard.

### Manual Signal-LCC validation

The three-aspect Clear/Approach/Stop case has already been demonstrated
manually using Signal-LCC Rule-to-Aspect behavior. That result is evidence for
the feature and should become an acceptance fixture, not a reason to create a
parallel developer-only application workflow.

### `020-abs-signaling`

The old `020-abs-signaling` branch remains historical implementation evidence.
Useful validation, display-binding, observation, planning, resource-claim, and
cleanup ideas may be reconstructed where they fit the new contracts.

The replacement feature must not revive the old branch's Tower-LCC-first
compiler, public ABS-specific template, target-selection UX, allocation model,
or prediction-first rendering merely because those implementations already
exist.

### Headless intent pipeline proposal

The durable parts of the headless proposal belong inside this feature:

- railroad-level Signal Mast intent;
- deterministic configuration planning;
- validation against current node capability and effective configuration;
- reviewable changes;
- normal draft/application behavior;
- ownership and cleanup;
- behavior-based acceptance.

A separately supported headless application product is not currently required.
The wizard is the first production client of these capabilities.

## Input For `speckit.specify`

Create a feature specification for **Guided Block Signal Setup**.

Bowties must guide a model railroader through creating a Signal Mast that
protects a plain block of track in one direction. The feature begins with a
lasting two-aspect Clear/Stop signal driven by protected-block occupancy, then
adds a lasting three-aspect Clear/Approach/Stop signal that also responds to
the next mast in advance. Both paths use the same guided workflow and remain
supported.

The user chooses the protected block, indication set, compatible physical
display, lamp outputs, and—when three aspects are selected—the next mast.
Bowties hides Event IDs, CDI paths, conditional rows, Track Circuit numbers,
and hardware-target decisions. It validates the target node and bindings,
shows a railroad-language confirmation, and applies changes through the normal
review/draft/save workflow. The user can later edit or remove the Signal Mast
without leaving Bowties-owned configuration or resources untracked.

The first hardware target is Signal-LCC using its built-in per-mast behavior.
The feature must be validated by configuration read-back and observed signal
behavior, not only by successful write calls. Turnouts, crossings,
single-track APB, dispatcher-controlled points, Signal Plants, Tower-LCC
fallback, public template formats, and a standalone developer apply tool are
out of scope.

Use the full proposal above, the signaling charter, design decisions, current
practical-first storyboard, and durable Signal-LCC hardware documentation as
supporting context.
