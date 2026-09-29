# Signaling UX — Proposal Set

Status: **in progress.** Charter, mockups, and a proposal draft exist. The proposal is still
iterating.

This directory captures where we want to take the Bowties UX for **signalling** — building signals
that a user describes in railroad terms, with Bowties owning every event ID, CDI field and logic rung.

It is deliberately *not* a Signal-LCC proposal or a Tower LCC+Q proposal. Hardware is an
implementation detail Bowties chooses on the user's behalf.

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
| [signaling-ux-practical-first-storyboard.html](signaling-ux-practical-first-storyboard.html) | **current direction** | Self-contained HTML storyboard for the shipping wizard. Opens with **"what does this signal protect?"**; a dedicated **plant step** captures shape / routes / termination via schematic tiles with an inline preview panel (see design-decisions.md); the aspect step then produces the indication table from two controls — signaling *family* and a single **vocabulary ladder** collapsing the earlier *fidelity* + *aspect-set-size* dials. Head count / style are derived. No build step; safe to email to reviewers. |
| [signaling-ux-mockups.html](signaling-ux-mockups.html) | **superseded — historical** | Original method-first mockups (asks the user to name ABS / APB / interlocking / CTC on screen one). Kept for side-by-side comparison and to preserve frames the practical-first storyboard defers to it (bring-up, facility card, capacity envelope, APB / CTC, vocabulary table). Not the intended direction for the user-facing flow. |
| [signaling-ux-proposal.md](signaling-ux-proposal.md) | **draft** | The prose proposal. Signal type as the core data object; how active indications and cascade are derived; how the model maps onto ABS / APB / interlocking / CTC; conversion of JMRI signal XML into the shipped catalog. |
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

Charter first, then mockups, then proposal prose. Drawing screens changes the model; writing prose
first means defending it instead of learning from it. The reviewer audience is both prototype-signalling
experts and Signal-LCC/LCC hardware experts. Signalling data in the mockups is illustrative and
marked as such — critique the *model* separately from the *data*.

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
