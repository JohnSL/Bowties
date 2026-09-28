# Signaling UX — Goals and Constraints

This document exists so we stop revisiting the same decisions. Every proposal,
mockup, and reviewer response in this directory must be consistent with what is
written here. If a proposal cannot be, either the proposal changes or this
document is amended by deliberate edit — not by drift.

## Primary audience: the 80%

Bowties signaling is designed **first and foremost** for model railroaders who
want working signals but do not know prototype signaling in depth. That means:

- Plain-English questions, not signaling vocabulary, at every guided step.
- Sensible defaults that produce a complete, respectable signal without
  requiring a rulebook choice.
- Recognizable railroad objects (existing named blocks, existing signals,
  existing turnouts) in the UI, not generic inputs or conditions.
- No event IDs. No CDI fields. No hand-authored conditionals in the guided
  flow.

The 80% number is a stance, not a survey. It defines whose experience the
default UX optimizes for.

## Secondary audience: the 20%

Modelers who know prototype signaling are supported by:

- Prototype fidelity is reachable from the default experience without a
  separate mode, a separate entry point, or restarting.
- Signal types are first-class shareable objects with published aspect
  charts and speed pairings.
- A signal type can be customized without touching any mast that uses it.

For **unusual cases the 20% cares about but the default experience does not
cover**, the stance is explicit:

> They can drop back to configuring the underlying LCC nodes directly.

Bowties will not add per-signal expert controls to cover every rulebook edge
case. The escape hatch is the node's own configuration.

## What we will not build to reach this goal

These are constraints, not open questions. Do not re-open them without an
explicit charter change.

1. **No required track diagram.** Bowties will not require the user to draw a
   track plan for signaling to work. A track plan may become useful later; it
   is not a prerequisite for value.
2. **No user-visible hardware architecture.** Bowties chooses whether logic
   lives on Signal-LCC alone or spills to Tower LCC+Q. The user is not asked
   to make that decision. The generated implementation is inspectable, not
   authored.
3. **No per-signal expert controls.** Depth for the 20% is served via the
   signal type catalog and the fidelity choice, not by per-signal overrides.
4. **No exposed event IDs, CDI fields, or generated STL** in the default
   experience. Those exist under the hood; Bowties owns them.
5. **No separate expert experience.** The 80% and the 20% share the same
   experience; prototype fidelity is a choice within it.

## Assumptions we start from

These are what we assume the user already has when they begin signal setup.
The mockups may show them; they are not things the user is asked to
re-declare during signal setup.

- Blocks are already named and connected to block detection.
- Existing signals, if any, are already named and configured with Bowties.
- Turnouts, if present and needed for the signal, have position feedback.
- Signal-LCC (and Tower LCC+Q when needed) is on the bus and known to
  Bowties.

If any of these is missing, Bowties says so plainly and offers to defer or
placeholder the affected part. Signal setup does not become a general input
editor.

## Delivery principle: incremental value

We build in vertical slices that each ship real value to the 80%.

- The first slice ships a working guided flow for the most common signal
  Bowties can configure, and that signal is genuinely useful to run trains
  with.
- Subsequent slices add signal shapes, layouts, and prototype options.
- We do **not** wait for a complete track model, a complete prototype
  catalog, or full Tower LCC+Q support before shipping the first useful
  slice.
- **The first release prefers Signal-LCC's built-in signaling logic.** Tower
  LCC+Q is used only when Signal-LCC's built-in capabilities cannot express
  the required behavior (for example, APB direction latching and
  interlocking route lockout). Mockups and proposals for early slices
  should design signal setup around what Signal-LCC alone can do, and reach
  for Tower LCC+Q deliberately, not by default. This is what Bowties owns
  under the hood; the user is not asked to choose.

If a proposal implies "we cannot ship until X is built," the proposal has to
name a smaller shippable version or be revised.

## Phasing

The signaling wizard delivers value in four named phases. **Phases are
scope commitments, not release commitments** — a phase may span multiple
releases. The phase name defines *what audience it serves*, not *when it
ships*.

