# Signal-LCC Node Knowledge Base

Durable reference for how the RR-CirKits **Signal-LCC** node actually behaves, what it cannot do, and which configuration approaches the LCC community recommends.

This exists because Bowties generates Signal-LCC configuration on the user's behalf. To produce a good UX we must encode the node's real constraints and idioms, not a plausible-sounding abstraction over them.

## Scope

| In scope | Out of scope |
|---|---|
| Signal-LCC (16 LED drivers), CDI rev C7c | Signal-LCC-32H (SSSB / WS2811, 32 heads) except where noted |
| Rule to Aspect, Direct Lamp Control, Conditionals, Track Circuits | Port I/O line configuration, Node Power Monitor |
| Recommended configuration patterns and known dead ends | Prototype signalling rulebook design |

Where a statement applies only to the **32H** variant it is called out explicitly.

## Read In This Order

1. [hardware-model.md](hardware-model.md) — the four subsystems and their hard capacity limits.
2. [rule-to-aspect.md](rule-to-aspect.md) — the mast/aspect state machine. **The preferred way to drive signals.**
3. [direct-lamp-control.md](direct-lamp-control.md) — individual lamps, block-occupancy indicators, panel LEDs.
4. [conditionals.md](conditionals.md) — the on-board logic engine, its top-down evaluation, and the short-circuit that trips most people up.
5. [track-circuits.md](track-circuits.md) — how a signal learns the next signal's speed.
6. [bowties-ux-implications.md](bowties-ux-implications.md) — what all of the above means for Bowties' UI and compiler.
7. [sources.md](sources.md) — every source URL cited across these documents.

For bench validation after a factory reset, follow [clean-room-test-procedure.md](clean-room-test-procedure.md). It rebuilds one mast, proves a stand-alone conditional, then runs the decisive two-entry group test before introducing an indicator or Track Circuit.

## Sourcing Rules For This Folder

Every non-obvious claim must cite one of:

- **CDI** — `docs/ref/RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml`, quoted by segment/field path. This is the strongest evidence: it is what the node actually publishes.
- **Manual** — section and page, via `profile-extractions/signal-lcc/field-descriptions.yaml` citations or `docs/ref/SignalLCC-manual-e.pdf`.
- **Community** — a `groups.io/g/layoutcommandcontrol` message URL, preferring messages from Dick Bronson (RR-CirKits), Rob Heikens (RR-CirKits firmware), Balazs Racz, Ken Cameron, and Mark Granville.

When the manual and the community disagree, prefer the CDI, then RR-CirKits staff, then experienced users. Note the disagreement rather than silently picking a side.

## Status

The vendor manual is still marked **DRAFT** beyond its early sections, and this is a recurring source of user frustration — see [#13995](https://groups.io/g/layoutcommandcontrol/message/13995) and [#14008](https://groups.io/g/layoutcommandcontrol/message/14008). Several behaviours documented here are recorded only in mailing-list messages, which is precisely why they are captured here.
