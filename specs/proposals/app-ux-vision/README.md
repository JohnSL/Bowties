# App UX Vision — Proposal Set

This directory captures the in-progress vision for the next generation of the Bowties application UX. The vision treats Bowties as the tool that turns hardware into a working railroad for users who think in railroad terms (blocks, turnouts, signals) rather than protocol terms (event IDs, CDI fields).

These are **proposals**, not specs. They synthesize multiple brainstorms into a coherent target. Individual slices will be promoted to feature specs under `specs/NNN-…/` as they become ready to implement; in the meantime, related feature specs (e.g. [018-block-indicator-facility](../../018-block-indicator-facility/)) are already being built against this vision.

## Read In This Order

1. **[app-ux-vision.md](./app-ux-vision.md)** — The north-star document. Describes the target user journey (Plan → Wire → Railroad → Operate), the two-workspace navigation model (Wiring / Railroad), channel roles/styles/bindings, facilities, placeholder nodes, template apply flow, and how the "bowtie" concept repeats at both the event and facility levels. **Start here.**
2. **[app-ux-vision-feasibility.md](./app-ux-vision-feasibility.md)** — Architectural companion to the vision. Covers template system layering, the persisted channel-model shape (`channels.yaml`), the constraint contract, JMRI bridge sync philosophy, and other technical decisions that make the vision achievable.
3. **[app-ux-vision-mockups.html](./app-ux-vision-mockups.html)** — Interactive HTML mockups that illustrate the workspace layouts and key flows. Open in a browser; useful when reading the vision document.

## Supporting Proposals

These three documents predate the unified vision and remain the source of detail for their respective areas. The vision document refers back to them.

- **[behavior-templates-proposal.md](./behavior-templates-proposal.md)** — The behavior-template and information-channel concept. Origin of the role/style/binding model, the facility concept, and the distinction between hardware templates and behavior templates.
- **[planner-proposal.md](./planner-proposal.md)** — The hardware planner wizard. Interviews users about their layout and produces a recommended LCC bill of materials with per-board rationale and a channel-level wiring outline. Drives the "Plan" phase of the vision.
- **[jmri-bridge-proposal.md](./jmri-bridge-proposal.md)** — Bidirectional sync with JMRI via a Jython plugin. How Bowties-managed channels and bowties become JMRI sensors, turnouts, signal masts, lights, and reporters automatically, and how JMRI-owned multi-protocol objects (DCC, LocoNet) flow back as channels.

## Scope And Status

- All files in this directory are **drafts**. Detail varies by area — some sections are decided, others are still under discussion.
- The vision is the target. Implementation proceeds in horizons (see the "Implementation Horizons" section of the vision document). Several capabilities described here are intentionally deferred.
- When this set conflicts with shipped behavior or the durable docs under `product/`, the durable docs and current code win — see `.github/copilot-instructions.md`.
