# Conditionals (On-Board Logic)

CDI segment: `Conditionals`, 32 `Logic` slots. Manual section 7, pages 25–31.

This is the node's *vital logic*: it can compute signal rules, route logic and simple interlocks with no computer attached. It is also where nearly every Signal-LCC user gets stuck, because **evaluation short-circuits** and the failure is silent.

## Anatomy Of One Logic Slot

```
Logic (x32)
├── Description        free text
├── Function           Blocked | Group | Last (Single)
├── Variable #1        Trigger, Source, Track Speed, set true (C), set false (C)
├── Logic Operation    V1 AND V2 | V1 OR V2 | V1 XOR V2 | ...changes | V1 Only | V2 Only | null => true
├── Variable #2        same shape as Variable #1
├── Action when true   Send then Exit Group | Send then Evaluate Next | Exit Group | Evaluate Next | AND with Next | OR with Next
├── Action when false  (same six options)
├── Time Delay         value, ms/s/min, retriggerable
└── Action (x4)
    ├── Condition      none | Immediately | After delay | Immediate if True | Immediate if False | Delayed if True | Delayed if False
    └── Action Event   (P) event sent
```

## The Evaluation Model

A **group** is a run of consecutive `Logic` slots. `Function` defines membership:

| `Function` | Behaviour |
|---|---|
| `Blocked` | Slot is not evaluated at all. Use to disable logic without losing its configuration. |
| `Group` | Part of a chained group; evaluation may continue into the next slot. |
| `Last (Single)` | End of a group, or a stand-alone rule. **Evaluation never falls past this entry.** |

Manual section 7.3, page 26.

Evaluation runs **top-down** through the group. Each slot computes true or false from its two variables, then consults `Action when true` / `Action when false` to decide two independent things: *whether to send its action events*, and *whether to continue*.

| Option | Sends events? | Continues? |
|---|:---:|:---:|
| `Send then Exit Group` | yes | **no** |
| `Send then Evaluate Next` | yes | yes |
| `Exit Group` | no | **no** |
| `Evaluate Next` | no | yes |
| `AND with Next` | no | yes, carrying a logical AND |
| `OR with Next` | no | yes, carrying a logical OR |

Manual sections 7.7 and 7.8, pages 29–30.

## The Short-Circuit — And Why Your ABS + Occupancy Group Fails

This is the behaviour you diagnosed, and it is real.

The idiomatic ABS ladder is written **most restrictive first**, with each rung ending in `Send then Exit Group` on true:

```
Logic 1  [Group]  next block occupied?        true -> Send "set Stop",     Exit Group
Logic 2  [Group]  next signal at Stop?        true -> Send "set Approach", Exit Group
Logic 3  [Last]   otherwise                   true -> Send "set Clear",    Exit Group
```

That is correct and intentional: the ladder is a priority list, and the first matching rung wins.

Now append an occupancy-indicator rung to the same group:

```
Logic 4  [Last]   block occupied -> light panel LED
```

**It will never run whenever any earlier rung matched.** The first `Exit Group` terminates the entire group. Because a train in the block makes rung 1 true, the indicator rung is skipped in exactly the situation you care about.

