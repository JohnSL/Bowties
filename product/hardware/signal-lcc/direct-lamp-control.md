# Direct Lamp Control (Indicators And Panel LEDs)

CDI segment: `Direct Lamp Control`, 16 `Lamp` slots.

The CDI describes the segment as:

> Direct control of lamps that are not used by any mast.

This is the path for **block-occupancy indicators, turnout-position LEDs, panel mimics, and building lighting** — anything that is not a signal aspect.

## Structure

```
Lamp (x16)
├── Lamp Description   free text
├── Lamp On            (C) event that lights the lamp
├── Lamp Off           (C) event that extinguishes the lamp
├── Lamp Fade          None | Incandescent
├── Lamp Selection     Used by Mast | #1 H1-G .. #16 H4-L
└── Lamp Phase (A-B) - Flash Rate   Steady | A-Slow/Med/Fast | B-Slow/Med/Fast
```

## Block Indicator LEDs — The Recommended Pattern

A block-occupancy indicator is a **direct restatement of one detector's state**. It needs no logic.

| Step | Setting |
|---|---|
| 1 | Pick a free LED driver, set `Lamp Selection` to it |
| 2 | Paste the detector's *occupied* event into `Lamp On` |
| 3 | Paste the detector's *unoccupied* event into `Lamp Off` |
| 4 | Leave `Lamp Phase` on `Steady` |

Consequences:

- **Zero conditional slots consumed.** The 32 conditionals stay available for signal logic.
- **No evaluation-order hazard.** It cannot be short-circuited by an aspect ladder — see [conditionals.md](conditionals.md).
- **Survives signal-logic changes**, because it is not coupled to them.

This is exactly the composition Bowties' Block Indicator facility produces: two bowties, occupied→lit and clear→unlit.

A user confirms this is the easy, working case even when the rest of the node is hard, Mike Chapman [#14009](https://groups.io/g/layoutcommandcontrol/message/14009):

> Ironically, I easily got an Signal-LCC-S board to light up position indicating LEDS for my helix.

## You Own Both Edges

Unlike a mast, a direct lamp has **no state machine**. Nothing turns it off for you. Ken Cameron, [#13986](https://groups.io/g/layoutcommandcontrol/message/13986):

> If you use direct lamp, you have to manage doing both the turn on and off of the lamps on the signal.

So a direct-lamp slot with `Lamp On` configured but `Lamp Off` empty will light once and stay lit forever. **`Lamp Off` being unset is a configuration error worth flagging in UI**, not an optional field.

This is also why direct lamps are the wrong tool for signal heads: with N aspects you would have to fan every aspect change out to N×lamps on/off events by hand.

## Exclusivity With Masts

`Lamp Selection = Used by Mast` (value 0) means "this slot is inactive; the Rule to Aspect table owns this output". Each of the 16 outputs must be claimed by **exactly one** path.

Two documented failure modes:

1. **Left in direct mode after testing.** A user tested each lamp via direct control, then found his conditionals did nothing because he never returned the rows to `Used by Mast`. Tom Cherepko's warning in the [Atlas signal thread](https://groups.io/g/layoutcommandcontrol/topic/signal_lcc_board_and_atlas/98184931):

   > **Important:** When you are done return lamp selection to Used by Mast.

2. **Claimed by both.** The field description notes configuration tools warn when the same lamp is also claimed by an Appearance.

Bowties should treat output ownership as a first-class allocation with a single owner per output.

## Using Lamps To Show Turnout Position

A frequent request: light LEDs from turnout position. Two viable shapes:

- **As an indicator** — direct lamp rows, `Lamp On`/`Lamp Off` bound to the turnout's normal/reverse events. Simple, no logic.
- **As a signal** — a mast whose aspect is chosen by a conditional testing turnout state. Ken Cameron, [#13986](https://groups.io/g/layoutcommandcontrol/message/13986):

  > the trick would be writing a couple of conditionals. The conditional would test the turnout states and then set the mast/lamp accordingly and exit.

Choose by intent: if the lamps mean "the route is set this way", use an indicator; if they mean "you may proceed at this speed", use a mast so cascade and track speed work.

The long [#13985 thread](https://groups.io/g/layoutcommandcontrol/topic/is_this_covered_anywhere/117823202) is the clearest record of how badly this simple goal is currently served by the documentation, and is worth reading in full as UX motivation.

## Flash Phases

`Lamp Phase (A-B)` offers `Steady` plus slow/medium/fast in two phase banks, **A** and **B**. Two lamps on the same rate but opposite banks alternate rather than blink together — the mechanism for alternating-flash grade-crossing signals and for wig-wags. The same field exists on mast `Appearance` entries.

## Related

- [hardware-model.md](hardware-model.md) — output namespace and exclusivity
- [rule-to-aspect.md](rule-to-aspect.md) — the mast alternative
- [conditionals.md](conditionals.md) — why indicators should avoid the aspect ladder
