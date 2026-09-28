# Signaling UX — Design Decisions

This file records **concrete design decisions** that emerged during the signaling UX
work: what we chose, why, what we considered and rejected, and what we deferred.
It is deliberately more detailed than the [charter](goals-and-constraints.md) —
the charter states non-negotiable goals; this file records the model, the
mechanisms, and the trade-offs.

If a decision here conflicts with the charter, the charter wins and the entry
should be revised or removed. If a decision here becomes a non-negotiable goal
in its own right, promote it into the charter and leave a pointer here.

Entries are dated. Newer entries appear at the top.

---

## 2026-09-07 — Audience-coverage phasing (supersedes earlier four-phase entry)

**Decision.** The signaling wizard phasing is defined by *audience segment*
and *deliverable value increments*, not by shape complexity or signal count.
Each phase is a **scope commitment, not a release commitment** — phase 1 will
likely span multiple releases. The phase name defines *what audience it
serves*, not *when it ships*. A phase is "done" when it covers its audience,
not when a specific release ships.

Phases 1 and 2 together deliver the 80% story: phase 1 ships Signal Mast
support (which, on top of Signal LCC, covers *every* single-signal case
including turnout-adjacent, holdout, bridge, boundary, and siding-end
single-signal variants); phase 2 introduces the Signal Plant object so
multi-signal facilities become ergonomic first-class citizens. Splitting
them gives the 80% audience useful signaling value from day one without
gating on the multi-signal-per-plant UX.

- **Phase 1 — Signal Masts (single-signal 80% coverage).** Every
  single-signal case a modeler can build, powered by Signal LCC's per-mast
  logic (block state, cascade from the next mast in advance, turnout
  binding, multi-aspect display, local overrides). One facility type
  (Signal Mast), one wizard (5 steps, no Plant step). No Plant object yet.
  - Signal Mast use cases:
    - Block signal
    - Distant / approach signal (cascade from a downstream Mast that
      belongs to a not-yet-built plant, or from another block signal)
    - Yard limit / restricting
    - Single-signal at a turnout (any of the turnout tabs — the Mast holds
      the turnout binding and the diverging-route aspects)
    - Single-signal holdout on plain track (Mast with local dispatcher-hold
      input)
    - Single-signal at a movable bridge (Mast with bridge-open input)
    - Single-signal at a territory boundary
    - Single-signal at a spur end
  - Full depth-on-demand: rulebook picker, aspect vocabulary rungs
    (Essential / Common / Rulebook), layout defaults with on-demand dialog,
    family override.
  - Multi-signal facilities work in phase 1 by configuring each signal as
    an independent Mast with the right cascade references. Ergonomically
    tedious for large facilities but functionally complete. Phase 2 removes
    the tedium.

- **Phase 2 — Signal Plants (multi-signal 80% coverage).** Introduces the
  Signal Plant facility type as an ergonomic layer over the existing Mast
  substrate. The pitch: *"phase 1 gets you signalling; phase 2 makes your
  sidings and interlockings feel like one thing."*
  - Signal Plant tiles (this phase):
    - Passing siding — the flagship 80% multi-signal case (two turnouts,
      optional middle block, up to 4 signals)
    - Multi-signal single-turnout interlocking (facing + trailing signals
      sharing one turnout)
  - **Mast → Plant conversion path.** Users who built sibling Masts in
    phase 1 (e.g. a passing siding as 4 independent Masts) can promote
    them into a single Plant. When adding a Mast at a location where a
    sibling already exists, Bowties offers *"combine these into a Signal
    Plant?"* and extracts shared bindings (turnout references, next-signal
    refs) into plant-level state. The user never has to abandon phase-1
    work.
  - Single-track APB behavior. APB coordination between opposing signals
    on a section is a shared-state case (matches the Plant rule below);
    lands in phase 2 either as a Plant tile spanning the section or as an
    APB attribute on a single-track Mast group. Exact framing is a phase-2
    design task.
  - Signal-type reuse across masts.

- **Phase 3 — Additional common Plant tiles and specialty depth.**
  Remaining 20%-common shapes and specialty behaviors.
  - Additional Plant tiles: wye, single crossover, double crossover,
    interlocked diamond, funnel / end-of-double-track, multi-signal
    movable bridge, multi-signal territory boundary, multi-signal holdout.
  - Signal-type authoring (user-owned rulebook YAML).
  - Distant-signal chaining edge cases beyond phase 1's basic support.

- **Phase 4 — Custom / compound plants.** Facilities whose topology varies
  per site (Mitchell Schuessler's *112R* is the canonical example).
  - Signal Plant with **imported topology** from JMRI Layout Editor.
  - Bowties compiles plant description + vocabulary rung + turnout bindings
    to **STL for Signal-LCC** (generation target only; not hand-authored).

**Rationale.**

- **Ships value earliest.** Phase 1 delivers a functional signaling
  experience for every single-signal case on day one, without waiting for
  the Plant object and multi-signal-per-plant UX to be designed and built.
  Modelers with signaled mainlines can start using Bowties as soon as
  phase 1 releases; those with multi-signal sidings and interlockings can
  either wait for phase 2 or build phase-1 Masts now and promote them to
  Plants when phase 2 arrives.
- **Signal LCC already handles most of what a Mast needs.** Cascade from
  next signal, cascade from turnout state, multi-aspect display, and local
  overrides are all per-mast capabilities in Signal LCC. The Plant object
  is not required for signalling correctness; it is required for
  configuration ergonomics and group-editing.
- **Plant is genuinely a phase-2 concept.** The Plant object earns its
  place when there is shared state to hold (multi-signal coordination,
  shared bindings, group-editing, user-perceived facility identity). None
  of that exists for a single-signal facility, so introducing the Plant
  object alongside single-signal support would be premature.
- **Restores the founding phasing discipline.** Serve the 80% first, the
  20% common second, the 20% specialty third, custom last. Earlier scope
  drafts (see superseded entry below) let *shape complexity* dictate the
  phasing and put passing sidings — an 80%-common facility — in phase 3.
  Splitting the 80% audience across phase 1 (Masts) and phase 2 (Plants)
  is still "80% first" — it delivers 80% value in the fastest sequence.

**Consequences for the mockup work.**

- Two walkthroughs, not one and not three:
  - **§1 Signal Mast walkthrough** — block signal (or another
    representative single-signal case) through the 5-step wizard.
  - **§2 Signal Plant walkthrough** — passing siding through the 6-step
    wizard, showing the multi-signal-per-plant UX and the Mast→Plant
    conversion flow. Labeled as *phase 2* in the storyboard.
- **Scope panel** at the top of the storyboard names all four phases and
  clarifies that phase 1 (Masts) is the current implementation target and
  phase 2 (Plants) is the immediate follow-on.
- **§3 Roadmap section** at the end covers phases 3 and 4 in prose with
  tile lists, no frames.
- Frame 1's existing four tabs stay in the phase-1 walkthrough: Block,
  Single-track section, Turnout / junction, and Dispatcher-controlled
  point all produce Signal Masts in phase 1. In phase 2, the same tabs
  can produce a Signal Plant when the user picks a multi-signal tile.

**Consequences for the forum reply.**

- Mitchell's 112R lands in **phase 4** (compound plant via import). The
  reply can name the phase and describe the STL generation target.
- Balazs's *"Rockville needs 3 signals"* lands in **phase 2** as a
  passing-siding-or-multi-signal-interlocking case, or spans phase 2 + 3
  if it includes a wye or crossover.
- Balazs's single-crossover tile request lands in **phase 3**.
- Balazs's ABS/APB observation applies to **phase 2** (APB coordination is
  a shared-state case).
