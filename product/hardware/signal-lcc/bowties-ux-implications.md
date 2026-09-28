# What This Means For Bowties UX

Design guidance derived from [hardware-model.md](hardware-model.md), [rule-to-aspect.md](rule-to-aspect.md), [direct-lamp-control.md](direct-lamp-control.md), [conditionals.md](conditionals.md), and [track-circuits.md](track-circuits.md).

## The Problem We Are Solving

The recurring complaint about Signal-LCC is not that it is under-powered. It is that a simple intent has no simple path. Mike Chapman, [#13985](https://groups.io/g/layoutcommandcontrol/message/13985):

> All I want to do is control the lights in four G-type Atlas signal heads based on turnout position […] You'd think it would be fairly easy to just light the lamps - but if so, I can't figure it out.

And [#14001](https://groups.io/g/layoutcommandcontrol/message/14001):

> designed BY engineers, FOR engineers, and not train engineers either.

Balazs Racz's counterpoint is fair and worth respecting, [#13991](https://groups.io/g/layoutcommandcontrol/message/13991): real signalling genuinely is a specialist discipline with hundreds of rulebooks. The resolution is not to hide the domain but to **let users express intent at the domain level and own the encoding**.

That is exactly Bowties' remit: users describe the railroad, Bowties allocates masts, rules, lamps, conditionals and circuits.

## Principles

### 1. Model signals as masts, always

Never emit Direct Lamp Control for something the user called a signal. The mast state machine gives automatic mutual exclusion of aspects for free; direct lamps make the user's logic responsible for every off edge. See [rule-to-aspect.md](rule-to-aspect.md).

### 2. Never mix an indicator into a signal's conditional group

This is the short-circuit trap. Indicators must be either their own group or, preferably, plain Direct Lamp Control bindings with no logic. Bowties should make it structurally impossible to append an indicator rung to an aspect ladder.

### 3. Prefer zero-logic bindings

Before allocating a conditional, ask whether the output is a direct restatement of one event. Block-occupancy LEDs, turnout-position LEDs and panel mimics usually are. Conditionals are the scarcest resource (32) and should be spent on genuine logic.

### 4. Own output allocation

The 16 LED drivers must have exactly one owner each. Bowties should:

- track ownership centrally,
- refuse to bind an output already claimed by a mast Appearance,
- automatically restore `Lamp Selection = Used by Mast` when an output stops being directly driven,
- never leave a row in direct mode after a test action.

### 5. Surface capacity, not just errors

Show remaining **masts (8)**, **rules per mast (8, or 16 when chained)**, **conditional slots (32)**, **track circuits (8)** and **LED outputs (16)** *before* the user commits, so a plan fails early rather than at write time.

### 6. Always paste event IDs

Retyping event IDs is a documented source of error, and the community advice is uniformly "copy and paste, never type". Bowties should never require a user to see an event ID at all — the [Bowties setup demo](https://groups.io/g/layoutcommandcontrol/message/15406) deliberately shows block detection configured "entirely based on behavior intent" with no event IDs handled manually. Keep that property.

### 7. Give latched state a first-class concept

Held signals, direction levers and route locking all need a variable set by one event and cleared by another, evaluated first in the ladder. Every latch needs an explicit clearer or the layout exhibits "fleeting". If Bowties offers held/route behaviour, it should generate the clearer automatically and warn when one is missing.

### 8. Offer what the vendor tooling cannot

The JMRI CDI editor cannot copy, move or reorder conditional entries; users retype whole ladders and introduce errors ([#7955](https://groups.io/g/layoutcommandcontrol/message/7955)). Reordering, duplicating and templating ladders is high-value, low-risk differentiation.

### 9. Mirror the community's bring-up sequence

Wiring → one lamp → one aspect → whole mast → logic → cascade. A guided flow that follows this order matches how experienced users actually debug, and isolates faults. Dick Bronson's advice is explicit that logic should come *after* the lights work.

Include a per-output test action ("flash this lamp") that is guaranteed to restore prior configuration afterwards.

### 10. Do not assume Dark means restrictive

`31-Dark` is an ordinary aspect that lights nothing, and approach-lit installations deliberately give it a permissive `Track Speed`. Any UI that infers severity from aspect name will get this wrong.

## Validation Rules Worth Encoding

| Check | Why |
|---|---|
| Direct lamp row has `Lamp On` but no `Lamp Off` | Lamp will latch on forever |
| Output claimed by both a mast Appearance and a direct lamp row | Undefined ownership |
| Direct lamp row left selected after a test | Silently breaks the mast that owns the output |
| Aspect ladder rung uses `Exit Group` with rungs after it that must run | Dead configuration |
| Conditional Action `Condition = none` | Event never sent |
| Mast rule missing `Track Speed` | Downstream cascade never updates |
| Track circuit references a mast facing the opposite direction | Not a valid "next" mast |
| Both variables of one conditional bound to the same source | Explicitly warned against |
| Signal mast paired in JMRI with all-zero event IDs | Known JMRI misbehaviour |

## Terminology To Keep Straight

Bowties' vocabulary should map cleanly onto the node's, because users will read both.

| Node term | Meaning | Bowties concept |
|---|---|---|
| Mast | One signal, one active aspect at a time | signal facility |
| Rule | One aspect of a mast, from a fixed 32-name vocabulary | aspect |
| Appearance | Up to 4 lamps lit for a rule | style / lamp mapping |
| Lamp / LED driver | One of 16 physical outputs | channel binding |
| Conditional (Logic) | One rung of a ladder | compiled rule |
| Group | A run of rungs, evaluated top-down, short-circuiting | compiled ladder |
| Track Circuit | Receiver for one remote mast's speed | cascade link |
| Link Address | A mast's published speed address | cascade source |

See `product/glossary.md` for Bowties-side canonical terms.

## Related

- [README.md](README.md)
- [sources.md](sources.md)
