# Signaling UX — Proposal Set

Status: **current UX direction selected, 2026-10-31.** The facility-first storyboard is the
current mockup. Detailed domain, persistence, and implementation decisions remain in progress.

Start with [signaling-facilities-storyboard.html](signaling-facilities-storyboard.html):
Add Facility offers Block Indicator, Interlocking Plant, and Automatic Block Signals (renamed from
Signal Plant and Signal Line in November 2026). Signal Mast remains the
individual signal being configured, not a separate top-level creation option. The two older
HTML mockups are in [archived/](archived/) for historical reference only.

This directory captures where we want to take the Bowties UX for **signalling** — building signals
that a user describes in railroad terms, with Bowties owning every event ID, CDI field and logic rung.

It is deliberately *not* a Signal-LCC proposal or a Tower LCC+Q proposal. Bowties owns hardware
translation; the Automatic Block Signals mockup lets the user choose the Signal LCC each signal
runs on and the pins its lamps use.

> **Dividing test (2026-11-14):** if any signal tells a train which route it will take (power
> switches, diverging indications, siding departure signals), it belongs to a **Interlocking Plant**.
> Otherwise it is **Automatic Block Signals** — ABS or APB — even with sidings and hand-thrown
> turnouts. There, signals stand along the main only, a turnout that is not normal restricts the
> approaching signals (Stop or Restricting), and the siding exit is unsignaled. Each turnout belongs to
> one block, and its position feeds that block's protecting signals; turnouts get no signals of
> their own. Automatic
> signals choose an aspect count (2, 3, or 4) rather than a signaling family or vocabulary;
> speed/route vocabulary stays with Plants.

> **Read this first:** [goals-and-constraints.md](goals-and-constraints.md) is the charter for this
> directory. Every mockup, proposal, and reviewer response has to be consistent with it. If you
> catch yourself re-arguing whether the primary audience is the 80% or the 20%, whether a track
> diagram is required, or whether Bowties should expose hardware to the user, stop and re-read the
> charter before continuing.

## Contents

| File | Status | Purpose |
|---|---|---|
| [goals-and-constraints.md](goals-and-constraints.md) | **charter** | Non-negotiable audience, goals, constraints, and re-litigation checkpoints. Read before writing or reviewing anything else here. |
| [design-decisions.md](design-decisions.md) | **living log** | Concrete design decisions and their rationale — opening question shape, layout-level defaults for signaling family, aspect set size dial, aspect-first ordering, deferred territory model. Sits between the high-level charter and the storyboard. |
| [signaling-facilities-storyboard.html](signaling-facilities-storyboard.html) | **current direction** | Self-contained facility-first storyboard. Automatic Block Signals has five create/edit pages: Blocks (one west-to-east list of detected blocks and hand-thrown turnouts, with a live diagram of the derived signals), Aspects (2/3/4 aspects, reversed-turnout behavior), Appearance (signal type, approach lighting, live and simulated-train previews), Wiring (the Signal LCC each signal runs on, lamp-to-pin seat map, Rule to Aspect table, Blink and Show checks), Review. Shared signal selection, per-signal aspect and appearance overrides, per-head drive choices, pin assignment, and node-scoped sample checks are interactive. Multiple-head and Advance Approach appearances are future UX, not implemented S2 support or certified railroad rules. Later pages still model one directional chain; reverse-chain/APB remain unvalidated. Save/apply stay disabled. The Plant walkthrough is now a CTC controlled siding; the Mast walkthrough is kept as a reference section at the end. No build step; safe to email to reviewers. |
| [block-signals-first-slice-proposal.md](../../020-block-signals-first-slice/block-signals-first-slice-proposal.md) | **active spec input** | Narrow E2E Automatic Block Signals: one direction, Speed/Essential, starting with one detected block and an occupancy-only signal; an early two-mast same-node increment leads to required cross-node cascade on at least two Signal-LCCs before completion. One two-driver red/green searchlight arrangement and normal create/edit/remove lifecycle. See the [feature specification](../../020-block-signals-first-slice/spec.md). |
| [block-signal-feature-proposal.md](block-signal-feature-proposal.md) | **earlier proposed scope** | Single-Mast guided setup proposal, with lasting two-aspect Clear/Stop support before three-aspect downstream behavior. Its Mast-first entry and delivery sequence need reconciliation with the facility-first Line slice before reuse. |
| [archived/signaling-ux-practical-first-storyboard.html](archived/signaling-ux-practical-first-storyboard.html) | **archived — historical** | Earlier practical-first wizard. Superseded by the facility-first storyboard; retained for comparison, not current UX guidance. |
| [archived/signaling-ux-mockups.html](archived/signaling-ux-mockups.html) | **archived — historical** | Original method-first mockups. Retained for historical examples of bring-up, facility cards, capacity, APB / CTC, and vocabulary, not current UX guidance. |
| [signaling-ux-proposal.md](signaling-ux-proposal.md) | **draft** | The prose proposal. Signal type as the core data object; how active indications and cascade are derived; how the model maps onto ABS / APB / interlocking / CTC; conversion of JMRI signal XML into the shipped catalog. |
| [apb-direction-lock-research.md](apb-direction-lock-research.md) | **research lead** | External STL example of occupancy-edge APB direction locking; records the candidate pattern, differing edge semantics, and questions before adoption. Not a design decision. |
| [headless-intent-pipeline-plan.md](headless-intent-pipeline-plan.md) | **proposed** | Headless architecture plan: typed Signal Mast intent and bundled behavior policy compile to a validated, reviewable Signal-LCC plan that developer scenarios and the future wizard apply through the same draft workflow. |
| [option-a-mainline-extraction-plan.md](option-a-mainline-extraction-plan.md) | **execution plan** | Mainline salvage sequence for the unmerged `020-abs-signaling` worktree: preserve current evidence, land developer-agent improvements first, extract independent capabilities, discard obsolete ABS production architecture, then build the headless Signal-LCC pipeline. |
| This README | living | Session-start index. Points to the charter, decisions log, storyboards, proposal, hardware facts, open questions, and backlog. |