- The story to tell modelers: *"phase 1 gets you every kind of
  single-signal facility. Phase 2 makes your sidings and interlockings
  first-class. Phase 3 adds the rest of the common canonical shapes.
  Phase 4 handles compound plants via imported diagrams."*

**Charter alignment.**

- Charter's *"no required track diagram"* stance still applies to phases
  1–3. Phase 4 requires imported topology.
- Charter's *"80% first"* stance is served by delivering phase 1 quickly
  and following with phase 2 to complete the 80% experience.

**Open follow-ups.**

- **APB framing in phase 2.** Whether APB coordination on a single-track
  section is a Plant tile (a *"single-track section"* Plant that spans
  multiple masts), an attribute on a group of Masts, or something else
  is a phase-2 design task.
- **Mast → Plant conversion UX.** Trigger surface (when to offer),
  confirmation flow, and how shared bindings are extracted — all
  phase-2 design tasks.
- **Single-crossover tile timing.** Now in phase 3. Could be argued
  earlier if user data warrants.
- **Crossover tile disambiguation.** The current *"Crossover"* SVG shows
  a double crossover. Phase 3 either adds a separate single-crossover
  tile (Balazs's request) or relabels the existing one.
- **Distant-signal chaining.** Basic distant support is a phase 1 Mast
  feature. Chained distants and complex advance-signal behavior are
  phase 3.

**Related.**

- Depends on the *Signal Mast + Signal Plant facility-type split* decision
  (below).
- Supersedes the *Four-phase roadmap with canonical multi-signal plants as
  phase 3* entry (further below).
- Amends [`goals-and-constraints.md`](goals-and-constraints.md): the phase
  names should be reflected there if not already present.

---

## 2026-09-07 — Signal Mast + Signal Plant as peer facility types

**Decision.** The single *"Signal facility"* concept is split into two peer
facility types:

- **Signal Mast** — the atomic signaling facility. Any *single-signal*
  installation is a Mast, regardless of what it protects. Block signals,
  distants, yard-limits, single-signal-at-a-turnout, single-signal
  holdouts, single-signal movable bridges, single-signal territory
  boundaries, and spur ends are all Masts. The Mast carries whatever
  bindings and cascade references its purpose requires (turnout position,
  next-signal-in-advance, block state, local overrides) — all of which
  Signal LCC already supports at the per-mast level.

- **Signal Plant** — an ergonomic and stateful container for *multiple*
  Masts that coordinate as a facility. Introduced in phase 2. A Plant
  holds shared state (turnout bindings referenced by multiple masts,
  shared middle-block name, cross-mast aspect coordination), group
  identity (facility name like *"Milepost 41 siding"*), and group-level
  operations (change aspect vocabulary for the whole plant, delete as a
  unit).

**The rule.** A facility is a Signal Plant when it holds *shared state*
across multiple coordinated masts — that is, when there are at least two
Masts at the same installation sharing bindings, deriving aspects from
each other, or being operated together. Otherwise it is a Signal Mast.

**Topology alone does not make a Plant.** A single-signal facility at a
turnout is a Mast — the turnout binding lives on the Mast, and Signal
LCC's per-mast cascade handles the aspect logic. The Plant object only
enters when a second Mast is added at the same turnout and shared state
becomes real.

**Rationale.**

- **YAGNI.** The Plant object earns its existence by carrying shared
  state. If a facility has only one signal, there is nothing shared and
  wrapping it in a Plant is ceremony. The earlier version of this entry
  triggered Plant on *topology*, which forced a Plant object into every
  single-signal-at-a-turnout case with no shared state to justify it.
- **Matches what Signal LCC provides.** Signal LCC operates at the mast
  level. It already handles cascade from next signal, cascade from
  turnout position, multi-aspect display, and local overrides on a
  single-mast basis. A Mast in Bowties is the front-end for that. The
  Plant is a *design-time* aggregation, not a *runtime* object on the
  wire.
- **Enables the phase 1 / phase 2 split.** Because the Plant object is
  not required for single-signal correctness, phase 1 ships Masts alone
  and delivers value. Phase 2 introduces the Plant object as an
  ergonomic layer with a promotion path from existing Masts.
- **Prototype-flexible.** Historical usage of *"plant"* is loose in the
  single-signal case anyway — a single home signal at a spring-thrown
  switch is not usually called a plant in prototype practice. The rule
  matches the intuition that a plant is a place where *coordinated
  signaling equipment* lives.
- **JMRI-aligned.** *Signal Mast* is JMRI's atomic object. Using the
  term aligns Bowties vocabulary with the reference implementation
  `lcc-rs` mirrors on the wire.

**Consequences.**

- **Wizard shape (phase 1).** All facilities are Masts. The wizard has
  no Plant step. 5 steps: Purpose, (no Plant), Aspects, Display,
  Bindings, Confirm — renumbered.
- **Wizard shape (phase 2).** Adding a facility at a location that will
  have multiple signals produces a Plant with member Masts. The wizard
  gains a Plant step (tile picker) for those cases. Single-signal cases
  still produce Masts on the 5-step path.
- **Growth path.** When a user adds a second Mast at the same
  installation (e.g. adds a trailing signal at a turnout where a facing
  signal already exists), Bowties offers *"combine these into a Signal
  Plant?"* and, on accept, promotes both Masts into members of a new
  Plant with shared bindings extracted from their common references.
  This is the Mast → Plant conversion path called out in the phasing
  decision.
- **Frame 1 vocabulary.** The four existing tabs (Block, Turnout /
  junction, Single-track section, Dispatcher-controlled point) stay in
  phase 1 and all produce Signal Masts. In phase 2, the same tabs offer
  multi-signal Plant tiles alongside the single-signal Mast tiles.
- **Distant signals are Masts.** A distant is a Mast with a *"distant
  for [plant or downstream mast]"* cascade reference. Not a Plant
  member.
- **Storyboard rename.** *"Signal facility"* becomes *"Signal Mast"*
  throughout phase 1. *"Signal Plant"* is introduced in phase 2
  storyboard material.
- **`aiwiki/` glossary.** Terms *Signal Mast* and *Signal Plant* should
  be added to the durable glossary when phases 1 and 2 ship
  respectively.

**Alternatives considered.**

- *Topology-triggered Plant (earlier draft of this entry).* Rejected:
  forced a Plant object into single-signal-at-a-turnout cases where
  there was no shared state to justify it, and made phase 1 depend on
  the Plant object even though Signal LCC handles the underlying logic
  per-mast.
- *Single facility type with a mode flag.* Rejected: obscures the
  domain distinction and forces every signal to answer plant-level
  questions that do not apply to it.
- *Rename Signal facility to Signal Mast, never introduce a Plant
  type.* Rejected: multi-signal facilities genuinely have shared state
  (turnout bindings referenced from two masts, cross-mast coordination,
  group identity) and configuring them as N independent Masts is
  ergonomically painful at scale.
- *Use "Interlocking" instead of "Plant".* Rejected: excludes automatic
  (ABS) multi-signal cases like passing sidings. Wrong category.
- *Use "Control Point" instead of "Plant".* Rejected: CTC-only;
  excludes ABS multi-signal cases and automatic movable bridges.

**Vocabulary note.** *"Plant"* is accurate but may be unfamiliar to some
80% modelers. First-use tooltip on *Signal Plant* is expected in the UI,
consistent with Balazs's terminology-tooltip suggestion on the MRH
thread.

**Related.**

- Enables the *Audience-coverage phasing* decision (above).
- Amends the vocabulary of the 2026-08-30 layout-defaults entry and the
  2026-08-29 dispatcher-controlled entry — the underlying decisions
  still hold; only the object names change.

---

## 2026-09-07 — Four-phase roadmap with canonical multi-signal plants as phase 3

> **Superseded** by the *Audience-coverage phasing* entry above. Kept for
> history: the four-phase structure was correct in count but wrong in tile
> assignment. It let *shape complexity* dictate the phasing, which pushed
> passing sidings (an 80%-common facility) into phase 3 and violated the
> founding *"serve the 80% first"* principle. Do not use this entry as the
> current phasing plan.

**Decision.** The signaling wizard ships in four numbered phases. Each phase
defines *what plant shapes the guided flow can express*, and each phase is
usable end-to-end for the shapes it covers.

- **Phase 1 — the 80%.** Plain block signals and single-turnout
  interlockings (automatic and dispatcher-controlled). Single-signal,
  single-shape plants. Wizard configures one signal at a time. No plant
  metadata required beyond a simple tile. **No track diagram required.**
- **Phase 2 — depth on demand within phase 1's shape + specialty
  single-signal cases.** Rulebook picker, per-signal family override,
  signal-type reuse, plus single-track (plain and APB), holdout on plain
  track, movable bridge, and territory boundary. Still one signal at a
  time, still no diagram.
- **Phase 3 — canonical multi-signal plants.** Plant tiles whose topology
  Bowties knows without a diagram, so the wizard can shift from *"configure
  one signal"* to *"configure this plant's N signals as a group"* with
  shared turnout bindings and defaulted signal-type reuse. **No diagram
  required.** The canonical tile set is:
  - **Single crossover** — one diagonal, two turnouts in tandem, one signal
    per direction of use (2 signals total).
  - **Double crossover / scissors** — two diagonals in an X (the current
    *"Crossover"* SVG in the storyboard is actually this). 4 signals.
  - **Interlocked diamond** — two tracks cross without connecting.
  - **Wye** — with the caveat that all three legs are treated as
    signaling-equivalent; asymmetric wyes fall to phase 4.
  - **Passing siding** — two turnouts sharing a known middle block; up to
    4 signals with shared turnout objects at each end.
  - **End of double-track / funnel** — two parallel tracks converge to
    one via a turnout or single crossover; typically 3 signals.
- **Phase 4 — arbitrary / compound plants.** Facilities whose topology
  varies per site (Mitchell Schuessler's *112R* is the canonical example:
  a crossover, a restricted-speed siding, and a medium-speed branch, all
  at one plant). Requires a **facility-scoped track diagram imported from
  JMRI Layout Editor** (or an equivalent format). **Bowties will not build
  a diagram editor** — import only. Bowties compiles the plant description
  plus vocabulary rung and turnout bindings to **STL for Signal-LCC**
  (STL is a generation target, not something the user hand-authors).

**Rationale.**

- The 80% audience is a phase-1 audience. Everything beyond that has to
  earn its place by being common enough on model railroads to belong in
  the guided flow rather than the escape hatch.
- Balazs Racz's forum feedback (2026-09) surfaced that multi-signal plants
  are unavoidable even on modest layouts (his Rockville example needs
  three signals; a double crossover needs four). Deferring all of them
  to a diagram-import phase would push a large fraction of realistic
  layouts out of the guided flow.
- The distinguishing feature of phase 3 is **canonical topology**:
  Bowties can bake the shape, the number of typical signal positions,
  the turnout count, and the route enumeration into the tile metadata.
  The user still supplies which canonical positions are present,
  per-signal block and next-mast names, and turnout bindings — but no
  diagram is required because there's nothing to draw that the tile
  doesn't already encode.
- Phase 4 exists precisely for the case where topology is not canonical.
  Rather than building a diagram editor for that, Bowties imports the
  layout diagram the modeler already has in JMRI Layout Editor.

**Renumbering.** An earlier session called the canonical-multi-signal
tier *phase 2.5* and the arbitrary-compound tier *phase 3*. The whole-number
renumbering (phase 3 / phase 4) is adopted here for clarity — half-numbered
phases signal "afterthought" and the canonical-multi-signal tier is not an
afterthought.

**Charter consequence.** The charter's *"no required track diagram"* stance
applies to phases 1, 2, and 3. Phase 4 requires an imported diagram.
Consistent with the charter's existing assumption that the modeler already
has a track plan somewhere; phase 4 turns that assumption into an import
format.

**What phase 3's tile set replaces.**

- **Three-way turnout.** The current step-2 tile advertises *"two diverging
  routes"* but the follow-up questions only accept one answer, so it cannot
  represent either the compound (two consecutive turnouts) or single-unit
  (rare, Peco) form honestly. The tile is **hidden in phases 1 and 2**;
  both forms are handled by phase 4 (imported diagram). A phase-3 tile
  for the single-unit variant is possible in principle but not prioritized.
- **Compound plants generally (Mitchell's 112R).** Phase 4, always.
  Signal-type reuse does not rescue compound plants — a signal type is a
  reusable display + indication list, not a plant description.
- **Routes-table stopgap.** An earlier session considered a step-2 routes
  table that would let the user declaratively enumerate routes for a
  compound plant. **Rejected as throw-away work** relative to the phase-4
  diagram-import path. Compound plants that need explicit route
  enumeration also benefit from a picture; jumping to phase 4's
  schematic-based description is cleaner than building an interim.

**Distant signals.** Balazs's *"Rockville needs three signals"* count
includes a distant signal in advance of the plant. Distant signals live
upstream on the approaching block, not at the plant itself. They configure
as a **phase-1 block signal** with an option *"this signal is a distant
for [interlocking name ahead]."* They are **not** part of a phase-3
multi-signal plant configuration.

**Wye caveat.** Wyes stay in phase 3 with the assumption that all three
legs are signaling-equivalent (typically the case; wyes on model layouts
are usually engine turnarounds). If the legs have materially different
signaling contexts, the user waits for phase 4.

**Open follow-ups.**

- **Single-crossover tile timing.** Could arguably land in phase 2 as a
  single-turnout-like case with one signal per direction. Decision
  deferred; leaving it in phase 3 keeps all crossovers together.
- **Crossover tile relabeling.** The current *"Crossover"* SVG shows a
  double crossover. When phase 3 lands, either add a separate
  single-crossover tile (Balazs's specific request) or relabel and
  disambiguate the existing tile.

**Related.**

- Storyboard §2 Frame 2 (plant tile pickers) will need a way to convey
  the phase boundary — decision on how to render this in the mockup
  is separate from this entry.
- [`goals-and-constraints.md`](goals-and-constraints.md) — the charter's
  no-diagram stance is scoped to phases 1–3 by this decision.
- Amends the 2026-08-29 *"Dispatcher-controlled plant step covers
  non-turnout Control Points"* entry: the eight-tile set defined there
  is a phase-1 + phase-2 set; the *interlocked diamond* and *wye* tiles
  named there also carry phase-3 semantics once phase 3 is built.

---

## 2026-08-30 — Layout defaults become an on-demand dialog + post-first-signal confirm

**Decision.** *Frame 0* is no longer a first-signal gate. Instead:

- The wizard starts every layout with **built-in defaults** (`speed signaling
  · Essential`). No pre-signal quiz.
- Frame 0 becomes a normal **on-demand *Layout defaults* dialog**, reachable
  from a small `change defaults…` link next to the aspect step's *Aspect
  vocabulary* field-label, and from a global settings menu. Same content and
  same *"Help me pick"* panel as before, different trigger.
- After the first signal completes, a small **confirm** asks *"Save Speed ·
  Essential as your layout defaults?"* with `Save as defaults` / `Not this
  time`. Shown once per layout, ever.
- The wizard-chrome **layout-defaults strip** (the `Layout defaults: speed ·
  essential · change` line above every step) is removed. The `· inherits
  layout default` microcopy on the step-3 field-labels carries the
  inheritance signal where it is most useful.

**Rationale.** The first-signal gate asked the user to declare preferences
before they had any evidence to answer with. The post-wizard confirm asks
with evidence — they have just made the choices and know what they wanted.
Matches how normal software works: use defaults, then codify preferences.
Frees the chrome strip to be removed, which was the largest single vertical
cost on every wizard step.

**Consequences for later frames.**

- **All chrome-defaults strips** were removed from the storyboard (10 strips
  across easy-case Frames 3–6 and hard-case Frames 1–6).
- **Frame 3 easy** grew a small `change defaults…` link next to the
  *Aspect vocabulary* field-label as the primary entry point into the
  on-demand *Layout defaults* dialog.
- A **post-first-signal confirm frame** was added after Frame 6 in the easy
  case, marked with a check-mark frame number instead of a step number to
  signal that it is not part of the six-step wizard.

**Alternatives considered.**

- *Confirm after every non-default signal.* Rejected: risks nagging. The
  on-demand dialog covers the "I just realized I want Common as my default"
  case explicitly.
- *Move the chrome strip to the dialog footer instead of removing it.*
  Rejected: still consumes vertical space on every step for a piece of
  information already carried by the field-label microcopy.
- *Keep Frame 0 as a first-signal gate.* Rejected: charter-consistent but
  poor UX — users answer a preferences quiz before they know what to prefer.

**Related.** Storyboard §1 Frame 0 (repurposed) and Frame 6+ (new confirm).
Amends the 2026-08-29 *"Help me pick" is a short curated helper* entry
below: the helper now lives inside the on-demand dialog rather than being
the first thing a new layout sees.

---

## 2026-08-29 — Dispatcher-controlled plant step covers non-turnout Control Points (Option A)

**Decision.** The step-2 *Dispatcher-controlled point* plant tab exposes eight
shapes rather than reusing the turnout tab's five. Five turnout-based
shapes carry over (single turnout, three-way, crossover, wye, interlocked
diamond); three non-turnout shapes are added:

- **Holdout on plain track** — absolute signal in a block with no turnout,
  used for dispatcher-forced meets, holds, or work-zone protection.
- **Movable bridge** — drawbridge / swing / lift span; the bridge tender
  forces Stop when the span is open.
- **Territory boundary** — the point where CTC meets automatic block, or
  similar dispatcher-controlled / automatic-block transition.

**Rationale.** The previous shape *"a dispatcher-controlled point"* on step 1
was mapped to *"CTC over an interlocking"* on the confirm screen, and the
step-2 plant tab was a thin wrapper over the turnout plant panel. That
implicitly claimed every dispatcher-controlled signal sits at a turnout.
Prototype practice puts dispatcher control at anything defined as a Control
Point, including plain-track holdout signals, movable-bridge protection, and
territory transitions. Adding non-turnout tiles here lets the wizard describe
those cases without asking *"is this dispatcher-controlled?"* as an extra
checkbox on step 1 (which the *"operator model is not a wizard question"*
decision below explicitly rejects).

**Alternatives considered.**

- *Option B — a "dispatcher-can-hold" checkbox on step 1 combined with any
  plant shape.* Rejected: reintroduces the operator-model question we
  already decided not to ask, and cross-multiplies step-1 shapes × dispatcher
  toggle without adding UX clarity for the 80%.
- *Leaving the turnout-only plant panel and asking the 20% to configure
  underlying nodes for holdout / bridge / boundary cases.* Rejected as the
  default: those cases are common enough on real layouts (single-track meets,
  drawbridges, division boundaries) that they belong in the guided flow, not
  the escape hatch.

**Confirm-screen consequence.** The confirm screen names the derived system
differently based on the tile:

- Turnout-based tiles → *CTC over an interlocking*
- *Holdout on plain track* → *Dispatcher-controlled absolute signal*
- *Movable bridge* → *Interlocked movable bridge*
- *Territory boundary* → *CTC / ABS boundary signal*

**Related.** Storyboard §1 Frame 2d.

---

## 2026-08-29 — Plant step uses a compact left-side picker plus one canonical diagram

**Decision.** On the turnout and dispatcher-controlled-point plant tabs (§1
Frames 2b and 2d), the shape picker is a **single-column list on the left**
with a tiny icon (56 × 28) and a short label per row. The right-hand preview
panel holds **one canonical diagram** for the currently selected shape. The
*"where does this signal sit?"* question is answered by clicking **radio-button
markers placed on the preview diagram itself**, at each valid signal entry
point — not by a second grid of position mini-tiles. Diverging-speed and
diverging-cascade follow-ups sit as a two-column pair of radio rows below the
diagram.

For shapes with only one valid signal position (diamond, holdout on plain
track, movable bridge, territory boundary), the diagram shows a single fixed
signal marker rather than a picker, and the diverging-speed / cascade rows
disappear.

**Rationale.** The earlier design used a 3-column tile grid where every
picker tile carried its own SVG (200 × 80), and the preview panel repeated a
larger version of the same SVG plus a second 3-tile grid for signal position.
Measured height on the turnout tab was ~1044 px, which overflowed a 1080p
viewport with browser chrome. The redesign:

- **Removes duplicated diagrams.** The canonical diagram appears once, in the
  preview. Picker rows only need tiny disambiguating icons.
- **Folds the "where does this signal sit?" question into the diagram.** The
  radio markers *are* the picture. Each shape's diagram carries its own valid
  marker set by construction — no conditional UI branch for shapes with
  different position options, no special-case for diamond / bridge / holdout /
  boundary.
- **Uses horizontal space the wide dialog already had.** Two-column layout
  (220 px picker + preview) fits comfortably at 1080p and at 1366 × 768
  laptop widths.

Measured heights after the change: turnout ~649 px, dispatcher-controlled
point ~768 px. Both fit inside a 1080p viewport with room for browser chrome.

**Trade-off named.** Users lose at-a-glance side-by-side visual comparison of
all shapes in the picker (the tiny icons disambiguate but do not fully
illustrate). The rebuttal is that shape names (*single turnout*, *wye*,
*movable bridge*) are unambiguous to the audience, and clicking a picker row
swaps the preview instantly so any shape's full diagram is one click away.
The trade-off was raised and accepted during the design conversation.

**Alternatives considered.**

- *Two-column follow-up layout inside the existing preview panel plus
  uniform tightening of paddings.* Would have saved ~65–130 px — enough for
  1080p but not for 1366 × 768. Rejected as insufficient headroom.
- *Collapse the tile grid to a compact chip once a tile is selected.* Big
  height savings but loses shape comparison entirely until the user reopens
  the picker. The compact left-side picker keeps the shape list permanently
  visible instead.
- *Progressive disclosure of the follow-up questions behind a "Configure"
  button.* Rejected: hides the questions the plant step exists to ask.
- *Splitting the turnout plant into two sub-steps (shape → follow-ups).*
  Rejected: adds a step and contradicts the 6-step decision from earlier the
  same day.

**Applied to.** Turnout / junction tab (§1 Frame 2b) and dispatcher-controlled
point tab (§1 Frame 2d). Not applied to block (§1 Frame 2a — already a
one-click confirmation with no picker) or single-track (§1 Frame 2c — only
three shapes, no position or route follow-ups; already fits comfortably).

**Related.** Storyboard §1 Frames 2b and 2d.

---

## 2026-08-29 — Plant is its own step; step count grows from 5 to 6

**Decision.** The wizard is restructured from five per-signal steps into six by
inserting a new *Plant* step between *Purpose* and *Aspects*:

1. **What does this signal protect?** — object-shaped answers (block / turnout /
   single-track / dispatcher point). Unchanged.
2. **Plant** — *new dedicated step*. What does the thing being protected
   actually look like (shape, routes, termination)? Its content depends on
   step 1's answer.
3. **Aspect vocabulary** — Family (speed / route) plus a *vocabulary ladder*
   (see the collapse decision below). Takes the plant plus step 1 as input and
   produces the indication table.
4. **Display** — heads + style. Head count is derived from the indication
   table; head style is filtered by what can physically display it.
5. **Bindings** — pick the specific named block / turnout / next signal on
   the layout; lamp wiring. Unchanged from the previous model.
6. **Confirm** — derived behavior, signaling-system label, pocket card.
   Unchanged.

**Rationale.** The previous single "step 2 produces the indication table"
worked for a block signal but *under-specified* the turnout / interlocking /
APB cases. Deriving the indication table for a turnout requires plant facts
(routes, diverging speed class, whether the diverging route reaches another
controlled signal) that don't exist for a block. Cramming those onto the
aspect step as a "conditional sub-panel" would have hidden them behind
adaptive UI text; promoting the plant to its own step gives it visible weight
and separates two categorically different decisions:

- **Plant** — what is the signal looking at?
- **Aspects** — what should the signal be capable of displaying?

The 80% signal (plain block) pays at most one extra confirmation click on the
plant step (see the block-variant decision below). The 20%-shaped signals
(interlockings, APB) get their plant questions promoted from a hidden
sub-panel to a first-class step. It also cleans up ownership: the plant
becomes a distinct object on the signal draft, and the aspect-derivation
helper takes `(plant, family, vocabulary-rung)` → indication table as a clean
signature.

**Alternatives considered.**

- *Conditional sub-panel on the aspect step.* Rejected: hides the plant
  questions behind adaptive text; muddles derivation ownership.
- *Skip the plant step entirely when step 1 = block.* Rejected in favor of a
  uniform 6-step flow with a one-click plant confirmation for the block case
  — consistency for the 80% wins over saving one click.
- *Fold Bindings into Plant.* Considered but rejected for this iteration.
  The bindings step does real work (picking the specific named block on the
  layout, picking the "next signal," assigning lamp wiring) that mixes
  layout-inventory questions with hardware wiring. Folding those into Plant
  would muddle Plant's job (plant shape) with layout-inventory. Kept
  Bindings as its own step; may revisit once the layout-inventory model is
  more settled.

**Related.** Storyboard §1 Frames 2a–2d (four plant-step variants), §1 Frame 3
(aspect step), §1 Frame 4 (display step), §1 Frame 5 (bindings), §1 Frame 6
(confirm).

---

## 2026-08-29 — Plant step uses schematic tiles with an inline preview panel

**Decision.** The plant step (except the block variant) presents plant shapes
as **medium-sized schematic tiles** — large enough (~200×120 px) to read a
wye vs. a 3-way turnout vs. a single crossover at a glance without squinting.
Selecting a tile expands an **inline preview panel** underneath containing:

- A larger annotated version of the same schematic (route labels, marked
  signal position, direction-of-travel arrows).
- **Contextual follow-up questions** for that plant shape only (e.g.
  *diverging speed class* for a single turnout, *"does the diverging route
  lead to another controlled signal?"* for turnouts and 3-ways). Follow-up
  questions vanish for shapes that don't need them (a wye implies both routes
  are slow).
- A one-sentence *"what this plant implies"* note (e.g. *"Because this
  diverging route reaches another controlled signal, the aspect vocabulary
  needs Approach Medium."*).

**Iconographic, not scale-accurate.** The schematics are illustration, not
authoring. They are fixed-size, non-interactive, standardised symbols. This
does not violate the charter's *"no required track diagram"* rule because the
user is picking from a menu of pre-drawn schematics, not drawing anything. If
the tiles ever become draggable or editable, we've slid into a track-plan
editor and must stop.

**Signal position marked.** Each schematic shows a small signal symbol at the
location of the signal being configured, so the user sees whether their
signal governs *the approach to* the plant or *is* the plant signal.

**Rationale.** Plain-English prose ("a turnout with one diverging route at
medium speed leading into another controlled signal") makes the reader
translate words into a picture; a schematic *is* the picture. It matches how
visual thinkers in the 80% actually reason about their layouts, and it
disambiguates for the 20% in one glance where prose has to spell out several
distinctions (3-way vs. single crossover vs. wye).

**Alternatives considered.**

- *Small icons in a tile grid* (~48 px). Rejected: 3-way, single crossover,
  and wye all look like "line with a fork" at that size; users would misread.
- *Text-first radio list with schematic below.* Rejected as default: prevents
  side-by-side visual comparison across options; presents the "which shape?"
  question in text first, which is backwards for the visual thinker. Kept as
  a fallback for any variant whose tile grid doesn't render legibly at the
  real UI width.

**Related.** Storyboard §1 Frames 2b (turnout), 2c (single-track), 2d (CTC
point).

---

## 2026-08-29 — Block-variant plant step is a one-click confirmation

**Decision.** When step 1 = *a block of track*, the plant step (§1 Frame 2a)
is a single confirmation panel: one schematic showing two signals bracketing
a block with the *this* signal marked, plus copy along the lines of *"A plain
block between two signals. Nothing here needs to be configured — detection
tells the signal what it needs to know."* Next advances to the aspect step.

**Rationale.** A block signal genuinely has no plant sub-questions to ask
(detection is authoritative). Keeping the block variant as an explicit step
— rather than skipping it — preserves a uniform 4-step flow across all
signals, gives the modeler the reassurance of Bowties acknowledging "yep,
plain block, nothing to configure," and keeps the wizard chrome consistent.

**Alternatives considered.**

- *Auto-skip the plant step for block signals.* Rejected: introduces a
  branch in the flow, makes the step counter jump, and denies the 80% modeler
  the small confirmation moment that says "you're on the standard track."

**Related.** Storyboard §1 Frame 2a.

---

## 2026-08-29 — Direction-latching lives in the plant, not the aspect step

**Decision.** For a single-track section, direction-latching (APB behavior —
traffic direction sticks until cleared) is a **plant fact**, expressed by
picking the *"single track with meeting sidings"* tile on the plant step. The
user does not check a "direction-latching" box; they pick the plant shape
whose behavior requires it, and Bowties derives the latching logic.

**Rationale.** Direction-latching is a property of the plant (how trains
actually run on this section), not of what the signal can display. Its
user-visible consequence — that *Restricting* must be in the aspect
vocabulary — falls out of the aspect-derivation helper, which is exactly the
kind of derivation the plant → aspect boundary is meant to carry.

**Related.** Storyboard §1 Frame 2c.

---

## 2026-08-29 — Fidelity and aspect-set size collapse into one "vocabulary" axis; Family stays separate

> **Revised same day (2026-08-30) — three rungs, backed by a JMRI-derived rulebook catalog.**
> The 2026-08-29 decision below took the ladder from 3 controls to a single 6-rung ladder. A
> follow-up review the next day observed that four of those rungs (Essential/Common ×
> plain/rulebook names, plus Full rulebook) were really two axes hand-authored to look linear,
> and that reduced-appearance variants of rulebooks are already shipped by JMRI as separate
> catalog entries. Collapsing further: the ladder becomes **three rungs — Essential, Common,
> Rulebook** — where *Rulebook* opens a searchable JMRI-derived catalog with a full/reduced
> filter and a *reduced* badge on reduced-appearance entries. Custom is removed from the ladder
> entirely; authoring a user-owned rulebook YAML file becomes a **separate UX outside facility
> definition**, and those user rulebooks will appear in the same catalog as JMRI-derived entries.
> Layout-default rung on Frame 0 shrinks to *Essential* / *Common* (picking a specific rulebook
> as the default is done via the *Help me pick* panel, which selects a rulebook starter).
>
> Rationale for the further collapse:
>
> - **Fewer honest rungs.** The two hybrid rungs (Essential/Common × rulebook names) served the
>   "prototype names but operator-friendly length" case. That case is served better *by picking
>   a reduced-appearance catalog entry*, because JMRI already publishes those variants — the
>   hand-authored hybrid rungs were rebuilding what the catalog already contains.
> - **One shape for a rulebook.** Custom-as-a-ladder-rung created a second shape for the "aspect
>   list" concept (freeform edit vs. catalog entry). Deferring Custom to "author a rulebook YAML"
>   collapses that to one shape — a rulebook file — that Bowties reads the same way whether it
>   came from the JMRI catalog or from the layout folder.
> - **Cleaner ladder shape.** Three rungs each answer a distinct user intent: *"I don't care
>   about names" → Essential*, *"I need diverging-speed indications without picking a rulebook"
>   → Common*, *"I want prototype names" → Rulebook*. No mixed-purpose rung.
>
> The 2026-08-29 record below is preserved because the collapse-in-two-steps history explains
> why Family stays separate and why *Full without a rulebook* was rejected — both still hold.

**Decision.** The aspect step (now step 3) has **two** controls, not three:

- **Family** — speed vs. route signaling. Kept as its own control; it is
  genuinely orthogonal to vocabulary (a signal can be Practical-Essential in
  speed *or* route flavor; it can be PRR-Full in either flavor depending on
  how diverging speeds are modeled). Family changes the *shape* of the
  indication table.
- **Vocabulary ladder** — a single ordered choice that absorbs the previous
  separate *fidelity* and *aspect-set-size* controls:
  1. *Essential set — plain names* (was: Practical + Essential)
  2. *Common set — plain names* (was: Practical + Common)
  3. *Essential set — my railroad's names* → pick rulebook (+ era if
     applicable)
  4. *Common set — my railroad's names* → pick rulebook (+ era)
  5. *Full rulebook* → pick rulebook (+ era)
  6. *Custom — I'll author the list*

**Rationale.** The previous three-control layout had an incoherent case:
*"Practical + Full"* is meaningless — there is no exhaustive canonical list
for a generic vocabulary to be *full of*. That case revealed that fidelity
and size are not two fully independent axes: **Full only makes sense with a
rulebook.** Modeling them as a single ordered ladder makes that soft
constraint explicit in the UI and removes the incoherent combination. It
also gives the modeler one honest question (*how deep do I want to go with
naming?*) instead of two overlapping ones.

Family stays separate because it *is* independent — it changes the number of
head positions and the shape of the indication table, not the naming
vocabulary.

**Rulebook picker appears when the ladder rung requires it.** Rungs 3–5
reveal a rulebook selector; rungs 1–2 do not. This keeps the 80% flow
uncluttered.

**Alternatives considered.**

- *Keep three separate controls (Family / Fidelity / Size).* Rejected:
  leaves the incoherent "Practical + Full" combination on-screen; hides that
  the two axes interact.
- *Fold Family into the vocabulary ladder.* Rejected: Family and vocabulary
  are genuinely independent; combining them would explode the ladder into
  ~10 rungs and re-tangle two separate decisions.

**Related.** Storyboard §1 Frame 3.

---

## 2026-08-29 — Era is a sub-choice inside the rulebook picker, only when applicable

**Decision.** When the vocabulary ladder selects the *Rulebook* rung (see the 2026-08-30
revision on the entry above), the rulebook picker exposes an **era sub-selector** — but only
for rulebooks whose aspect chart has era-dependent variants worth exposing (PRR pre- vs.
post-1956, NORAC 9th vs. 11th edition, GCOR revisions, and similar). For
rulebooks without meaningful era splits, the sub-selector doesn't appear.

The default era is the most-modeled edition for the selected rulebook.

Era **never appears** at the *Essential* or *Common* rungs (plain-name vocabulary).

**Rationale.** Prototype rulebooks change over time; a 20% modeler picking a
rulebook may care about a specific edition. But surfacing era as a top-level
input would inflate the aspect step for every modeler including the 80%, and
most rulebooks don't need era at all. Sub-selector inside the rulebook picker
is the right home: contextual to the choice that needs it, invisible
otherwise.

**Curated helper carries an era hint.** The Frame 0 "help me pick" list
already picks a specific era per railroad entry (e.g. *"PRR — 1956+ position
light"*), so a modeler picking a road implicitly picks an era. That
propagates through as a default at the rulebook picker.

**Related.** Storyboard §1 Frame 3, §1 Frame 0.

---

## 2026-08-29 — Absolute vs. permissive is derived from step 1, not asked

**Decision.** Whether the signal is *absolute* (must show Stop) or
*permissive* (can show Stop-and-Proceed / Restricting) is derived from step
1's answer, not a user question:

- *Block of track* → permissive
- *Turnout / junction* → typically absolute (interlocking home signal)
- *Single-track section* → permissive by default; APB variant may add
  Restricting
- *Dispatcher-controlled point* → always absolute

**Rationale.** The user already told us what the signal protects on step 1.
Asking again "is this signal absolute?" would be asking them to translate
their own object-shaped answer into signaling-department vocabulary — the
exact translation the charter's plain-English default is meant to spare
them. The absolute/permissive distinction is a downstream property of the
plant, not an independent user decision.

**Related.** Aspect-derivation helper; storyboard §1 Frame 5 (confirm names
the derived permissiveness in plain English).

---

## 2026-08-29 — Operator model (tower / station operator / CTC) is not a wizard question

**Decision.** *Who controls the signal* (a tower operator, station operator,
dispatcher via CTC, or automatic block logic) is **not asked** on any wizard
step. It is derived from step 1's answer combined with layout-level context.

**Rationale.** The operator model is an operations question, not an aspect
question. Its effect on the aspect table is mediated through
absolute-vs-permissive, which step 1 already determines. Its effect on
cascade logic is generated automatically by Bowties. Asking it directly
would be asking the modeler to think about who runs the railroad — not
useful information at signal setup time for the 80%, and the 20% modeler
who cares gets it from step 1's *"dispatcher-controlled point"* option.

**Related.** Storyboard §1 Frame 1 (options on step 1 imply the operator
model).

---

## 2026-08-29 — Wizard is aspect-first: step 2 produces the indication table; step 3 displays it

> **Superseded later the same day** by the *"Plant is its own step"* decision
> at the top of this file. The aspect-first ordering principle still stands
> (aspects determine head count, not the other way round), but the step
> numbers below ("step 2", "step 3") shifted: the aspect step is now step 3,
> the display step is step 4. Read this entry for the *why*, then apply the
> new numbering.

**Decision.** Steps 1 + 2 together fully determine the indication table (the list
of aspects this signal must be able to display). Head count is derived from the
table. Step 3 becomes *"Display these N aspects on H heads"* — head style is a
filtered display-technology choice; head count is a shown constraint, not a user
choice.

**Rationale.** The previous storyboard presented step 3 (Signal style) as a peer
of step 2. It is not: the number of heads and their per-head lamp count fall out
of the indication table, and the head style is a filter over what can physically
display that table. Presenting head count as a *fait accompli* on step 3 hid the
fact that step 2 had already decided it. Aspect-first ordering matches how the
underlying model actually flows.

**Prototype-rulebook shortcut.** If step 2 selects a rulebook that implies a
style (PRR → position light; most modern US roads → color light), Bowties
preselects the style on step 3. The user can change it.

**Alternatives considered.**

- *Keep style as a peer of fidelity.* Rejected: hides the derivation and lets an
  unbuildable style/aspect pairing be offered.
- *Fold style into step 2.* Rejected: bundles two decisions with different
  vocabulary (indications vs. display technology) into one screen.

**Related.** Storyboard §1 Frame 3 and §3 Frame 3.

---

## 2026-08-29 — Aspect set size is a first-class dial

> **Superseded later the same day** by the *"Fidelity and aspect-set size
> collapse into one vocabulary axis"* decision at the top of this file. The
> underlying insight — that operator comprehension matters and modelers ship
> reduced-appearance rulebook variants — still stands and drives the shape
> of the new vocabulary ladder (Essential / Common / Full rungs). The
> separate *Aspect set size* control is gone; those size levels are now
> rungs on the single vocabulary ladder.

**Decision.** Step 2 exposes an *aspect set size* choice with three levels:

- **Essential** (3–4 aspects) — the smallest coherent set for what step 1
  protects. Fits on a business-card cheat sheet.
- **Common** (5–7 aspects) — adds diverging-speed or extended-approach
  indications. Fits on an index-card cheat sheet.
- **Full prototype** (all rulebook aspects) — only meaningful when a Prototype
  rulebook is selected. Fits on a rulebook page.

Default is *Essential* under Practical and under route signaling; *Common* under
speed signaling when step 1 is a turnout or a dispatcher-controlled point.
*Full prototype* only becomes the default when the user explicitly selects a
Prototype rulebook. The user can always dial up or down.

**Rationale.** Operator comprehension is a real design constraint on model
railroads. Community practice already ships reduced-appearance rulebook variants
(JMRI does this) and layouts routinely use a smaller subset of a rulebook's
aspects specifically so operators can read at a glance. The previous *Practical*
label hid this choice inside a single opaque option. Making it an explicit dial
lets a modeler prioritise operator ergonomics without going Custom, and lets a
*Prototype*-labelled signal still be operator-friendly.

**Confirm-screen consequence.** The confirm screen offers a *"Print pocket card"*
action so the operator ergonomics are visible at completion. The card size
mirrors the aspect-set label (business card / index card / rulebook page), which
makes the trade-off tangible.

**Alternatives considered.**

- *Leave the choice inside Practical.* Rejected: opaque; a Prototype-labelled
  signal would either be Full or nothing.
- *Model it as a per-signal free-form aspect picker.* Rejected: reintroduces the
  "author every indication" burden the wizard is meant to avoid.

**Related.** Storyboard §1 Frame 2 and §3 Frame 2.

---

## 2026-08-29 — Speed vs. route signaling is a layout-level default with per-signal override

**Decision.** *Signaling family* — speed vs. route — is a layout-scoped setting
shown in the wizard chrome (`Layout defaults: speed signaling · Essential
aspects · change`). Any signal inherits it silently. A per-signal override lives
on step 2 as a plain segmented control, so changing it for one signal is a
one-click action, not a hidden path.

The override panel offers a checkbox: *"Also make this the layout default."*
Off by default (per-signal change only); the 20% modeler helping the 80% modeler
can flip one signal without ripple.

**First-time capture.** When no layout default exists yet, Bowties asks once at
first-signal setup with a short *"help me pick"* panel (see the next decision).
Default fallback: *speed signaling* (matches modern commercial-kit exposure).

**Rationale.** Community pushback on hiding the speed/route/interlocking choice
is explicit — from the [MRH forum thread on the two mockups][forum-mockups]:

> "There is no such thing as a universal signal system, they are all different.
> So option B is nearing a non sequitur. You need to decide on interlocking,
> speed, or route. Then you need to decide what flavor of those."

Hiding the family inside *Practical* was a silent commitment to speed
signaling. Making it visible and easy to override addresses that pushback
without adding a mandatory question to every signal. It also handles the
mixed-family case (merged railroads, mixed eras) gracefully with per-signal
overrides.

**Alternatives considered.**

- *Silent commitment to speed signaling.* Rejected: hides a real design fork; 20%
  pushback is direct.
- *Ask family on every signal.* Rejected: noise for the 80%; contradicts the
  charter's plain-English default.
- *Per-territory defaults.* Deferred (see the next section).

**Related.** Storyboard §1 Frame 0 (first-time question) and Frame 2 (per-signal
override on the chrome strip).

[forum-mockups]: https://forum.mrhmag.com/post/%E2%80%9Ctwo-very-different-ways-to-configure-lcc-signals%E2%80%94which-one-makes-more-sense-13879612

---

## 2026-08-29 — "Help me pick" is a short curated helper, not a database

> **Trigger changed 2026-08-30.** The helper originally lived on a
> first-signal gate (Frame 0). It now lives inside the on-demand *Layout
> defaults* dialog (see the 2026-08-30 entry at the top of this file). The
> curated-list-not-database decision below still stands; only where the list
> appears has changed.

**Decision.** The first-time layout-default question ships a short curated list
of ~15 well-known railroad + era pairs with a typical signaling family answer,
plus a *"just recommend one"* button (defaults to *speed signaling*). The
helper is a *starting-point picker*, not a rulebook-signaling database. Users
who do not see their railroad skip the helper and pick directly.

**Rationale.** A comprehensive railroad-and-era catalog is a rabbit hole that
competes with signaling logic for maintenance attention and does not materially
help a modeler make a starting choice. A short curated list is enough to make
the first-time question feel guided without becoming a research project. Anyone
who cares deeply about their railroad's era-specific signaling can select a
Prototype rulebook per signal.

**Alternatives considered.**

- *Comprehensive catalog.* Rejected: maintenance burden; scope creep.
- *No helper at all.* Rejected: leaves the first-time question awkward, and
  gives the modeler no way in.

---

## 2026-08-29 — No per-territory defaults yet (YAGNI)

**Decision.** The model has two levels: *layout-level default* and *per-signal
override*. No territory / division / segment layer is added. If a real user hits
"half my layout is speed, half is route" and per-signal overrides feel
cumbersome, we add a territory layer then, not before.

**Rationale.** Per-signal override covers merged-railroad and mixed-era cases in
practice. A territory layer is a data-model change (grouping objects, resolving
overrides, exposing them in the UI) with real cost. There is no confirmed
demand.

---

## 2026-08-29 — Opening question is "What does this signal protect?"

**Decision.** The per-signal wizard opens with *"What does this signal
protect?"* with recognisable railroad objects as answers:

- *A block of track* → automatic block signal
- *A turnout or junction* → interlocking
- *A single-track section* → APB
- *A dispatcher-controlled point* → CTC over an interlocking

The signaling-system name (ABS / interlocking / APB / CTC) never appears as an
input; it appears once, on the confirm screen, as a description of derived
behaviour.

**Rationale.** The charter's principle *"signaling system is a territory-level
property, not a per-mast question"* forbids method-first ordering. Object-shaped
answers keep step 1 answerable without any signaling vocabulary, which matches
the 80% audience commitment.

**Alternatives considered.**

- *Method-first* (open with "ABS / APB / interlocking / CTC?"). Rejected: off
  charter.
- *Facts-first* (open with routes / movement authority / available inputs).
  Rejected: requires operations-department vocabulary and violated the 80/20
  commitment. The exploratory file was deleted.
- *Object-first from a layout canvas* ("pick the block or turnout"). Deferred:
  leans on a track model that the charter says is not required.

**Related.** Storyboard §1 Frame 1 and §3 Frame 1.

---

## Deferred / future work

- **Per-territory defaults.** If per-signal overrides prove cumbersome on
  mixed-family layouts, add a territory layer. Not built now.
- **Comprehensive railroad + era catalog.** Not scoped to this feature. If a
  community catalog emerges, connect to it; do not maintain one in-tree.
- **Object-first opening** (pick a block/turnout from a canvas as step 1).
  Depends on a layout inventory that the charter does not require. Reconsider
  when a track-plan model exists.
