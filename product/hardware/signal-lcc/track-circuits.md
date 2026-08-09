# Track Circuits (Signal Cascade)

CDI segment: `Track Circuits`, 8 `Circuit` slots. The CDI describes each as:

> Each track circuit may receive speed information from one remote mast.

A track circuit is how a signal learns **what the next signal is showing**, expressed as a speed rather than as an aspect name. It is the mechanism that makes ABS cascade work without a computer.

## The Model

- Every mast with `Mast Processing = Normal` **publishes** a `Link Address` — a read-only producer address in `Rule to Aspect/Mast/Link Address`.
- Each rule on that mast carries a `Track Speed`. When the mast's aspect changes, it pushes the new speed out on its link address.
- A node that needs to *read* that mast defines a **Track Circuit** whose `Link Address` (consumer) is a copy of the remote mast's published address.
- Conditionals then read the circuit by setting a variable's `Source` to `Track Circuit N` and its `Track Speed` to the value being tested.

Ken Cameron, [#8700](https://groups.io/g/layoutcommandcontrol/message/8700):

> The mast pushes the track circuit change it doesn't consume it.

David Harris frames it as an ordinary producer/consumer pairing, [#9324](https://groups.io/g/layoutcommandcontrol/message/9324):

> Just like with events, which require a producer and a consumer to share the same eventid, a track-circuit requires a transmitter and a receiver to share the same track-circuit-eventids.

## Wiring One Cascade

For "Mast 1 should show Approach when Mast 2 shows Stop":

1. On **Mast 2**, set each rule's `Track Speed` correctly (`Stop` on the stop rule, etc.).
2. Copy **Mast 2**'s `Link Address`.
3. On the node owning **Mast 1**, paste it into a free `Track Circuits/Circuit/Link Address`, say Circuit 1. Name it after the remote mast.
4. In a conditional on Mast 1's ladder, set `Variable #1 Source = Track Circuit 1` and `Variable #1 Track Speed = Stop`, `Logic Operation = V1 Only`.
5. In that conditional's Action, send Mast 1's "Approach" `set aspect` event with `Condition = Immediately`.

Rob Heikens walks through precisely this in [#8699](https://groups.io/g/layoutcommandcontrol/message/8699), including the reminder to work most-restrictive-first and to use a **group of consecutive conditionals** to cover all of Mast 1's possible aspects.

Ken Cameron's expected event trace when an aspect changes, [#8700](https://groups.io/g/layoutcommandcontrol/message/8700):

1. old aspect cleared event
2. new aspect set event
3. a track circuit event **if the speed changes**

If item 3 is missing, the `Track Speed` fields are wrong.

## Allocation Rules

| Rule | Source |
|---|---|
| A track circuit is only valid **in the node where it is defined**. To use a mast's speed in another node, define a circuit there too using the same link address. | Mark Granville, [#9295](https://groups.io/g/layoutcommandcontrol/message/9295) |
| Circuit number has **no relationship** to mast number. Mast 1's link address may become circuit 3 in the same or a different node. | Mark Granville, [#9319](https://groups.io/g/layoutcommandcontrol/message/9319) |
| You need **one circuit per distinct "next" mast** a signal must read. | Mark Granville, [#9319](https://groups.io/g/layoutcommandcontrol/message/9319) |
| At a diverging turnout, the throat signal needs the circuits of **both** next masts; the two frog-end masts each need only the one mast they both see. | Ken Cameron, [#9321](https://groups.io/g/layoutcommandcontrol/message/9321) |
| There is **no linkage** between the Rule to Aspect segment and the Track Circuits segment; any circuit number may be used as long as usage is consistent and unique. | Rob Heikens, [#8699](https://groups.io/g/layoutcommandcontrol/message/8699) |
| Each block between masts needs its **own** circuit and its own conditional group. | Rob Heikens, [#8703](https://groups.io/g/layoutcommandcontrol/message/8703) |

With 8 receivers per node, circuit exhaustion is a real constraint on dense interlockings, and a good candidate for a Bowties capacity indicator.

## "Next Mast" Means Same Direction

The most common modelling error is treating a mast facing the opposite direction as "next". Mark Granville, [#9297](https://groups.io/g/layoutcommandcontrol/message/9297):

> both ABS and APB systems logic look only at masts facing the same direction.

On a loop, abandon compass directions and use **clockwise / counter-clockwise**, which both Ken Cameron and Mark Granville recommend after a long thread of confusion ([#9316](https://groups.io/g/layoutcommandcontrol/message/9316), [#9317](https://groups.io/g/layoutcommandcontrol/message/9317)). The two directions form two independent signal chains that never reference each other.

Ken Cameron's summary of what a chain is, [#9316](https://groups.io/g/layoutcommandcontrol/message/9316):

> from the one signal, the logic included all blocks and turnouts on the path up to the next signal and then that signals track circuit to do all the logic for the aspect.

That sentence is the whole ABS model: **occupancy and turnouts up to the next signal, plus the next signal's track circuit.**

## Speed Vocabulary

Both the mast rule and the conditional variable use the same 8-value scale:

`Stop`, `Restricting/Tumble Down`, `Slow`, `Medium`, `Limited`, `Approach`, `Approach-Medium`, `Clear/Proceed`

The manual-derived description notes the variable is true while the linked circuit reports that speed "or better, per NORAC ordering" (manual 7.4.2.2, pages 28–29). Treat exact-match versus threshold semantics carefully when compiling, and verify against hardware before relying on "or better".

## Cascade Depth Needs No Lookahead

Advance aspects fall out of the chain automatically. Mast 3 at Stop makes Mast 2 Approach, which makes Mast 1 Advance-Approach — each signal reads only its neighbour. Confirmed by Dick Bronson, [#7873](https://groups.io/g/layoutcommandcontrol/message/7873), and by the worked example in [#8702](https://groups.io/g/layoutcommandcontrol/message/8702).

## Related

- [rule-to-aspect.md](rule-to-aspect.md) — where `Track Speed` and `Link Address` live
- [conditionals.md](conditionals.md) — how circuits are read
