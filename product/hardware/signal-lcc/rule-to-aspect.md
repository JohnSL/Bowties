# Rule to Aspect

The mast-and-aspect state machine. **This is the recommended way to drive any signal head**, and the reason is mutual exclusion.

CDI segment: `Rule to Aspect`. Manual section 8.

## Why Prefer It Over Direct Lamp Control

A mast is a state machine with exactly one active aspect. Selecting an aspect automatically extinguishes the previous one. The CDI says so directly on the `set aspect` field:

> `(C) Event to Set Aspect. Note: Aspects are cleared automatically by the logic.`

Ken Cameron states the practical consequence, [#13986](https://groups.io/g/layoutcommandcontrol/message/13986):

> The biggest advantage of using a mast is that it will turn all the lamps off in the mast and then turn on the pattern requested. If you use direct lamp, you have to manage doing both the turn on and off of the lamps on the signal.

So with a mast, your logic sends **one event per aspect**. With direct lamps, your logic must send an on event *and* matching off events for every lamp of every other aspect — an O(n²) hand-maintained mess that gets out of step the moment you add an aspect.

**Rule: if the output is a signal head, model it as a mast.**

## Structure

```
Mast (x8)
├── Mast Processing     Unused | Normal | Linked to Previous
├── Mast Description    free text
├── Link Address        (P) track-circuit publication, read-only
├── Lamp Fade           None | Incandescent
└── Rule (x8)
    ├── Rule Name       fixed vocabulary, 0-Stop .. 31-Dark
    ├── Track Speed     speed this aspect implies, on approach to signal
    ├── set aspect      (C) event that selects this aspect
    ├── aspect is set   (P) fired when this aspect becomes active
    ├── aspect cleared  (P) fired when this aspect stops being active
    ├── Appearance (x4)
    │   ├── Lamp Selection    which of the 16 outputs
    │   └── Lamp Phase/Flash  Steady | A-Slow/Med/Fast | B-Slow/Med/Fast
    ├── Appearance Effects    None | Transition down | H2 Red Flash | Strobe
    └── Effects Lamp
```

## Rule Names Are A Fixed Vocabulary

`Rule Name` is an enum of 32 standard indications, not free text:

`0-Stop`, `1-Take Siding`, `2-Stop Orders`, `3-Stop Proceed`, `4-Restricting`, `5-Permissive`, `6-Slow-Approach`, `7-Slow`, `8-Slow-Medium`, `9-Slow-Limited`, `10-Slow-Clear`, `11-Medium-Approach`, `12-Medium-Slow`, `13-Medium`, `14-Medium-Limited`, `15-Medium-Clear`, `16-Limited-Approach`, `17-Limited-Slow`, `18-Limited-Medium`, `19-Limited`, `20-Limited-Clear`, `21-Approach`, `22-Advance-Approach`, `23-Approach-Slow`, `24-Advance-Approach-Slow`, `25-Approach-Medium`, `26-Advance-Approach-Medium`, `27-Approach-Limited`, `28-Advance-Approach-Limited`, `29-Clear`, `30-Cab-Speed`, `31-Dark`.

The name is a **label for humans and for JMRI mapping**. The node does not derive behaviour from it; behaviour comes from `Appearance` (which lamps) and `Track Speed` (what downstream signals see).

A 3-aspect ABS mast therefore uses three of the eight rule slots, conventionally `0-Stop`, `21-Approach`, `29-Clear`.

### `31-Dark` is a real aspect

Approach lighting is implemented as an ordinary rule that lights **no** lamps. John Joyce, [#10560](https://groups.io/g/layoutcommandcontrol/message/10560):

> I've always thought of DARK as a Rule-to-Aspect alongside STOP, APPROACH, CLEAR, and the like. It is rule 31 in the CDI drop down, after all.

Important subtlety: a dark signal still publishes a `Track Speed` to its track circuit. Joyce sets his dark aspect's track speed to `Clear` so that signals reading it behave as if the route is clear ([#10533](https://groups.io/g/layoutcommandcontrol/message/10533)). Bowties must not assume "dark" implies "restrictive".

## Mast Processing

| Value | Meaning |
|---|---|
| `Unused` | Slot inert. Everything below it — description, link address, fade, all rules — has no effect. |
| `Normal` | Stand-alone mast with 8 rules; publishes its own Link Address. |
| `Linked to Previous` | Extends the previous mast: shared identity, 16 rule slots total. |

`Linked to Previous` exists to give a single mast more than 8 aspects — it is **not** the way to model a second head on the same mast. On this node, additional heads on a physical mast are simply more lamps in the same rule's `Appearance` list (up to 4 lit lamps per aspect). Robert Heller sketches the multi-head convention at a turnout in [#14006](https://groups.io/g/layoutcommandcontrol/message/14006).

> On the **32H**, "Linked to Previous" means something different — there it chains additional *heads*, and the first enabled head's 16 rules are used for all linked heads. See Dick Bronson's walkthrough in the [32H head configuration thread](https://groups.io/g/layoutcommandcontrol/topic/signal_lcc_32h_only_the_first/99817405). Do not carry 32H mental models onto the Signal-LCC.

## The Three Events Per Rule

| Event | Direction | Purpose |
|---|---|---|
| `set aspect` | Consumer | Selects this aspect. Bind to a conditional Action Event or a JMRI sensor. |
| `aspect is set` | Producer | Fires when the aspect becomes active. Use for signal repeaters, CTC indication, panel mimics. |
| `aspect cleared` | Producer | Fires when the aspect stops being active. |

You never need a "clear" consumer — clearing is automatic.

The producers **cannot be disabled**. A user asked for a way to silence them to reduce bus chatter; Ken Cameron confirmed there is none short of not updating the signal ([#10450](https://groups.io/g/layoutcommandcontrol/message/10450), request at [#10449](https://groups.io/g/layoutcommandcontrol/message/10449)).

## Track Speed Is Not Decoration

`Track Speed` on each rule is what the mast publishes to its track circuit, described in the CDI as the speed "on approach to signal". Getting it wrong is the number-one cause of cascades that do nothing. Ken Cameron's first diagnostic question, [#8700](https://groups.io/g/layoutcommandcontrol/message/8700):

> For the first step, inspect your signal Rule to Aspect table. Did you set the correct 'Track Speed' on each aspect of the mast? […] If the signal isn't setup right, the track circuit doesn't change. The mast pushes the track circuit change it doesn't consume it.

See [track-circuits.md](track-circuits.md).

## Bring-Up Procedure

The community converges on the same staged sequence. Do not skip ahead; each stage isolates one class of fault.

1. **Prove the wiring.** Use Direct Lamp Control to light each output in turn and record which physical lamp responds. Return every row to `Used by Mast` afterwards. ([Tom Cherepko](https://groups.io/g/layoutcommandcontrol/topic/signal_lcc_board_and_atlas/98184931))
2. **Prove one aspect.** Configure Mast 1, Rule 1, set its Appearance, then fire `set aspect` directly. In JMRI/LccPro: open Configuration, navigate to the aspect, use `Set Aspect` → `More…` → `Trigger`. Bob Jacobsen, [#15214](https://groups.io/g/layoutcommandcontrol/message/15214). Dick Bronson describes the same `More` affordance in [#15216](https://groups.io/g/layoutcommandcontrol/message/15216).
3. **Prove the whole mast.** Trigger each aspect and confirm exactly one is lit at a time.
4. **Only then add logic.** Dick Bronson, [#-tutorial thread](https://groups.io/g/layoutcommandcontrol/topic/tutorial_on_setting_up_a/29696659):

   > Why not start out by simply making a signal mast work […]? After you get the masts lighting up with different aspects, then you can introduce the concept of logic in a different tutorial. It is more fun once the lights are working.

5. **Then add cascade** via track circuits. Ken Cameron, [#9878](https://groups.io/g/layoutcommandcontrol/message/9878): "After you proved the electric part, then move to 'Rule to Aspect' to make up the masts."

### JMRI mast pairing caveat

Create the hardware masts first and the JMRI masts alongside them. Dick Bronson warns of a JMRI issue triggered by signal masts left with default all-zero event IDs:

> there is a JMRI issue (bug?) that rears its head when there are signal masts created but left with the default 00….00 entries. Better to create the masts first in the hardware and create the matching JMRI masts as you go. Be sure to either enter the actual EventIDs from the hardware, or else disable the aspects.

Source: [tutorial thread](https://groups.io/g/layoutcommandcontrol/topic/tutorial_on_setting_up_a/29696659). Always paste event IDs; never retype them.

## Related

- [conditionals.md](conditionals.md) — what sends the `set aspect` events
- [track-circuits.md](track-circuits.md) — what `Track Speed` feeds
- [direct-lamp-control.md](direct-lamp-control.md) — the other, non-exclusive output path