Tom Patterson hit the same wall building a CTC held-signal, [#15224](https://groups.io/g/layoutcommandcontrol/message/15224):

> The conditionals for the signal look for a true, and once found, exit.

This is not a bug and it is not fixable by reordering. It is the semantics of a priority ladder.

### Fix 1 — Don't compute indicators with conditionals at all (preferred)

A block-occupancy LED is not logic. It is a direct restatement of one detector's state. Bind the detector's *occupied* event to a Direct Lamp Control row's `Lamp On`, and its *clear* event to `Lamp Off`. Zero conditionals consumed, no ordering hazard, and it keeps working no matter how the aspect ladder evolves.

See [direct-lamp-control.md](direct-lamp-control.md). This is what Bowties' Block Indicator facility already does.

### Fix 2 — Give the indicator its own group

Groups are delimited purely by `Function`. Terminate the aspect ladder with `Last (Single)` and begin a **new** group for the indicator, with its own variable trigger. Independent groups do not short-circuit one another.

This costs extra conditional slots out of the 32, so prefer Fix 1 when the indicator is a plain mirror of an event.

### Fix 3 — Emit both events from the rung that already matched

Every slot has **four** action event slots, each with its own `Condition`. A rung that decides "Stop" can simultaneously send the aspect event and the indicator event:

```
Logic 1  [Group]  next block occupied?
    Action 1: Immediate if True   -> "set Stop"
    Action 2: Immediate if True   -> "panel LED on"
    Action 3: Immediate if False  -> "panel LED off"
    when true -> Send then Exit Group
```

`Immediate if True` / `Immediate if False` let one slot branch its outputs without a second slot. This is the most slot-efficient option and is under-used.

### Fix 4 — `Send then Evaluate Next`

Use only when you genuinely want later rungs to run. Be careful: a later rung sending a different `set aspect` to the same mast will **overwrite** the aspect the earlier rung just chose. Reserve this for rungs whose actions target different consumers.

## Conditionals Have No Memory — Model State As A Variable

Conditionals are re-evaluated from current variable values; they do not remember what they decided last time. Anything that must *persist* has to be an explicit variable driven by an event pair.

This is the root of the classic "signal will not stay knocked down" problem. Dick Bronson, [#15225](https://groups.io/g/layoutcommandcontrol/message/15225):

> I think the problem is that you […] need to think of the direction as an independent variable that gives the permission to go one way or the other. (not the lever itself, and not something that can be calculated from other variables) The lever position plus code button (plus OK conditions) set the direction variable. OS or block occupancy clears it. If you disable the clearing, then you have fleeting where the signal automatically clears again […]

And earlier, [#15221](https://groups.io/g/layoutcommandcontrol/message/15221):

> I.e. something needs to act as a flip-flop on the direction enable. The code button enables it, something else needs to disable it.

Ken Cameron's implementation of the same pattern, [#15223](https://groups.io/g/layoutcommandcontrol/message/15223):

> The logic I've used has an event pair for the held/release state. […] The held/release is one of the first things in the logic for a signal to evaluate and if held, display stop.

**Pattern: latched state.** Allocate a variable whose `set true` and `set false` are distinct events, set it from the enabling action, clear it from the terminating action, and evaluate it as the **first rung** of the ladder. Every latch needs an explicit clearer — omit it and you get "fleeting" behaviour.

Dick's summary of capability, [#15225](https://groups.io/g/layoutcommandcontrol/message/15225): *"It has nothing to do with the logic capabilities. Either type can do it."* The Signal-LCC conditionals can express a held signal; the modelling has to be right.

## Variables

Each variable has three settings that interact.

**`Trigger`** — when this variable causes re-evaluation. Manual 7.4.1, page 27.

| Value | Meaning |
|---|---|
| `On Variable Change` | Re-evaluate only when the stored value actually flips. |
| `On Matching Event` | Re-evaluate on every matching event, even with no state change. Use when an action must re-send. |
| `None` | Variable is inactive — neither triggers nor contributes. |

**`Source`** — where the value comes from. Manual 7.4.2, pages 27–29.

- `Use Variable #N's (C) Events` — driven by the `set true` / `set false` consumer event pair below it.
- `Track Circuit 1..8` — mirrors a track circuit's speed indication compared against `Track Speed`.

**`Track Speed`** — only meaningful when `Source` is a track circuit. The variable is true while the linked circuit reports that speed.

When you only need one variable, set the other's `Trigger` to `None` and choose `V1 Only` (or `V2 Only`) as the operation.

## Logic Operations

| Operation | Notes |
|---|---|
| `V1 AND V2`, `V1 OR V2`, `V1 XOR V2` | Steady-state combination. |
| `V1 AND V2 => change`, `V1 OR V2 => change` | Fire only on the transition into true, not while it stays true. |
| `V1 AND Then V2 => true` | **Temporal** AND: true only when V1 became true *before* V2. |
| `V1 Only`, `V2 Only` | Single-variable. |
| `null => true` | Unconditionally true; useful for slots that exist purely to fire delayed events. |

Manual section 7.6, page 29.

`V1 AND Then V2` exists specifically for **direction detection**. Dick Bronson, [#7880](https://groups.io/g/layoutcommandcontrol/message/7880):

> The 'if V1 AND-THEN V2' logic in the LCC nodes is in there so that its easy to calculate direction. Just use it between any two adjacent blocks to get the answer.

That is the sanctioned way to get APB tumble-down direction without infrared sensors or DCC correlation.

## Action Slots

Four per conditional, each with a `Condition`:

| Condition | Fires |
|---|---|
| `none` | never (slot unused) |
| `Immediately` | on evaluation, regardless of result |
| `After delay` | after the configured delay |
| `Immediate if True` / `Immediate if False` | branch on the result |
| `Delayed if True` / `Delayed if False` | branch on the result, after the delay |

A common beginner failure is leaving `Condition = none`, which silently disables the event. Rob Heikens, [#8699](https://groups.io/g/layoutcommandcontrol/message/8699):

> Don't forget to set the Condition of this Action to something other then 'None'; simplest is 'Immediately'.

## Design Rules Distilled

1. **Order rungs most-restrictive-first.** The ladder is a priority list.
2. **`Clear` belongs last, as the fall-through.** Ken Cameron, [#7879](https://groups.io/g/layoutcommandcontrol/message/7879), notes you can save a slot by making Clear the `on false` outcome of the last real test rather than its own rung.
3. **Look only at the next signal.** Do not chain occupancy two blocks ahead. Dick Bronson, [#7873](https://groups.io/g/layoutcommandcontrol/message/7873):

   > Signal logic never requires looking any further than to the next signal to determine the correct aspect. […] This signal does not need to know why the next is 'stop', it just needs to show 'approach'.

   The next signal's `Track Speed` already encodes everything upstream.
4. **One group per mast.** Mixing masts, or mixing a mast with unrelated outputs, in one group re-creates the short-circuit problem.
5. **Latch anything that must persist,** and give every latch an explicit clearer.
6. **Never share a variable slot between two purposes** — Mark Granville, [#8704](https://groups.io/g/layoutcommandcontrol/message/8704): "You do not set the two variables of a Conditional to the same thing."

## Known Limits

- **32 slots.** A 3-aspect ABS mast costs roughly 3–5. Complex CTC quickly exhausts the node.
- **No copy/paste or reordering of logic entries** in the JMRI CDI editor. Bob Gamble had to retype everything after changing his rung count, which introduced errors ([#7955](https://groups.io/g/layoutcommandcontrol/message/7955)). Bowties can add real value here.
- **Two variables per slot.** Wider expressions must be spread across slots using `AND with Next` / `OR with Next`.
- For full CTC panels, Dick Bronson considers the **Tower LCC+Q with STL** the right tool rather than Signal-LCC conditionals ([#15224](https://groups.io/g/layoutcommandcontrol/message/15224) records his view). Signal-LCC conditionals remain appropriate for ABS and simple interlocking.

## Community Reference Documents

Mark Granville maintains two widely-recommended write-ups in the group's **Files → LCC Topics** area (Groups.io login required):

- *LCC Topics Understanding SignalLCC Conditionals.pdf* — attached to [#14034](https://groups.io/g/layoutcommandcontrol/message/14034) and [#8704](https://groups.io/g/layoutcommandcontrol/message/8704)
- *LCC Topics Understanding SignalLCC & Track Circuits.pdf* — attached to [#8704](https://groups.io/g/layoutcommandcontrol/message/8704)

Files area: <https://groups.io/g/layoutcommandcontrol/files/LCC%20Topics>

## Related

- [rule-to-aspect.md](rule-to-aspect.md)
- [track-circuits.md](track-circuits.md)
- [bowties-ux-implications.md](bowties-ux-implications.md)