1. **Phase 1 — Signal Masts (single-signal 80% coverage).** Every
   single-signal facility a modeler can build, powered by Signal LCC's
   per-mast logic (block state, cascade from the next mast, turnout
   binding, multi-aspect display, local overrides). Block signals,
   distants, yard limits, single-signal at a turnout, single-signal
   holdout, single-signal movable bridge, single-signal territory
   boundary, spur ends. One facility type (Signal Mast), 5-step wizard,
   no Plant object. Full depth-on-demand (rulebook picker, aspect
   vocabulary rungs, layout defaults, family override).
2. **Phase 2 — Signal Plants (multi-signal 80% coverage).** Introduces
   the Signal Plant object as an ergonomic layer over the Mast
   substrate. Flagship tile: passing siding. Also: multi-signal
   single-turnout interlocking, APB coordination on single-track
   sections, signal-type reuse. Includes a **Mast → Plant conversion
   path** so users who built phase-1 sibling Masts can promote them
   into a Plant without redoing configuration.
3. **Phase 3 — Additional common Plant tiles and specialty depth.**
   Wye, single crossover, double crossover, interlocked diamond,
   funnel / end-of-double-track, multi-signal movable bridge,
   multi-signal holdout, multi-signal territory boundary,
   signal-type authoring (user-owned rulebook YAML), distant-signal
   chaining edge cases.
4. **Phase 4 — Custom / compound plants.** Facilities whose topology
   varies per site. Signal Plant with imported topology from JMRI
   Layout Editor. Bowties compiles the plant description + vocabulary
   rung + turnout bindings to STL for Signal-LCC (generation target;
   not hand-authored).

**How to use the phasing.** Before adding a tile, feature, or wizard
question to a phase, ask: *does this serve the phase's audience?* If it
does not, it belongs in a later phase. If a proposal violates the phase
boundary in the name of engineering neatness (e.g. "introduce the Plant
object in phase 1 for symmetry"), the proposal is revised, not the
phasing.

**Charter interaction.** *"No required track diagram"* applies to phases
1–3. Phase 4 requires an imported diagram — consistent with the
"modeler already has a track plan somewhere" assumption, made concrete
as an import format.

## Decisions that have been re-litigated (do not re-open without a charter change)

- Primary audience is the 80%. The 20% is served, not centered.
- No required track diagram.
- No hardware architecture in the user-visible UI.
- Bowties owns event IDs and generated logic.
- One shared signal-setup experience, with fidelity as a choice inside it.
- Signal type is a first-class shareable object.
- Practical fidelity is a complete answer for the 80%, not a lesser option.
- Existing blocks with detection are assumed to already exist.
- Progressive construction across years is not a first-class UX story.
  Progressive addition of signals to an existing layout is.
- First release prefers Signal-LCC's built-in signaling logic; Tower LCC+Q
  is used only where Signal-LCC does not suffice.
- Signaling system (ABS / APB / interlocking / CTC) is a territory-level
  property, not a per-mast question. Per-mast setup asks what the signal
  protects and what it can display; the signaling-system label is a
  description of derived behavior, not a user-authored input.
- The opening question for a signal is **"What does this signal protect?"**
  — answered with recognisable railroad objects (a block, a turnout, a
  single-track section, a dispatcher-controlled point), not with signaling
  concepts (approach signal, home signal, distant, absolute). The
  signaling-system name appears only on the confirm screen as a description
  of what Bowties derived.
- The plain-English framing has a known and accepted boundary at
  **multi-head junctions**: on a mast where two heads are read together as
  one indication, terms like *Medium Clear* and *Medium Approach* have no
  short plain-English substitute. The wizard introduces those names on the
  confirm screen, alongside the plain-English condition. The boundary is
  documented, not hidden; it is not a reason to re-open the "name the
  signaling system first" ordering.

## How this document is used

- Before writing a new mockup or proposal in this directory, read this file.
- When a reviewer's feedback would change the direction implied here, treat
  it as a charter question first: either say the charter still holds and the
  feedback belongs in a smaller refinement, or open a proposal to amend the
  charter.
- Reviewers whose position is inconsistent with the primary-audience choice
  are still valuable. Their feedback is absorbed as refinements to how the
  20% is served, not as a reason to re-center the experience on them.

## What this document does not do

This document states goals and constraints. It does not specify the
solution. It does not require a wizard, a stepper, a specific number of
screens, a specific ordering of questions, or any particular interaction
pattern. Those are decisions for the mockups and the proposal, subject only
to the constraints above.