An earlier "facts-first" exploration (Approach C — routes, movement authority, and available inputs
as independent top-level questions) was removed after review: it required the reader to think in
signaling-department vocabulary and therefore violated the 80/20 audience commitment in the
charter.

Follows the convention set by [../app-ux-vision/](../app-ux-vision/README.md): proposal markdown plus
one self-contained mockups HTML, using Fluent v9 tokens matching
[../../018-block-indicator-facility/mockups.html](../../018-block-indicator-facility/mockups.html).

## Working method

The 2026-10-31 entry in [design-decisions.md](design-decisions.md) records the current entrance
decision. Older Mast-first wizard and Plant-threshold statements in the charter, decision log,
and prose proposal still need reconciliation; they do not restore a Signal Mast creation option.
Selecting this mockup does not by itself settle delivery phasing or Automatic Block Signals
persistence. Spec and code still use the Automatic Block Signals name; they will be updated once the mockup
is settled.

Charter first, then mockups, then proposal prose. Drawing screens changes the model; writing prose
first means defending it instead of learning from it. The reviewer audience is both prototype-signalling
experts and Signal-LCC/LCC hardware experts. Signalling data in the mockups is illustrative and
marked as such — critique the *model* separately from the *data*.

Automatic Block Signals separates meaning from visual encoding. Aspects retains defaults for all signals and
per-signal policy overrides; Appearance owns signal type and approach lighting; Wiring owns the
Signal LCC each signal runs on (one node per signal), each head's electrical implementation, the
lamp-to-pin assignments (a seat-map pin grid), and the Rule to Aspect table derived from
Appearance (closed by default, read-only unless customized). See the 2026-11-18 entry in [design-decisions.md](design-decisions.md). The Wiring
editor is a future reuse candidate for Plant, not a change to the existing Plant pages. Legacy
`#line-*` anchors remain aliases.

For review, on Blocks select each turnout to see which derived signals protect it; move the Depot track
turnout up and down to move it between blocks; pick turnout shapes in the diagram header;
use "⋯" on Mill curve's row to remove the signal between it and the block above; switch Westward to see the same
list drawn right to left with westward signals; use the "+ Plant" chips at either end of the track
(including "Nothing beyond · treat as Stop"); and switch to both directions. On Aspects, select
Mill curve East to see the turnout conditions, switch the reversed-turnout behavior to Restricting, and try 2, 3, and 4 aspects (including a 2-aspect override on one signal).
On Appearance, change the signal type for all signals and for one signal, choose Red over lunar
to add the lunar head, switch approach lighting, and run a simulated train in both directions.
Dark is display policy, not authority or unknown occupancy. On Wiring, try RG/RYG searchlight
drives and the position-light center (wired always on, or on a pin when approach lit); move a signal to another Signal LCC with the node
search dialog; place, clear, and swap lamps on the pin grid and use an H1–H4 header label to place
a whole head; open the Rule to Aspect table and customize a rule past 4 lamps. Verify Review links
back to Wiring without automatic reassignment. Equal pin numbers on different nodes do not
collide. Switch signals and Back/Next to check retained assignments.
Edit compares against the page-load sample; reload resets it. All checks are local mock checks,
not catalog, rulebook, hardware, or safe-operation validation.

## Where extended context lives

- **Design decisions and their rationale** — opening question, layout-level defaults, aspect set
  size dial, aspect-first ordering, deferred work: [design-decisions.md](design-decisions.md).
- **Design decisions, signal-type shape, cascade derivation, JMRI catalog, per-system logic.**
  [signaling-ux-proposal.md](signaling-ux-proposal.md). §0 maps the proposal onto the charter.
- **Signal-LCC and Tower LCC+Q hardware facts** — data model, ceilings, load-bearing invariants
  that must not be re-derived, APB primitives, community-known idioms:
  [../../../product/hardware/signal-lcc/](../../../product/hardware/signal-lcc/README.md). Read
  `rule-to-aspect.md`, `conditionals.md`, `track-circuits.md`, and `bowties-ux-implications.md`
  before touching the proposal.
- **Open questions.** Proposal §8.
- **Backlog items** — glossary gaps and worked-example extractions:
  [../../backlog.md](../../backlog.md).
