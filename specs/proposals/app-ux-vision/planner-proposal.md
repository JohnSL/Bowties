# Proposal: Hardware Planner Wizard

**Status:** Draft proposal — to be filed as a GitHub `kind/idea` issue and later expanded into a spec via `/speckit.specify`.
**Origin:** Conversation about issue #8 and the Configuration Modes + Profile Explorer proposal. Surfaced as a "what-if" follow-on feature.

---

## Problem

A new LCC user choosing hardware for a layout currently has no in-app way to answer the question:

> *"For my layout — N turnouts, M blocks, signaling around two sidings, CTC panel, etc. — what LCC boards do I need, and roughly how would I wire them?"*

They must read multiple board manuals, mentally model what each board provides, and compose a shopping list by hand. This is a real adoption barrier for the kind of new user the TurnoutBoss board itself is explicitly designed for.

The same user later benefits from the Profile Explorer (separate proposal) to *understand* a board they already know about. The planner answers the earlier question: *which* boards should they look at in the first place.

---

## Concept: Planner Wizard

A guided wizard that interviews the user about their layout and outputs a recommended LCC bill of materials with a high-level wiring outline.

Example flow (illustrative, not contractual):

1. **Layout shape.** Number and arrangement of turnouts, sidings, blocks, mainline runs.
2. **Signaling intent.** None / basic block signaling / prototypical signaling around turnouts / CTC integration.
3. **Detection method.** Current-sensing, optical, none.
4. **Turnout control method.** Tortoise / servo / other; one-button vs two-button local control.
5. **Feedback preference.** None / one-switch / two-switch per turnout.
6. **External integration.** JMRI / standalone / CTC panel.

Output:

- A recommended set of boards (e.g. *"2× TurnoutBoss for the passing siding, 1× Tower-LCC with BOD4 daughterboards for the mainline blocks"*).
- A short rationale per recommended board grounded in the user's answers.
- A wiring outline at the resource level — for connectorized boards this reads as connector + input (e.g. "Tower-LCC Connector A Input 1"); for boards without daughter-board sockets it reads as line/LED/mast (e.g. "Signal LCC Line 3" or "Signal LCC Mast 2"). The vocabulary comes from the profile's resource catalog, not from the planner itself.
- Optional: deep-link into the Profile Explorer for each recommended board so the user can immediately inspect what they'd be buying.

---

## Dependency: Board Capability Metadata

The planner needs a structured **capability layer** on top of each profile that exposes board-level facts in queryable form:

- How many turnouts does it control?
- How many occupancy blocks does it sense?
- How many signal heads does it drive?
- What types of feedback / control / detection inputs does it expose?
- What network or panel integration features does it provide?
- What are its known operational modes (Left/Right, daughterboard variants, etc.)?

This metadata is **out of scope** for the Configuration Modes + Profile Explorer proposal. It should be designed *with* the planner, not ahead of it, so the capability vocabulary is grounded in real planner queries rather than speculative ones.

---

## Relationship to Other Work

- **Configuration Modes + Profile Explorer** (separate proposal): provides the underlying profile model and the per-board exploration UX. The planner naturally hands off to the explorer ("look at this board in more detail"). Should ship first.
- **Recipes.** Existing `recipes.yaml` content describes step-by-step setup of a board. The planner is more about *which* boards; recipes are about *how to set up* one. Both stay separate concerns.
- **Status modules / runtime data** (`specs/ideas/features/status-page-and-status-modules.md`): unrelated; status modules are about live-node runtime data display.

---

## Non-Goals

- Pricing, vendor links, or automatic order placement.
- Live-network discovery or auto-detection of already-installed boards.
- Final wiring diagrams or PCB-level guidance — the output is connector-level intent, not engineering drawings.
- Recommending non-LCC hardware (DCC command stations, throttles, etc.) beyond what's needed to identify integration points.

---

## Open Questions (for `/speckit.specify` later)

1. What is the smallest useful interview that still produces a credible recommendation? (Avoid 50-question wizards.)
2. How are recommendations expressed when multiple board choices satisfy the same need (e.g. Tower-LCC variants, future SPROG boards)?
3. Should the planner output be persistable / shareable, or always a transient one-shot exploration?
4. How does the planner handle in-progress / partial layouts (already own some boards, planning the next phase)?
5. What's the right vocabulary for capability metadata so it stays small, board-agnostic, and useful to the planner without becoming a meta-modeling exercise?
6. Should the planner be Bowties-resident, or a separate tool that consumes the same profile bundles?

---

## Success Criteria (eventual, not for the idea-stage issue)

- A new user with no LCC boards can answer the wizard in a few minutes and walk away with a credible list of boards to buy for their layout.
- Every recommended board has a one-paragraph rationale tied to the user's answers.
- Each recommendation deep-links into the Profile Explorer.
- The capability metadata layer is small, documented, and extended once per new board profile contributed.

---

## Suggested GitHub Issue (for approval before filing)

**Title:** Hardware planner wizard for layout pre-build sizing

**Labels:** `kind/idea`, `area/profiles`, `area/ux`

**Body:** (use this proposal verbatim, or a condensed form — finalize at issue-filing time)

---

## Pointers

- `profiles/<node-name>/` — per-board source artifacts, will need capability metadata added.
- `app/src-tauri/profiles/` — bundled profiles that the planner would consume.
- Configuration Modes + Profile Explorer proposal — direct upstream dependency for the per-board exploration handoff.
